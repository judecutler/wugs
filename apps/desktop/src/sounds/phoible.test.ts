import assert from "node:assert/strict";
import { test } from "node:test";
import { againstRelatives, checkInventory, phoibleLine, phonemeKey, shareOf, type PhoibleCompared } from "./phoible.ts";

const ours = [
  { sound: "a", count: 90 },
  { sound: "t͡ʃ", count: 12 },
  { sound: "g", count: 8 },
  { sound: "ɬ", count: 3 },
  { sound: "k", count: 40 },
];

test("sounds are matched apart from tie bars and ASCII look-alikes", () => {
  assert.equal(phonemeKey("t͡ʃ"), phonemeKey("t̠ʃ"));
  assert.equal(phonemeKey("g"), phonemeKey("ɡ"));
  assert.equal(phonemeKey("k'"), "kʼ");
  assert.notEqual(phonemeKey("t̪"), phonemeKey("t"));
});

test("the project's sounds against an inventory and its relatives", () => {
  const check = checkInventory(ours, ["k", "ɡ", "t̠ʃ", "a", "i", "u"]);
  assert.deepEqual(check.notFound, ["i", "u"]);
  assert.deepEqual(check.notListed, ["ɬ"]);
  assert.deepEqual(check.inBoth, ["k", "ɡ", "t̠ʃ", "a"]);
  const c: PhoibleCompared = {
    name: "Kelani",
    own: [{ id: "7", source: "upsid", name: "Kelani", phonemes: ["k"] }],
    comparedWith: "Austronesian languages",
    relatives: 40,
    shares: [
      { phoneme: "a", languages: 40 },
      { phoneme: "i", languages: 39 },
      { phoneme: "k", languages: 38 },
      { phoneme: "ʔ", languages: 21 },
      { phoneme: "t̠ʃ", languages: 10 },
      { phoneme: "ɬ", languages: 1 },
    ],
    edition: "PHOIBLE 2.0",
  };
  const r = againstRelatives(ours, c);
  assert.deepEqual(r.missing.map((s) => s.phoneme), ["i", "ʔ"]);
  assert.deepEqual(r.rare, [{ sound: "g", languages: 0 }, { sound: "ɬ", languages: 1 }]);
  assert.equal(shareOf(21, 40), "21 of 40 (53%)");
  assert.match(phoibleLine(c), /^PHOIBLE has 1 inventory of Kelani \(UPSID\)\. Its relatives are 40 Austronesian languages with an inventory there\./);
  assert.match(phoibleLine({ ...c, own: [], relatives: 0 }), /^PHOIBLE has no inventory of Kelani\. A sound/);
  assert.deepEqual(againstRelatives(ours, { shares: [], relatives: 0 }), { missing: [], rare: [] });
});
