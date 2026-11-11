//! wugs-model: identifiers, hybrid logical clocks, JSON values, and the op envelope.
//!
//! Zero external dependencies by design (see CLAUDE.md). The JSON module is a
//! strict, minimal implementation sufficient for the op-log format; swap for
//! serde_json at the edges if/when crates.io is available.

pub mod base64;
pub mod blob;
pub mod hlc;
pub mod id;
pub mod json;
pub mod op;
pub mod sortkey;
pub mod text;

pub use blob::{parse_blob_ref, scan_blob_refs, BLOB_REF_PREFIX, REF_VECTORS};
pub use hlc::{Clock, Hlc};
pub use id::{new_id, ulid};
pub use json::Value;
pub use op::Op;
pub use text::{Normalizer, TextMerger};

/// The fields that carry language data, by name across every entity kind:
/// the core NFC-normalizes them at entry and the projection indexes them
/// for search (DATA-MODEL §8). Kit extension fields (`x.*`) stay out.
pub const LANGUAGE_FIELDS: &[&str] = &[
    "content", "title", "lemma", "citation", "gloss", "definition", "usage", "form", "surface",
    "notes", "body", "name",
];

/// Prefix of the per-language gloss fields (`gloss:es`): `gloss` itself is the
/// primary analysis language, each further language is a sibling field so
/// per-field LWW merges them independently (GLOSSING-MODEL, decision 3).
pub const GLOSS_LANG_PREFIX: &str = "gloss:";

/// Does this field carry language data (NFC at entry, indexed for search)?
/// The named fields plus every `gloss:<lang>`.
pub fn is_language_field(field: &str) -> bool {
    LANGUAGE_FIELDS.contains(&field) || field.starts_with(GLOSS_LANG_PREFIX)
}

/// The case-folded key under which a form is looked up: tokens, lexeme
/// lemmas and allomorph forms that differ only in case share it. NFC is the
/// caller's job (the `Normalizer` seam); this crate has no Unicode tables.
///
/// Turkic dotted and dotless i fold together, without knowing the language:
/// `I`, `İ`, `ı` and `i` all become `i`, and a combining dot above an i
/// (`İ` decomposed, or what `to_lowercase` makes of `İ`) is dropped. So
/// `Işık` finds `ışık` and `İstanbul` finds `istanbul`. The cost is that
/// Turkish pairs like `kır`/`kir` share a key; keys only rank suggestions
/// and lookups, which the linguist still accepts or not, so a wider match is
/// the safe side.
pub fn fold_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut after_i = false;
    for c in s.chars() {
        if c == '\u{307}' && after_i {
            continue;
        }
        after_i = matches!(c, 'I' | 'İ' | 'ı' | 'i');
        if after_i {
            out.push('i');
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// `fold_key` for lexicon entries, which may spell an affix with its morph
/// marker (`-ler`, `=ki`, `re~`, `<um>`): the edge markers do not take part
/// in the match.
pub fn lexicon_key(s: &str) -> String {
    fold_key(s).trim_matches(|c| matches!(c, '-' | '=' | '~' | '<' | '>')).to_string()
}

/// Current op schema version (DATA-MODEL.md §10).
pub const SCHEMA_VERSION: u32 = 2;

/// Simple sortable position key for ordered lists.
/// TODO: replace with true fractional indexing (between(a, b)) before
/// concurrent reordering ships; this is sufficient for append-ordered v0 data.
pub fn pos_seq(i: usize) -> String {
    format!("{:08x}", i)
}

/// A position key that sorts after `a` and before `b` (or after `a` alone),
/// over the lowercase hex digits `pos_seq` writes: a split segment's second
/// part goes right after the first without renumbering anyone. `b`, when
/// given, must sort after `a`.
pub fn pos_between(a: &str, b: Option<&str>) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let digit = |s: &[u8], i: usize| s.get(i).and_then(|c| HEX.iter().position(|h| h == c));
    let (a, b) = (a.as_bytes(), b.map(str::as_bytes));
    let mut out = Vec::new();
    let mut i = 0;
    // Copy the common prefix; where the two part, take a digit between them
    // if there is room, else a's digit and then anything above a's tail.
    if let Some(b) = b {
        loop {
            let da = digit(a, i).unwrap_or(0);
            let Some(db) = digit(b, i) else { break };
            if da == db {
                out.push(HEX[da]);
                i += 1;
                continue;
            }
            if db > da + 1 {
                out.push(HEX[(da + db) / 2]);
                return String::from_utf8(out).expect("hex");
            }
            if db > da {
                out.push(HEX[da]);
                i += 1;
            }
            break;
        }
    }
    loop {
        match digit(a, i) {
            None => {
                out.push(b'8');
                break;
            }
            Some(15) => {
                out.push(b'f');
                i += 1;
            }
            Some(d) => {
                out.push(HEX[(d + 16 + 1) / 2]);
                break;
            }
        }
    }
    String::from_utf8(out).expect("hex")
}

/// Does this string look like a prefixed entity id (`tok_01ARZ…`)?
pub fn looks_like_id(s: &str) -> bool {
    let Some((prefix, rest)) = s.split_once('_') else {
        return false;
    };
    if !(2..=3).contains(&prefix.len()) || rest.len() != 26 {
        return false;
    }
    prefix.bytes().all(|b| b.is_ascii_lowercase())
        && rest
            .bytes()
            .all(|b| id::CROCKFORD.contains(&b.to_ascii_uppercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_shapes() {
        let id = new_id("tok");
        assert!(looks_like_id(&id), "{id}");
        assert!(!looks_like_id("hello"));
        assert!(!looks_like_id("tok_short"));
        assert!(!looks_like_id("TOK_01234567890123456789012345"));
    }

    #[test]
    fn fold_key_folds_turkic_i_without_mangling_it() {
        assert_eq!(fold_key("Işık"), fold_key("ışık"));
        assert_eq!(fold_key("Işık"), "işik");
        assert_eq!(fold_key("İstanbul"), "istanbul");
        assert_eq!(fold_key("İstanbul"), fold_key("istanbul"));
        assert_eq!(fold_key("I\u{307}stanbul"), "istanbul", "a decomposed İ");
        assert_eq!(fold_key(&"İstanbul".to_lowercase()), "istanbul", "Rust's lowercase of İ");
        assert_eq!(fold_key("KÖPEKLER"), "köpekler");
        assert_eq!(fold_key("a\u{307}"), "a\u{307}", "a dot above anything but i is kept");
    }

    #[test]
    fn lexicon_key_ignores_edge_markers_infix_brackets_included() {
        for spelled in ["-ler", "ler-", "=ler", "~ler", "<ler>", "LER"] {
            assert_eq!(lexicon_key(spelled), "ler", "{spelled}");
        }
    }

    #[test]
    fn pos_orders() {
        assert!(pos_seq(1) < pos_seq(2));
        assert!(pos_seq(9) < pos_seq(10));
        assert!(pos_seq(255) < pos_seq(4096));
        let (a, b) = (pos_seq(3), pos_seq(4));
        let m = pos_between(&a, Some(&b));
        assert!(a < m && m < b, "{m}");
        let m2 = pos_between(&a, Some(&m));
        assert!(a < m2 && m2 < m, "{m2}");
        let m3 = pos_between(&m2, Some(&m));
        assert!(m2 < m3 && m3 < m, "{m3}");
        assert!(pos_between(&a, None) > a);
        assert!(pos_between("0f", None).as_str() > "0f");
        let m5 = pos_between("0f", Some("1"));
        assert!("0f" < m5.as_str() && m5.as_str() < "1", "{m5}");
        assert_eq!(pos_between("00000003", Some("00000009")), "00000006");
        let m4 = pos_between("03", Some("0301"));
        assert!("03" < m4.as_str() && m4.as_str() < "0301", "{m4}");
    }
}
