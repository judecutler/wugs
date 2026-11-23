//! How an entry is said: its pronunciation as written (usually IPA) and,
//! when there is one, where to hear it in a recording (FLEx's
//! Pronunciation with its media file, LIFT's `<pronunciation>`, MDF's
//! `\ph`). Each is a Pronunciation entity (DATA-MODEL §3: kind
//! `pronunciation`, lexicon scope) with `lexeme`, `pos`, `form` and an
//! optional `audio`: one object `{source, start, end}` (a recording and a
//! span in milliseconds), kept in one field so the recording and its span
//! always change together. An entry may have several (dialects, speakers,
//! careful and fast speech).

use wugs_model::Value;

use crate::{CommitResult, Project, SCOPE_LEXICON};

/// The longest pronunciation kept (characters).
pub const MAX_PRONUNCIATION: usize = 200;

/// Where a pronunciation is heard: a recording and a span in milliseconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub source: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pronunciation {
    pub id: String,
    pub form: String,
    pub audio: Option<Heard>,
}

fn clean(p: &Project, form: &str) -> Result<String, String> {
    let form = p.proj.nfc(form.split_whitespace().collect::<Vec<_>>().join(" ").as_str());
    if form.chars().count() > MAX_PRONUNCIATION {
        return Err(format!("A pronunciation is at most {MAX_PRONUNCIATION} characters."));
    }
    Ok(form)
}

fn heard_value(h: &Heard) -> Value {
    Value::obj(vec![("source", Value::str(&h.source)), ("start", Value::Num(h.start)), ("end", Value::Num(h.end))])
}

fn heard_of(v: &Value) -> Option<Heard> {
    Some(Heard { source: v.get_str("source")?.to_string(), start: v.get("start")?.as_f64()?, end: v.get("end")?.as_f64()? })
}

impl Project {
    /// An entry's live pronunciations in order.
    pub fn pronunciations(&self, lexeme: &str) -> Result<Vec<Pronunciation>, String> {
        let mut out = Vec::new();
        for id in self.proj.ordered_children("lexeme", lexeme, "pronunciation")? {
            let form = self.proj.field_str(&id, "form")?.unwrap_or_default();
            let audio = self.proj.field(&id, "audio")?.and_then(|(v, _)| heard_of(&v));
            out.push(Pronunciation { id, form, audio });
        }
        Ok(out)
    }

    /// The span must be a live recording's and run forward.
    fn check_heard(&self, h: &Heard) -> Result<(), String> {
        self.require(&h.source, "source")?;
        if !(h.start.is_finite() && h.end.is_finite() && h.start >= 0.0 && h.end > h.start) {
            return Err("A pronunciation's span must start before it ends.".into());
        }
        if h.end - h.start > 60_000.0 {
            return Err("A pronunciation's recording is at most a minute long.".into());
        }
        Ok(())
    }

    /// A new pronunciation after the entry's last. It needs a written form,
    /// a recording, or both.
    pub fn add_pronunciation(&mut self, lexeme: &str, form: &str, audio: Option<&Heard>) -> Result<(String, CommitResult), String> {
        self.require(lexeme, "lexeme")?;
        let form = clean(self, form)?;
        if form.is_empty() && audio.is_none() {
            return Err("A pronunciation needs a written form or a recording.".into());
        }
        if let Some(h) = audio {
            self.check_heard(h)?;
        }
        let mut last = String::new();
        for id in self.proj.ordered_children("lexeme", lexeme, "pronunciation")? {
            last = last.max(self.proj.field_str(&id, "pos")?.unwrap_or_default());
        }
        let mut fields = vec![("lexeme", Value::str(lexeme)), ("pos", Value::str(&crate::lexicon::after(&last)))];
        if !form.is_empty() {
            fields.push(("form", Value::str(&form)));
        }
        if let Some(h) = audio {
            fields.push(("audio", heard_value(h)));
        }
        let mut t = self.intent("add-pronunciation");
        let id = t.create("pronunciation", "prn", SCOPE_LEXICON, fields);
        Ok((id, t.commit()?))
    }

    /// Retype a pronunciation (a person's edit). The same form writes nothing.
    pub fn set_pronunciation_form(&mut self, pronunciation: &str, form: &str) -> Result<Option<CommitResult>, String> {
        self.require(pronunciation, "pronunciation")?;
        let form = clean(self, form)?;
        if self.proj.field_str(pronunciation, "form")?.unwrap_or_default() == form {
            return Ok(None);
        }
        let mut t = self.intent("edit-pronunciation").resolving();
        t.set("pronunciation", pronunciation, SCOPE_LEXICON, "form", Value::str(&form))?;
        t.commit().map(Some)
    }

    /// Hear a pronunciation in a span of a recording, or (None) in none.
    pub fn set_pronunciation_audio(&mut self, pronunciation: &str, audio: Option<&Heard>) -> Result<Option<CommitResult>, String> {
        self.require(pronunciation, "pronunciation")?;
        if let Some(h) = audio {
            self.check_heard(h)?;
        }
        let now = self.proj.field(pronunciation, "audio")?.and_then(|(v, _)| heard_of(&v));
        if now.as_ref() == audio {
            return Ok(None);
        }
        let mut t = self.intent("edit-pronunciation").resolving();
        t.set("pronunciation", pronunciation, SCOPE_LEXICON, "audio", audio.map(heard_value).unwrap_or(Value::Null))?;
        t.commit().map(Some)
    }

    pub fn delete_pronunciation(&mut self, pronunciation: &str) -> Result<CommitResult, String> {
        self.require(pronunciation, "pronunciation")?;
        let mut t = self.intent("delete-pronunciation");
        t.delete("pronunciation", pronunciation, SCOPE_LEXICON);
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::Heard;
    use crate::Project;

    #[test]
    fn pronunciations_are_written_heard_and_deleted() {
        let root = std::env::temp_dir().join(format!("wugs-prn-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        let ((lex, _), _) = p.add_lexeme("kani", "eat").unwrap();
        assert!(p.add_pronunciation(&lex, "  ", None).is_err(), "nothing to keep");
        let (a, _) = p.add_pronunciation(&lex, " ˈka  ni ", None).unwrap();
        let src = p.add_source("elicitation 1", None, "audio", None).unwrap();
        let heard = Heard { source: src.clone(), start: 1200.0, end: 1850.0 };
        assert!(p.add_pronunciation(&lex, "", Some(&Heard { end: 1000.0, ..heard.clone() })).is_err(), "a span runs forward");
        assert!(p.add_pronunciation(&lex, "", Some(&Heard { source: lex.clone(), ..heard.clone() })).is_err(), "only a recording");
        let (b, _) = p.add_pronunciation(&lex, "", Some(&heard)).unwrap();
        let got = p.pronunciations(&lex).unwrap();
        assert_eq!(got.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), [a.as_str(), b.as_str()]);
        assert_eq!((got[0].form.as_str(), got[0].audio.as_ref()), ("ˈka ni", None));
        assert_eq!(got[1].audio.as_ref(), Some(&heard));
        assert!(p.set_pronunciation_form(&b, "ˈkani").unwrap().is_some());
        assert!(p.set_pronunciation_form(&b, "ˈkani").unwrap().is_none(), "unchanged writes nothing");
        assert!(p.set_pronunciation_audio(&a, Some(&heard)).unwrap().is_some());
        assert!(p.set_pronunciation_audio(&a, Some(&heard)).unwrap().is_none());
        assert!(p.set_pronunciation_audio(&b, None).unwrap().is_some());
        let got = p.pronunciations(&lex).unwrap();
        assert_eq!((got[0].audio.as_ref(), got[1].audio.as_ref(), got[1].form.as_str()), (Some(&heard), None, "ˈkani"));
        p.delete_pronunciation(&a).unwrap();
        assert_eq!(p.pronunciations(&lex).unwrap().len(), 1);
        // Search finds an entry's pronunciation by its form.
        assert!(p.proj.search("ˈkani", &["pronunciation"], 10).unwrap().iter().any(|h| h.field == "form"));
        std::fs::remove_dir_all(root).ok();
    }
}
