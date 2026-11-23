// The Timeline's spectrogram (src-tauri/src/spectrogram.rs): the layout of
// a chunk and how its values become pixels. Pure, so node --test runs it.

/** Mirrors of the Rust constants. */
export const SPECTRO_CHUNK_MS = 10_000;
export const SPECTRO_STEP_MS = 4;
export const SPECTRO_ROWS = 100;
export const SPECTRO_MAX_HZ = 5000;
/** Praat's default dynamic range: anything this far below the loudest is blank. */
export const DYNAMIC_RANGE_DB = 50;

/** The loudest value in some chunks (half-dB steps), 0 when there are none. */
export function loudest(chunks: Iterable<Uint8Array>): number {
  let max = 0;
  for (const c of chunks) for (let i = 0; i < c.length; i++) if (c[i]! > max) max = c[i]!;
  return max;
}

/** How dark a value draws, 0–1: the top `DYNAMIC_RANGE_DB` below `top`. */
export function darkness(value: number, top: number): number {
  const floor = top - DYNAMIC_RANGE_DB * 2;
  if (value <= floor) return 0;
  return Math.min(1, (value - floor) / (top - floor));
}

/** A chunk as RGBA pixels, one column per step and one row per band, the
 * highest frequency in the top row, in `ink` with darkness as opacity. */
export function pixels(chunk: Uint8Array, top: number, ink: [number, number, number]): { width: number; height: number; data: Uint8ClampedArray<ArrayBuffer> } {
  const width = Math.floor(chunk.length / SPECTRO_ROWS);
  const height = SPECTRO_ROWS;
  const data = new Uint8ClampedArray(new ArrayBuffer(width * height * 4));
  for (let c = 0; c < width; c++) {
    for (let r = 0; r < height; r++) {
      const at = ((height - 1 - r) * width + c) * 4;
      data[at] = ink[0];
      data[at + 1] = ink[1];
      data[at + 2] = ink[2];
      data[at + 3] = Math.round(darkness(chunk[c * SPECTRO_ROWS + r]!, top) * 255);
    }
  }
  return { width, height, data };
}

/** Where a chunk draws: its left edge and width in pixels. */
export function chunkSpan(index: number, columns: number, t0Ms: number, pxPerSec: number): { x: number; width: number } {
  const start = index * SPECTRO_CHUNK_MS;
  return { x: ((start - t0Ms) * pxPerSec) / 1000, width: (columns * SPECTRO_STEP_MS * pxPerSec) / 1000 };
}

/** `#rrggbb` or `rgb(r, g, b)` as numbers; dark grey when unreadable. */
export function rgbOf(color: string): [number, number, number] {
  const hex = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/iu.exec(color.trim());
  if (hex) return [parseInt(hex[1]!, 16), parseInt(hex[2]!, 16), parseInt(hex[3]!, 16)];
  const rgb = /^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/iu.exec(color.trim());
  if (rgb) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
  return [27, 26, 23];
}
