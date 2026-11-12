import assert from "node:assert/strict";
import { test } from "node:test";
import { asExpex, asGb4e, asHtml, asText, clipboardFor, glossRuns, latexEscape, parseFormat, width, wordLines, type Example } from "./example.ts";

const ex: Example = {
  number: 3,
  translation: "The dogs ran.",
  words: [
    { form: "köpekler", morphs: [{ surface: "köpek", type: "root", gloss: "dog" }, { surface: "ler", type: "suffix", gloss: "PL" }] },
    { form: "koştu", morphs: [{ surface: "koş", type: "root", gloss: "run" }, { surface: "tu", type: "suffix", gloss: "PST.3SG" }] },
  ],
};

test("morph and gloss lines carry the same boundary markers", () => {
  assert.deepEqual(wordLines(ex.words[0]!), { morphs: "köpek-ler", glosses: "dog-PL" });
  assert.deepEqual(wordLines({ form: "ev", morphs: [] }), { morphs: "ev", glosses: "" });
  assert.deepEqual(wordLines({ form: "=ki", morphs: [{ surface: "ki", type: "clitic", gloss: null }] }), { morphs: "=ki", glosses: "=?" });
});

test("grammatical pieces are found between Leipzig separators", () => {
  assert.deepEqual(
    glossRuns("dog-PL.ACC").map((r) => [r.text, r.caps]),
    [["dog", false], ["-", false], ["PL", true], [".", false], ["ACC", true]],
  );
});

test("plain text lines up columns, counting a base and its diacritic as one", () => {
  assert.equal(width("ɓ̃a"), 2);
  assert.equal(asText(ex), ["(3) köpek-ler  koş-tu", "    dog-PL     run-PST.3SG", "    ‘The dogs ran.’"].join("\n"));
});

test("gb4e and ExPex set grammatical glosses in small capitals and escape LaTeX", () => {
  const g = asGb4e(ex);
  assert.match(g, /\\gll köpek-ler koş-tu\\\\/);
  assert.match(g, /dog-\\textsc\{pl\} run-\\textsc\{pst\}\.\\textsc\{3sg\}\\\\/);
  assert.match(g, /\\glt `The dogs ran\.'/);
  assert.match(asExpex(ex), /\\glb dog-\\textsc\{pl\} run-\\textsc\{pst\}\.\\textsc\{3sg\}\/\//);
  assert.equal(latexEscape("50% & $_#"), "50\\% \\& \\$\\_\\#");
  // A word with no analysis keeps its column with an empty gloss.
  assert.match(asGb4e({ number: 1, translation: null, words: [{ form: "ev", morphs: [] }, ex.words[0]!] }), /\\gll ev köpek-ler\\\\\n {5}\{\} dog-/);
});

test("the Word format is a table with small capitals and the plain text beside it", () => {
  const html = asHtml(ex);
  assert.match(html, /<table/);
  assert.match(html, /dog-<span style="font-variant: small-caps">pl<\/span>/);
  assert.match(html, /‘The dogs ran\.’/);
  assert.doesNotMatch(asHtml({ ...ex, translation: "<b>" }), /<b>/);
  const both = clipboardFor(ex, "word");
  assert.ok(both.html !== null && both.text.startsWith("(3)"));
  assert.equal(clipboardFor(ex, "gb4e").html, null);
});

test("an unknown remembered format falls back to Word", () => {
  assert.equal(parseFormat(null), "word");
  assert.equal(parseFormat("expex"), "expex");
  assert.equal(parseFormat("pdf"), "word");
});
