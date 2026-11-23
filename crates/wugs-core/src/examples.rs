//! Example sentences of a sense: the illustrative sentence a dictionary
//! prints under a meaning, with its translation (FLEx's Example and
//! Translation, LIFT's `<example>`, MDF's `\xv` / `\xe`). Each is an
//! Example entity (DATA-MODEL §3: kind `example`, lexicon scope) with
//! `sense`, `pos`, `sentence`, `translation`, and optionally `source` (the
//! segment it was taken from) and `reference` (where it comes from, as
//! written: a LIFT `source`, a book and page). Sentence and translation are
//! collaborative text, so two devices editing one example merge.

use wugs_model::Value;

use crate::{CommitResult, Project, SCOPE_LEXICON};

/// The longest example sentence or translation kept (characters).
pub const MAX_EXAMPLE: usize = 2000;

/// The two text fields of an example a person edits.
pub const EXAMPLE_FIELDS: [&str; 2] = ["sentence", "translation"];

#[derive(Clone, Debug, PartialEq)]
pub struct Example {
    pub id: String,
    pub sentence: String,
    pub translation: String,
    /// The segment it was taken from, when it was.
    pub source: Option<String>,
    pub reference: Option<String>,
}

fn clean(p: &Project, text: &str) -> Result<String, String> {
    let text = p.proj.nfc(text.split_whitespace().collect::<Vec<_>>().join(" ").as_str());
    if text.chars().count() > MAX_EXAMPLE {
        return Err(format!("An example is at most {MAX_EXAMPLE} characters."));
    }
    Ok(text)
}

impl Project {
    /// A sense's live examples in order.
    pub fn examples(&self, sense: &str) -> Result<Vec<Example>, String> {
        let mut out = Vec::new();
        for id in self.proj.ordered_children("sense", sense, "example")? {
            let text = |f: &str| self.proj.field_str(&id, f).map(Option::unwrap_or_default);
            let opt = |f: &str| self.proj.field_str(&id, f).map(|v| v.filter(|s| !s.is_empty()));
            out.push(Example { sentence: text("sentence")?, translation: text("translation")?, source: opt("source")?, reference: opt("reference")?, id: id.clone() });
        }
        Ok(out)
    }

    /// A new example after the sense's last. An empty sentence is refused;
    /// `source` must be a live segment when given.
    pub fn add_example(&mut self, sense: &str, sentence: &str, translation: &str, source: Option<&str>, reference: Option<&str>) -> Result<(String, CommitResult), String> {
        self.require(sense, "sense")?;
        let sentence = clean(self, sentence)?;
        let translation = clean(self, translation)?;
        if sentence.is_empty() {
            return Err("An example needs a sentence.".into());
        }
        if let Some(seg) = source {
            self.require(seg, "segment")?;
        }
        let mut last = String::new();
        for id in self.proj.ordered_children("sense", sense, "example")? {
            last = last.max(self.proj.field_str(&id, "pos")?.unwrap_or_default());
        }
        let mut fields = vec![
            ("sense", Value::str(sense)),
            ("pos", Value::str(&crate::lexicon::after(&last))),
            ("sentence", Value::str(&sentence)),
        ];
        if !translation.is_empty() {
            fields.push(("translation", Value::str(&translation)));
        }
        if let Some(seg) = source {
            fields.push(("source", Value::str(seg)));
        }
        if let Some(r) = reference.map(str::trim).filter(|r| !r.is_empty()) {
            fields.push(("reference", Value::str(r)));
        }
        let mut t = self.intent("add-example");
        let id = t.create("example", "exm", SCOPE_LEXICON, fields);
        Ok((id, t.commit()?))
    }

    /// Retype an example's sentence or translation (a person's edit, so it
    /// settles a conflict on that field). The same text writes nothing; an
    /// empty sentence is refused (delete the example instead).
    pub fn set_example_text(&mut self, example: &str, field: &str, text: &str) -> Result<Option<CommitResult>, String> {
        self.require(example, "example")?;
        if !EXAMPLE_FIELDS.contains(&field) {
            return Err(format!("{field} is not a field of an example"));
        }
        let text = clean(self, text)?;
        if field == "sentence" && text.is_empty() {
            return Err("An example needs a sentence; delete it to remove it.".into());
        }
        if self.proj.field_str(example, field)?.unwrap_or_default() == text {
            return Ok(None);
        }
        let mut t = self.intent("edit-example").resolving();
        t.text_edit("example", example, SCOPE_LEXICON, field, &text)?;
        t.commit().map(Some)
    }

    pub fn delete_example(&mut self, example: &str) -> Result<CommitResult, String> {
        self.require(example, "example")?;
        let mut t = self.intent("delete-example");
        t.delete("example", example, SCOPE_LEXICON);
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use crate::Project;

    #[test]
    fn examples_are_added_in_order_edited_and_deleted() {
        let root = std::env::temp_dir().join(format!("wugs-examples-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        let ((_, sen), _) = p.add_lexeme("kani", "eat").unwrap();
        assert!(p.add_example(&sen, "  ", "x", None, None).is_err());
        let (a, _) = p.add_example(&sen, " Kani  ako. ", "I eat.", None, Some("Notebook 2, p. 14")).unwrap();
        let (b, _) = p.add_example(&sen, "Kani ka!", "", None, None).unwrap();
        let ex = p.examples(&sen).unwrap();
        assert_eq!(ex.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), [a.as_str(), b.as_str()]);
        assert_eq!((ex[0].sentence.as_str(), ex[0].translation.as_str(), ex[0].reference.as_deref()), ("Kani ako.", "I eat.", Some("Notebook 2, p. 14")));
        assert_eq!(ex[1].translation, "", "no translation yet");
        assert!(p.set_example_text(&b, "translation", "Eat!").unwrap().is_some());
        assert!(p.set_example_text(&b, "translation", "Eat!").unwrap().is_none(), "unchanged writes nothing");
        assert!(p.set_example_text(&b, "sentence", "").is_err());
        assert!(p.set_example_text(&b, "lemma", "x").is_err());
        assert_eq!(p.examples(&sen).unwrap()[1].translation, "Eat!");
        // An example from a sentence in a text keeps where it came from.
        let text = p.create_text("Story").unwrap();
        let (tier, _) = p.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
        let (seg, _) = p.add_segment(&text, &tier, 0, Some(0.0), Some(900.0), "kani ako").unwrap();
        let (c, _) = p.add_example(&sen, "kani ako", "I eat", Some(&seg), None).unwrap();
        assert_eq!(p.examples(&sen).unwrap()[2].source.as_deref(), Some(seg.as_str()));
        assert!(p.add_example(&sen, "x", "", Some(&sen), None).is_err(), "a source must be a segment");
        p.delete_example(&a).unwrap();
        assert_eq!(p.examples(&sen).unwrap().iter().map(|e| e.id.clone()).collect::<Vec<_>>(), [b, c]);
        // Search finds an example by its sentence and its translation.
        assert!(p.proj.search("ako", &["example"], 10).unwrap().iter().any(|h| h.field == "sentence"));
        std::fs::remove_dir_all(root).ok();
    }
}
