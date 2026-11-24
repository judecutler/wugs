import { test } from "node:test";
import assert from "node:assert/strict";
import { around } from "./around.ts";

const seg = (id: string, tier: string, content: string, startMs: number | null) => ({ id, tier, content, startMs });

test("a line opens to the sentences before and after it on its own track", () => {
  const segments = [
    seg("c", "utt", "third", 3000),
    seg("a", "utt", "first", 1000),
    seg("t", "tr", "a translation", 1000),
    seg("b", "utt", "second", 2000),
    seg("e", "utt", "", 3500),
    seg("d", "utt", "fourth", 4000),
    seg("u", "utt", "untimed", null),
  ];
  const r = around(segments, "c")!;
  assert.deepEqual(r.before.map((s) => s.id), ["a", "b"]);
  assert.deepEqual(r.after.map((s) => s.id), ["d", "u"]);
  assert.deepEqual(around(segments, "a", 1)!.before, []);
  assert.equal(around(segments, "zz"), null);
  assert.deepEqual(around(segments, "t")!.after, []);
});
