import { test } from "node:test";
import assert from "node:assert/strict";
import { addedLine, consentLine, consentNote, driftSummary, settledLine, exampleGapsLine, examplesAddedLine, treesLine, addLabel, cardNote, findable, glyph, inventoryNote, missingSummary, summary, unifiedLine, useLabel, type CharRow, type Inconsistent, type Inventory } from "./logic.ts";

const v = (gloss: string, count: number) => ({ gloss, count, textId: "t", textTitle: "T", tierId: "u", segmentId: "s" });
const ka: Inconsistent = { surface: "ka", kind: "suffix", key: "ka", total: 3, variants: [v("PL", 2), v("pl.", 1)], slip: true };
const mi: Inconsistent = { surface: "mi", kind: "root", key: "mi", total: 3, variants: [v("water", 2), v("drink", 1)], slip: false };

test("the summary counts morphs and slips", () => {
  assert.equal(summary([]), "Every morph is glossed one way. Nothing to check.");
  assert.equal(summary([ka, mi]), "2 morphs are glossed more than one way. 1 looks like a typing slip (the glosses differ only in capitals, spaces or dots).");
  assert.equal(summary([mi]), "1 morph is glossed more than one way.");
});

test("cards and buttons say what will happen", () => {
  assert.match(cardNote(ka), /slip/u);
  assert.match(cardNote(mi), /lexicon entry/u);
  assert.equal(useLabel(ka.variants[0]!, ka, false), "Use «PL» for all 3");
  assert.equal(useLabel(ka.variants[0]!, ka, true), "Change 1: sure?");
  assert.equal(unifiedLine(1, "PL"), "1 morph now reads «PL».");
  assert.equal(unifiedLine(0, "PL"), "Nothing needed changing any more.");
});

test("roots with no entry are counted and added in words", () => {
  assert.equal(missingSummary([]), "Every glossed root has a lexicon entry.");
  const rows = [{ surface: "mi", key: "mi", total: 3, variants: [v("water", 2), v("drink", 1)] }, { surface: "to", key: "to", total: 1, variants: [v("dog", 1)] }];
  assert.equal(missingSummary(rows), "2 roots are glossed in the texts (4 uses) but not in the lexicon yet.");
  assert.equal(addLabel(rows[0]!), "Add with 2 senses");
  assert.equal(addLabel(rows[1]!), "Add to lexicon");
  assert.equal(addedLine({ entries: 1, senses: 2, linked: 3 }), "Added 1 entry with 2 senses, and linked 3 uses in the texts to them.");
  assert.equal(addedLine({ entries: 0, senses: 0, linked: 0 }), "Nothing to add any more.");
});

test("the character inventory reads in words", () => {
  const row = (ch: string, kind: CharRow["kind"], flags: string[] = []): CharRow => ({ ch, code: `U+${ch.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}`, name: "", script: "Latin", kind, inTexts: 1, inLexicon: 0, place: null, entry: null, flags });
  assert.equal(glyph(row("́", "mark")), "◌́");
  assert.equal(glyph(row(" ", "invisible")), "U+00A0");
  assert.equal(glyph(row("ɡ", "letter")), "ɡ");
  assert.equal(findable(row("ɡ", "letter")), true);
  assert.equal(findable(row("?", "other")), false);
  assert.equal(findable(row(",", "other")), false);
  const inv: Inventory = { rows: [row("a", "letter"), row("а", "letter", ["A Cyrillic letter among Latin ones."]), row("́", "mark")], mainScript: "Latin", notNfc: 0 };
  assert.equal(inventoryNote(inv), "3 characters in use: 2 letters, mostly Latin, 1 combining mark. 1 is worth a look:");
  assert.equal(inventoryNote({ ...inv, rows: [row("a", "letter"), row("b", "letter")] }), "2 characters in use: 2 letters, mostly Latin. Nothing looks like a slip.");
  assert.match(inventoryNote({ ...inv, notNfc: 2 }), /2 transcriptions or headwords are stored decomposed/u);
  assert.equal(inventoryNote({ rows: [], mainScript: "", notNfc: 0 }), "No transcriptions or lexicon entries yet.");
});

test("example sentences still to find", () => {
  assert.equal(exampleGapsLine({ without: 4, fillable: 4 }), "4 senses have no example sentence; all are glossed in a text that is open, so wugs can take one from there.");
  assert.equal(exampleGapsLine({ without: 4, fillable: 1 }), "4 senses have no example sentence; 1 of them is glossed in a text that is open, so wugs can take one from there.");
  assert.equal(exampleGapsLine({ without: 1, fillable: 0 }), "1 sense has no example sentence, and none is glossed in a text that is open yet.");
  assert.equal(examplesAddedLine(2), "Added 2 example sentences from the texts, each with its translation where it has one. Edit them in the Lexicon.");
});

test("dependency trees with a problem", () => {
  assert.equal(treesLine(0, 12, 0), "All 12 parsed sentences have a well-formed tree: one root, no cycles, every head a word of the sentence.");
  assert.equal(treesLine(2, 12, 2), "2 of 12 parsed sentences have a tree problem. Click one to correct it in the Sentence panel.");
  assert.equal(treesLine(1, 4, 1), "1 of 4 parsed sentences has a tree problem. Click one to correct it in the Sentence panel.");
  assert.equal(treesLine(600, 900, 500), "600 of 900 parsed sentences have a tree problem. Click one to correct it in the Sentence panel. The first 500 are listed.");
});

test("glosses unlike their sense", () => {
  assert.equal(driftSummary([]), "Every word linked to the lexicon is glossed as its sense is.");
  const v = { gloss: "uno", count: 3, textId: "t", textTitle: "T", tierId: "u", segmentId: "s" };
  const d = { senseId: "s1", entryId: "e1", headword: "wan", homograph: null, senseGloss: "one", textGloss: "uno", count: 3, first: v };
  assert.equal(driftSummary([d]), "3 linked words are glossed in the texts unlike the lexicon sense they link to (1 gloss). Keep one of the two for each.");
  assert.equal(driftSummary([{ ...d, count: 1 }]), "1 linked word is glossed in the texts unlike the lexicon sense it links to (1 gloss). Keep one of the two for each.");
  assert.equal(settledLine(3, true, "one"), "3 words now read «one», as in the lexicon.");
  assert.equal(settledLine(1, false, "uno"), "The lexicon now glosses this sense «uno».");
});

test("consent records, in words", () => {
  const label = (t: string) => ({ "web-ok": "Publish online" })[t] ?? t;
  assert.equal(consentLine({ texts: [], total: 0, records: 0 }), "");
  assert.match(consentLine({ texts: [{ textId: "a", title: "A", records: 0, no: [], communityOnly: false }], total: 1, records: 0 }), /^No consent records yet/);
  assert.equal(consentLine({ texts: [], total: 3, records: 2 }), "Every text is covered by a consent record that limits nothing.");
  const two = [
    { textId: "a", title: "A", records: 0, no: [], communityOnly: false },
    { textId: "b", title: "B", records: 1, no: ["web-ok"], communityOnly: true },
  ];
  assert.equal(consentLine({ texts: two, total: 4, records: 3 }), "1 text has no consent record covering it; 1 text is covered by a record that limits it. Records inform what you share; they never block your work.");
  assert.equal(consentNote(two[0]!, label), "No consent record covers this text, its recording or the whole project.");
  assert.equal(consentNote(two[1]!, label), "A record: no to «Publish online», shared within the community only.");
});

test("segment timing in words", async () => {
  const { timingLine, timingNote } = await import("./logic.ts");
  const i = { kind: "overlap" as const, textId: "t", textTitle: "T", tierId: "u", segmentId: "s", sentence: "two", startMs: 800, endMs: 2000, other: "s0", overlapMs: 200 };
  assert.equal(timingNote(i), "Starts 0.20 s before the segment before it ends (0.80 s–2.00 s)");
  assert.equal(timingNote({ ...i, kind: "long", startMs: 3000, endMs: 40_000 }), "37.0 s long (3.00 s–40.0 s): split it for speech tools");
  assert.match(timingLine({ timed: 5, overlaps: 1, short: 1, long: 0, issues: [] }), /^Of 5 timed segments, 1 overlaps the one before it on its track, 1 is under 0\.15 s\./);
  assert.match(timingLine({ timed: 3, overlaps: 0, short: 0, long: 0, issues: [] }), /^All 3 timed segments are fine/);
  assert.equal(timingLine({ timed: 0, overlaps: 0, short: 0, long: 0, issues: [] }), "No segment has times yet.");
});

test("archive readiness in words", async () => {
  const { readinessLine, readinessNote } = await import("./logic.ts");
  const t = { textId: "t", title: "Fox", missing: ["date", "speakers", "recording", "consent", "transcription", "translation"], sentences: 10, transcribed: 7, translated: 4 };
  assert.equal(readinessNote(t), "Missing: date, speakers, recording and consent record; 3 of 10 sentences still to transcribe; 3 transcribed sentences without a translation.");
  assert.equal(readinessNote({ ...t, missing: ["transcription"], sentences: 0, transcribed: 0 }), "Nothing transcribed yet.");
  assert.match(readinessLine({ total: 3, ready: 1, texts: [t], language: ["glottocode", "iso639_3"] }), /^1 of 3 texts have what .* no Glottocode or ISO 639-3 code yet/u);
  assert.match(readinessLine({ total: 2, ready: 2, texts: [], language: [] }), /^Every text has/u);
});

test("sentences written twice are counted in words", async () => {
  const { twiceLine, twiceNote } = await import("./logic.ts");
  const copy = (textId: string, textTitle: string) => ({ textId, textTitle, tierId: "u", segmentId: `${textId}-1` });
  const t = { sentence: "kuri mopa tela senu", copies: [copy("a", "Fish"), copy("a", "Fish"), copy("b", "Fish again")] };
  assert.equal(twiceNote(t), "3 times in 2 texts");
  assert.equal(twiceNote({ ...t, copies: t.copies.slice(0, 2) }), "2 times in «Fish»");
  assert.match(twiceLine({ listed: [t], groups: 1, extra: 2, read: 40 }), /^1 sentence is written more than once .*2 copies beyond the first/);
  assert.equal(twiceLine({ listed: [], groups: 0, extra: 0, read: 40 }), "None of the 40 sentences of four words or more is written twice.");
});
