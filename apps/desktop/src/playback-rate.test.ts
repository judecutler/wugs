import assert from "node:assert/strict";
import { test } from "node:test";
import { parseRate, rateLabel } from "./playback-rate.ts";

test("a stored speed is one of the menu's, else normal", () => {
  assert.equal(parseRate("0.5"), 0.5);
  assert.equal(parseRate("0.75"), 0.75);
  assert.equal(parseRate("3"), 1);
  assert.equal(parseRate(null), 1);
  assert.equal(parseRate("abc"), 1);
});

test("speeds in words", () => {
  assert.equal(rateLabel(1), "1× speed");
  assert.equal(rateLabel(0.5), "0.5× speed");
});
