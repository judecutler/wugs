import assert from "node:assert/strict";
import { test } from "node:test";
import { duplicateNote, duplicatesLine, glossKeys, likelyDuplicates, type EntryLike } from "./entries.ts";

const e = (id: string, lemma: string, glosses: string[], used = false, homograph: number | null = null): EntryLike => ({ id, lemma, homograph, glosses, used });

test("gloss parts are compared loosely", () => {
  assert.deepEqual(glossKeys("To  Eat; consume, bite/chew"), ["eat", "consume", "bite", "chew"]);
  assert.deepEqual(glossKeys(""), []);
});

test("near headwords with a shared meaning are paired", () => {
  const entries = [
    e("a", "kesi", ["river"]),
    e("b", "kési", ["river; stream"], true),
    e("c", "tela", ["water"], false, 1),
    e("d", "tela", ["water"], false, 2),
    e("f", "tela", ["water"]),
    e("g", "sanu", ["to eat"]),
    e("h", "sanuk", ["eat"]),
    e("i", "ka", ["go"]),
    e("j", "ki", ["go"]),
    e("k", "mono", ["fish"]),
    e("l", "mona", ["bird"]),
  ];
  const pairs = likelyDuplicates(entries);
  const ids = pairs.map((p) => `${p.keep.id}${p.other.id}:${p.why}`);
  // Two numbered homographs are told apart; an unnumbered one beside them is not.
  assert.deepEqual(ids, ["cf:same", "df:same", "ba:accents", "gh:letter"]);
  assert.deepEqual(pairs[2]!.shared, ["river; stream"], "the used entry is the one kept");
  assert.equal(duplicateNote(pairs[2]!), "headwords that differ only in accents, both ‘river; stream’; only the first is used in a text");
  assert.match(duplicatesLine(pairs, entries.length), /^4 pairs of entries share a meaning/);
  assert.match(duplicatesLine([], 3), /^No two of the 3 entries/);
});
