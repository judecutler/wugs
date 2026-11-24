// Distinctive features: each sound of the inventory as + and − on the
// features phonology courses use (after Hayes, Introductory Phonology, 2009,
// simplified), worked out from where the sound sits on the IPA chart. A
// feature that does not apply to a sound (high for a coronal) is left blank.
// Picking values gives the natural class: every sound with all of them. Pure;
// no DOM.
import { placeOf, type Manner, type Place } from "./ipa.ts";

export type Value = "+" | "-" | "";
export const FEATURES = ["syllabic", "consonantal", "sonorant", "continuant", "delayed release", "nasal", "lateral", "voice", "labial", "coronal", "dorsal", "high", "low", "back", "round"] as const;
export type Feature = (typeof FEATURES)[number];
export type Features = Record<Feature, Value>;

const LABIAL: ReadonlySet<Place> = new Set(["bilabial", "labiodental"]);
const CORONAL: ReadonlySet<Place> = new Set(["dental", "alveolar", "postalveolar", "retroflex"]);
const DORSAL: ReadonlySet<Place> = new Set(["palatal", "velar", "uvular"]);
const pm = (b: boolean): Value => (b ? "+" : "-");

function consonantFeatures(place: Place, manner: Manner | "affricate", voiced: boolean): Features {
  const glide = manner === "approximant" && (place === "palatal" || place === "velar" || place === "labiodental" || place === "bilabial");
  const laryngeal = place === "glottal" || place === "pharyngeal";
  const obstruent = manner === "plosive" || manner === "fricative" || manner === "lateral-fricative" || manner === "affricate";
  const dorsal = DORSAL.has(place);
  return {
    syllabic: "-",
    consonantal: pm(!glide && !(laryngeal && !obstruent) && place !== "glottal"),
    sonorant: pm(!obstruent),
    continuant: pm(!(manner === "plosive" || manner === "nasal" || manner === "affricate")),
    "delayed release": obstruent ? pm(manner !== "plosive") : "",
    nasal: pm(manner === "nasal"),
    lateral: pm(manner === "lateral-fricative" || manner === "lateral-approximant"),
    voice: pm(voiced),
    labial: pm(LABIAL.has(place)),
    coronal: pm(CORONAL.has(place)),
    dorsal: pm(dorsal),
    high: dorsal ? pm(place !== "uvular") : "",
    low: dorsal || place === "pharyngeal" ? pm(place === "pharyngeal") : "",
    back: dorsal ? pm(place !== "palatal") : "",
    round: "",
  };
}

/** A sound's features, or null when the chart does not place it (clicks, unplaced letters). */
export function featuresOf(sound: string): Features | null {
  const at = placeOf(sound);
  if (at.kind === "vowel") {
    const { height, back, rounded } = at.at;
    return {
      syllabic: "+",
      consonantal: "-",
      sonorant: "+",
      continuant: "+",
      "delayed release": "",
      nasal: pm(sound.normalize("NFD").includes("̃")),
      lateral: "-",
      voice: "+",
      labial: pm(rounded),
      coronal: "-",
      dorsal: "+",
      high: pm(height <= 1),
      low: pm(height >= 5),
      back: pm(back >= 1),
      round: pm(rounded),
    };
  }
  if (at.kind === "consonant") return consonantFeatures(at.place, at.manner, at.voiced);
  if (at.kind === "affricate") {
    const letters = [...sound.normalize("NFD")].map((c) => placeOf(c)).filter((p) => p.kind === "consonant");
    const first = letters[0];
    const last = letters[letters.length - 1];
    if (first?.kind !== "consonant" || last?.kind !== "consonant") return null;
    return consonantFeatures(last.place, "affricate", first.voiced);
  }
  return null;
}

/** The sounds that have every picked value. */
export function matching(sounds: readonly string[], picked: ReadonlyMap<Feature, "+" | "-">): string[] {
  return sounds.filter((s) => {
    const f = featuresOf(s);
    if (f === null) return false;
    for (const [k, v] of picked) if (f[k] !== v) return false;
    return true;
  });
}

/** The picked values written as a feature bundle: [+voice, −sonorant]. */
export function bundle(picked: ReadonlyMap<Feature, "+" | "-">): string {
  return `[${[...picked].map(([k, v]) => `${v === "+" ? "+" : "−"}${k}`).join(", ")}]`;
}

/** The next value a click on a feature gives: none → + → − → none. */
export const nextValue = (v: "+" | "-" | undefined): "+" | "-" | undefined => (v === undefined ? "+" : v === "+" ? "-" : undefined);

/** Every placed sound with its features, as a table. */
export function featuresTsv(sounds: readonly string[]): string {
  const rows = [["sound", ...FEATURES].join("\t")];
  for (const s of sounds) {
    const f = featuresOf(s);
    if (f !== null) rows.push([s, ...FEATURES.map((k) => (f[k] === "-" ? "−" : f[k]))].join("\t"));
  }
  return rows.join("\n") + "\n";
}
