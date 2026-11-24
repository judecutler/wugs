import assert from "node:assert/strict";
import { test } from "node:test";
import { nextWhere, progress, sentencesOf, sentenceTracks, showOf, toTranslate, topicsIn, type Line } from "./logic.ts";

test("sentences are the utterance track's, translations left out, in number order", () => {
  const segs = [
    { id: "b", tier: "utt", parent: null, startMs: 2000, content: "two" },
    { id: "a", tier: "utt", parent: null, startMs: 1000, content: "one" },
    { id: "t", tier: "tr", parent: "a", startMs: null, content: "uno" },
    { id: "x", tier: "other", parent: null, startMs: 500, content: "x" },
  ];
  const numbers = new Map([["a", 1], ["b", 2]]);
  assert.deepEqual(sentencesOf(segs, [{ id: "utt", utterance: true }, { id: "tr", utterance: false }], numbers).map((s) => s.id), ["a", "b"]);
  assert.deepEqual(sentencesOf(segs.slice(2), [], new Map()).map((s) => s.id), ["x"], "no utterance track: the first track with sentences");
});

test("a transcript with a track per speaker reads every speaker in time order", () => {
  const segs = [
    { id: "m1", tier: "MOT", parent: null, startMs: 0, content: "look" },
    { id: "c1", tier: "CHI", parent: null, startMs: 900, content: "ball" },
    { id: "m2", tier: "MOT", parent: null, startMs: 1500, content: "yes" },
    { id: "t", tier: "free", parent: null, startMs: 0, content: "mira" },
  ];
  const tiers = [{ id: "utt", utterance: true }, { id: "MOT", utterance: false }, { id: "CHI", utterance: false }, { id: "free", utterance: false, role: "translation" }];
  assert.deepEqual(sentenceTracks(segs, tiers), ["MOT", "CHI"], "an utterance track this text does not use is passed over, a translation track left out");
  const numbers = new Map([["m1", 1], ["c1", 2], ["m2", 3]]);
  assert.deepEqual(sentencesOf(segs, tiers, numbers).map((s) => s.id), ["m1", "c1", "m2"]);
});

test("progress counts sentences and words", () => {
  const w = (n: number) => ({ form: "w", morphs: Array.from({ length: n }, () => ({ surface: "w", type: "root", gloss: "g" })) });
  const lines: Line[] = [
    { id: "a", number: 1, content: "", words: [w(1), w(0)], translation: null, startMs: null },
    { id: "b", number: 2, content: "", words: [w(0)], translation: null, startMs: null },
  ];
  assert.equal(progress(lines), "0 of 2 sentences glossed; 1 of 3 words (33%).", "a sentence with a word left is not glossed yet");
  assert.equal(progress([lines[0]!, { ...lines[0]!, id: "c", words: [w(2)] }]), "1 of 2 sentences glossed; 2 of 3 words (67%).");
  assert.equal(progress([]), "This text has no sentences yet.");
});

test("the next sentence to gloss comes after the current one and wraps round", async () => {
  const { nextToGloss, toGloss } = await import("./logic.ts");
  const w = (n: number) => ({ form: "w", morphs: Array.from({ length: n }, () => ({ surface: "w", type: "root", gloss: "g" })) });
  const line = (id: string, words: ReturnType<typeof w>[], content = "x"): Line => ({ id, number: 0, content, words, translation: null, startMs: null });
  const lines = [line("a", [w(1)]), line("b", [w(0)]), line("c", [w(1), w(1)]), line("d", []), line("e", [], "  ")];
  assert.deepEqual(lines.map(toGloss), [false, true, false, true, false], "untokenized counts; empty does not");
  assert.equal(nextToGloss(lines, null)?.id, "b");
  assert.equal(nextToGloss(lines, "b")?.id, "d");
  assert.equal(nextToGloss(lines, "d")?.id, "b", "wraps round");
  assert.equal(nextToGloss(lines, "gone")?.id, "b");
  assert.equal(nextToGloss([line("a", [w(1)])], "a"), null);
  assert.equal(nextToGloss([line("a", [w(0)])], null)?.id, "a");
});

test("sentences without a translation, and the next one", () => {
  const l = (id: string, content: string, translation: string | null): Line => ({ id, number: 1, content, words: [], translation, startMs: null });
  const lines = [l("a", "kani", "count"), l("b", "ŋai", null), l("c", "", null), l("d", "toɾu", " ")];
  assert.deepEqual(lines.filter(toTranslate).map((x) => x.id), ["b", "d"], "untranscribed sentences are not asked for a translation");
  assert.equal(nextWhere(lines, "b", toTranslate)?.id, "d");
  assert.equal(nextWhere(lines, "d", toTranslate)?.id, "b", "wrapping round");
  assert.equal(showOf("translate").next, "Next to translate");
  assert.equal(showOf("nonsense").id, "all");
});

test("sentences filed under a grammar topic can be listed alone", () => {
  const l = (id: string, topics: string[]): Line => ({ id, number: 0, content: "x", words: [], translation: null, startMs: null, topics });
  const lines = [l("a", ["Negation"]), l("b", []), l("c", ["negation", "Numerals"])];
  assert.deepEqual(topicsIn(lines), ["Negation", "Numerals"]);
  const neg = showOf("topic:Negation");
  assert.equal(neg.label, "On «Negation»");
  assert.deepEqual(lines.filter(neg.keep).map((x) => x.id), ["a", "c"], "capitals aside");
  assert.match(showOf("topic:Tone").done, /No sentence of this text is filed under «Tone»/u);
});
