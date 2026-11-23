import { test } from "node:test";
import assert from "node:assert/strict";
import { counts, othersLine, ownLine, profileNote, profileTable, shown, unlike, type Answer, type Profile } from "./logic.ts";

const a = (id: string, value: string | null, others: [string, number][], label: string | null = null): Answer => ({
  id,
  question: `Question ${id}?`,
  value,
  label,
  others: others.map(([v, count]) => ({ value: v, label: v === "1" ? "present" : "absent", count })),
  othersCoded: others.reduce((n, [, c]) => n + c, 0),
});

const p: Profile = {
  glottocode: "abcd1234",
  name: "Abcd",
  codedAs: "abcd1234",
  codedName: "Abcd",
  comparedWith: "Austronesian languages",
  comparedCount: 300,
  edition: "Grambank 1.0 (CC BY 4.0)",
  page: "https://grambank.clld.org/languages/abcd1234",
  features: [a("GB020", "0", [["1", 80], ["0", 20]], "absent"), a("GB021", "1", [["1", 70], ["0", 30]], "present"), a("GB022", null, [["0", 3]]), a("GB023", "?", [])],
};

test("a value unlike most relatives is found, and only with enough of them", () => {
  assert.equal(unlike(p.features[0]!), true);
  assert.equal(unlike(p.features[1]!), false);
  assert.equal(unlike(a("GB1", "0", [["1", 3]])), false, "three relatives say too little");
  assert.equal(unlike(a("GB1", "0", [["1", 5], ["2", 5], ["0", 1]])), false, "no majority");
});

test("the list can show everything, the open questions or the unusual answers", () => {
  assert.deepEqual(counts(p), { all: 4, open: 2, unlike: 1 });
  assert.deepEqual(shown(p, "open", "").map((x) => x.id), ["GB022", "GB023"]);
  assert.deepEqual(shown(p, "all", "gb02").length, 4);
  assert.deepEqual(shown(p, "all", "question gb021").map((x) => x.id), ["GB021"]);
});

test("answers are said in words", () => {
  assert.equal(ownLine(p.features[0]!), "absent");
  assert.equal(ownLine(p.features[2]!), "Not coded");
  assert.equal(ownLine(p.features[3]!), "Not known");
  assert.equal(othersLine(p.features[0]!), "present 80%, absent 20% of 100");
  assert.equal(othersLine(p.features[3]!), "None of them coded");
  assert.equal(profileNote(p, "grambank"), "Grambank's answers for Abcd. Compared with 300 Austronesian languages in Grambank.");
  assert.match(profileNote({ ...p, codedAs: null }, "wals"), /^WALS has not coded Abcd: the features below are a checklist/u);
  assert.match(profileNote({ ...p, codedAs: "efgh1234", codedName: "Efgh" }, "grambank"), /dialect: these are Grambank's answers for Efgh/u);
  assert.equal(profileTable(p).split("\n")[1], "GB020\tQuestion GB020?\tabsent\tpresent 80%, absent 20% of 100");
});
