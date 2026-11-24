import assert from "node:assert/strict";
import { test } from "node:test";
import { decades, logSpaced, zipfFit, zipfLine, zipfPoints } from "./zipf.ts";

test("log-spaced ranks start at 1, end at n and thin the tail", () => {
  const r = logSpaced(10_000, 50);
  assert.equal(r[0], 1);
  assert.equal(r.at(-1), 10_000);
  assert.ok(r.length <= 50);
  assert.deepEqual(logSpaced(3, 50), [1, 2, 3]);
  assert.deepEqual(logSpaced(0), []);
});

test("an exact Zipf list fits alpha 1", () => {
  const counts = Array.from({ length: 2000 }, (_, i) => 10_000 / (i + 1));
  const fit = zipfFit(zipfPoints(counts))!;
  assert.ok(Math.abs(fit.alpha - 1) < 1e-9, String(fit.alpha));
  assert.ok(Math.abs(fit.intercept - 4) < 1e-9);
  assert.ok(fit.r2 > 0.999);
  assert.match(zipfLine(fit, 2000, false), /^Zipf's exponent 1\.00 \(r² 1\.00\), close to 1/);
  assert.match(zipfLine({ alpha: 1.5, intercept: 0, r2: 0.9 }, 5000, true), /steeper than usual.*Only the 5,000 commonest/);
  assert.equal(zipfLine(zipfFit(zipfPoints([3, 2])), 2, false), "Too few different items to fit a line.");
});

test("points read counts most frequent first", () => {
  assert.deepEqual(zipfPoints([1, 5, 0, 3]), [{ rank: 1, count: 5 }, { rank: 2, count: 3 }, { rank: 3, count: 1 }]);
  assert.deepEqual(decades(250), [1, 10, 100, 1000]);
  assert.deepEqual(decades(1), [1]);
});
