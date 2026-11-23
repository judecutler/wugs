//! Lexical relations: one entry or sense pointing at another as its
//! synonym, antonym, a word to compare, its whole or its part (FLEx's
//! Lexical Relations, LIFT's `<relation>`, MDF's `\sy` / `\an` / `\cf`).
//! Each is a LexicalRelation entity (DATA-MODEL §3: kind
//! `lexical-relation`, lexicon scope) with `from`, `to` (a lexeme or a
//! sense) and `relation` (its type). Symmetric types read the same from
//! both ends; a directed one (`part`, `whole`) reads as its converse from
//! the other end, so the relation is written once.

use wugs_model::Value;

use crate::{CommitResult, Project, SCOPE_LEXICON};

/// The types wugs names in words. Any other type (one a LIFT file brings,
/// such as FLEx's `Calendar`) is kept and shown as written.
pub const RELATION_TYPES: [&str; 5] = ["synonym", "antonym", "compare", "part", "whole"];

/// The type read from the other end: symmetric types are their own converse.
pub fn converse(relation: &str) -> &str {
    match relation {
        "part" => "whole",
        "whole" => "part",
        other => other,
    }
}

/// Types that read the same from both ends. Another file's type is taken as
/// directed, since wugs cannot know (FLEx's `Calendar` names a sequence).
pub fn symmetric(relation: &str) -> bool {
    matches!(relation, "synonym" | "antonym" | "compare")
}

/// A relation as seen from one entry: its type read from this end, and the
/// entry or sense at the other end.
#[derive(Clone, Debug, PartialEq)]
pub struct Related {
    pub id: String,
    pub relation: String,
    /// The entry or sense of this entry the relation is attached to.
    pub here: String,
    /// The entry or sense at the other end.
    pub other: String,
}

fn clean_type(t: &str) -> Result<String, String> {
    let t = t.trim();
    if t.is_empty() || t.chars().count() > 60 || t.chars().any(char::is_control) {
        return Err("A relation needs a short type, such as synonym or antonym.".into());
    }
    Ok(t.to_string())
}

impl Project {
    /// The lexeme an entry-or-sense belongs to (itself for a lexeme), when
    /// it and its entry are live.
    fn live_entry_of(&self, id: &str) -> Result<Option<String>, String> {
        let live = |x: &str| -> Result<bool, String> { Ok(self.proj.entity_kind(x)?.is_some() && !self.proj.is_tombstoned(x)?) };
        if !live(id)? {
            return Ok(None);
        }
        match self.proj.entity_kind(id)?.as_deref() {
            Some("lexeme") => Ok(Some(id.to_string())),
            Some("sense") => match self.proj.field_str(id, "lexeme")? {
                Some(lex) if live(&lex)? => Ok(Some(lex)),
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    /// Every live relation touching an entry or one of its senses, read
    /// from this entry's side, oldest first. A relation whose other end was
    /// deleted is left out.
    pub fn relations(&self, lexeme: &str) -> Result<Vec<Related>, String> {
        let mut ends = vec![lexeme.to_string()];
        ends.extend(self.proj.ordered_children("lexeme", lexeme, "sense")?);
        let mut out = Vec::new();
        for here in &ends {
            for (field, other_field) in [("from", "to"), ("to", "from")] {
                let mut ids = self.proj.children_by_ref(field, here, "lexical-relation")?;
                ids.sort();
                for id in ids {
                    let Some(other) = self.proj.field_str(&id, other_field)? else { continue };
                    let relation = self.proj.field_str(&id, "relation")?.unwrap_or_default();
                    if self.live_entry_of(&other)?.is_none() {
                        continue;
                    }
                    let relation = if field == "from" { relation } else { converse(&relation).to_string() };
                    out.push(Related { id, relation, here: here.clone(), other });
                }
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out.dedup_by(|a, b| a.id == b.id);
        Ok(out)
    }

    /// Relate an entry or sense to another. The same relation twice (either
    /// way round for a symmetric type) and an entry related to itself are
    /// refused.
    pub fn add_relation(&mut self, from: &str, relation: &str, to: &str) -> Result<(String, CommitResult), String> {
        let relation = clean_type(relation)?;
        let (Some(a), Some(b)) = (self.live_entry_of(from)?, self.live_entry_of(to)?) else {
            return Err("A relation joins two live entries or senses.".into());
        };
        if a == b {
            return Err("An entry cannot be related to itself.".into());
        }
        for r in self.relations(&a)? {
            if r.here == from && r.other == to && r.relation == relation {
                return Err(format!("These two are already related as {relation}."));
            }
        }
        let mut t = self.intent("add-relation");
        let id = t.create(
            "lexical-relation",
            "rel",
            SCOPE_LEXICON,
            vec![("from", Value::str(from)), ("to", Value::str(to)), ("relation", Value::str(&relation))],
        );
        Ok((id, t.commit()?))
    }

    /// Is there a live relation of this type from `from` to `to` (or, for a
    /// symmetric type, either way round)?
    pub fn relations_between(&self, from: &str, to: &str, relation: &str, symmetric: bool) -> Result<bool, String> {
        for (a, b) in [(from, to), (to, from)] {
            for id in self.proj.children_by_ref("from", a, "lexical-relation")? {
                if self.proj.is_tombstoned(&id)? || self.proj.field_str(&id, "relation")?.as_deref() != Some(relation) {
                    continue;
                }
                if self.proj.field_str(&id, "to")?.as_deref() == Some(b) {
                    return Ok(true);
                }
            }
            if !symmetric {
                break;
            }
        }
        Ok(false)
    }

    pub fn delete_relation(&mut self, relation: &str) -> Result<CommitResult, String> {
        self.require(relation, "lexical-relation")?;
        let mut t = self.intent("delete-relation");
        t.delete("lexical-relation", relation, SCOPE_LEXICON);
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use crate::Project;

    #[test]
    fn relations_read_from_both_ends() {
        let root = std::env::temp_dir().join(format!("wugs-rel-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        let ((big, _), _) = p.add_lexeme("dakila", "big").unwrap();
        let ((large, _), _) = p.add_lexeme("laki", "large").unwrap();
        let ((small, small_sense), _) = p.add_lexeme("liit", "small").unwrap();
        let ((hand, _), _) = p.add_lexeme("kamay", "hand").unwrap();
        let ((finger, _), _) = p.add_lexeme("daliri", "finger").unwrap();
        let (syn, _) = p.add_relation(&big, "synonym", &large).unwrap();
        assert!(p.add_relation(&big, "synonym", &large).is_err(), "twice");
        assert!(p.add_relation(&big, " ", &large).is_err(), "no type");
        assert!(p.add_relation(&big, "synonym", &big).is_err(), "itself");
        p.add_relation(&big, "antonym", &small_sense).unwrap();
        p.add_relation(&finger, "part", &hand).unwrap();
        let from_big: Vec<(String, String)> = p.relations(&big).unwrap().into_iter().map(|r| (r.relation, r.other)).collect();
        assert_eq!(from_big, [("synonym".to_string(), large.clone()), ("antonym".to_string(), small_sense.clone())]);
        let from_large = p.relations(&large).unwrap();
        assert_eq!((from_large[0].relation.as_str(), from_large[0].other.as_str()), ("synonym", big.as_str()), "symmetric from the other end");
        let from_small = p.relations(&small).unwrap();
        assert_eq!((from_small[0].relation.as_str(), from_small[0].here.as_str()), ("antonym", small_sense.as_str()), "a sense's relation shows on its entry");
        assert_eq!(p.relations(&hand).unwrap()[0].relation, "whole", "a directed type reads as its converse");
        assert_eq!(p.relations(&finger).unwrap()[0].relation, "part");
        // Deleting either end hides the relation; deleting the relation removes it.
        p.delete_lexeme(&small).unwrap();
        assert_eq!(p.relations(&big).unwrap().len(), 1);
        p.delete_relation(&syn).unwrap();
        assert!(p.relations(&big).unwrap().is_empty() && p.relations(&large).unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }
}
