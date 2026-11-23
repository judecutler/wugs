//! The language a project documents (DATA-MODEL §3, `Language`: name,
//! glottocode, iso639-3). One entity in the config scope whose id is derived
//! from a fixed key, so two devices that fill it in before they sync name the
//! same entity and the ordinary conflict card settles a disagreement. The
//! seal's OLAC metadata and the CLDF export read it.

use wugs_model::Value;

use crate::lexicon::id_from_key;
use crate::{CommitResult, Project, SCOPE_CONFIG};

/// The id of the project's own language entity.
pub fn project_language_id() -> String {
    id_from_key("lng", "project-language")
}

/// What the project says about its language. Empty strings when unset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectLanguage {
    pub name: String,
    pub glottocode: String,
    pub iso: String,
}

/// A Glottocode: four lowercase letters or digits, then four digits (`stan1293`).
pub fn valid_glottocode(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 8 && b[..4].iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && b[4..].iter().all(u8::is_ascii_digit)
}

/// An ISO 639-3 code: three lowercase letters (`tur`).
pub fn valid_iso639_3(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|c| c.is_ascii_lowercase())
}

impl Project {
    /// The project's language as recorded, or all empty.
    pub fn project_language(&self) -> Result<ProjectLanguage, String> {
        let id = project_language_id();
        if self.proj.entity_kind(&id)?.as_deref() != Some("language") || self.proj.is_tombstoned(&id)? {
            return Ok(ProjectLanguage::default());
        }
        let get = |f: &str| self.proj.field_str(&id, f).map(Option::unwrap_or_default);
        Ok(ProjectLanguage { name: get("name")?, glottocode: get("glottocode")?, iso: get("iso639-3")? })
    }

    /// Record the project's language. Codes are checked (a Glottocode like
    /// `stan1293`, ISO 639-3 like `tur`) and may be left empty; each changed
    /// field is a person's edit, so it settles a conflict on that field.
    pub fn set_project_language(&mut self, name: &str, glottocode: &str, iso: &str) -> Result<CommitResult, String> {
        let (name, glottocode, iso) = (name.trim(), glottocode.trim().to_ascii_lowercase(), iso.trim().to_ascii_lowercase());
        if !glottocode.is_empty() && !valid_glottocode(&glottocode) {
            return Err(format!("«{glottocode}» is not a Glottocode; they look like stan1293 (four letters, four digits)"));
        }
        if !iso.is_empty() && !valid_iso639_3(&iso) {
            return Err(format!("«{iso}» is not an ISO 639-3 code; they are three letters, like tur"));
        }
        let id = project_language_id();
        let now = self.project_language()?;
        let exists = self.proj.entity_kind(&id)?.as_deref() == Some("language");
        let deleted = exists && self.proj.is_tombstoned(&id)?;
        let mut t = self.intent("set-project-language").resolving();
        if !exists {
            let mut fields = vec![("name", Value::str(name))];
            if !glottocode.is_empty() {
                fields.push(("glottocode", Value::str(&glottocode)));
            }
            if !iso.is_empty() {
                fields.push(("iso639-3", Value::str(&iso)));
            }
            t.push_create("language", &id, SCOPE_CONFIG, fields);
        } else {
            if deleted {
                t.restore("language", &id, SCOPE_CONFIG);
            }
            for (field, old, new) in [("name", now.name.as_str(), name), ("glottocode", now.glottocode.as_str(), glottocode.as_str()), ("iso639-3", now.iso.as_str(), iso.as_str())] {
                if old != new {
                    t.set("language", &id, SCOPE_CONFIG, field, Value::str(new))?;
                }
            }
        }
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_checked_and_edits_land_on_one_entity() {
        assert!(valid_glottocode("stan1293") && valid_glottocode("nucl1301") && !valid_glottocode("Stan1293") && !valid_glottocode("stan129"));
        assert!(valid_iso639_3("tur") && !valid_iso639_3("tr") && !valid_iso639_3("TUR"));
        let root = std::env::temp_dir().join(format!("wugs-lang-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        assert_eq!(p.project_language().unwrap(), ProjectLanguage::default());
        p.set_project_language("Turkish", "nucl1301", "TUR").unwrap();
        assert_eq!(p.project_language().unwrap(), ProjectLanguage { name: "Turkish".into(), glottocode: "nucl1301".into(), iso: "tur".into() });
        p.set_project_language("Turkish", "", "tur").unwrap();
        assert_eq!(p.project_language().unwrap().glottocode, "");
        assert!(p.set_project_language("Turkish", "turkish", "").unwrap_err().contains("Glottocode"));
        assert_eq!(p.proj.entities_of_kind("language").unwrap(), vec![project_language_id()]);
        std::fs::remove_dir_all(root).ok();
    }
}
