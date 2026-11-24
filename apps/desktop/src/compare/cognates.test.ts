import assert from "node:assert/strict";
import { test } from "node:test";
import { cognacy, cognacyLine, edictorWithCognates, judgeKey, judgementsText, nextJudgement, parseJudgements } from "./cognates.ts";
import { compareRows, type OtherForm, type OurConcept } from "./logic.ts";

const concepts: OurConcept[] = [
  { id: "c1", number: 1, gloss: "hand", concepticonGloss: "HAND", answers: [{ lemma: "lima" }] },
  { id: "c2", number: 2, gloss: "eye", concepticonGloss: "EYE", answers: [{ lemma: "mata" }] },
  { id: "c3", number: 3, gloss: "dog", concepticonGloss: "DOG", answers: [{ lemma: "asu" }] },
];
const others: OtherForm[] = [
  { language: "Teke", concept: "hand", form: "nima" },
  { language: "Teke", concept: "eye", form: "maʔa" },
  { language: "Teke", concept: "dog", form: "wolo" },
  { language: "Sopo", concept: "hand", form: "kubu" },
];

test("judgements round-trip through the kept table", () => {
  const j = parseJudgements("y\tTeke\thand\tnima\nn\tTeke\tdog\twolo\nmaybe\tx\ty\tz\n");
  assert.equal(j.size, 2);
  assert.equal(j.get(judgeKey("Teke", " Hand ", "nima")), "y");
  assert.equal(judgementsText(j), "n\tTeke\tdog\twolo\ny\tTeke\thand\tnima");
  assert.equal(nextJudgement(undefined), "y");
  assert.equal(nextJudgement("y"), "n");
  assert.equal(nextJudgement("n"), undefined);
});

test("cognate shares and cognate sets", () => {
  const rows = compareRows(concepts, others);
  const j = parseJudgements("y\tTeke\thand\tnima\ny\tTeke\teye\tmaʔa\nn\tTeke\tdog\twolo\nn\tSopo\thand\tkubu");
  const c = cognacy(rows, ["Teke", "Sopo", "Moru"], j);
  assert.deepEqual(c, [
    { language: "Teke", judged: 3, cognate: 2 },
    { language: "Sopo", judged: 1, cognate: 0 },
    { language: "Moru", judged: 0, cognate: 0 },
  ]);
  assert.equal(cognacyLine(c[0]!), "Teke: 2 of 3 judged concepts cognate (67%).");
  assert.equal(cognacyLine(c[2]!), "Moru: nothing judged yet.");
  const tsv = edictorWithCognates(rows, "Kanu", j).trim().split("\n");
  assert.equal(tsv[0], "ID\tDOCULECT\tCONCEPT\tFORM\tTOKENS\tCOGID");
  const cog = (form: string) => tsv.find((l) => l.split("\t")[3] === form)!.split("\t")[5];
  assert.equal(cog("lima"), cog("nima"), "judged cognate: one set");
  assert.notEqual(cog("lima"), cog("kubu"));
  assert.notEqual(cog("asu"), cog("wolo"));
  assert.equal(new Set(tsv.slice(1).map((l) => l.split("\t")[5])).size, 5);
});
