import assert from "node:assert/strict";
import { test } from "node:test";
import { align, correspondenceLine, correspondences, correspondenceTsv, GAP } from "./correspondences.ts";
import { compareRows, type OtherForm, type OurConcept } from "./logic.ts";

test("alignment lines vowels up with vowels", () => {
  assert.deepEqual(align("lima", "nima"), [["l", "n"], ["i", "i"], ["m", "m"], ["a", "a"]]);
  assert.deepEqual(align("mata", "maʔa"), [["m", "m"], ["a", "a"], ["t", "ʔ"], ["a", "a"]]);
  // A dropped final vowel is a gap, not a consonant paired with a vowel.
  assert.deepEqual(align("tela", "tel"), [["t", "t"], ["e", "e"], ["l", "l"], ["a", GAP]]);
  // Sounds, not letters: a long vowel is one sound; markers ignored.
  assert.deepEqual(align("taː-", "to"), [["t", "t"], ["aː", "o"]]);
});

const concept = (n: number, gloss: string, lemma: string): OurConcept => ({ id: `c${n}`, number: n, gloss, concepticonGloss: gloss.toUpperCase(), answers: [{ lemma }] });
const ours = [concept(1, "hand", "lima"), concept(2, "eye", "mata"), concept(3, "louse", "kutu"), concept(4, "fire", "api"), concept(5, "dog", "asu")];
const theirs: OtherForm[] = [
  { language: "Teke", concept: "hand", form: "nima" },
  { language: "Teke", concept: "eye", form: "maʔa" },
  { language: "Teke", concept: "louse", form: "kuʔu" },
  { language: "Teke", concept: "fire", form: "afi" },
  { language: "Teke", concept: "dog", form: "wolo" },
];

test("recurring correspondences are counted across concepts", () => {
  const rows = compareRows(ours, theirs);
  const c = correspondences(rows, "Teke", 0.5);
  assert.equal(c.pairs, 4, "wolo is too unlike asu to align");
  const t = c.list.find((x) => x.ours === "t" && x.theirs === "ʔ")!;
  assert.equal(t.count, 2);
  assert.deepEqual(t.concepts, ["eye", "louse"]);
  assert.equal(c.list[0]!.ours, "l", "ordered by the project's sounds as first met");
  assert.ok(c.list.find((x) => x.ours === "a" && x.theirs === "a")!.same);
  assert.match(correspondenceLine(c, "Teke", 0.5), /^4 pairs of words at least 50% alike, aligned sound by sound\. 1 correspondence of different sounds recurs;/);
  assert.match(correspondenceLine(correspondences(rows, "Teke", 0.99), "Teke", 0.99), /^No word of Teke is 99% alike/);
  assert.equal(correspondenceTsv(c, "Teke").split("\n")[0], "This project\tTeke\tCount\tConcepts");
  assert.ok(correspondenceTsv(c, "Teke").includes("t\tʔ\t2\teye, louse\n"));
});
