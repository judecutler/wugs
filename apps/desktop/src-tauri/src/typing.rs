//! A project's typing shortcuts (`;n` → ŋ), kept as one project setting so
//! every device and everyone on the project types the same way. The table
//! is read in the webview (`src/transcribe/shortcuts.ts`); here it is only
//! stored, in NFC, and bounded.

use crate::session::{project, Session};

pub const KEY: &str = "typing-shortcuts";
/// Far more than 300 lines of eight characters a side need.
const MAX_BYTES: usize = 16 * 1024;

pub fn get(s: &mut Session) -> Result<String, String> {
    Ok(project(s)?.setting(KEY)?.unwrap_or_default())
}

pub fn set(s: &mut Session, table: &str) -> Result<String, String> {
    let p = project(s)?;
    let table = p.proj.nfc(table.trim_end()).replace("\r\n", "\n");
    if table.len() > MAX_BYTES {
        return Err("The table is too long: keep it under 300 lines.".into());
    }
    if p.setting(KEY)?.unwrap_or_default() != table {
        p.set_setting(KEY, &table)?;
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_kept_with_the_project() {
        let dir = std::env::temp_dir().join(format!("wugs-typing-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        assert_eq!(get(&mut s).unwrap(), "");
        assert_eq!(set(&mut s, ";n ŋ\r\ne\u{301} x\n\n").unwrap(), ";n ŋ\né x");
        assert_eq!(get(&mut s).unwrap(), ";n ŋ\né x");
        assert!(set(&mut s, &"x".repeat(20_000)).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
