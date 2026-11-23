//! Glottolog's languages and dialects (5.4, CC BY 4.0; `datasets/glottolog/`),
//! embedded so the Texts panel's Language row can offer the Glottocode and
//! ISO 639-3 code for a name as it is typed, with nothing sent anywhere.

use serde::Serialize;
use std::sync::OnceLock;

const TABLE: &str = include_str!("../../../../datasets/glottolog/languoids.tsv");

/// The edition the table was built from, for the panel's credit line.
pub const EDITION: &str = "Glottolog 5.4 (CC BY 4.0)";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Languoid {
    pub glottocode: &'static str,
    pub iso: &'static str,
    pub name: &'static str,
    /// A dialect rather than a language.
    pub dialect: bool,
    /// The family's name, `isolate`, or empty (unclassified, sign languages…).
    pub family: &'static str,
    pub macroarea: &'static str,
    /// For a dialect, the language it belongs to.
    pub language: &'static str,
}

struct Row {
    item: Languoid,
    key: String,
}

/// Lower case, Turkic i's together, no diacritics, apostrophes and
/// modifier letters (ʼ ʻ ' ’) dropped, and anything else but letters and
/// digits read as a space, so «Kʼicheʼ», «K'iche'» and «kiche» all meet.
pub fn key(s: &str) -> String {
    use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
    let folded: String = wugs_model::fold_key(s).nfd().filter(|c| !is_combining_mark(*c)).collect();
    let spaced: String = folded
        .chars()
        .filter(|c| !matches!(c, '\'' | '\u{2019}' | '\u{2018}' | '`' | '\u{2B0}'..='\u{2FF}'))
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| {
        TABLE
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut f = line.split('\t');
                let item = Languoid {
                    glottocode: f.next()?,
                    iso: f.next()?,
                    name: f.next()?,
                    dialect: f.next()? == "D",
                    family: f.next()?,
                    macroarea: f.next()?,
                    language: f.next().unwrap_or(""),
                };
                Some(Row { key: key(item.name), item })
            })
            .collect()
    })
}

/// How well a row answers the query: lower is better, None is no match.
fn rank(row: &Row, q: &str, code: &str) -> Option<u8> {
    if row.item.glottocode == code || (!row.item.iso.is_empty() && row.item.iso == code) {
        return Some(0);
    }
    if q.is_empty() {
        return None;
    }
    if row.key == q {
        Some(1)
    } else if row.key.starts_with(q) {
        Some(2)
    } else if row.key.split(' ').any(|w| w.starts_with(q)) || row.key.contains(&format!(" {q}")) {
        Some(3)
    } else if q.chars().count() >= 3 && row.key.contains(q) {
        Some(4)
    } else {
        None
    }
}

/// The languoids a typed name or code fits, best first: an exact
/// Glottocode or ISO code, then the whole name, then names starting with
/// it, then a word in the name starting with it, then (three letters or
/// more) anywhere in it; languages before dialects, then shorter names.
pub fn find(query: &str, limit: usize) -> Vec<Languoid> {
    let q = key(query);
    let code = query.trim().to_lowercase();
    if q.chars().count() < 2 && code.chars().count() < 3 {
        return Vec::new();
    }
    let mut hits: Vec<(u8, &Row)> = rows().iter().filter_map(|r| rank(r, &q, &code).map(|k| (k, r))).collect();
    hits.sort_by(|(a, x), (b, y)| a.cmp(b).then(x.item.dialect.cmp(&y.item.dialect)).then(x.item.name.len().cmp(&y.item.name.len())).then(x.item.name.cmp(y.item.name)));
    hits.into_iter().take(limit).map(|(_, r)| r.item.clone()).collect()
}

/// One languoid by its Glottocode.
pub fn by_glottocode(code: &str) -> Option<Languoid> {
    let code = code.trim().to_lowercase();
    rows().iter().find(|r| r.item.glottocode == code).map(|r| r.item.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_whole_and_well_formed() {
        assert!(rows().len() > 20_000, "{}", rows().len());
        for r in rows() {
            assert!(wugs_core::language::valid_glottocode(r.item.glottocode), "{}", r.item.glottocode);
            assert!(r.item.iso.is_empty() || wugs_core::language::valid_iso639_3(r.item.iso), "{} {}", r.item.glottocode, r.item.iso);
            assert!(!r.item.name.is_empty());
        }
    }

    #[test]
    fn names_and_codes_find_their_language() {
        let tr = find("Turkish", 8);
        assert_eq!(tr[0].glottocode, "nucl1301");
        assert_eq!(tr[0].iso, "tur");
        assert_eq!(tr[0].family, "Turkic");
        assert_eq!(find("tur", 3)[0].name, "Turkish", "an ISO code first");
        assert_eq!(find("stan1293", 3)[0].name, "English", "a Glottocode first");
        assert_eq!(find("  STAN1293 ", 3)[0].name, "English");
        assert!(find("turk", 8).iter().any(|l| l.glottocode == "nucl1301"), "a name's start");
        // Diacritics and case do not matter.
        assert_eq!(find("nahuatl", 30).iter().filter(|l| l.name.to_lowercase().contains("nahuatl") || l.name.contains("Náhuatl")).count(), find("nahuatl", 30).len());
        assert!(find("t", 8).is_empty(), "one letter finds nothing");
        assert!(find("zzzzqqq", 8).is_empty());
    }

    #[test]
    fn languages_come_before_dialects() {
        let hits = find("German", 20);
        let first_dialect = hits.iter().position(|l| l.dialect).unwrap_or(hits.len());
        assert!(hits[..first_dialect].iter().all(|l| !l.dialect));
        assert_eq!(by_glottocode("stan1295").map(|l| l.name), Some("German"));
    }

    #[test]
    fn keys_ignore_marks_and_punctuation() {
        assert_eq!(key("Kʼicheʼ"), key("K'iche'"));
        assert_eq!(key("Kʼicheʼ"), "kiche");
        assert_eq!(key("  Nāhuatl  "), "nahuatl");
        assert_eq!(key("İstanbul Türkçesi"), "istanbul turkcesi");
    }
}
