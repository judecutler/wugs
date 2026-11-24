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
  assert.match(html, /dog-<span style="font-size: 80%">PL<\/span>/);
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

test("a gloss with a space stays one LaTeX column; infix brackets print as < and >", () => {
  const ex2: Example = {
    number: 1,
    translation: null,
    words: [
      { form: "lumabas", morphs: [{ surface: "l", type: "root", gloss: "go out" }, { surface: "um", type: "infix", gloss: "AV" }, { surface: "abas", type: "root", gloss: null }] },
      { form: "na", morphs: [{ surface: "na", type: "root", gloss: "already" }] },
    ],
  };
  const g = asGb4e(ex2);
  assert.match(g, /\\gll l\\textless\{\}um\\textgreater\{\}abas na\\\\/);
  assert.match(g, /\{go out\\textless\{\}\\textsc\{av\}\\textgreater\{\}\} already\\\\/);
  // ExPex drops a lone group's braces and the space inside one, so its spaces are `\ `.
  assert.match(asExpex(ex2), /\\glb \{go\\ out\\textless/);
  const lone: Example = { number: 1, translation: null, words: [{ form: "a b", morphs: [{ surface: "a b", type: "root", gloss: "go out" }] }] };
  assert.match(asExpex(lone), /\\gla \{a\\ b\}\/\/\n\\glb \{go\\ out\}\/\//);
  assert.match(asGb4e(lone), /\\gll \{a b\}\\\\\n     \{go out\}\\\\/);
  // No translation: no empty quotes.
  assert.doesNotMatch(g, /\\glt/);
  assert.doesNotMatch(asExpex(ex2), /\\glft/);
});

test("the root around an infix is glossed once", () => {
  const w = { form: "sumulat", morphs: [{ surface: "s", type: "root", gloss: "write" }, { surface: "um", type: "infix", gloss: "AV" }, { surface: "ulat", type: "root", gloss: "write" }] };
  assert.deepEqual(wordLines(w), { morphs: "s<um>ulat", glosses: "write<AV>" });
  assert.equal(wordLines({ ...w, morphs: [w.morphs[0]!, w.morphs[1]!, { ...w.morphs[2]!, gloss: null }] }).glosses, "write<AV>");
});

test("several examples go on the clipboard one after another", async () => {
  const { clipboardForAll } = await import("./example.ts");
  const ex = (n: number, gloss: string) => ({ number: n, translation: `t${n}`, words: [{ form: "kesi", morphs: [{ surface: "kesi", type: "root", gloss }] }] });
  const both = clipboardForAll([ex(1, "three"), ex(2, "three")], "text");
  assert.equal(both.html, null);
  assert.equal(both.text.split("\n\n").length, 2);
  assert.match(both.text, /^\(1\)/);
  const word = clipboardForAll([ex(1, "three"), ex(2, "three")], "word");
  assert.equal((word.html ?? "").match(/<table/g)?.length, 2);
});

test("an example can name its source after the translation", async () => {
  const { asText, asGb4e, asExpex, asHtml } = await import("./example.ts");
  const ex = { number: 2, translation: "I count one.", source: "Heole 2010: 12", words: [{ form: "nikani", morphs: [{ surface: "ni", type: "prefix", gloss: "1SG" }, { surface: "kani", type: "root", gloss: "count" }] }] };
  assert.match(asText(ex), /‘I count one\.’ \(Heole 2010: 12\)$/);
  assert.match(asGb4e(ex), /\\glt `I count one\.' \(Heole 2010: 12\)/);
  assert.match(asExpex(ex), /\\glft `I count one\.' \(Heole 2010: 12\)\/\//);
  assert.match(asHtml(ex), /‘I count one\.’ \(Heole 2010: 12\)/);
  assert.match(asText({ ...ex, translation: null }), /\(Heole 2010: 12\)$/, "a source with no translation still shows");
});
