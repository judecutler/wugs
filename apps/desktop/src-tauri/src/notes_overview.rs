//! Every note in the project in one list: the notes on sentences (the Gloss
//! panel's Notes) and the notes on lexicon entries and senses. Field
//! linguists keep their questions for the next session in these ("ask M. if
//! this is the plural", "check the tone"); the Notes panel gathers them so
//! they can be worked through, copied or printed.

use std::collections::HashSet;

use serde::Serialize;
use wugs_model::fold_key;

use crate::concordance::{each_segment, Read, Scope};
use crate::lexicon::{homograph, str_field};
use crate::session::{project, Session};

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SentenceNote {
    pub note_id: String,
    pub body: String,
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
    pub content: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntryNote {
    pub entry_id: String,
    pub headword: String,
    pub homograph: Option<u32>,
    /// The sense's number in the entry (from 1), or None for the entry's own note.
    pub sense: Option<usize>,
    /// The sense's gloss, or the first sense's for an entry note.
    pub gloss: String,
    pub body: String,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotesView {
    /// In text order: texts by title, sentences as the tools read them.
    pub sentences: Vec<SentenceNote>,
    /// Entries by folded headword, the entry's own note before its senses'.
    pub entries: Vec<EntryNote>,
}

pub fn all_notes(session: &mut Session) -> Result<NotesView, String> {
    let p = project(session)?;
    let mut out = NotesView::default();

    // Sentence notes: the targets first, so a project without notes reads no text.
    let mut targets = HashSet::new();
    for id in p.proj.entities_of_kind("annotation")? {
        if p.proj.field_str(&id, "kind")?.as_deref() == Some("note") {
            if let Some(t) = p.proj.field_str(&id, "target")? {
                targets.insert(t);
            }
        }
    }
    if !targets.is_empty() {
        let mut segs = Vec::new();
        each_segment(p, Read::Words, &Scope::all(), |seg| {
            if targets.contains(&seg.id) {
                segs.push((seg.text.clone(), seg.title.clone(), seg.tier.clone(), seg.id.clone()));
            }
            Ok(())
        })?;
        segs.sort_by_cached_key(|(text, title, _, _)| (fold_key(title), text.clone()));
        for (text, title, tier, seg) in segs {
            let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
            for (note, body) in p.segment_notes(&seg)? {
                if body.trim().is_empty() {
                    continue;
                }
                out.sentences.push(SentenceNote {
                    note_id: note,
                    body,
                    text_id: text.clone(),
                    text_title: title.clone(),
                    tier_id: tier.clone(),
                    segment_id: seg.clone(),
                    content: content.clone(),
                });
            }
        }
    }

    // Lexicon notes: an entry's own, then each sense's.
    let mut entries = Vec::new();
    for id in p.proj.entities_of_kind("lexeme")? {
        let headword = str_field(p, &id, "lemma")?;
        let senses = p.proj.ordered_children("lexeme", &id, "sense")?;
        let first_gloss = match senses.first() {
            Some(s) => str_field(p, s, "gloss")?,
            None => String::new(),
        };
        let hom = homograph(p, &id)?;
        let mut notes = Vec::new();
        let own = str_field(p, &id, "notes")?;
        if !own.trim().is_empty() {
            notes.push(EntryNote { entry_id: id.clone(), headword: headword.clone(), homograph: hom, sense: None, gloss: first_gloss, body: own });
        }
        for (i, s) in senses.iter().enumerate() {
            let body = str_field(p, s, "notes")?;
            if !body.trim().is_empty() {
                notes.push(EntryNote { entry_id: id.clone(), headword: headword.clone(), homograph: hom, sense: Some(i + 1), gloss: str_field(p, s, "gloss")?, body });
            }
        }
        if !notes.is_empty() {
            entries.push((fold_key(&headword), hom, id, notes));
        }
    }
    entries.sort_by(|a, b| (&a.0, a.1, &a.2).cmp(&(&b.0, b.1, &b.2)));
    out.entries = entries.into_iter().flat_map(|e| e.3).collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_on_sentences_entries_and_senses_are_gathered() {
        let dir = std::env::temp_dir().join(format!("wugs-notes-all-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        assert!(all_notes(&mut s).unwrap().sentences.is_empty());
        let p = project(&mut s).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let b = p.create_text("Bees").unwrap();
        let a = p.create_text("ants").unwrap();
        let (sb, _) = p.add_segment(&b, &tier, 0, None, None, "zum zum").unwrap();
        let (sa, _) = p.add_segment(&a, &tier, 0, None, None, "tik").unwrap();
        p.add_segment_note(&b, &sb, "Ask about the tone").unwrap();
        p.add_segment_note(&a, &sa, "  ").ok();
        p.add_segment_note(&a, &sa, "Plural?").unwrap();
        let ((zebra, _), _) = p.add_lexeme("zebra", "striped horse").unwrap();
        let ((apple, sense), _) = p.add_lexeme("apple", "fruit").unwrap();
        p.add_lexeme("cat", "feline").unwrap();
        p.set_lexeme_notes(&zebra, "Loan?").unwrap();
        p.set_sense_text(&sense, "notes", "Only the red kind").unwrap();
        p.set_lexeme_notes(&apple, "Check the vowel").unwrap();

        let r = all_notes(&mut s).unwrap();
        let bodies: Vec<_> = r.sentences.iter().map(|n| (n.text_title.as_str(), n.body.as_str(), n.content.as_str())).collect();
        assert_eq!(bodies, [("ants", "Plural?", "tik"), ("Bees", "Ask about the tone", "zum zum")]);
        let entries: Vec<_> = r.entries.iter().map(|n| (n.headword.as_str(), n.sense, n.body.as_str())).collect();
        assert_eq!(entries, [("apple", None, "Check the vowel"), ("apple", Some(1), "Only the red kind"), ("zebra", None, "Loan?")]);
        assert_eq!(r.entries[0].gloss, "fruit");
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
