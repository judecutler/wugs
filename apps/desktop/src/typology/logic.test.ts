import { test } from "node:test";
import assert from "node:assert/strict";
import { answeredCount, cldfValues, counts, differs, ourLine, othersLine, ownLine, profileNote, profileTable, shown, unlike, type Answer, type Ours, type Profile } from "./logic.ts";

const a = (id: string, value: string | null, others: [string, number][], label: string | null = null): Answer => ({
  id,
  question: `Question ${id}?`,
  value,
  label,
  others: others.map(([v, count]) => ({ value: v, label: v === "1" ? "present" : "absent", count })),
  othersCoded: others.reduce((n, [, c]) => n + c, 0),
  codes: [
    { value: "0", label: "absent" },
    { value: "1", label: "present" },
  ],
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
  assert.deepEqual(counts(p), { all: 4, open: 2, unlike: 1, differs: 0 });
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

const ours = (...xs: [string, string, string][]): Map<string, Ours> => new Map(xs.map(([id, value, note]) => [id, { id, value, note }]));

test("the project's own answers close questions and show where they differ from the dataset", () => {
  const o = ours(["GB020", "1", "the word «ang», Schachter 1972: 50"], ["GB022", "0", ""], ["GB023", "?", "ask again"], ["GB021", "", "only a note"]);
  assert.deepEqual(counts(p, o), { all: 4, open: 1, unlike: 1, differs: 1 });
  assert.deepEqual(shown(p, "open", "", o).map((x) => x.id), ["GB023"], "a `?` of our own keeps it open");
  assert.equal(differs(p.features[0]!, o), true);
  assert.equal(differs(p.features[2]!, o), false, "the dataset has no answer to differ from");
  assert.deepEqual(shown(p, "all", "schachter", o).map((x) => x.id), ["GB020"], "notes are searched");
  assert.equal(ourLine(p.features[0]!, o.get("GB020")), "present");
  assert.equal(ourLine(p.features[3]!, o.get("GB023")), "Not known");
  assert.equal(ourLine(p.features[1]!, o.get("GB021")), "Not answered");
  assert.equal(ourLine(p.features[1]!, undefined), "Not answered");
  assert.equal(answeredCount(p, o), 3);
});

test("the project's answers copy as a table and as CLDF values", () => {
  const o = ours(["GB020", "1", 'says "ang", often'], ["GB023", "?", ""], ["GB021", "", "only a note"]);
  const table = profileTable(p, o).split("\n");
  assert.equal(table[0], "ID\tQuestion\tAbcd\tAustronesian languages\tThis project\tNote");
  assert.equal(table[1]!.split("\t").slice(4).join("|"), 'present|says "ang", often');
  assert.equal(
    cldfValues(p, o),
    'ID,Language_ID,Parameter_ID,Value,Code_ID,Comment\nabcd1234-GB020,abcd1234,GB020,1,GB020-1,"says ""ang"", often"\nabcd1234-GB023,abcd1234,GB023,?,,\n',
  );
});
