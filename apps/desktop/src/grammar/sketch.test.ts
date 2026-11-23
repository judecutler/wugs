import { test } from "node:test";
import assert from "node:assert/strict";
import { featuresOf, grammarSketch, graphemesOf, morphemeKind, morphemesOf, type SketchInput } from "./sketch.ts";
import { toGrid } from "../orthography/logic.ts";
import { inventory } from "../sounds/ipa.ts";

const base: SketchInput = {
  language: { name: "Kalam", glottocode: "kala1397", iso: "kmh" },
  sounds: inventory([
    { word: "pata", count: 3 },
    { word: "mbiku", count: 1 },
    { word: "sɨŋ", count: 2 },
  ]),
  soundTrack: "IPA",
  graphemes: [{ grapheme: "ng", ipa: "ŋ" }],
  morphemes: [
    { form: "-ok", kind: "suffix", glosses: ["PST"], category: "" },
    { form: "ni-", kind: "prefix", glosses: ["1SG"], category: "" },
  ],
  entries: 12,
  features: [{ id: "GB020", question: "Are there definite articles?", answer: "absent", note: "a <demonstrative> serves" }],
  featureSource: "Grambank 1.0",
  texts: [{ title: "Pig hunt", sentences: 4, words: 20, glossed: 8 }],
  date: "2026-11-23",
  generator: "wugs 0.4",
};

test("a lemma's markers say what kind of morpheme it is", () => {
  assert.equal(morphemeKind("-ler"), "suffix");
  assert.equal(morphemeKind("ni-"), "prefix");
  assert.equal(morphemeKind("=ma"), "enclitic");
  assert.equal(morphemeKind("ma="), "proclitic");
  assert.equal(morphemeKind("<um>"), "infix");
  assert.equal(morphemeKind("kuti"), null);
  assert.equal(morphemeKind("-"), null);
  assert.equal(morphemeKind("-a-"), null);
});

test("the sketch lays out what the project knows, chapter by chapter", () => {
  const html = grammarSketch(base);
  assert.match(html, /^<!DOCTYPE html>/);
  assert.match(html, /<h1>A grammar sketch of Kalam<\/h1>/);
  assert.match(html, /Glottocode kala1397, ISO 639-3 kmh/);
  assert.match(html, /1 text \(20 words, 8 of them glossed\) and a lexicon of 12 entries/);
  // Consonants by place and manner, vowels, then frequency.
  assert.match(html, /<th>Bilabial<\/th>/);
  assert.match(html, /<th>Plosive<\/th>/);
  assert.match(html, /<td class="ipa">p<\/td>/);
  assert.match(html, /«IPA» track/);
  // Orthography and morphology.
  assert.match(html, /<td>ng<\/td><td class="ipa">ŋ<\/td>/);
  assert.ok(html.indexOf("Prefixes") < html.indexOf("Suffixes"));
  assert.match(html, /<td class="gl">PST<\/td>/);
  // Typology, escaped.
  assert.match(html, /GB020/);
  assert.match(html, /a &lt;demonstrative&gt; serves/);
  assert.match(html, /<h2>7\. Texts<\/h2>/);
  // Places to write.
  assert.match(html, /class="todo">\[Word order/);
});

test("an empty project still gives a sketch with places to write", () => {
  const html = grammarSketch({ ...base, language: { name: "", glottocode: "", iso: "" }, sounds: [], graphemes: [], morphemes: [], features: [], texts: [], entries: 0 });
  assert.match(html, /A grammar sketch of the language/);
  assert.match(html, /No transcriptions to read sounds from yet/);
  assert.match(html, /orthography profile/);
  assert.match(html, /Affixes and clitics/);
  assert.doesNotMatch(html, /Typological profile/);
  assert.doesNotMatch(html, /Texts<\/h2>/);
});

test("the lexicon, the profile and the typology answers become the sketch's tables", () => {
  const ms = morphemesOf([
    { lemma: "kuti", glosses: ["dog"], posOfSpeech: "n" },
    { lemma: "-ok", glosses: ["PST", ""], posOfSpeech: "" },
  ]);
  assert.deepEqual(ms, [{ form: "-ok", kind: "suffix", glosses: ["PST"], category: "" }]);
  assert.deepEqual(graphemesOf(toGrid("# spelling\nGrapheme\tFrequency\tIPA\nng\t3\tŋ\nsh\t1\t\n")), [
    { grapheme: "ng", ipa: "ŋ" },
    { grapheme: "sh", ipa: "" },
  ]);
  assert.deepEqual(graphemesOf(toGrid("Grapheme\tPhoneme\nc\tt͡ʃ\n")), [{ grapheme: "c", ipa: "t͡ʃ" }]);
  assert.deepEqual(graphemesOf(toGrid("Letter\tIPA\nc\tx\n")), []);
  const answer = (id: string) => ({ id, question: `Q ${id}`, value: null, label: null, others: [], othersCoded: 0, codes: [{ value: "0", label: "absent" }, { value: "1", label: "present" }] });
  assert.deepEqual(
    featuresOf([answer("GB1"), answer("GB2"), answer("GB3"), answer("GB4")], [
      { id: "GB3", value: "1", note: "" },
      { id: "GB1", value: "?", note: "unclear" },
      { id: "GB4", value: "", note: "" },
    ]),
    [
      { id: "GB1", question: "Q GB1", answer: "not known", note: "unclear" },
      { id: "GB3", question: "Q GB3", answer: "present", note: "" },
    ],
  );
});
