// The spectrogram under the waveform band: computed by wugs itself
// (src-tauri/src/spectrogram.rs), ten seconds at a time, once each, and
// drawn as Praat draws it, darker where louder. Pixel rules are in ./spectro.ts.
import { errorText, spectrogramChunk } from "../api";
import { chunksFor } from "../praat/analysis";
import { THEME_EVENT } from "../theme/theme";
import { token } from "../theme/tokens";
import { chunkSpan, loudest, pixels, rgbOf, SPECTRO_MAX_HZ } from "./spectro";
import type { Viewport } from "./viewport";

/** About ten minutes kept in memory (a chunk is 250 KB). */
const KEEP_CHUNKS = 60;
/** Chunks are painted again when the loudest value moves this much (half-dB steps). */
const REPAINT_STEP = 6;

/** Where the band chosen is kept on this computer. */
export const BAND_KEY = "wugs.timeline.spectroBand";

type Painted = { bytes: Uint8Array; canvas: HTMLCanvasElement | null; top: number };

let ink: [number, number, number] | null = null;
window.addEventListener(THEME_EVENT, () => (ink = null));

export class SpectrogramLayer {
  private source: string | null = null;
  private on = false;
  private chunks = new Map<number, Painted>();
  private inflight = false;
  private problem: string | null = null;
  private generation = 0;
  private repaintAll = false;
  /** Narrowband (harmonics as lines) rather than broadband (formants as bands). */
  private narrow = localStorage.getItem(BAND_KEY) === "narrow";
  /** Told when the layer turns itself off (a failure), so the toggle redraws. */
  onChange: () => void = () => {};

  setSource(source: string | null): void {
    if (source === this.source) return;
    this.source = source;
    this.chunks.clear();
    this.inflight = false;
    this.problem = null;
    this.generation++;
  }

  enabled(): boolean {
    return this.on;
  }

  isNarrow(): boolean {
    return this.narrow;
  }

  /** Switch between broadband and narrowband; the chunks are made again. */
  setNarrow(narrow: boolean): void {
    if (narrow === this.narrow) return;
    this.narrow = narrow;
    try {
      localStorage.setItem(BAND_KEY, narrow ? "narrow" : "broad");
    } catch {
      // Kept for this run only.
    }
    this.chunks.clear();
    this.inflight = false;
    this.problem = null;
    this.generation++;
  }

  toggle(): void {
    this.on = !this.on;
    // Either way the note of an earlier failure goes; turning on tries again.
    this.problem = null;
  }

  /** Whether the waveform should fade so the spectrogram reads. */
  showing(): boolean {
    return this.on && this.source !== null && this.problem === null;
  }

  private fetch(wanted: number[]): void {
    if (this.inflight || this.source === null) return;
    const next = wanted.find((n) => !this.chunks.has(n));
    if (next === undefined) return;
    const source = this.source;
    const gen = this.generation;
    this.inflight = true;
    spectrogramChunk({ source, index: next, narrow: this.narrow }).then(
      (buffer) => {
        if (gen !== this.generation) return;
        this.inflight = false;
        this.chunks.set(next, { bytes: new Uint8Array(buffer), canvas: null, top: -1 });
        for (const old of this.chunks.keys()) {
          if (this.chunks.size <= KEEP_CHUNKS) break;
          if (!wanted.includes(old)) this.chunks.delete(old);
        }
        this.repaintAll = true;
      },
      (e: unknown) => {
        if (gen !== this.generation) return;
        this.inflight = false;
        this.problem = errorText(e);
        this.on = false;
        this.onChange();
      },
    );
  }

  /** Draw into the band from `top` to `top + height`. */
  draw(ctx: CanvasRenderingContext2D, v: Viewport, top: number, height: number): void {
    if (this.problem !== null) {
      this.note(ctx, v, top, `The spectrogram could not be made: ${this.problem}`);
      return;
    }
    if (!this.on || this.source === null) return;
    const endMs = v.t0Ms + (v.widthPx / v.pxPerSec) * 1000;
    const wanted = chunksFor(v.t0Ms, endMs, v.durationMs);
    if (wanted === null) {
      this.note(ctx, v, top, "Zoom in to a minute or less to see the spectrogram.");
      return;
    }
    this.fetch(wanted);
    const shown = wanted.map((n) => this.chunks.get(n)).filter((c): c is Painted => c !== undefined);
    const level = loudest(shown.map((c) => c.bytes));
    ink ??= rgbOf(token("--ink", "#1b1a17"));
    ctx.save();
    ctx.imageSmoothingEnabled = true;
    for (const n of wanted) {
      const c = this.chunks.get(n);
      if (c === undefined) continue;
      if (c.canvas === null || (this.repaintAll && Math.abs(c.top - level) >= REPAINT_STEP)) this.paint(c, level);
      const columns = c.canvas!.width;
      const span = chunkSpan(n, columns, v.t0Ms, v.pxPerSec);
      if (span.x > v.widthPx || span.x + span.width < 0) continue;
      ctx.drawImage(c.canvas!, span.x, top, span.width, height);
    }
    this.repaintAll = false;
    ctx.restore();
    ctx.font = "10px system-ui";
    ctx.fillStyle = token("--muted", "#625b4a");
    ctx.fillText(`${SPECTRO_MAX_HZ} Hz`, 4, top + 10);
    ctx.fillText("0 Hz", 4, top + height - 3);
    if (shown.length < wanted.length) this.note(ctx, v, top, "Making the spectrogram…");
  }

  private paint(c: Painted, level: number): void {
    const p = pixels(c.bytes, level, ink ?? [27, 26, 23]);
    const canvas = c.canvas ?? document.createElement("canvas");
    canvas.width = Math.max(1, p.width);
    canvas.height = p.height;
    canvas.getContext("2d")?.putImageData(new ImageData(p.data, canvas.width, p.height), 0, 0);
    c.canvas = canvas;
    c.top = level;
  }

  private note(ctx: CanvasRenderingContext2D, v: Viewport, top: number, text: string): void {
    ctx.font = "11px system-ui";
    ctx.fillStyle = token("--muted", "#625b4a");
    ctx.textAlign = "right";
    ctx.fillText(text, v.widthPx - 70, top + 26);
    ctx.textAlign = "left";
  }
}
