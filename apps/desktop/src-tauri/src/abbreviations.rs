//! What a project's gloss abbreviations mean, as its linguists wrote them
//! (the Interlinear panel's Abbreviations list; `src/gloss/abbreviations.ts`
//! has the Leipzig defaults and finds which ones a text uses). Each meaning
//! is its own project setting, `abbreviation:<abbr>`, so it syncs to every
//! member and two people explaining different abbreviations never conflict.

use crate::session::{self, Session};

/// The longest abbreviation kept (in bytes); glosses are short codes.
const MAX_ABBREVIATION: usize = 32;
/// The longest meaning kept (in characters).
const MAX_MEANING: usize = 200;

/// The setting an abbreviation's meaning lives in. Setting keys are ASCII,
/// so any byte other than a letter or digit is written `%XX`.
pub fn key(abbr: &str) -> Result<String, String> {
    if abbr.is_empty() || abbr.len() > MAX_ABBREVIATION || abbr.chars().any(char::is_whitespace) {
        return Err(format!("«{abbr}» is not a gloss abbreviation"));
    }
    let mut out = String::from("abbreviation:");
    for b in abbr.bytes() {
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    Ok(out)
}

/// The project's meaning for each abbreviation asked about (None when nobody wrote one).
pub fn meanings(session: &mut Session, abbrs: &[String]) -> Result<Vec<Option<String>>, String> {
    let p = session::project(session)?;
    abbrs.iter().map(|a| Ok(p.setting(&key(a)?)?.filter(|v| !v.trim().is_empty()))).collect()
}

/// Write (or, when empty, clear) the project's meaning of `abbr`. Nothing is
/// written when it already says that.
pub fn set(session: &mut Session, abbr: &str, meaning: &str) -> Result<(), String> {
    let k = key(abbr)?;
    let p = session::project(session)?;
    let meaning = p.proj.nfc(&meaning.split_whitespace().collect::<Vec<_>>().join(" "));
    if meaning.chars().count() > MAX_MEANING {
        return Err(format!("A meaning is at most {MAX_MEANING} characters."));
    }
    if p.setting(&k)?.unwrap_or_default() == meaning {
        return Ok(());
    }
    p.set_setting(&k, &meaning).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_ascii_and_one_per_abbreviation() {
        assert_eq!(key("PL").unwrap(), "abbreviation:PL");
        assert_eq!(key("3SG").unwrap(), "abbreviation:3SG");
        assert_eq!(key("ÇOK").unwrap(), "abbreviation:%C3%87OK");
        assert_ne!(key("A.B").unwrap(), key("A%2EB").unwrap());
        assert!(key("").is_err());
        assert!(key("A B").is_err());
        assert!(key(&"X".repeat(40)).is_err());
    }

    #[test]
    fn meanings_are_kept_per_project_and_cleared_when_empty() {
        let dir = std::env::temp_dir().join(format!("wugs-abbreviations-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let asked = vec!["AV".to_string(), "PL".to_string()];
        assert_eq!(meanings(&mut session, &asked).unwrap(), vec![None, None]);
        set(&mut session, "AV", "  actor\n voice ").unwrap();
        assert_eq!(meanings(&mut session, &asked).unwrap(), vec![Some("actor voice".into()), None]);
        let before = session::project(&mut session).unwrap().proj.max_hlc().unwrap();
        set(&mut session, "AV", "actor voice").unwrap();
        assert_eq!(session::project(&mut session).unwrap().proj.max_hlc().unwrap(), before, "the same meaning writes nothing");
        set(&mut session, "AV", "").unwrap();
        assert_eq!(meanings(&mut session, &asked).unwrap(), vec![None, None]);
        assert!(set(&mut session, "AV", &"x".repeat(201)).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
