//! Cognate judgements made in the Compare word lists panel, kept as one
//! project setting so they sync and are sealed with the project: a line per
//! judgement, `y` or `n`, the other language, the concept and the other
//! language's form, tab-separated. Judged and counted in the webview
//! (`src/compare/cognates.ts`); here they are checked, put in NFC and in
//! order, and bounded.

use crate::session::{project, Session};

pub const KEY: &str = "compare-cognates";
/// What one setting holds (`wugs_core::settings::MAX_VALUE`), with room to spare.
const MAX_BYTES: usize = 60 * 1024;

pub fn get(s: &mut Session) -> Result<String, String> {
    Ok(project(s)?.setting(KEY)?.unwrap_or_default())
}

pub fn set(s: &mut Session, table: &str) -> Result<String, String> {
    let p = project(s)?;
    let mut lines: Vec<String> = Vec::new();
    for (n, line) in p.proj.nfc(table).lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 4 || !matches!(f[0], "y" | "n") || f[1..].iter().any(|x| x.trim().is_empty()) {
            return Err(format!("Line {} is not a judgement (y or n, a language, a concept and a form).", n + 1));
        }
        lines.push(f.iter().map(|x| x.trim()).collect::<Vec<_>>().join("\t"));
    }
    lines.sort();
    lines.dedup();
    let table = lines.join("\n");
    if table.len() > MAX_BYTES {
        return Err("Too many judgements to keep with the project (60 KB at most).".into());
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
    fn judgements_are_kept_in_order() {
        let dir = std::env::temp_dir().join(format!("wugs-cog-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        assert_eq!(get(&mut s).unwrap(), "");
        assert_eq!(set(&mut s, "y\tTeke\thand\tnima\nn\tSopo\thand\tkubu \n\ny\tTeke\thand\tnima\n").unwrap(), "n\tSopo\thand\tkubu\ny\tTeke\thand\tnima");
        assert_eq!(get(&mut s).unwrap(), "n\tSopo\thand\tkubu\ny\tTeke\thand\tnima");
        assert!(set(&mut s, "maybe\tTeke\thand\tnima").unwrap_err().contains("Line 1"));
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
