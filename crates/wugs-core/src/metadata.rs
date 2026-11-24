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

/// The detail keys that hold a "lat, lon" pair, in the order they are read
/// (the Progress panel's map reads the same, `src/progress/places.ts`).
pub const COORDINATE_KEYS: [&str; 6] = ["coordinates", "gps", "latlon", "lat_lon", "location", "geo"];

/// One coordinate as written: a number, then optional minutes and seconds
/// after a degree sign, then an optional N S E W.
fn coordinates_in(text: &str) -> Vec<(f64, Option<char>)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let number = |i: &mut usize| -> Option<f64> {
        let start = *i;
        while *i < chars.len() && (chars[*i].is_ascii_digit() || chars[*i] == '.') {
            *i += 1;
        }
        chars[start..*i].iter().collect::<String>().parse().ok()
    };
    let skip_space = |i: &mut usize| {
        while *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
    };
    while i < chars.len() {
        let negative = matches!(chars[i], '-' | '−');
        let signed = negative || chars[i] == '+';
        if !(chars[i].is_ascii_digit() || (signed && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()))) {
            i += 1;
            continue;
        }
        if signed {
            i += 1;
        }
        let Some(deg) = number(&mut i) else { return Vec::new() };
        let mut value = deg;
        skip_space(&mut i);
        if i < chars.len() && matches!(chars[i], '°' | 'º' | '˚') {
            i += 1;
            for divisor in [60.0, 3600.0] {
                skip_space(&mut i);
                let at = i;
                match number(&mut i) {
                    Some(n) => {
                        skip_space(&mut i);
                        if i < chars.len() && matches!(chars[i], '\'' | '′' | '’' | '"' | '″' | '”') {
                            i += 1;
                            value += n / divisor;
                        } else {
                            i = at;
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
        skip_space(&mut i);
        let hemi = chars.get(i).map(|c| c.to_ascii_uppercase()).filter(|c| matches!(c, 'N' | 'S' | 'E' | 'W'));
        if hemi.is_some() {
            i += 1;
        }
        let south_west = matches!(hemi, Some('S' | 'W'));
        out.push((if negative || south_west { -value } else { value }, hemi));
    }
    out
}

/// "lat, lon" in the usual writings (signed decimals, N S E W, degrees,
/// minutes and seconds; longitude first when its hemisphere says so), or
/// None when it does not read as a place on Earth.
pub fn parse_coordinates(text: &str) -> Option<(f64, f64)> {
    let found = coordinates_in(text);
    let [mut a, mut b] = <[(f64, Option<char>); 2]>::try_from(found).ok()?;
    if matches!(a.1, Some('E' | 'W')) && matches!(b.1, Some('N' | 'S')) {
        std::mem::swap(&mut a, &mut b);
    }
    if matches!(a.1, Some('E' | 'W')) || matches!(b.1, Some('N' | 'S')) {
        return None;
    }
    (a.0.abs() <= 90.0 && b.0.abs() <= 180.0).then_some((a.0, b.0))
}

/// Where a text (or recording) was made, from its details: a pair under
/// `COORDINATE_KEYS`, else `latitude` and `longitude`.
pub fn place_of(meta: &[(String, String)]) -> Option<(f64, f64)> {
    let get = |k: &str| meta.iter().find(|(key, v)| key == k && !v.trim().is_empty()).map(|(_, v)| v.as_str());
    if let Some(p) = COORDINATE_KEYS.iter().filter_map(|k| get(k)).find_map(parse_coordinates) {
        return Some(p);
    }
    let one = |keys: &[&str]| keys.iter().find_map(|k| get(k)).map(coordinates_in).filter(|v| v.len() == 1).map(|v| v[0].0);
    let (lat, lon) = (one(&["latitude", "lat"])?, one(&["longitude", "lon", "long", "lng"])?);
    (lat.abs() <= 90.0 && lon.abs() <= 180.0).then_some((lat, lon))
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
    #[test]
    fn coordinates_are_read_in_the_usual_writings() {
        use super::{parse_coordinates, place_of};
        let near = |p: Option<(f64, f64)>, lat: f64, lon: f64| p.is_some_and(|(a, b)| (a - lat).abs() < 1e-4 && (b - lon).abs() < 1e-4);
        assert!(near(parse_coordinates("-8.65, 115.22"), -8.65, 115.22));
        assert!(near(parse_coordinates("−8.65 115.22"), -8.65, 115.22));
        assert!(near(parse_coordinates("8.65 S, 115.22 E"), -8.65, 115.22));
        assert!(near(parse_coordinates("115.22E 8.65S"), -8.65, 115.22));
        assert!(near(parse_coordinates("8°39'0\"S 115°13'12\"E"), -8.65, 115.22));
        assert_eq!(parse_coordinates("the village by the river"), None);
        assert_eq!(parse_coordinates("95, 10"), None);
        let meta = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        assert!(near(place_of(&meta(&[("place", "Ubud"), ("gps", "-8.5, 115.26")])), -8.5, 115.26));
        assert!(near(place_of(&meta(&[("latitude", "8.5S"), ("longitude", "115.26")])), -8.5, 115.26));
        assert_eq!(place_of(&meta(&[("place", "Ubud")])), None);
    }

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
