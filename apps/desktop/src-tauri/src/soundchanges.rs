//! A project's sound-change rules (the Sound changes panel), kept as one
//! project setting so they sync and are sealed with the project. The rules
//! are read and run in the webview (`src/soundchange/sca.ts`); here they
//! are only stored, in NFC, and bounded.

use crate::session::{project, Session};

pub const KEY: &str = "sound-changes";
/// Several hundred rules with comments.
const MAX_BYTES: usize = 32 * 1024;

pub fn get(s: &mut Session) -> Result<String, String> {
    Ok(project(s)?.setting(KEY)?.unwrap_or_default())
}

pub fn set(s: &mut Session, rules: &str) -> Result<String, String> {
    let p = project(s)?;
    let rules = p.proj.nfc(rules.trim_end()).replace("\r\n", "\n");
    if rules.len() > MAX_BYTES {
        return Err("The rules are too long to keep with the project (32 KB at most).".into());
    }
    if p.setting(KEY)?.unwrap_or_default() != rules {
        p.set_setting(KEY, &rules)?;
    }
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_kept_with_the_project() {
        let dir = std::env::temp_dir().join(format!("wugs-sca-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        assert_eq!(get(&mut s).unwrap(), "");
        assert_eq!(set(&mut s, "V = a e\r\ne\u{301} > i / _#\n\n").unwrap(), "V = a e\né > i / _#");
        assert_eq!(get(&mut s).unwrap(), "V = a e\né > i / _#");
        assert!(set(&mut s, &"x".repeat(40_000)).is_err());
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
