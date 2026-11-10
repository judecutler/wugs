// Praat's pitch, intensity and formants drawn over the waveform band
// (roadmap item 46). Asks the backend for the 10 s chunks the view needs,
// once each, and draws what has arrived; nothing is fetched while every kind
// is off. Display rules are in ../praat/analysis.ts.
import { errorText, praatAnalysis } from "../api";
import { chunksFor, frameMs, RANGES, yFor, type Chunk, type Kind } from "../praat/analysis";
import type { Viewport } from "./viewport";

const COLORS: Record<Kind, string> = { pitch: "#1f5fbf", intensity: "#c9a400", formants: "#c0392b" };

export class AnalysisLayer {
  private source: string | null = null;
  private on = new Set<Kind>();
  private chunks = new Map<number, Chunk>();
  private asked = new Set<number>();
  private problem: string | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** The chunks the pending timer will ask for; draw runs every frame, so
   * the timer restarts only when this changes (the view moved). */
  private pendingKey = "";
  /** Bumped on every source change, so a late answer for the old one is dropped. */
  private generation = 0;

  setSource(source: string | null): void {
    if (source === this.source) return;
    this.source = source;
    this.chunks.clear();
    this.asked.clear();
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
    if (this.on.size > 0 && this.problem !== null) {
      this.problem = null;
      this.asked.clear();
    }
  }

  private request(wanted: number[]): void {
    const missing = wanted.filter((n) => !this.chunks.has(n) && !this.asked.has(n));
    if (missing.length === 0 || this.source === null) return;
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
      for (const n of missing) this.asked.add(n);
      praatAnalysis({ source, chunks: missing }).then(
        (got) => {
          if (gen !== this.generation) return;
          for (const c of got) this.chunks.set(c.index, c);
        },
        (e: unknown) => {
          if (gen !== this.generation) return;
          for (const n of missing) this.asked.delete(n);
          this.problem = errorText(e);
          this.on.clear();
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
    ctx.fillStyle = COLORS[kind];
    for (let i = 0; i < values.length; i++) {
      const val = values[i];
      if (val === null || val === undefined) continue;
      const y = yFor(kind, val, top, height);
      if (y === null) continue;
      ctx.fillRect(xOf(frameMs(c, i)) - r / 2, y - r / 2, r, r);
    }
  }

  private line(ctx: CanvasRenderingContext2D, c: Chunk, values: (number | null)[], kind: Kind, xOf: (ms: number) => number, top: number, height: number): void {
    ctx.strokeStyle = COLORS[kind];
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
      ctx.fillStyle = COLORS[kind];
      ctx.fillText(`${hi} ${unit}`, v.widthPx - 4, top + 10 + row * 11);
      ctx.fillText(`${lo} ${unit}`, v.widthPx - 4, top + height - 3 - row * 11);
      row++;
    }
    ctx.textAlign = "left";
  }

  private note(ctx: CanvasRenderingContext2D, v: Viewport, top: number, text: string): void {
    ctx.font = "11px system-ui";
    ctx.fillStyle = "#7a756c";
    ctx.textAlign = "right";
    ctx.fillText(text, v.widthPx - 70, top + 12);
    ctx.textAlign = "left";
  }
}
