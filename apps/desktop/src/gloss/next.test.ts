import { test } from "node:test";
import assert from "node:assert/strict";
import { nextStill, nothingLeft } from "./next.ts";

const seg = (id: string, startMs: number | null, tier = "utt") => ({ id, tier, startMs });

test("the next sentence still to gloss on the same track, in number order, wrapping", () => {
  const segs = [seg("c", 3000), seg("a", 1000), seg("u", null), seg("b", 2000), seg("t", 1500, "translation")];
  const still = new Set(["a", "c", "u", "t"]);
  assert.equal(nextStill(segs, segs[1]!, still)?.id, "c", "after a (1 s) comes b (done), then c");
  assert.equal(nextStill(segs, segs[0]!, still)?.id, "u", "untimed ones come last");
  assert.equal(nextStill(segs, segs[2]!, still)?.id, "a", "wraps to the start");
  assert.equal(nextStill(segs, segs[3]!, new Set(["b"])), null, "never itself; other tracks never");
  assert.match(nothingLeft(true), /last sentence/u);
  assert.match(nothingLeft(false), /glossed through/u);
});
