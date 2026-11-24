// Pitch, intensity and formants (Praat's, or wugs's own pitch and intensity) drawn over the waveform band
// (roadmap item 46). Asks the backend for the 10 s chunks the view needs,
// once each, and draws what has arrived; nothing is fetched while every kind
// is off. Display rules are in ../praat/analysis.ts.
import { token } from "../theme/tokens";
import { THEME_EVENT } from "../theme/theme";
import { errorText, acousticAnalysis } from "../api";
import { chunksFor, frameMs, rangesFor, readAt, readingText, yFor, type Chunk, type Kind, type Voice } from "../praat/analysis";
import type { Viewport } from "./viewport";

/** About ten minutes of analysis kept in memory. */
const KEEP_CHUNKS = 60;

const FALLBACK: Record<Kind, string> = { pitch: "#1f5fbf", intensity: "#c9a400", formants: "#c0392b" };
/** Colours from the design tokens (--pitch, --intensity, --formants, --muted),
 * read once: the draw loop runs every frame and must not query styles. */
let palette: (Record<Kind, string> & { muted: string }) | null = null;
// Read again after a theme change (src/theme/theme.ts).
window.addEventListener(THEME_EVENT, () => (palette = null));
const colors = () =>
  (palette ??= {
    pitch: token("--pitch", FALLBACK.pitch),
    intensity: token("--intensity", FALLBACK.intensity),
    formants: token("--formants", FALLBACK.formants),
    muted: token("--muted", "#625b4a"),
  });
const color = (kind: Kind): string => colors()[kind];

export class AnalysisLayer {
  private source: string | null = null;
  private on = new Set<Kind>();
  private chunks = new Map<number, Chunk>();
  /** One request at a time; the next draw asks for what is still missing. */
  private inflight = false;
  private problem: string | null = null;
  /** Told when the layer turns itself off (a failure), so toggles redraw. */
  onChange: () => void = () => {};
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** The chunks the pending timer will ask for; draw runs every frame, so
   * the timer restarts only when this changes (the view moved). */
  private pendingKey = "";
  /** Bumped on every source change, so a late answer for the old one is dropped. */
  private generation = 0;
  private width = 0;
  private voiceId: string | null = null;
  private ranges = rangesFor(null);

  /** The recording's voice: a change drops what was analysed with the old one. */
  setVoice(voice: Voice): void {
    this.ranges = rangesFor(voice);
    if (voice.id === this.voiceId) return;
    const first = this.voiceId === null;
    this.voiceId = voice.id;
    if (first) return;
    this.chunks.clear();
    this.inflight = false;
    this.problem = null;
    this.generation++;
    clearTimeout(this.timer);
    this.pendingKey = "";
  }

  setSource(source: string | null): void {
    if (source === this.source) return;
    this.source = source;
    this.chunks.clear();
    this.inflight = false;
    this.problem = null;
    this.generation++;
    clearTimeout(this.timer);
    this.pendingKey = "";
  }

  /** Whether anything is drawn over the waveform (it is then faded). */
  showing(): boolean {
    return this.on.size > 0 && this.source !== null && this.problem === null;
  }

  enabled(kind: Kind): boolean {
    return this.on.has(kind);
  }

  toggle(kind: Kind): void {
    if (this.on.has(kind)) this.on.delete(kind);
    else this.on.add(kind);
    // Turning a kind on is the way to try again after a failure.
    if (this.on.size > 0 && this.problem !== null) this.problem = null;
  }

  private request(wanted: number[]): void {
    const missing = wanted.filter((n) => !this.chunks.has(n));
    if (missing.length === 0 || this.source === null || this.inflight) return;
    const key = missing.join(",");
    if (key === this.pendingKey) return;
    this.pendingKey = key;
    // Wait for scrolling to settle before analysing.
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.pendingKey = "";
      const source = this.source;
      const gen = this.generation;
      if (source === null) return;
      this.inflight = true;
      acousticAnalysis({ source, chunks: missing }).then(
        (got) => {
          if (gen !== this.generation) return;
          this.inflight = false;
          for (const c of got) this.chunks.set(c.index, c);
          // Keep the newest chunks; the backend cache makes refetching cheap.
          for (const old of this.chunks.keys()) {
            if (this.chunks.size <= KEEP_CHUNKS) break;
            this.chunks.delete(old);
          }
        },
        (e: unknown) => {
          if (gen !== this.generation) return;
          this.inflight = false;
          this.problem = errorText(e);
          this.on.clear();
          this.onChange();
        },
      );
    }, 150);
  }

  /** Draw into the band from `top` to `top + height`. */
  draw(ctx: CanvasRenderingContext2D, v: Viewport, top: number, height: number, hoverMs: number | null = null): void {
    if (this.problem !== null) {
      this.note(ctx, v, top, `Analysis failed: ${this.problem}`);
      return;
    }
    if (this.on.size === 0 || this.source === null) return;
    const endMs = v.t0Ms + (v.widthPx / v.pxPerSec) * 1000;
    const wanted = chunksFor(v.t0Ms, endMs, v.durationMs);
    if (wanted === null) {
      this.note(ctx, v, top, "Zoom in to a minute or less to see the analysis.");
      return;
    }
    this.width = v.widthPx;
    this.request(wanted);
    const xOf = (ms: number) => ((ms - v.t0Ms) * v.pxPerSec) / 1000;
    let waiting = false;
    for (const n of wanted) {
      const c = this.chunks.get(n);
      if (c === undefined) {
        waiting = true;
        continue;
      }
      if (this.on.has("intensity")) this.line(ctx, c, c.intensity, "intensity", xOf, top, height);
      if (this.on.has("formants")) for (const f of [c.f1, c.f2, c.f3]) this.dots(ctx, c, f, "formants", xOf, top, height, 1.5);
      if (this.on.has("pitch")) this.dots(ctx, c, c.pitch, "pitch", xOf, top, height, 2);
    }
    this.legend(ctx, v, top, height);
    if (waiting) this.note(ctx, v, top, "Analysing…");
    if (hoverMs !== null) this.readout(ctx, v, top, height, hoverMs);
  }

  /** Praat's cursor readout: a hairline at the pointer and the values there. */
  private readout(ctx: CanvasRenderingContext2D, v: Viewport, top: number, height: number, ms: number): void {
    const r = readAt(this.chunks.values(), ms);
    if (r === null) return;
    const x = ((ms - v.t0Ms) * v.pxPerSec) / 1000;
    ctx.strokeStyle = colors().muted;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(Math.round(x) + 0.5, top);
    ctx.lineTo(Math.round(x) + 0.5, top + height);
    ctx.stroke();
    const text = readingText(r, this.on);
    ctx.font = "11px system-ui";
    const w = ctx.measureText(text).width + 8;
    // Beside the pointer, on whichever side has room.
    const left = x + 8 + w > v.widthPx ? x - 8 - w : x + 8;
    ctx.fillStyle = "rgba(255, 255, 255, 0.88)";
    ctx.fillRect(left, top + height - 20, w, 16);
    ctx.fillStyle = colors().muted;
    ctx.fillText(text, left + 4, top + height - 8);
  }

  private dots(ctx: CanvasRenderingContext2D, c: Chunk, values: (number | null)[], kind: Kind, xOf: (ms: number) => number, top: number, height: number, r: number): void {
    ctx.fillStyle = color(kind);
    // One dot per pixel column is all the eye sees; off-screen frames are skipped.
    let lastCol = Number.NaN;
    for (let i = 0; i < values.length; i++) {
      const val = values[i];
      if (val === null || val === undefined) continue;
      const x = xOf(frameMs(c, i));
      if (x < -r || x > this.width + r) continue;
      const col = Math.round(x);
      if (col === lastCol) continue;
      const y = yFor(kind, val, top, height, this.ranges);
      if (y === null) continue;
      lastCol = col;
      ctx.fillRect(x - r / 2, y - r / 2, r, r);
    }
  }

  private line(ctx: CanvasRenderingContext2D, c: Chunk, values: (number | null)[], kind: Kind, xOf: (ms: number) => number, top: number, height: number): void {
    ctx.strokeStyle = color(kind);
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    let pen = false;
    for (let i = 0; i < values.length; i++) {
      const val = values[i];
      const y = val === null || val === undefined ? null : yFor(kind, Math.min(Math.max(val, this.ranges[kind].lo), this.ranges[kind].hi), top, height, this.ranges);
      if (y === null) {
        pen = false;
        continue;
      }
      const x = xOf(frameMs(c, i));
      if (x < -2 || x > this.width + 2) {
        pen = false;
        continue;
      }
      if (pen) ctx.lineTo(x, y);
      else ctx.moveTo(x, y);
      pen = true;
    }
    ctx.stroke();
  }

  private legend(ctx: CanvasRenderingContext2D, v: Viewport, top: number, height: number): void {
    ctx.font = "10px system-ui";
    ctx.textAlign = "right";
    let row = 0;
    for (const kind of ["pitch", "intensity", "formants"] as Kind[]) {
      if (!this.on.has(kind)) continue;
      const { lo, hi, unit } = this.ranges[kind];
      ctx.fillStyle = color(kind);
      ctx.fillText(`${hi} ${unit}`, v.widthPx - 4, top + 10 + row * 11);
      ctx.fillText(`${lo} ${unit}`, v.widthPx - 4, top + height - 3 - row * 11);
      row++;
    }
    ctx.textAlign = "left";
  }

  private note(ctx: CanvasRenderingContext2D, v: Viewport, top: number, text: string): void {
    ctx.font = "11px system-ui";
    ctx.fillStyle = colors().muted;
    ctx.textAlign = "right";
    ctx.fillText(text, v.widthPx - 70, top + 12);
    ctx.textAlign = "left";
  }
}
