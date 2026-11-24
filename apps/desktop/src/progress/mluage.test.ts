import { test } from "node:test";
import assert from "node:assert/strict";
import { ageLabel, agePoints, ageNote, ageTicks, ageTsv, chatAgeMonths, inMorphemes, monthsBetween, type TextLength } from "./mluage.ts";

const t = (title: string, age: string, date: string, utterances: number, words: number, analysed: number, morphemes: number): TextLength => ({ text: title, title, age, date, utterances, words, analysed, morphemes });

test("ages read as CHAT writes them, or as whole years", () => {
  assert.equal(chatAgeMonths("2;06"), 30);
  assert.ok(Math.abs(chatAgeMonths("2;06.15")! - 30.49) < 0.01);
  assert.equal(chatAgeMonths("3;"), 36);
  assert.equal(chatAgeMonths("4"), 48);
  assert.equal(chatAgeMonths("2;13"), null, "no thirteenth month");
  assert.equal(chatAgeMonths("toddler"), null);
  assert.equal(chatAgeMonths(""), null);
});

test("an age from a birth date and a recording date", () => {
  assert.ok(Math.abs(monthsBetween("2020-01-15", "2022-07-15")! - 30) < 0.1);
  assert.ok(Math.abs(monthsBetween("2020-01", "2021-01-01")! - 12) < 0.1);
  assert.equal(monthsBetween("2022-01-01", "2020-01-01"), null, "recorded before birth");
  assert.equal(monthsBetween("", "2020-01-01"), null);
});

test("points: each text with an age, youngest first, its MLU", () => {
  const points = agePoints({
    born: "2019-06-01",
    byText: [t("park", "2;09.01", "", 40, 120, 40, 140), t("bath", "2;03", "", 100, 150, 100, 180), t("lunch", "", "2022-01-01", 50, 100, 0, 0), t("none", "", "", 10, 20, 0, 0), t("empty", "2;00", "", 0, 0, 0, 0)],
  });
  assert.deepEqual(points.map((p) => p.title), ["bath", "lunch", "park"], "lunch's age comes from its date; a text with no age or no utterance is left out");
  assert.equal(points[0]!.mluMorphemes, 1.8);
  assert.equal(points[1]!.mluMorphemes, null);
  assert.equal(points[2]!.mluWords, 3);
  assert.ok(!inMorphemes(points));
  assert.ok(inMorphemes([points[0]!, points[2]!]));
});

test("labels, ticks, note and table", () => {
  assert.equal(ageLabel(30.49), "2;06");
  assert.equal(ageLabel(36), "3;00");
  assert.deepEqual(ageTicks(27, 33), [27, 28, 29, 30, 31, 32, 33]);
  assert.deepEqual(ageTicks(24, 48), [24, 27, 30, 33, 36, 39, 42, 45, 48]);
  assert.deepEqual(ageTicks(20, 70), [24, 36, 48, 60]);
  const points = agePoints({ born: "", byText: [t("bath", "2;03", "", 100, 150, 100, 180), t("park", "2;09", "", 40, 120, 40, 140)] });
  assert.equal(ageNote("CHI", points, true), "CHI's MLU in morphemes rose from 1.80 at 2;03 to 3.50 at 2;09 over 2 sessions (Brown's stage I to IV). One session has fewer than 50 utterances, so its MLU is rough; Brown counted 100.");
  assert.equal(ageNote("CHI", points.slice(0, 1), true), "", "one session is no change");
  assert.match(ageNote("MOT", points, false), /^MOT's MLU in morphemes rose .* over 2 sessions\. One/u, "no stages for an adult");
  assert.equal(ageTsv(points).split("\n")[1], "bath\t2;03\t27.0\t100\t1.50\t1.80");
});
