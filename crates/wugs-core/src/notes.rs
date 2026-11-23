//! Notes on a sentence: the field linguist's remarks ("speaker hesitated",
//! "check this form with M.", "cf. text 3"), FLEx's Note line. Each note is
//! an Annotation (DATA-MODEL §3: `target`, `kind`, `body`) of kind `note`,
//! in its text's scope, so it shares the text's sharing level and syncs with
//! it. A sentence can have several: two people noting the same sentence
//! before a sync both keep theirs. The body is collaborative text, so two
//! devices editing one note merge.

use wugs_model::Value;

use crate::{CommitResult, Project};

/// The longest note kept (characters); a note, not a chapter.
pub const MAX_NOTE: usize = 4000;

fn clean(p: &Project, body: &str) -> Result<String, String> {
    let body = p.proj.nfc(body.trim());
    if body.chars().count() > MAX_NOTE {
        return Err(format!("A note is at most {MAX_NOTE} characters."));
    }
    Ok(body)
}

impl Project {
    /// The live notes on a segment, oldest first: `(note id, body)`.
    pub fn segment_notes(&self, segment: &str) -> Result<Vec<(String, String)>, String> {
        let mut ids = self.proj.children_by_ref("target", segment, "annotation")?;
        ids.sort();
        let mut out = Vec::new();
        for id in ids {
            if self.proj.field_str(&id, "kind")?.as_deref() == Some("note") {
                out.push((id.clone(), self.proj.field_str(&id, "body")?.unwrap_or_default()));
            }
        }
        Ok(out)
    }

    /// Add a note to a segment of `text`. An empty note is refused.
    pub fn add_segment_note(&mut self, text: &str, segment: &str, body: &str) -> Result<(String, CommitResult), String> {
        self.require_in_text(segment, "segment", text)?;
        let body = clean(self, body)?;
        if body.is_empty() {
            return Err("A note needs some text.".into());
        }
        let mut t = self.intent("add-note");
        let id = t.create(
            "annotation",
            "ann",
            text,
            vec![("target", Value::str(segment)), ("kind", Value::str("note")), ("body", Value::str(&body))],
        );
        Ok((id, t.commit()?))
    }

    fn require_note(&self, text: &str, note: &str) -> Result<(), String> {
        self.require(text, "text")?;
        self.require(note, "annotation")?;
        // A note's text is its sentence's (the text is also the note's scope).
        let target = self.proj.field_str(note, "target")?.unwrap_or_default();
        if self.proj.field_str(note, "kind")?.as_deref() != Some("note") || self.proj.entity_kind(&target)?.as_deref() != Some("segment") || self.proj.field_str(&target, "text")?.as_deref() != Some(text) {
            return Err(format!("{note} is not a note of text {text}"));
        }
        Ok(())
    }

    /// Retype a note (a person's edit, so it settles a conflict on it). An
    /// empty body deletes the note; the same body writes nothing.
    pub fn set_note(&mut self, text: &str, note: &str, body: &str) -> Result<Option<CommitResult>, String> {
        self.require_note(text, note)?;
        let body = clean(self, body)?;
        if body.is_empty() {
            return self.delete_note(text, note).map(Some);
        }
        if self.proj.field_str(note, "body")?.as_deref() == Some(body.as_str()) {
            return Ok(None);
        }
        let mut t = self.intent("edit-note").resolving();
        t.text_edit("annotation", note, text, "body", &body)?;
        t.commit().map(Some)
    }

    pub fn delete_note(&mut self, text: &str, note: &str) -> Result<CommitResult, String> {
        self.require_note(text, note)?;
        let mut t = self.intent("delete-note");
        t.delete("annotation", note, text);
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use crate::Project;

    #[test]
    fn notes_are_kept_per_sentence_edited_and_deleted() {
        let root = std::env::temp_dir().join(format!("wugs-notes-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        let text = p.create_text("Story").unwrap();
        let (tier, _) = p.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
        let (seg, _) = p.add_segment(&text, &tier, 0, Some(0.0), Some(900.0), "kani").unwrap();
        let (other, _) = p.add_segment(&text, &tier, 1, Some(900.0), Some(1800.0), "teka").unwrap();
        assert!(p.add_segment_note(&text, &seg, "  ").is_err());
        let (a, _) = p.add_segment_note(&text, &seg, " Speaker hesitated. ").unwrap();
        let (b, _) = p.add_segment_note(&text, &seg, "Check with M.").unwrap();
        assert_eq!(p.segment_notes(&seg).unwrap(), vec![(a.clone(), "Speaker hesitated.".into()), (b.clone(), "Check with M.".into())]);
        assert!(p.segment_notes(&other).unwrap().is_empty());
        assert!(p.set_note(&text, &a, "Speaker hesitated.").unwrap().is_none(), "unchanged writes nothing");
        p.set_note(&text, &a, "Speaker hesitated before «kani».").unwrap().unwrap();
        assert_eq!(p.segment_notes(&seg).unwrap()[0].1, "Speaker hesitated before «kani».");
        p.set_note(&text, &b, "").unwrap();
        assert_eq!(p.segment_notes(&seg).unwrap().len(), 1, "an emptied note is deleted");
        // A note belongs to its text: another text cannot edit it.
        let elsewhere = p.create_text("Other").unwrap();
        assert!(p.set_note(&elsewhere, &a, "x").is_err());
        assert!(p.add_segment_note(&elsewhere, &seg, "x").is_err());
        drop(p);
        // The log is the truth: a replay gives the same notes.
        std::fs::remove_dir_all(root.join("cache")).ok();
        let p = Project::open(&root, "prs_Tester").unwrap();
        assert_eq!(p.segment_notes(&seg).unwrap().len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }
}
