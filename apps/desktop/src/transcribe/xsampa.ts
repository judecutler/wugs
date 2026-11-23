// X-SAMPA to IPA (Wells 1995; the chart Wikipedia and most phonetics
// handouts print): what a person who types IPA on a plain keyboard writes,
// turned into the symbols wugs stores. Longest match first, so `t_h` is tʰ,
// `r\`` is ɻ and `G\_<` is ʛ; a character with no X-SAMPA meaning is kept as
// typed, so IPA already in the field and spaces, hyphens and dots stay.
// Pure, npm-tested.

const TABLE: Record<string, string> = {
  // Lower case: the same letters, but g is IPA's ɡ.
  g: "ɡ",
  // Upper case.
  A: "ɑ", B: "β", C: "ç", D: "ð", E: "ɛ", F: "ɱ", G: "ɣ", H: "ɥ", I: "ɪ", J: "ɲ", K: "ɬ", L: "ʎ", M: "ɯ", N: "ŋ",
  O: "ɔ", P: "ʋ", Q: "ɒ", R: "ʁ", S: "ʃ", T: "θ", U: "ʊ", V: "ʌ", W: "ʍ", X: "χ", Y: "ʏ", Z: "ʒ",
  // Digits and signs.
  "1": "ɨ", "2": "ø", "3": "ɜ", "4": "ɾ", "5": "ɫ", "6": "ɐ", "7": "ɤ", "8": "ɵ", "9": "œ",
  "@": "ə", "{": "æ", "}": "ʉ", "&": "ɶ", "?": "ʔ", '"': "ˈ", "%": "ˌ", ":": "ː", ":\\": "ˑ", "'": "ʲ",
  "@\\": "ɘ", "@`": "ɚ", "3\\": "ɞ", "3`": "ɝ", "?\\": "ʕ", "<\\": "ʢ", ">\\": "ʡ",
  "^": "ꜛ", "!": "ꜜ", "!\\": "ǃ", "|\\": "ǀ", "||\\": "ǁ", "=\\": "ǂ", "-\\": "‿", "||": "‖",
  "~": "̃", "=": "̩", "`": "˞",
  // Backslash and retroflex letters.
  "B\\": "ʙ", "b_<": "ɓ", "d_<": "ɗ", "d`": "ɖ", "G\\": "ɢ", "G\\_<": "ʛ", "g_<": "ɠ", "h\\": "ɦ", "H\\": "ʜ",
  "I\\": "ᵻ", "j\\": "ʝ", "J\\": "ɟ", "J\\_<": "ʄ", "K\\": "ɮ", "L\\": "ʟ", "l\\": "ɺ", "l`": "ɭ", "M\\": "ɰ",
  "N\\": "ɴ", "n`": "ɳ", "O\\": "ʘ", "p\\": "ɸ", "r\\": "ɹ", "r\\`": "ɻ", "R\\": "ʀ", "r`": "ɽ", "s\\": "ɕ",
  "s`": "ʂ", "t`": "ʈ", "U\\": "ᵿ", "v\\": "ʋ", "X\\": "ħ", "x\\": "ɧ", "z\\": "ʑ", "z`": "ʐ",
  // Diacritics, written after their letter.
  '_"': "̈", "_+": "̟", "_-": "̠", "_/": "̌", "_0": "̥", "_=": "̩", "_>": "ʼ",
  "_?\\": "ˤ", "_\\": "̂", "_^": "̯", "_}": "̚", "_~": "̃", "_A": "̘", "_a": "̺",
  "_B": "̏", "_B_L": "᷅", "_c": "̜", "_d": "̪", "_e": "̴", "_F": "̂", "_G": "ˠ",
  "_H": "́", "_H_T": "᷄", "_h": "ʰ", "_j": "ʲ", "_k": "̰", "_L": "̀", "_l": "ˡ", "_M": "̄",
  "_m": "̻", "_N": "̼", "_n": "ⁿ", "_O": "̹", "_o": "̞", "_q": "̙", "_R": "̌",
  "_R_F": "᷈", "_r": "̝", "_T": "̋", "_t": "̤", "_v": "̬", "_w": "ʷ", "_X": "̆",
  "_x": "̽",
  // A lone underscore ties two letters (t_S = t͡ʃ).
  _: "͡",
};

const LONGEST = Math.max(...Object.keys(TABLE).map((k) => k.length));

/** The IPA for an X-SAMPA string, NFC. */
export function xsampaToIpa(input: string): string {
  let out = "";
  let i = 0;
  while (i < input.length) {
    let taken = 0;
    for (let n = Math.min(LONGEST, input.length - i); n > 0; n--) {
      const piece = input.slice(i, i + n);
      const ipa = TABLE[piece];
      if (ipa !== undefined) {
        out += ipa;
        taken = n;
        break;
      }
    }
    if (taken === 0) {
      // Not X-SAMPA (a plain letter, IPA already typed, a space): kept as typed.
      const cp = input.codePointAt(i)!;
      const ch = String.fromCodePoint(cp);
      out += ch;
      taken = ch.length;
    }
    i += taken;
  }
  return out.normalize("NFC");
}

/** What the preview under the field shows: the IPA when it differs from what was typed. */
export function preview(typed: string): string {
  const ipa = xsampaToIpa(typed);
  return ipa === typed.normalize("NFC") ? "" : `→ ${ipa}`;
}

export const XSAMPA_KEY = "wugs.transcribe.xsampa";
