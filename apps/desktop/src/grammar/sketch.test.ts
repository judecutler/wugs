import { test } from "node:test";
import assert from "node:assert/strict";
import { featuresOf, grammarSketch, graphemesOf, morphemeKind, morphemesOf, type SketchInput } from "./sketch.ts";
import { toGrid } from "../orthography/logic.ts";

const base: SketchInput = {
  language: { name: "Kalam", glottocode: "kala1397", iso: "kmh" },
  words: [
    { word: "pata", count: 3 },
    { word: "mbiku", count: 1 },
    { word: "sɨŋ", count: 2 },
  ],
  soundTrack: "IPA",
  graphemes: [{ grapheme: "ng", ipa: "ŋ" }],
  morphemes: [
    { form: "-ok", kind: "suffix", glosses: ["PST"], category: "" },
    { form: "ni-", kind: "prefix", glosses: ["1SG"], category: "" },
  ],
  entries: 12,
  features: [{ id: "GB020", question: "Are there definite articles?", answer: "absent", note: "a <demonstrative> serves" }],
  featureSource: "Grambank 1.0",
  texts: [{ title: "Pig hunt", sentences: 4, words: 20, glossed: 8, morphs: 18 }],
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
  assert.match(html, /has 2\.3 morphemes on average \(18 in 8 words\)/);
  // Consonants by place and manner, vowels, then frequency.
  assert.match(html, /<th>Bilabial<\/th>/);
  assert.match(html, /<th>Plosive<\/th>/);
  assert.match(html, /<td class="ipa">p<\/td>/);
  assert.match(html, /«IPA» track/);
  // Word shapes with their share of running words.
  assert.match(html, /<td>CVCV<\/td><td>50%<\/td><td class="ipa">pata<\/td>/);
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

test("the glossed words' slots become a position-class chart", () => {
  const html = grammarSketch({ ...base, slots: [{ label: "−1", affixes: ["ni- 1SG", "ka- 2SG"] }, { label: "Root", affixes: [] }, { label: "+1", affixes: ["-ra PST"] }] });
  assert.match(html, /<h3>4\.3 Position classes<\/h3>/);
  assert.match(html, /<tr><th>−1<\/th><th>Root<\/th><th>\+1<\/th><\/tr><tr><td class="ipa">ni- 1SG<\/td><td class="ipa">ROOT<\/td><td class="ipa">-ra PST<\/td><\/tr><tr><td class="ipa">ka- 2SG<\/td><td class="ipa"><\/td><td class="ipa"><\/td><\/tr>/);
  assert.doesNotMatch(grammarSketch(base), /Position classes/);
});

test("glosses written more than one way get an allomorphy section after the chart", () => {
  const html = grammarSketch({
    ...base,
    slots: [{ label: "Root", affixes: [] }, { label: "+1", affixes: ["-si PL", "-su PL"] }],
    allomorphs: [{ gloss: "PL", forms: [{ form: "-si", uses: 9, next: "e, i (front vowels)", vowel: "e, i (front vowels)" }, { form: "-su", uses: 6, next: "u, o (back vowels)", vowel: "u, o (back vowels)" }], note: "The stem's last vowel keeps the forms apart." }],
  });
  assert.match(html, /<h3>4\.4 Allomorphy<\/h3>/);
  assert.match(html, /<th colspan="4" class="gl">PL<\/th>/);
  assert.match(html, /<td class="ipa">-su<\/td><td>6<\/td><td class="ipa">u, o \(back vowels\)<\/td>/);
  assert.match(html, /<p>The stem&#39;s last vowel|<p>The stem's last vowel keeps the forms apart\.<\/p>/);
  assert.doesNotMatch(grammarSketch(base), /Allomorphy/);
});

test("an empty project still gives a sketch with places to write", () => {
  const html = grammarSketch({ ...base, language: { name: "", glottocode: "", iso: "" }, words: [], graphemes: [], morphemes: [], features: [], texts: [], entries: 0 });
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

test("word orders counted in the trees open the syntax chapter", () => {
  const o = (label: string, code: string, count: number) => ({ label, code, count, example: null });
  const html = grammarSketch({ ...base, order: { parsed: 80, patterns: [{ id: "83A", title: "Order of object and verb", orders: [o("OV", "1", 40), o("VO", "2", 2)] }, { id: "86A", title: "Order of genitive and noun", orders: [o("Genitive-Noun", "1", 0), o("Noun-Genitive", "2", 0)] }] } });
  assert.match(html, /<h3>5\.1 Word order<\/h3><p>Counted in 80 parsed sentences/);
  assert.match(html, /<tr><td>Order of object and verb \(WALS 83A\)<\/td><td>OV 40, VO 2<\/td><td>OV<\/td><\/tr><\/table>/);
  assert.doesNotMatch(html, /genitive and noun/, "a construction never found is left out");
  assert.match(html, /class="todo">\[Check these against the examples/);
});

test("a locator map and the neighbours open the introduction", async () => {
  const { locatorMap, placeLines, placeWords } = await import("./sketch.ts");
  const place = {
    lat: 53,
    lon: -1,
    family: "Indo-European",
    macroarea: "Eurasia;Africa",
    neighbours: [
      { glottocode: "scot1243", name: "Scots", family: "Indo-European", lat: 56, lon: -3.5, km: 371.4 },
      { glottocode: "wels1247", name: "Welsh", family: "Indo-European", lat: 52, lon: -3.8, km: 214 },
    ],
    land: "M0 0L1 1Z",
  };
  assert.equal(placeWords(place), "53.00° N, 1.00° W");
  assert.deepEqual(placeLines(place, "English"), [
    "Glottolog classifies English in the Indo-European family, in Eurasia, Africa.",
    "The languages Glottolog places nearest are Scots (Indo-European, 371 km) and Welsh (Indo-European, 214 km).",
  ]);
  assert.match(placeLines({ ...place, family: "isolate", neighbours: [] }, "Basque")[0]!, /as an isolate/u);
  const svg = locatorMap(place, "English");
  assert.match(svg, /viewBox="154 22 50 30"/u);
  assert.equal((svg.match(/<path /gu) ?? []).length, 3, "the land and a copy either side");
  assert.match(svg, /<title>Scots<\/title>/u);
  // Across 180°: a neighbour on the far side is drawn beside, not across the world.
  const fiji = locatorMap({ ...place, lat: -17, lon: 179, neighbours: [{ ...place.neighbours[0]!, lat: -14, lon: -178 }] }, "Fijian");
  assert.match(fiji, /cx="362" cy="104"/u);
  const html = grammarSketch({ ...base, place });
  assert.match(html, /Figure 1\. Where Glottolog places/u);
  assert.match(grammarSketch(base), /Where .* is spoken, by whom, its family and neighbours/u);
});

test("similar sounds that keep apart are listed in the phonology chapter", () => {
  const words = [
    { word: "pada", count: 4 },
    { word: "kodu", count: 3 },
    { word: "pat", count: 4 },
    { word: "kut", count: 2 },
    { word: "pipi", count: 5 },
  ];
  const html = grammarSketch({ ...base, words });
  assert.match(html, /may be allophones of one phoneme/);
  assert.match(html, /<td class="ipa">\/d\/ → \[t\] \/ _ a word edge<\/td>/);
});

test("tone-marked words get a tone section with their melodies; unmarked ones none", () => {
  assert.doesNotMatch(grammarSketch(base), /2\.5 Tone/);
  const html = grammarSketch({ ...base, words: [{ word: "bálà", count: 3 }, { word: "kálá", count: 2 }, { word: "bá", count: 1 }] });
  assert.match(html, /<h3>2\.5 Tone<\/h3>/);
  assert.match(html, /<td class="ipa">H\.L<\/td><td>1<\/td><td class="ipa">bálà<\/td>/);
  assert.match(html, /never met: L\.H, L\.L/);
});

test("endings shared by stems in the words get a section, even with nothing glossed", () => {
  assert.doesNotMatch(grammarSketch(base), /Endings the words suggest/);
  const words = ["kanu", "kanula", "kanuli", "tepa", "tepala", "tepali", "siro", "sirola", "siroli"].map((word) => ({ word, count: 2 }));
  const html = grammarSketch({ ...base, words });
  assert.match(html, /<h3>4\.\d Endings the words suggest<\/h3>/);
  assert.match(html, /<td class="ipa">∅ -la -li<\/td><td>3<\/td>/);
});

test("vowel harmony shows in the vowels section", () => {
  const front = ["pi", "ti", "ki", "me", "le", "se"];
  const back = ["pu", "tu", "ku", "mo", "lo", "so"];
  const words = [...front.flatMap((a) => front.map((b) => a + b)), ...back.flatMap((a) => back.map((b) => a + b))].map((word) => ({ word, count: 1 }));
  const html = grammarSketch({ ...base, words });
  assert.match(html, /pairs of a vowel and the next vowel in a word.*avoided.*may point to vowel harmony/su);
  assert.doesNotMatch(grammarSketch(base), /may point to vowel harmony/u, "no standout, no paragraph");
});

test("loanwords give the introduction a contact section", () => {
  const html = grammarSketch({ ...base, entries: 40, loans: [{ language: "Malay", count: 6 }, { language: "Dutch", count: 2 }] });
  assert.match(html, /<h3>1\.1 Contact and loanwords<\/h3><p>8 entries are marked as loanwords \(20% of the lexicon\): Malay 6, Dutch 2\.<\/p>/);
  assert.doesNotMatch(grammarSketch(base), /Contact and loanwords/);
});
