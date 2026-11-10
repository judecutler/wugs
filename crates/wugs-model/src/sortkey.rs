//! Alphabet-order sort keys (PRD §7 custom sort orders), zero-dependency.
//!
//! Stored format of a writing system's `sort` field: units in alphabet
//! order, separated by whitespace. A unit may list case or spelling
//! variants joined by `/`, first variant first:
//!
//! ```text
//! a/A b/B c/C ç/Ç ch/Ch/CH dz/Dz/DZ
//! ```
//!
//! Parse it with [`sort_units`]. ICU tailorings are the future edge-crate
//! upgrade; this format is what the core understands until then.
//!
//! Keys compare in three levels:
//! 1. primary: unit ranks, with separators ([`is_separator`]) skipped, so
//!    "a b" and "ab" share a primary key and "-ka" files under k. A
//!    character no unit matches sorts after the whole alphabet, by codepoint.
//! 2. variant: which variant of each unit matched, so "Chata" sits right
//!    next to "chata".
//! 3. separators: where each separator sat and which one it was, so two
//!    distinct strings never share a key.
//!
//! Matching is greedy longest-first across every variant of every unit, and
//! units match before separators, so an alphabet may list `'` as a letter.

/// ASCII and guillemet punctuation that never decides alphabet order and
/// that the charset audit exempts. Other punctuation must be listed in the
/// charset (or as a unit) to be treated as known.
pub const PUNCTUATION: &[char] = &[
    '.', ',', ';', ':', '!', '?', '\'', '"', '(', ')', '[', ']', '{', '}', '…', '«', '»',
];

/// Morpheme boundary marks (affix and clitic hyphens, infix tilde).
pub const BOUNDARY_MARKS: &[char] = &['-', '=', '~'];

/// Ignored at the primary sort level: whitespace, [`PUNCTUATION`] and
/// [`BOUNDARY_MARKS`].
pub fn is_separator(c: char) -> bool {
    c.is_whitespace() || PUNCTUATION.contains(&c) || BOUNDARY_MARKS.contains(&c)
}

/// Parse a stored `sort` string into units, each a list of variants.
pub fn sort_units(stored: &str) -> Vec<Vec<String>> {
    stored
        .split_whitespace()
        .map(|unit| unit.split('/').filter(|v| !v.is_empty()).map(str::to_string).collect::<Vec<_>>())
        .filter(|variants| !variants.is_empty())
        .collect()
}

/// The sort key of `s` under `units` (from [`sort_units`]). Compare keys
/// with `Ord`; distinct strings always get distinct keys.
pub fn sort_key(units: &[Vec<String>], s: &str) -> Vec<u32> {
    let after_alphabet = units.len() as u32 + 1;
    let mut primary = Vec::new();
    let mut variant = Vec::new();
    // 1 = a unit or unknown character, 2 + codepoint = a separator
    let mut layout = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let mut best: Option<(usize, u32, u32)> = None;
        for (rank, variants) in units.iter().enumerate() {
            for (vi, v) in variants.iter().enumerate() {
                if rest.starts_with(v.as_str()) && best.is_none_or(|(len, _, _)| v.len() > len) {
                    best = Some((v.len(), rank as u32 + 1, vi as u32 + 1));
                }
            }
        }
        if let Some((len, rank, vi)) = best {
            primary.push(rank);
            variant.push(vi);
            layout.push(1);
            rest = &rest[len..];
            continue;
        }
        let c = rest.chars().next().expect("non-empty");
        if is_separator(c) {
            layout.push(2 + c as u32);
        } else {
            primary.push(after_alphabet + c as u32);
            variant.push(1);
            layout.push(1);
        }
        rest = &rest[c.len_utf8()..];
    }
    // 0 ends each level, below every value a level holds, so a shorter
    // level sorts first and never compares against the next level's values
    let mut key = primary;
    key.push(0);
    key.extend(variant);
    key.push(0);
    key.extend(layout);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted<'a>(alphabet: &str, words: &[&'a str]) -> Vec<&'a str> {
        let units = sort_units(alphabet);
        let mut v: Vec<&str> = words.to_vec();
        v.sort_by_key(|w| sort_key(&units, w));
        v
    }

    #[test]
    fn turkish_c_cedilla_sorts_after_c() {
        assert_eq!(sorted("a b c ç d e", &["çay", "can", "dede"]), ["can", "çay", "dede"]);
    }

    #[test]
    fn digraphs_sort_as_one_unit() {
        // Slovak: h < ch < i
        assert_eq!(sorted("a b c d e f g h ch i", &["chata", "hora", "idea"]), ["hora", "chata", "idea"]);
    }

    #[test]
    fn unknown_characters_sort_after_the_alphabet() {
        assert_eq!(sorted("a b", &["b", "a", "zz", "7"]), ["a", "b", "7", "zz"]);
    }

    #[test]
    fn stored_format_parses_units_and_variants() {
        assert_eq!(
            sort_units("  a/A ch/Ch/CH\tb // "),
            vec![vec!["a", "A"], vec!["ch", "Ch", "CH"], vec!["b"]]
        );
        assert!(sort_units("").is_empty());
    }

    const ALPHABET: &str = "a/A b/B c/C ch/Ch d/D o/O m/M t/T";

    #[test]
    fn case_variants_share_the_primary_rank() {
        let got = sorted(ALPHABET, &["dom", "Chata", "abo", "ab", "chata", "Bod", "a b", "cot"]);
        // "Bod" files under b, not after the alphabet; "Chata" sits next to
        // "chata", lowercase first (variant order); "ab" and "a b" share a
        // primary key, so nothing sorts between them, and the unspaced
        // spelling wins the final tiebreak.
        assert_eq!(got, ["ab", "a b", "abo", "Bod", "cot", "chata", "Chata", "dom"]);
    }

    #[test]
    fn separators_do_not_decide_primary_order() {
        // counted at the primary level, a separator would sort after the
        // alphabet and push "a b" past "aba", and "-ka" past "kk"
        assert_eq!(sorted("a b", &["aba", "ab", "aa", "a b", "a-b", "b"]), ["aa", "ab", "a b", "a-b", "aba", "b"]);
        assert_eq!(sorted("a k", &["kk", "ka", "-ka", "aka"]), ["aka", "ka", "-ka", "kk"]);
    }

    #[test]
    fn a_unit_may_be_a_punctuation_character() {
        // glottal stop written with an apostrophe, listed as a letter after k
        assert_eq!(sorted("a k ' o", &["o", "'o", "ko"]), ["ko", "'o", "o"]);
    }

    #[test]
    fn longest_match_spans_overlapping_units_and_variants() {
        // Hungarian-style: d < dz < dzs; the trigraph wins over its prefixes
        let hu = "a d/D dz/Dz dzs/Dzs/DZS e";
        assert_eq!(sorted(hu, &["dzsem", "dze", "de", "Dzsa", "DZSE"]), ["de", "dze", "Dzsa", "dzsem", "DZSE"]);
        let units = sort_units(hu);
        assert_eq!(sort_key(&units, "dzs")[0], 4, "one dzs unit, rank 4");
        assert_eq!(sort_key(&units, "dzs")[1], 0, "and nothing after it");
    }

    #[test]
    fn empty_alphabet_and_empty_string() {
        assert_eq!(sorted("", &["b", "", "a"]), ["", "a", "b"]);
        assert_eq!(sort_key(&sort_units(ALPHABET), ""), vec![0, 0]);
    }

    #[test]
    fn distinct_strings_never_share_a_key() {
        let units = sort_units(ALPHABET);
        let corpus = [
            "", " ", "a", "A", "a ", " a", "ab", "a b", "a  b", "a-b", "a.b", "Ab", "aB", "c", "ch", "c h", "Ch",
            "cH", "C", "h", "chch", "ch ch", "x", "é", "e\u{301}", "-", "--", "«a»", "a…", "7",
        ];
        for (i, a) in corpus.iter().enumerate() {
            for b in &corpus[i + 1..] {
                assert_ne!(sort_key(&units, a), sort_key(&units, b), "{a:?} vs {b:?}");
            }
        }
    }
}
