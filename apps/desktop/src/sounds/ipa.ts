// The Sounds panel's rules: cutting a transcription into sounds and placing
// each on the IPA chart (2020 revision), pure and npm-tested. A sound is a
// base letter with every diacritic and modifier written after it (tʰ, ã,
// aː, kʼ), a tie bar joining the next letter (t͡ʃ), and a superscript nasal
// written before a consonant (ᵐb). Stress, tone letters, syllable dots and
// other punctuation belong to no sound.

export type Manner = "plosive" | "nasal" | "trill" | "tap" | "fricative" | "lateral-fricative" | "approximant" | "lateral-approximant";
export type Place =
  | "bilabial"
  | "labiodental"
  | "dental"
  | "alveolar"
  | "postalveolar"
  | "retroflex"
  | "palatal"
  | "velar"
  | "uvular"
  | "pharyngeal"
  | "glottal";

export const PLACES: readonly Place[] = ["bilabial", "labiodental", "dental", "alveolar", "postalveolar", "retroflex", "palatal", "velar", "uvular", "pharyngeal", "glottal"];
export const MANNERS: readonly Manner[] = ["plosive", "nasal", "trill", "tap", "fricative", "lateral-fricative", "approximant", "lateral-approximant"];

export const PLACE_NAMES: Record<Place, string> = {
  bilabial: "Bilabial",
  labiodental: "Labiodental",
  dental: "Dental",
  alveolar: "Alveolar",
  postalveolar: "Postalveolar",
  retroflex: "Retroflex",
  palatal: "Palatal",
  velar: "Velar",
  uvular: "Uvular",
  pharyngeal: "Pharyngeal",
  glottal: "Glottal",
};

export const MANNER_NAMES: Record<Manner, string> = {
  plosive: "Plosive",
  nasal: "Nasal",
  trill: "Trill",
  tap: "Tap or flap",
  fricative: "Fricative",
  "lateral-fricative": "Lateral fricative",
  approximant: "Approximant",
  "lateral-approximant": "Lateral approximant",
};

type Cons = { place: Place; manner: Manner; voiced: boolean };

/** The pulmonic consonants: [manner, place, voiceless, voiced]; "" where the
 * chart has no symbol. ASCII «g» sits beside IPA «ɡ», since both are typed. */
const PULMONIC: [Manner, Place, string, string][] = [
  ["plosive", "bilabial", "p", "b"],
  ["plosive", "alveolar", "t", "d"],
  ["plosive", "retroflex", "ʈ", "ɖ"],
  ["plosive", "palatal", "c", "ɟ"],
  ["plosive", "velar", "k", "ɡg"],
  ["plosive", "uvular", "q", "ɢ"],
  ["plosive", "glottal", "ʔ", ""],
  ["nasal", "bilabial", "", "m"],
  ["nasal", "labiodental", "", "ɱ"],
  ["nasal", "alveolar", "", "n"],
  ["nasal", "retroflex", "", "ɳ"],
  ["nasal", "palatal", "", "ɲ"],
  ["nasal", "velar", "", "ŋ"],
  ["nasal", "uvular", "", "ɴ"],
  ["trill", "bilabial", "", "ʙ"],
  ["trill", "alveolar", "", "r"],
  ["trill", "uvular", "", "ʀ"],
  ["tap", "labiodental", "", "ⱱ"],
  ["tap", "alveolar", "", "ɾ"],
  ["tap", "retroflex", "", "ɽ"],
  ["fricative", "bilabial", "ɸ", "β"],
  ["fricative", "labiodental", "f", "v"],
  ["fricative", "dental", "θ", "ð"],
  ["fricative", "alveolar", "s", "z"],
  ["fricative", "postalveolar", "ʃ", "ʒ"],
  ["fricative", "retroflex", "ʂ", "ʐ"],
  ["fricative", "palatal", "ç", "ʝ"],
  ["fricative", "velar", "x", "ɣ"],
  ["fricative", "uvular", "χ", "ʁ"],
  ["fricative", "pharyngeal", "ħ", "ʕ"],
  ["fricative", "glottal", "h", "ɦ"],
  ["lateral-fricative", "alveolar", "ɬ", "ɮ"],
  ["approximant", "labiodental", "", "ʋ"],
  ["approximant", "alveolar", "", "ɹ"],
  ["approximant", "retroflex", "", "ɻ"],
  ["approximant", "palatal", "", "j"],
  ["approximant", "velar", "", "ɰ"],
  ["lateral-approximant", "alveolar", "", "l"],
  ["lateral-approximant", "retroflex", "", "ɭ"],
  ["lateral-approximant", "palatal", "", "ʎ"],
  ["lateral-approximant", "velar", "", "ʟ"],
];

const CONSONANTS: Map<string, Cons> = (() => {
  const m = new Map<string, Cons>();
  for (const [manner, place, voiceless, voiced] of PULMONIC) {
    for (const c of voiceless) m.set(c, { place, manner, voiced: false });
    for (const c of voiced) m.set(c, { place, manner, voiced: true });
  }
  return m;
})();

/** Consonants the chart lists outside the grid, by the heading it gives them. */
export const OTHER_GROUPS: readonly [string, string][] = [
  ["Clicks", "ʘǀǃǂǁ"],
  ["Implosives", "ɓɗʄɠʛ"],
  ["Other consonants", "ʍwɥʜʢʡɕʑɺɧ"],
];

export type Height = 0 | 1 | 2 | 3 | 4 | 5 | 6;
export const HEIGHT_NAMES = ["Close", "Near-close", "Close-mid", "Mid", "Open-mid", "Near-open", "Open"] as const;

/** A vowel's place on the chart: height 0 (close) to 6 (open), backness 0
 * (front) to 2 (back), and whether it is rounded. */
export type VowelPlace = { height: Height; back: number; rounded: boolean };

const VOWEL_ROWS: [Height, number, string, string][] = [
  [0, 0, "i", "y"],
  [0, 1, "ɨ", "ʉ"],
  [0, 2, "ɯ", "u"],
  [1, 0.35, "ɪ", "ʏ"],
  [1, 1.65, "", "ʊ"],
  [2, 0, "e", "ø"],
  [2, 1, "ɘ", "ɵ"],
  [2, 2, "ɤ", "o"],
  [3, 1, "ə", ""],
  [4, 0, "ɛ", "œ"],
  [4, 1, "ɜ", "ɞ"],
  [4, 2, "ʌ", "ɔ"],
  [5, 0, "æ", ""],
  [5, 1, "ɐ", ""],
  [6, 0, "a", "ɶ"],
  [6, 2, "ɑ", "ɒ"],
];

const VOWELS: Map<string, VowelPlace> = (() => {
  const m = new Map<string, VowelPlace>();
  for (const [height, back, unrounded, rounded] of VOWEL_ROWS) {
    if (unrounded !== "") m.set(unrounded, { height, back, rounded: false });
    if (rounded !== "") m.set(rounded, { height, back, rounded: true });
  }
  return m;
})();

/** Superscript nasals that mark prenasalization when written before a consonant. */
const PRENASAL = new Set(["ᵐ", "ⁿ", "ᵑ", "ᶬ", "ᶮ", "ᶯ", "ᶰ"]);
/** Marks that belong to a syllable or a word, not to one sound. */
const SUPRASEGMENTAL = new Set(["ˈ", "ˌ", ".", "‿", "|", "‖", "↗", "↘", "ꜛ", "ꜜ"]);
const APOSTROPHES = new Set(["'", "’", "ʼ"]);
const TIES = new Set(["͡", "͜"]);

const isMark = (c: string): boolean => /\p{M}/u.test(c);
const isModifier = (c: string): boolean => /\p{Lm}/u.test(c);
const isLetter = (c: string): boolean => /\p{L}/u.test(c);
const isTone = (c: string): boolean => /[˥-˩꜀-ꜟ]/u.test(c);

/** Whether a base letter is a consonant the chart places. */
const isConsonant = (c: string): boolean => CONSONANTS.has(c) || OTHER_GROUPS.some(([, set]) => set.includes(c));

/** The sounds of one word, in order, each in NFC. */
export function sounds(word: string): string[] {
  const out: string[] = [];
  const chars = [...word.normalize("NFD")];
  let current: string | null = null;
  let joinNext = false;
  let prefix = "";
  const close = () => {
    if (current !== null) out.push(current.normalize("NFC"));
    current = null;
    joinNext = false;
  };
  chars.forEach((c, i) => {
    const next = chars[i + 1];
    if (TIES.has(c)) {
      if (current !== null) {
        current += c;
        joinNext = true;
      }
    } else if (isMark(c)) {
      if (current !== null) current += c;
    } else if (SUPRASEGMENTAL.has(c) || isTone(c)) {
      close();
    } else if (PRENASAL.has(c) && next !== undefined && isConsonant(next)) {
      close();
      prefix += c;
    } else if (isModifier(c) || APOSTROPHES.has(c)) {
      // ʰ ʷ ː ʼ … modify the sound before them; at a word's start an
      // apostrophe is a letter of its own (a glottal stop in many spellings).
      if (current !== null) current += c;
      else current = c;
    } else if (isLetter(c)) {
      if (current !== null && joinNext) {
        current += c;
        joinNext = false;
      } else {
        close();
        current = prefix + c;
        prefix = "";
      }
    } else {
      close();
      prefix = "";
    }
  });
  close();
  return out;
}

/** A sound's base letter: its first letter after any prenasal superscript. */
export function baseOf(sound: string): string {
  const chars = [...sound.normalize("NFD")];
  return chars.find((c) => !PRENASAL.has(c)) ?? sound;
}

export type Sound = {
  sound: string;
  /** Times it occurs in running words. */
  count: number;
  /** Word types it occurs in. */
  words: number;
  /** Its commonest word. */
  example: string;
};

/** Every sound in a list of word types, commonest first. */
export function inventory(words: readonly { word: string; count: number }[]): Sound[] {
  const by = new Map<string, Sound>();
  for (const { word, count } of words) {
    const seen = new Set<string>();
    for (const s of sounds(word)) {
      let row = by.get(s);
      if (row === undefined) {
        row = { sound: s, count: 0, words: 0, example: word };
        by.set(s, row);
      }
      row.count += count;
      if (!seen.has(s)) {
        row.words += 1;
        seen.add(s);
      }
    }
  }
  return [...by.values()].sort((a, b) => b.count - a.count || a.sound.localeCompare(b.sound));
}

export type Placed =
  | { kind: "consonant"; place: Place; manner: Manner; voiced: boolean }
  | { kind: "vowel"; at: VowelPlace }
  | { kind: "affricate" }
  | { kind: "other"; group: string }
  | { kind: "unplaced"; why: string };

/** Where a sound goes on the chart. */
export function placeOf(sound: string): Placed {
  const nfd = sound.normalize("NFD");
  const base = baseOf(sound);
  if ([...TIES].some((t) => nfd.includes(t))) return { kind: "affricate" };
  const c = CONSONANTS.get(base);
  if (c !== undefined) return { kind: "consonant", ...c };
  const v = VOWELS.get(base);
  if (v !== undefined) return { kind: "vowel", at: v };
  for (const [group, set] of OTHER_GROUPS) if (set.includes(base)) return { kind: "other", group };
  if (APOSTROPHES.has(base)) return { kind: "unplaced", why: "an apostrophe written as a letter" };
  if (base !== base.toLowerCase()) return { kind: "unplaced", why: "a capital letter" };
  if (/\p{Script=Latin}/u.test(base)) return { kind: "unplaced", why: "a letter the IPA does not use" };
  return { kind: "unplaced", why: "not a Latin-script letter" };
}

export type Chart = {
  /** Phones per cell: `consonants[manner][place]` = [voiceless, voiced]. */
  consonants: Map<Manner, Map<Place, [Sound[], Sound[]]>>;
  vowels: { sound: Sound; at: VowelPlace }[];
  affricates: Sound[];
  others: Map<string, Sound[]>;
  unplaced: { sound: Sound; why: string }[];
};

/** The inventory laid out the way the IPA chart lays it out. */
export function chart(inv: readonly Sound[]): Chart {
  const out: Chart = { consonants: new Map(), vowels: [], affricates: [], others: new Map(), unplaced: [] };
  for (const s of inv) {
    const at = placeOf(s.sound);
    switch (at.kind) {
      case "consonant": {
        const row = out.consonants.get(at.manner) ?? new Map<Place, [Sound[], Sound[]]>();
        out.consonants.set(at.manner, row);
        const cell = row.get(at.place) ?? [[], []];
        row.set(at.place, cell);
        cell[at.voiced ? 1 : 0].push(s);
        break;
      }
      case "vowel":
        out.vowels.push({ sound: s, at: at.at });
        break;
      case "affricate":
        out.affricates.push(s);
        break;
      case "other": {
        const list = out.others.get(at.group) ?? [];
        list.push(s);
        out.others.set(at.group, list);
        break;
      }
      case "unplaced":
        out.unplaced.push({ sound: s, why: at.why });
    }
  }
  return out;
}

/** The rows and columns to draw: those in use, or the whole chart. */
export function gridOf(c: Chart, whole: boolean): { manners: Manner[]; places: Place[] } {
  if (whole) return { manners: [...MANNERS], places: [...PLACES] };
  const used = new Set<Place>();
  for (const row of c.consonants.values()) for (const p of row.keys()) used.add(p);
  return { manners: MANNERS.filter((m) => c.consonants.has(m)), places: PLACES.filter((p) => used.has(p)) };
}

/** The track a sound inventory reads by default: one named for IPA or a
 * phonetic transcription, else the fullest. */
export function defaultTrack(tracks: readonly { id: string; name: string; words: number }[]): string | null {
  const phonetic = tracks.find((t) => /\b(ipa|phon\w*)\b/i.test(t.name));
  return (phonetic ?? tracks[0])?.id ?? null;
}

/** Where a vowel sits in a box `width` × `height`, as on the IPA trapezoid:
 * the front edge slants in towards the bottom; rounded vowels to the right
 * of their point, unrounded to the left. */
export function vowelXY(at: VowelPlace, width: number, height: number): { x: number; y: number } {
  const y = (at.height / 6) * height;
  const front = (at.height / 6) * width * 0.45;
  const x = front + (at.back / 2) * (width - front) + (at.rounded ? 11 : -11);
  return { x, y };
}

/** The line under the chart, in words. */
export function soundsNote(inv: readonly Sound[], c: Chart, running: number): string {
  if (running === 0) return "No transcribed words on this track yet.";
  const parts = [`${inv.length} ${inv.length === 1 ? "sound" : "sounds"} in ${running.toLocaleString()} running ${running === 1 ? "word" : "words"}`];
  if (c.unplaced.length > 0) parts.push(`${c.unplaced.length} not on the IPA chart`);
  return `${parts.join(", ")}. The track is read as IPA: a spelling with digraphs shows its letters one by one.`;
}
