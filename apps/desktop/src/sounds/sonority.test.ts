import assert from "node:assert/strict";
import { test } from "node:test";
import { keeps, profileOf, sonority, sonorityBreaks, sonorityNote, sPlusStop } from "./sonority.ts";

const cluster = (c: string, initial: number, final: number, example = c) => ({ cluster: c, initial, medial: 0, final, words: 1, example });

test("sonority rises from plosives to glides", () => {
  assert.deepEqual(["p", "t͡ʃ", "s", "m", "l", "ɹ", "j", "a"].map(sonority), [1, 2, 3, 4, 5, 5, 6, 7]);
  assert.equal(sonority("ʘ"), null);
  assert.equal(profileOf("pl"), "rises");
  assert.equal(profileOf("rt"), "falls");
  assert.equal(profileOf("pt"), "level");
  assert.equal(profileOf("spl"), "mixed");
  assert.equal(profileOf("p"), null);
  assert.ok(keeps("rises", "onset") && keeps("falls", "coda") && !keeps("level", "onset"));
  assert.ok(sPlusStop("st") && sPlusStop("ʃp") && !sPlusStop("ts") && !sPlusStop("sl"));
});

test("onsets that do not rise and codas that do not fall break sequencing", () => {
  const clusters = [cluster("pl", 30, 0), cluster("rt", 4, 12, "kart"), cluster("st", 9, 6), cluster("nt", 0, 20), cluster("mb", 3, 0, "mbiku")];
  const breaks = sonorityBreaks(clusters);
  assert.deepEqual(
    breaks.map((b) => `${b.cluster} ${b.where} ${b.profile}`),
    ["st onset falls", "rt onset falls", "mb onset falls"],
  );
  assert.ok(breaks[0]!.sStop && !breaks[1]!.sStop);
  const note = sonorityNote(clusters, breaks);
  assert.match(note, /^3 clusters break sonority sequencing at a word's edge: st \(starting words, falls\), rt \(starting words, falls\), mb \(starting words, falls\)\. st is an s before a plosive, the commonest exception across languages\. The others often hide/);
  assert.match(sonorityNote([cluster("st", 2, 0)], sonorityBreaks([cluster("st", 2, 0)])), /All are an s before a plosive/);
  assert.match(sonorityNote([cluster("pl", 1, 0)], []), /^Every word-initial cluster rises/);
  assert.equal(sonorityNote([{ ...cluster("pl", 0, 0), medial: 4 }], []), "");
});
