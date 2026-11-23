//! Metadata per document (SCROLLS.md §2): each text carries `meta:<key>`
//! fields (author, date, genre, speaker, source…), so a filter is a field
//! query and two devices that fill in different keys never meet. Keys are
//! normalized (`key_of`) so a CSV header `Text Genre` and a TEI genre land
//! on the same `text_genre`-style key the CQP filter `match.text_genre`
//! names. Values are text as written, in NFC; an empty value clears.

use wugs_model::Value;

use crate::{CommitResult, Project};

/// The field prefix every metadata key is written under.
pub const PREFIX: &str = "meta:";

/// The longest key kept.
pub const MAX_KEY: usize = 40;

/// The longest value kept, in characters.
pub const MAX_VALUE: usize = 2000;

/// A key as typed or read from a header, made into a metadata key: lower
/// case, letters and digits of any script kept, every run of anything else
/// one `_`, no `_` at either end; None when nothing is left or it starts
/// with a digit (a CQP attribute name cannot). `Text Genre` → `text_genre`,
/// `dc:date` → `dc_date`.
pub fn key_of(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut gap = false;
    for c in raw.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    let first = out.chars().next()?;
    if first.is_numeric() {
        return None;
    }
    Some(out.chars().take(MAX_KEY).collect::<String>().trim_end_matches('_').to_string())
}

/// One text's metadata, by key.
pub type Metadata = Vec<(String, String)>;

impl Project {
    /// A live text's metadata, sorted by key; empty values left out.
    pub fn text_metadata(&self, text: &str) -> Result<Metadata, String> {
        let mut out: Metadata = self
            .proj
            .fields_of(text)?
            .into_iter()
            .filter_map(|(f, v)| match (f.strip_prefix(PREFIX), v) {
                (Some(k), Value::Str(s)) if !s.is_empty() => Some((k.to_string(), s)),
                _ => None,
            })
            .collect();
        out.sort();
        Ok(out)
    }

    /// Every key some live text has, with how many texts have it, by key.
    pub fn metadata_keys(&self) -> Result<Vec<(String, usize)>, String> {
        let mut keys: std::collections::BTreeMap<String, usize> = Default::default();
        for text in self.proj.entities_of_kind("text")? {
            for (k, _) in self.text_metadata(&text)? {
                *keys.entry(k).or_default() += 1;
            }
        }
        Ok(keys.into_iter().collect())
    }

    /// Set (or, with an empty value, clear) one key of a text's metadata: a
    /// person's edit, so resolving. The key is normalized with [`key_of`];
    /// returns the key it was written under.
    pub fn set_text_metadata(&mut self, text: &str, key: &str, value: &str) -> Result<(String, CommitResult), String> {
        self.require(text, "text")?;
        let key = key_of(key).ok_or_else(|| format!("«{key}» cannot be a metadata name: it needs a letter first"))?;
        let value = self.metadata_value(value)?;
        let mut t = self.intent("set-text-metadata").resolving();
        t.set("text", text, text, &format!("{PREFIX}{key}"), value)?;
        Ok((key, t.commit()?))
    }

    /// Metadata for many texts in one plain txn (an import: a table of
    /// documents, a corpus's headers). Keys are normalized; a value equal
    /// to what the text has is not written again, and an empty one is
    /// skipped, so importing the same table twice writes nothing.
    /// Returns how many fields were written.
    pub fn import_text_metadata(&mut self, rows: &[(String, String, String)]) -> Result<(usize, Option<CommitResult>), String> {
        let mut writes = Vec::new();
        for (text, key, value) in rows {
            self.require(text, "text")?;
            let Some(key) = key_of(key) else { continue };
            let Value::Str(v) = self.metadata_value(value)? else { continue };
            let field = format!("{PREFIX}{key}");
            if self.proj.field_str(text, &field)?.as_deref() == Some(v.as_str()) {
                continue;
            }
            writes.push((text.clone(), field, v));
        }
        if writes.is_empty() {
            return Ok((0, None));
        }
        let mut t = self.intent("import-text-metadata");
        for (text, field, v) in &writes {
            t.set("text", text, text, field, Value::str(v))?;
        }
        Ok((writes.len(), Some(t.commit()?)))
    }

    /// A value as stored: trimmed, line breaks as spaces, NFC; empty is Null.
    fn metadata_value(&self, value: &str) -> Result<Value, String> {
        let v: String = value.split(['\r', '\n', '\t']).map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        if v.chars().count() > MAX_VALUE {
            return Err(format!("a metadata value is kept up to {MAX_VALUE} characters"));
        }
        Ok(if v.is_empty() { Value::Null } else { Value::Str(self.proj.nfc(&v)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_normalized_the_way_a_query_names_them() {
        assert_eq!(key_of("Text Genre").as_deref(), Some("text_genre"));
        assert_eq!(key_of("  dc:date ").as_deref(), Some("dc_date"));
        assert_eq!(key_of("Автор").as_deref(), Some("автор"));
        assert_eq!(key_of("लेखक का नाम").as_deref(), Some("लेखक_का_नाम"), "vowel signs are Alphabetic");
        assert_eq!(key_of("__speaker__").as_deref(), Some("speaker"));
        assert_eq!(key_of("1st"), None);
        assert_eq!(key_of("--"), None);
        assert_eq!(key_of(&"a".repeat(60)).map(|k| k.len()), Some(MAX_KEY));
    }
}
