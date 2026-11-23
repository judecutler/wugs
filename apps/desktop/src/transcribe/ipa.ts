// The IPA palette's symbols, one table. Combining marks are stored as code
// points so the source stays unambiguous; the palette shows them on a dotted circle.
export type IpaSymbol = { ch: string; name: string; combining?: true };
export type IpaGroup = { id: string; label: string; symbols: IpaSymbol[] };

const mark = (cp: number, name: string): IpaSymbol => ({ ch: String.fromCodePoint(cp), name, combining: true });
// "ch=name ch=name": a new entry starts at a one-character key followed by "=".
const plain = (spec: string): IpaSymbol[] => spec.split(/ (?=[^\s=]=)/u).map((entry) => {
  const [ch = "", ...name] = entry.split("=");
  return { ch, name: name.join("=") };
});

export const IPA_GROUPS: IpaGroup[] = [
  {
    id: "vowels",
    label: "Vowels",
    symbols: plain(
      "i=close front unrounded y=close front rounded ɨ=close central unrounded ʉ=close central rounded ɯ=close back unrounded u=close back rounded " +
        "ɪ=near-close front unrounded ʏ=near-close front rounded ʊ=near-close back rounded e=close-mid front unrounded ø=close-mid front rounded " +
        "ɘ=close-mid central unrounded ɵ=close-mid central rounded ɤ=close-mid back unrounded o=close-mid back rounded ə=schwa " +
        "ɛ=open-mid front unrounded œ=open-mid front rounded ɜ=open-mid central unrounded ɞ=open-mid central rounded ʌ=open-mid back unrounded " +
        "ɔ=open-mid back rounded æ=near-open front ɐ=near-open central a=open front unrounded ɶ=open front rounded ɑ=open back unrounded ɒ=open back rounded ɚ=rhotic schwa",
    ),
  },
  {
    id: "consonants",
    label: "Consonants",
    symbols: plain(
      "p=voiceless bilabial plosive b=voiced bilabial plosive t=voiceless alveolar plosive d=voiced alveolar plosive ʈ=voiceless retroflex plosive ɖ=voiced retroflex plosive " +
        "c=voiceless palatal plosive ɟ=voiced palatal plosive k=voiceless velar plosive ɡ=voiced velar plosive q=voiceless uvular plosive ɢ=voiced uvular plosive ʔ=glottal stop " +
        "m=bilabial nasal ɱ=labiodental nasal n=alveolar nasal ɳ=retroflex nasal ɲ=palatal nasal ŋ=velar nasal ɴ=uvular nasal " +
        "ʙ=bilabial trill r=alveolar trill ʀ=uvular trill ⱱ=labiodental flap ɾ=alveolar tap ɽ=retroflex flap " +
        "ɸ=voiceless bilabial fricative β=voiced bilabial fricative f=voiceless labiodental fricative v=voiced labiodental fricative θ=voiceless dental fricative ð=voiced dental fricative " +
        "s=voiceless alveolar fricative z=voiced alveolar fricative ʃ=voiceless postalveolar fricative ʒ=voiced postalveolar fricative ʂ=voiceless retroflex fricative ʐ=voiced retroflex fricative " +
        "ɕ=voiceless alveolo-palatal fricative ʑ=voiced alveolo-palatal fricative ç=voiceless palatal fricative ʝ=voiced palatal fricative x=voiceless velar fricative ɣ=voiced velar fricative " +
        "χ=voiceless uvular fricative ʁ=voiced uvular fricative ħ=voiceless pharyngeal fricative ʕ=voiced pharyngeal fricative h=voiceless glottal fricative ɦ=voiced glottal fricative " +
        "ɬ=voiceless lateral fricative ɮ=voiced lateral fricative ʋ=labiodental approximant ɹ=alveolar approximant ɻ=retroflex approximant j=palatal approximant ɰ=velar approximant " +
        "l=alveolar lateral approximant ɭ=retroflex lateral approximant ʎ=palatal lateral approximant ʟ=velar lateral approximant ʍ=voiceless labial-velar fricative w=labial-velar approximant ɥ=labial-palatal approximant " +
        "ʘ=bilabial click ǀ=dental click ǃ=alveolar click ǂ=palatoalveolar click ǁ=lateral click ɓ=bilabial implosive ɗ=alveolar implosive ʄ=palatal implosive ɠ=velar implosive ʛ=uvular implosive",
    ),
  },
  {
    id: "diacritics",
    label: "Diacritics",
    symbols: [
      mark(0x0325, "voiceless"), mark(0x032c, "voiced"), mark(0x0324, "breathy voiced"), mark(0x0330, "creaky voiced"),
      mark(0x032a, "dental"), mark(0x033a, "apical"), mark(0x033b, "laminal"), mark(0x033c, "linguolabial"),
      mark(0x0339, "more rounded"), mark(0x031c, "less rounded"), mark(0x031f, "advanced"), mark(0x0320, "retracted"),
      mark(0x0308, "centralized"), mark(0x033d, "mid-centralized"), mark(0x0329, "syllabic"), mark(0x032f, "non-syllabic"),
      mark(0x031d, "raised"), mark(0x031e, "lowered"), mark(0x0318, "advanced tongue root"), mark(0x0319, "retracted tongue root"),
      mark(0x0303, "nasalized"), mark(0x031a, "no audible release"), mark(0x0334, "velarized or pharyngealized"),
      mark(0x0361, "tie bar above"), mark(0x035c, "tie bar below"),
      ...plain("ʰ=aspirated ʷ=labialized ʲ=palatalized ˠ=velarized ˤ=pharyngealized ⁿ=nasal release ˡ=lateral release ʼ=ejective ˞=rhoticity"),
    ],
  },
  {
    id: "suprasegmentals",
    label: "Suprasegmentals",
    symbols: plain("ː=long ˑ=half-long ˈ=primary stress ˌ=secondary stress .=syllable break |=minor group ‖=major group ‿=linking"),
  },
  {
    id: "tone",
    label: "Tone",
    symbols: [
      ...plain("˥=extra high tone letter ˦=high tone letter ˧=mid tone letter ˨=low tone letter ˩=extra low tone letter"),
      mark(0x030b, "extra high tone"), mark(0x0301, "high tone"), mark(0x0304, "mid tone"), mark(0x0300, "low tone"), mark(0x030f, "extra low tone"),
      mark(0x030c, "rising tone"), mark(0x0302, "falling tone"),
    ],
  },
];

/** The most of a project's own letters the palette offers. */
export const PROJECT_LETTERS = 32;

/**
 * The letters and marks the project's own transcriptions and headwords use
 * that a plain keyboard does not type (anything outside ASCII), commonest
 * first, as the palette's first row; null when there are none. A letter
 * of another script than the project's main one is left out, since it is
 * usually a look-alike typed by mistake (Cyrillic «а» for Latin «a»). Rows
 * come from the character inventory (src-tauri/src/characters.rs).
 */
export function projectGroup(rows: { ch: string; name: string; kind: string; script: string; inTexts: number; inLexicon: number }[], mainScript: string): IpaGroup | null {
  const ownScript = (s: string) => s === mainScript || s === "Common" || s === "Inherited" || mainScript === "";
  const symbols = rows
    .filter((r) => (r.kind === "letter" || r.kind === "mark") && r.inTexts + r.inLexicon > 0 && ownScript(r.script) && [...r.ch].some((c) => c.codePointAt(0)! > 0x7f))
    .sort((a, b) => b.inTexts + b.inLexicon - (a.inTexts + a.inLexicon) || a.ch.localeCompare(b.ch))
    .slice(0, PROJECT_LETTERS)
    .map((r): IpaSymbol => (r.kind === "mark" ? { ch: r.ch, name: r.name, combining: true } : { ch: r.ch, name: r.name }));
  return symbols.length === 0 ? null : { id: "project", label: "This project", symbols };
}
