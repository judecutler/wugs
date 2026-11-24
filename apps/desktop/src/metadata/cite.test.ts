import { test } from "node:test";
import assert from "node:assert/strict";
import { citation, people, yearOf } from "./cite.ts";

const lang = { name: "Kanu", glottocode: "kanu1234", iso: "" };

test("a recorded text is cited by its speakers, year, archive and handle", () => {
  const d = new Map([
    ["speakers", "Awi Heole; Ilawi Amosa"],
    ["date", "2010-06-06"],
    ["genre", "procedural"],
    ["place", "Huya"],
    ["archive", "ELAR"],
    ["handle", "https://hdl.handle.net/2196/0001"],
  ]);
  const c = citation("How we catch fish", d, lang);
  assert.equal(c.text, "Heole, Awi & Ilawi Amosa (speakers). 2010. How we catch fish. Procedural in Kanu [kanu1234], Huya. ELAR. https://hdl.handle.net/2196/0001");
  assert.equal(
    c.bibtex,
    "@misc{heole2010,\n  author = {Heole, Awi and Amosa, Ilawi},\n  title = {{How we catch fish}},\n  year = {2010},\n  howpublished = {Procedural in Kanu [kanu1234], Huya},\n  note = {ELAR},\n  url = {https://hdl.handle.net/2196/0001},\n}\n",
  );
});

test("a written text keeps its author, and a DOI becomes a link", () => {
  const c = citation("Letters & notes", new Map([["author", "Mary Smith"], ["year", "1923"], ["doi", "10.1234/abc"]]), { name: "", glottocode: "", iso: "" });
  assert.equal(c.text, "Smith, Mary. 1923. Letters & notes. Text. https://doi.org/10.1234/abc");
  assert.match(c.bibtex, /title = \{\{Letters \\& notes\}\}/);
  assert.match(c.bibtex, /doi = \{10\.1234\/abc\}/);
});

test("nothing known gives a year of n.d.", () => {
  assert.equal(citation("", new Map(), lang).text, "n.d. Untitled. Text in Kanu [kanu1234].");
  assert.deepEqual(people("A and B & C"), ["A", "B", "C"]);
  assert.equal(yearOf("c. 1890s"), "1890");
});
