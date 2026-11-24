// Natural classes: what a set of sounds has in common, in the words a
// phonologist would use ("front vowels", "voiceless stops", "coronal
// consonants"), worked out from where each sits on the IPA chart. Pure and
// npm-tested; the Sounds panel's Environments note uses it to say what the
// sounds one side of a pair occurs next to share.

import { baseOf, placeOf, type Manner, type Place, type VowelPlace } from "./ipa.ts";

type Cons = { place: Place | null; manner: Manner | "affricate"; voiced: boolean | null };

const MAJOR: Record<Place, string> = {
  bilabial: "labial",
  labiodental: "labial",
  dental: "coronal",
  alveolar: "coronal",
  postalveolar: "coronal",
  retroflex: "coronal",
  palatal: "dorsal",
  velar: "dorsal",
  uvular: "dorsal",
  pharyngeal: "laryngeal",
  glottal: "laryngeal",
};

const MANNER_PLURAL: Record<Manner | "affricate", string> = {
  plosive: "stops",
  nasal: "nasals",
  trill: "trills",
  tap: "taps",
  fricative: "fricatives",
  "lateral-fricative": "lateral fricatives",
  approximant: "approximants",
  "lateral-approximant": "lateral approximants",
  affricate: "affricates",
};

const OBSTRUENT = new Set<Manner | "affricate">(["plosive", "fricative", "lateral-fricative", "affricate"]);
const LATERAL = new Set<Manner | "affricate">(["lateral-fricative", "lateral-approximant"]);
const LIQUID = new Set<Manner | "affricate">(["trill", "tap", "lateral-approximant"]);

/** A consonant's features, an affricate's from its two letters (t͡ʃ: voiceless, postalveolar). */
function consonant(sound: string): Cons | null {
  const at = placeOf(sound);
  if (at.kind === "consonant") return { place: at.place, manner: at.manner, voiced: at.voiced };
  if (at.kind !== "affricate") return null;
  const letters = [...sound.normalize("NFD")].map((c) => placeOf(c)).filter((p) => p.kind === "consonant");
  const first = letters[0];
  const last = letters[letters.length - 1];
  if (first === undefined || last === undefined || first.kind !== "consonant" || last.kind !== "consonant") return { place: null, manner: "affricate", voiced: null };
  return { place: last.place, manner: "affricate", voiced: first.voiced };
}

const same = <T>(xs: readonly T[]): T | undefined => (xs.length > 0 && xs.every((x) => x === xs[0]) ? xs[0] : undefined);

function consonantClass(cs: readonly Cons[]): string {
  const voiced = same(cs.map((c) => c.voiced));
  const voicing = voiced === true ? "voiced" : voiced === false ? "voiceless" : "";
  const place = same(cs.map((c) => c.place));
  const major = same(cs.map((c) => (c.place === null ? null : MAJOR[c.place])));
  const where = place !== undefined && place !== null ? place : major !== undefined && major !== null ? major : "";
  const manner = same(cs.map((c) => c.manner));
  let what: string;
  if (manner !== undefined) what = MANNER_PLURAL[manner];
  else if (cs.every((c) => LATERAL.has(c.manner))) what = "laterals";
  else if (cs.every((c) => LIQUID.has(c.manner))) what = "liquids";
  else if (cs.every((c) => OBSTRUENT.has(c.manner))) what = "obstruents";
  else if (cs.every((c) => !OBSTRUENT.has(c.manner))) what = "sonorants";
  else what = "consonants";
  // Sonorants are voiced as a rule, so saying so adds nothing.
  const sayVoicing = voicing !== "" && !(voiced === true && cs.every((c) => !OBSTRUENT.has(c.manner)));
  return [sayVoicing ? voicing : "", where, what].filter((w) => w !== "").join(" ");
}

const HEIGHT = (h: number): string => (h <= 1 ? "high" : h <= 4 ? "mid" : "low");
const BACK = ["front", "central", "back"] as const;

function vowelClass(vs: readonly VowelPlace[]): string {
  const height = same(vs.map((v) => HEIGHT(v.height)));
  const back = same(vs.map((v) => BACK[Math.max(0, Math.min(2, Math.round(v.back)))]!));
  const rounded = same(vs.map((v) => v.rounded));
  // Front vowels are unrounded and back ones rounded as a rule; say rounding only against that.
  const roundness = rounded === true && back !== "back" ? "rounded" : rounded === false && back === "back" ? "unrounded" : "";
  return [height ?? "", roundness, back ?? "", "vowels"].filter((w) => w !== "").join(" ");
}

/** What the sounds share, or null when they are one sound, mix vowels and
 * consonants, or include a sound off the chart. `#` (a word edge) is set
 * aside and named after the class. */
export function naturalClass(sounds: readonly string[], edge = "#"): string | null {
  const set = [...new Set(sounds)];
  const hasEdge = set.includes(edge);
  const rest = set.filter((s) => s !== edge);
  if (rest.length < 2) return null;
  const tail = hasEdge ? " or a word edge" : "";
  const vowels = rest.map((s) => placeOf(s)).map((p) => (p.kind === "vowel" ? p.at : null));
  if (vowels.every((v) => v !== null)) return vowelClass(vowels as VowelPlace[]) + tail;
  const cons = rest.map(consonant);
  if (cons.every((c) => c !== null)) return consonantClass(cons as Cons[]) + tail;
  return null;
}

/** Whether a sound is on the chart as a vowel or consonant (so a class can name it). */
export const classable = (sound: string): boolean => {
  const p = placeOf(sound);
  return p.kind === "vowel" || p.kind === "consonant" || p.kind === "affricate" || baseOf(sound) === "#";
};
