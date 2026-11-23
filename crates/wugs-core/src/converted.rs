//! Converted tracks: a transcription track written from another by rule,
//! such as an IPA track made from a practical orthography with an
//! orthography profile (the desktop's Orthography panel works out the
//! text). Each converted segment is an ordinary segment on its own
//! time-aligned transcription track, so the Timeline, Transcribe, the
//! Sounds panel and the corpus tools read it like any other, and a person
//! can correct it. It names its source in `x.converted-from` and keeps the
//! text the conversion wrote in `x.converted`, so converting again after the
//! rules change rewrites only segments nobody has edited since.

use wugs_model::Value;

use crate::{CommitResult, Project};

/// One source segment and what the conversion makes of it.
pub struct ConvertedRow<'a> {
    pub source: &'a str,
    pub content: &'a str,
}

/// What one conversion wrote.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConvertReport {
    /// New segments on the converted track.
    pub created: usize,
    /// Segments rewritten because the conversion now gives other text.
    pub updated: usize,
    /// Segments that already say what the conversion gives.
    pub unchanged: usize,
    /// Segments someone edited after they were converted, left as they are.
    pub kept: usize,
    /// Source segments with nothing to convert (empty), none written.
    pub empty: usize,
}

impl Project {
    /// The live segments on `tier` that were converted from `source`.
    pub fn converted_from(&self, source: &str, tier: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for id in self.proj.children_by_ref("x.converted-from", source, "segment")? {
            if self.proj.field_str(&id, "tier")?.as_deref() == Some(tier) {
                out.push(id);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Write `rows` of `text` onto the converted track `tier` in one plain
    /// txn (a rule wrote them, so they close no conflict). A source segment
    /// with no converted segment yet gets one with its times and place; one
    /// whose converted segment still holds what an earlier conversion wrote
    /// (or nothing) is rewritten; one someone has edited since is kept.
    /// `by` names the rules (e.g. the profile's column), kept in
    /// `x.converted-by`. Nothing is committed when nothing changes.
    pub fn write_converted(&mut self, text: &str, tier: &str, rows: &[ConvertedRow], by: &str) -> Result<(ConvertReport, Option<CommitResult>), String> {
        self.require(text, "text")?;
        self.require(tier, "tier")?;
        let mut report = ConvertReport::default();
        // Work everything out before the txn opens (it borrows the project).
        enum Write {
            Create(Vec<(&'static str, Value)>),
            Rewrite(String, String),
        }
        let mut writes = Vec::new();
        for row in rows {
            self.require_in_text(row.source, "segment", text)?;
            if self.proj.field_str(row.source, "tier")?.as_deref() == Some(tier) {
                return Err("a track cannot be converted onto itself".into());
            }
            let content = self.proj.nfc(row.content.trim());
            let existing = self.converted_from(row.source, tier)?;
            let Some(target) = existing.first() else {
                if content.is_empty() {
                    report.empty += 1;
                    continue;
                }
                let pos = self.proj.field_str(row.source, "pos")?.unwrap_or_else(|| wugs_model::pos_seq(0));
                let mut fields = vec![
                    ("text", Value::str(text)),
                    ("tier", Value::str(tier)),
                    ("pos", Value::str(&pos)),
                    ("content", Value::str(&content)),
                    ("x.converted-from", Value::str(row.source)),
                    ("x.converted", Value::str(&content)),
                    ("x.converted-by", Value::str(by)),
                ];
                for edge in ["start", "end"] {
                    if let Some(ms) = self.proj.field_num(row.source, edge)? {
                        fields.push((edge, Value::Num(ms)));
                    }
                }
                writes.push(Write::Create(fields));
                report.created += 1;
                continue;
            };
            let current = self.proj.field_str(target, "content")?.unwrap_or_default();
            if current == content {
                report.unchanged += 1;
                continue;
            }
            let written = self.proj.field_str(target, "x.converted")?.unwrap_or_default();
            if current.is_empty() || current == written {
                writes.push(Write::Rewrite(target.clone(), content));
                report.updated += 1;
            } else {
                report.kept += 1;
            }
        }
        if writes.is_empty() {
            return Ok((report, None));
        }
        let mut t = self.intent("convert-track");
        for w in writes {
            match w {
                Write::Create(fields) => {
                    t.create("segment", "seg", text, fields);
                }
                Write::Rewrite(target, content) => {
                    t.text_edit("segment", &target, text, "content", &content)?;
                    t.set("segment", &target, text, "x.converted", Value::str(&content))?;
                    t.set("segment", &target, text, "x.converted-by", Value::str(by))?;
                }
            }
        }
        Ok((report, Some(t.commit()?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(tag: &str) -> (Project, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("wugs-converted-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (Project::open(&dir, "prs_Tester").unwrap(), dir)
    }

    #[test]
    fn a_track_converts_once_and_again_keeping_hand_edits() {
        let (mut p, dir) = project("again");
        let text = p.create_text("Story").unwrap();
        let (ortho, _) = p.add_tier("orthography", "time-aligned", Some("transcription"), None).unwrap();
        let (ipa, _) = p.add_tier("IPA", "time-aligned", Some("transcription"), None).unwrap();
        let (a, _) = p.add_segment(&text, &ortho, 0, Some(0.0), Some(900.0), "shalom").unwrap();
        let (b, _) = p.add_segment(&text, &ortho, 1, Some(1000.0), Some(1800.0), "tov").unwrap();
        let (c, _) = p.add_segment(&text, &ortho, 2, None, None, "").unwrap();

        let rows = [ConvertedRow { source: &a, content: "ʃalom" }, ConvertedRow { source: &b, content: "tov" }, ConvertedRow { source: &c, content: "" }];
        let (r, res) = p.write_converted(&text, &ipa, &rows, "IPA").unwrap();
        assert_eq!(r, ConvertReport { created: 2, empty: 1, ..Default::default() });
        assert!(res.is_some());
        let made = p.converted_from(&a, &ipa).unwrap();
        assert_eq!(made.len(), 1);
        assert_eq!(p.proj.field_str(&made[0], "content").unwrap().as_deref(), Some("ʃalom"));
        assert_eq!(p.proj.field_num(&made[0], "end").unwrap(), Some(900.0));
        assert_eq!(p.proj.field_str(&made[0], "text").unwrap().as_deref(), Some(text.as_str()));

        // The same conversion again writes nothing.
        let (r, res) = p.write_converted(&text, &ipa, &rows, "IPA").unwrap();
        assert_eq!(r, ConvertReport { unchanged: 2, empty: 1, ..Default::default() });
        assert!(res.is_none());

        // Someone corrects one; new rules change both: the corrected one stays.
        let tov = p.converted_from(&b, &ipa).unwrap()[0].clone();
        p.edit_content(&text, &tov, "toβ").unwrap();
        let rows = [ConvertedRow { source: &a, content: "ʃaˈlom" }, ConvertedRow { source: &b, content: "tɔv" }];
        let (r, _) = p.write_converted(&text, &ipa, &rows, "IPA").unwrap();
        assert_eq!(r, ConvertReport { updated: 1, kept: 1, ..Default::default() });
        assert_eq!(p.proj.field_str(&made[0], "content").unwrap().as_deref(), Some("ʃaˈlom"));
        assert_eq!(p.proj.field_str(&tov, "content").unwrap().as_deref(), Some("toβ"));

        assert!(p.write_converted(&text, &ortho, &rows, "IPA").is_err(), "a track onto itself");
        drop(p);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
