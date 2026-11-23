import { test } from "node:test";
import assert from "node:assert/strict";
import { chunkSpan, darkness, loudest, pixels, rgbOf, SPECTRO_ROWS } from "./spectro.ts";

test("the loudest value sets the top; 50 dB below it is blank", () => {
  assert.equal(loudest([new Uint8Array([3, 9]), new Uint8Array([200])]), 200);
  assert.equal(loudest([]), 0);
  assert.equal(darkness(200, 200), 1);
  assert.equal(darkness(100, 200), 0, "50 dB down is 100 half-dB steps");
  assert.equal(darkness(150, 200), 0.5);
  assert.equal(darkness(250, 200), 1);
});

test("a chunk becomes pixels with the highest band on top", () => {
  const chunk = new Uint8Array(SPECTRO_ROWS * 2);
  chunk[0] = 200; // column 0, lowest band
  chunk[SPECTRO_ROWS + SPECTRO_ROWS - 1] = 200; // column 1, highest band
  const p = pixels(chunk, 200, [10, 20, 30]);
  assert.equal(p.width, 2);
  assert.equal(p.height, SPECTRO_ROWS);
  const alpha = (x: number, y: number) => p.data[(y * p.width + x) * 4 + 3];
  assert.equal(alpha(0, SPECTRO_ROWS - 1), 255, "low frequency at the bottom");
  assert.equal(alpha(1, 0), 255, "high frequency at the top");
  assert.equal(alpha(0, 0), 0);
  assert.deepEqual([...p.data.slice(0, 3)], [10, 20, 30]);
});

test("chunks sit at their time", () => {
  assert.deepEqual(chunkSpan(1, 2500, 5000, 100), { x: 500, width: 1000 });
  assert.deepEqual(rgbOf("#1b1a17"), [27, 26, 23]);
  assert.deepEqual(rgbOf("rgb(1, 2, 3)"), [1, 2, 3]);
});
