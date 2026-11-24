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

/// The Source fields DATA-MODEL §3 names that a recording's details edit directly.
const SOURCE_FIELDS: [&str; 2] = ["date", "place"];

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

    /// A person's details (born, languages, from…; what an archive's actor
    /// record asks for), `meta:<key>` fields like a text's metadata, sorted by key.
    pub fn person_details(&self, person: &str) -> Result<Metadata, String> {
        self.text_metadata(person)
    }

    /// Set (or, with an empty value, clear) one of a person's details: a
    /// person's edit, so resolving, in the people scope. Returns the key it
    /// was written under.
    pub fn set_person_detail(&mut self, person: &str, key: &str, value: &str) -> Result<(String, CommitResult), String> {
        self.require(person, "person")?;
        let key = key_of(key).ok_or_else(|| format!("«{key}» cannot be a detail's name: it needs a letter first"))?;
        let value = self.metadata_value(value)?;
        let field = format!("{PREFIX}{key}");
        let mut t = self.intent("set-person-detail").resolving();
        t.set("person", person, crate::SCOPE_PEOPLE, &field, value)?;
        Ok((key, t.commit()?))
    }

    /// A recording's details (date, place, recorded by, equipment…): its
    /// own `date` and `place` fields (DATA-MODEL §3 Source) and its
    /// `meta:<key>` fields, sorted by key.
    pub fn source_details(&self, source: &str) -> Result<Metadata, String> {
        let mut out = self.text_metadata(source)?;
        for key in SOURCE_FIELDS {
            if let Some((Value::Str(v), _)) = self.proj.field(source, key)?.filter(|(v, _)| matches!(v, Value::Str(s) if !s.is_empty())) {
                out.retain(|(k, _)| k != key);
                out.push((key.to_string(), v));
            }
        }
        out.sort();
        Ok(out)
    }

    /// Set (or clear) one of a recording's details, in the recording's own
    /// scope: `date` and `place` are the Source's own fields, any other key
    /// a `meta:<key>` field. Resolving, a person's edit.
    pub fn set_source_detail(&mut self, source: &str, key: &str, value: &str) -> Result<(String, CommitResult), String> {
        self.require(source, "source")?;
        let key = key_of(key).ok_or_else(|| format!("«{key}» cannot be a detail's name: it needs a letter first"))?;
        let value = self.metadata_value(value)?;
        let field = if SOURCE_FIELDS.contains(&key.as_str()) { key.clone() } else { format!("{PREFIX}{key}") };
        let mut t = self.intent("set-source-detail").resolving();
        t.set("source", source, source, &field, value)?;
        Ok((key, t.commit()?))
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

    #[test]
    fn a_recording_s_details_use_its_own_date_and_place() {
        let root = std::env::temp_dir().join(format!("wugs-source-details-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Test").unwrap();
        let src = p.add_source("Session 1", None, "audio", None).unwrap();
        p.set_source_detail(&src, "Date", "2026-11-03").unwrap();
        p.set_source_detail(&src, "Recorded by", "J. C.").unwrap();
        p.set_source_detail(&src, "Place", "Kelani").unwrap();
        assert_eq!(p.proj.field_str(&src, "date").unwrap().as_deref(), Some("2026-11-03"), "the Source's own field");
        assert_eq!(
            p.source_details(&src).unwrap(),
            vec![("date".to_string(), "2026-11-03".to_string()), ("place".to_string(), "Kelani".to_string()), ("recorded_by".to_string(), "J. C.".to_string())]
        );
        p.set_source_detail(&src, "place", "").unwrap();
        assert_eq!(p.source_details(&src).unwrap().len(), 2, "cleared: {:?}", p.source_details(&src).unwrap());
        assert!(p.set_source_detail("src_nothing", "date", "x").is_err());
        drop(p);
        std::fs::remove_dir_all(root).ok();
    }
}
