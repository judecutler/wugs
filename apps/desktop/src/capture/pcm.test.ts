import assert from "node:assert/strict";
import { test } from "node:test";
import { defaultLabel, floatToPcm16, formatElapsed, levelOf, meterFill, micErrorMessage } from "./pcm.ts";

const samples = (bytes: Uint8Array) => {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return Array.from({ length: bytes.length / 2 }, (_, i) => v.getInt16(i * 2, true));
};

test("float converts to PCM16 with scale 32767 and clips out-of-range input", () => {
  const pcm = floatToPcm16([Float32Array.of(0, 1, -1, 0.5, 2, -3, NaN)]);
  assert.deepEqual(samples(pcm), [0, 32767, -32767, 16384, 32767, -32767, 0]);
});

test("channels interleave frame by frame", () => {
  const pcm = floatToPcm16([Float32Array.of(1, 0), Float32Array.of(-1, 0.5)]);
  assert.deepEqual(samples(pcm), [32767, -32767, 0, 16384]);
});

test("no channels or no frames yields no bytes", () => {
  assert.equal(floatToPcm16([]).length, 0);
  assert.equal(floatToPcm16([new Float32Array(0)]).length, 0);
});

test("level reports peak and rms across channels", () => {
  const { peak, rms } = levelOf([Float32Array.of(0.5, -0.5), Float32Array.of(0, 0)]);
  assert.equal(peak, 0.5);
  assert.ok(Math.abs(rms - Math.sqrt(0.125)) < 1e-9);
  assert.deepEqual(levelOf([]), { peak: 0, rms: 0 });
  assert.equal(levelOf([Float32Array.of(1.7)]).peak, 1, "peak never exceeds full scale");
});

test("meter fill is 0 at the floor, 1 at full scale, and linear in dB", () => {
  assert.equal(meterFill(0), 0);
  assert.equal(meterFill(1), 1);
  assert.equal(meterFill(1e-4), 0);
  assert.ok(Math.abs(meterFill(10 ** (-30 / 20)) - 0.5) < 1e-9);
  assert.equal(meterFill(5), 1);
});

test("default label is local time to the minute, zero padded", () => {
  assert.equal(defaultLabel(new Date(2026, 10, 9, 8, 5, 59)), "Recording 2026-11-09 08:05");
});

test("elapsed time formats minutes and hours", () => {
  assert.equal(formatElapsed(0), "0:00");
  assert.equal(formatElapsed(7_999), "0:07");
  assert.equal(formatElapsed(750_000), "12:30");
  assert.equal(formatElapsed(3_723_000), "1:02:03");
  assert.equal(formatElapsed(-5), "0:00");
});

test("a denied permission has its own message", () => {
  assert.match(micErrorMessage("NotAllowedError"), /blocked/);
  assert.match(micErrorMessage("NotFoundError"), /No microphone/);
  assert.match(micErrorMessage("Weird"), /Weird/);
});
