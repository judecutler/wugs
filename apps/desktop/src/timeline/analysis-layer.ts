// Praat's pitch, intensity and formants drawn over the waveform band
// (roadmap item 46). Asks the backend for the 10 s chunks the view needs,
// once each, and draws what has arrived; nothing is fetched while every kind
// is off. Display rules are in ../praat/analysis.ts.
import { token } from "../theme/tokens";
import { errorText, acousticAnalysis } from "../api";
import { chunksFor, frameMs, RANGES, yFor, type Chunk, type Kind } from "../praat/analysis";
import type { Viewport } from "./viewport";

/** About ten minutes of analysis kept in memory. */
const KEEP_CHUNKS = 60;

const FALLBACK: Record<Kind, string> = { pitch: "#1f5fbf", intensity: "#c9a400", formants: "#c0392b" };
/** A curve's colour from the design tokens (--pitch, --intensity, --formants). */
const color = (kind: Kind): string => token(`--${kind}`, FALLBACK[kind]);

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
    // Wait for scrolling to settle before running Praat.
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
  draw(ctx: CanvasRenderingContext2D, v: Viewport, top: number, height: number): void {
    if (this.problem !== null) {
      this.note(ctx, v, top, `Praat analysis failed: ${this.problem}`);
      return;
    }
    if (this.on.size === 0 || this.source === null) return;
    const endMs = v.t0Ms + (v.widthPx / v.pxPerSec) * 1000;
    const wanted = chunksFor(v.t0Ms, endMs, v.durationMs);
    if (wanted === null) {
      this.note(ctx, v, top, "Zoom in to a minute or less to see Praat's analysis.");
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
    if (waiting) this.note(ctx, v, top, "Praat is analysing…");
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
      const y = yFor(kind, val, top, height);
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
      const y = val === null || val === undefined ? null : yFor(kind, Math.min(Math.max(val, RANGES[kind].lo), RANGES[kind].hi), top, height);
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
      const { lo, hi, unit } = RANGES[kind];
      ctx.fillStyle = color(kind);
      ctx.fillText(`${hi} ${unit}`, v.widthPx - 4, top + 10 + row * 11);
      ctx.fillText(`${lo} ${unit}`, v.widthPx - 4, top + height - 3 - row * 11);
      row++;
    }
    ctx.textAlign = "left";
  }

  private note(ctx: CanvasRenderingContext2D, v: Viewport, top: number, text: string): void {
    ctx.font = "11px system-ui";
    ctx.fillStyle = token("--muted", "#7a756c");
    ctx.textAlign = "right";
    ctx.fillText(text, v.widthPx - 70, top + 12);
    ctx.textAlign = "left";
  }
}
