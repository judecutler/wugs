import { test } from "node:test";
import assert from "node:assert/strict";
import { addedLine, exampleGapsLine, examplesAddedLine, addLabel, cardNote, findable, glyph, inventoryNote, missingSummary, summary, unifiedLine, useLabel, type CharRow, type Inconsistent, type Inventory } from "./logic.ts";

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
