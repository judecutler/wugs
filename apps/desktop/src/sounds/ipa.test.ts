import { test } from "node:test";
import assert from "node:assert/strict";
import { baseOf, chart, defaultTrack, gridOf, inventory, placeOf, sounds, soundsNote, vowelXY } from "./ipa.ts";

test("a sound keeps the diacritics and modifiers written after it", () => {
  assert.deepEqual(sounds("tʰaː"), ["tʰ", "aː"]);
  assert.deepEqual(sounds("kʼãn"), ["kʼ", "ã", "n"]);
  assert.deepEqual(sounds("k'a"), ["k'", "a"]);
  assert.deepEqual(sounds("t͡ʃa"), ["t͡ʃ", "a"]);
  assert.deepEqual(sounds("t͡ʃʰa"), ["t͡ʃʰ", "a"]);
  assert.deepEqual(sounds("é"), ["é"], "stored in NFC");
});

test("a superscript nasal before a consonant prenasalizes it", () => {
  assert.deepEqual(sounds("ᵐbaⁿda"), ["ᵐb", "a", "ⁿd", "a"]);
  assert.deepEqual(sounds("aⁿ"), ["aⁿ"], "after a vowel it stays on the vowel");
  assert.equal(baseOf("ᵐb"), "b");
});

test("stress, tone, syllable dots and punctuation belong to no sound", () => {
  assert.deepEqual(sounds("ˈka.ta˥˩"), ["k", "a", "t", "a"]);
  assert.deepEqual(sounds("ka-ta"), ["k", "a", "t", "a"]);
  assert.deepEqual(sounds("'a"), ["'", "a"], "a word-initial apostrophe is a letter of its own");
});

test("sounds are placed as the IPA chart places them", () => {
  assert.deepEqual(placeOf("tʰ"), { kind: "consonant", place: "alveolar", manner: "plosive", voiced: false });
  assert.deepEqual(placeOf("g"), { kind: "consonant", place: "velar", manner: "plosive", voiced: true });
  assert.deepEqual(placeOf("ŋ"), { kind: "consonant", place: "velar", manner: "nasal", voiced: true });
  assert.deepEqual(placeOf("ɔ̃"), { kind: "vowel", at: { height: 4, back: 2, rounded: true } });
  assert.deepEqual(placeOf("t͡s"), { kind: "affricate" });
  assert.deepEqual(placeOf("ɓ"), { kind: "other", group: "Implosives" });
  assert.deepEqual(placeOf("ǃ"), { kind: "other", group: "Clicks" });
  assert.equal(placeOf("ł").kind, "unplaced");
  assert.deepEqual(placeOf("A"), { kind: "unplaced", why: "a capital letter" });
  assert.deepEqual(placeOf("ж"), { kind: "unplaced", why: "not a Latin-script letter" });
});

test("the inventory counts running sounds and word types", () => {
  const inv = inventory([
    { word: "tata", count: 3 },
    { word: "ta", count: 1 },
  ]);
  assert.deepEqual(inv, [
    { sound: "a", count: 7, words: 2, example: "tata" },
    { sound: "t", count: 7, words: 2, example: "tata" },
  ]);
});

test("the chart keeps only rows and columns in use unless asked for all", () => {
  const c = chart(inventory([{ word: "pamaɓa t͡ʃa ła", count: 1 }].flatMap((w) => w.word.split(" ").map((word) => ({ word, count: 1 })))));
  const grid = gridOf(c, false);
  assert.deepEqual(grid, { manners: ["plosive", "nasal"], places: ["bilabial"] });
  assert.equal(gridOf(c, true).places.length, 11);
  assert.equal(c.consonants.get("plosive")?.get("bilabial")?.[0][0]?.sound, "p");
  assert.equal(c.affricates[0]?.sound, "t͡ʃ");
  assert.equal(c.others.get("Implosives")?.[0]?.sound, "ɓ");
  assert.equal(c.unplaced[0]?.sound.sound, "ł");
  assert.equal(c.vowels.length, 1);
  assert.match(soundsNote(inventory([{ word: "ła", count: 2 }]), c, 2), /^2 sounds in 2 running words, 1 not on the IPA chart\./u);
  assert.equal(soundsNote([], c, 0), "No transcribed words on this track yet.");
});

test("an IPA track is read by default, else the fullest", () => {
  assert.equal(defaultTrack([{ id: "a", name: "utterance", words: 9 }, { id: "b", name: "IPA", words: 3 }]), "b");
  assert.equal(defaultTrack([{ id: "a", name: "utterance", words: 9 }, { id: "b", name: "phonetic", words: 3 }]), "b");
  assert.equal(defaultTrack([{ id: "a", name: "utterance", words: 9 }]), "a");
  assert.equal(defaultTrack([]), null);
});

test("the vowel trapezoid narrows towards the bottom", () => {
  const i = vowelXY({ height: 0, back: 0, rounded: false }, 300, 200);
  const a = vowelXY({ height: 6, back: 0, rounded: false }, 300, 200);
  const u = vowelXY({ height: 0, back: 2, rounded: true }, 300, 200);
  assert.ok(a.x > i.x, "open front sits right of close front");
  assert.equal(a.y, 200);
  assert.equal(u.x, 311);
});
