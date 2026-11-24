import { test } from "node:test";
import assert from "node:assert/strict";
import { likeNote, likeness, likeTitle, type Like } from "./like.ts";

const l = (score: number, translation: string | null, shared: string[] = []): Like => ({ textId: "t", textTitle: "Market", tierId: "u", segmentId: "s", content: "ka nimo", translation, shared, score });

test("how alike, in words", () => {
  assert.equal(likeness(1), "the same words");
  assert.equal(likeness(0.7), "very like it");
  assert.equal(likeness(0.4), "like it");
  assert.equal(likeness(0.21), "a little like it");
});

test("the note and a row's title", () => {
  assert.equal(likeNote([]), "No other sentence shares this one's rarer words yet.");
  assert.equal(likeNote([l(1, "Two fish.")]), "1 sentence shares words with this one, with its translation. Click one to open it.");
  assert.equal(likeNote([l(1, "Two fish."), l(0.5, null)]), "2 sentences share words with this one, 1 with a translation. Click one to open it.");
  assert.equal(likeNote([l(1, null), l(0.5, " ")]), "2 sentences share words with this one. Click one to open it.");
  assert.equal(likeTitle(l(0.456, null, ["nimo", "pela"])), "like it (46%) in «Market». Both have «nimo», «pela».");
});
