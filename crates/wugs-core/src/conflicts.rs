//! Settling a conflict (roadmap item 24, DATA-MODEL §5.3): a person picks one
//! of a field's open versions, or types a new value, and the write names
//! every open version, so the conflict closes on every device that receives
//! it. The losing values stay in the log.

use wugs_model::Value;

use crate::{CommitResult, Project};

/// The error `resolve_field` returns when the field changed under the person.
pub const STALE: &str = "This field changed again since it was shown (another device edited or settled it). The list has been refreshed: choose again.";

impl Project {
    /// Write `value` as the field's value, naming every open version (a
    /// resolving write, under intent `resolve-conflict`). A field whose
    /// winner is a text-edit is written as a text-edit, so its Loro lineage
    /// continues; any other field as a `set`. The op goes to the scope the
    /// field's current winner was written in.
    ///
    /// `shown` are the field's open versions (HLCs, `Projection::heads`) the
    /// person was looking at. If the field has moved since (another device
    /// settled it, or a new version arrived), nothing is written and the
    /// error says so: a write names the versions its writer was shown
    /// (DATA-MODEL §2.2), and a stale screen must not override a decision.
    pub fn resolve_field(&mut self, target: &str, field: &str, value: Value, shown: &[String]) -> Result<CommitResult, String> {
        let kind = self.proj.entity_kind(target)?.ok_or_else(|| format!("no such entity: {target}"))?;
        let mut now = self.proj.heads(target, field)?;
        let mut was = shown.to_vec();
        now.sort();
        was.sort();
        if now != was {
            return Err(STALE.to_string());
        }
        let (_, winner_op, verb) = self
            .proj
            .field_with_verb(target, field)?
            .ok_or_else(|| format!("{target} has no {field}"))?;
        let scope = self.proj.op_scope(&winner_op)?.ok_or_else(|| format!("no scope known for {winner_op}"))?;
        let mut t = self.intent("resolve-conflict").resolving();
        if verb == "text-edit" {
            let text = value.as_str().ok_or("a text field takes text")?.to_string();
            t.text_edit(&kind, target, &scope, field, &text)?;
        } else {
            t.set(&kind, target, &scope, field, value)?;
        }
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Op;

    fn pair() -> ((std::path::PathBuf, Project), (std::path::PathBuf, Project)) {
        let open = |actor: &str| {
            let root = std::env::temp_dir().join(format!("wugs-resolve-test-{}", wugs_model::ulid()));
            let p = Project::open(&root, actor).unwrap();
            (root, p)
        };
        (open("prs_Ana"), open("prs_Bo"))
    }

    fn theirs(p: &Project) -> Vec<Op> {
        p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == p.device()).collect()
    }

    #[test]
    fn picking_a_version_closes_the_conflict_on_both_devices() {
        let ((ra, mut a), (rb, mut b)) = pair();
        let ((_, sen), _) = a.add_lexeme("ev", "house").unwrap();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        a.set_sense_gloss(&sen, "home", None).unwrap();
        b.set_sense_gloss(&sen, "dwelling", None).unwrap();
        a.ingest(theirs(&b)).unwrap();
        b.ingest(theirs(&a)).unwrap();
        let open = a.proj.open_conflicts().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(a.proj.open_conflict_count().unwrap(), 1);
        let c = &open[0];
        assert_eq!((c.kind.as_str(), c.field.as_str(), c.text), ("sense", "gloss", false));
        let mut values: Vec<Value> = c.others.iter().map(|v| v.value.clone()).chain([c.winner.value.clone()]).collect();
        values.sort_by_key(|v| v.to_json());
        assert_eq!(values, [Value::str("dwelling"), Value::str("home")]);

        // Ana keeps Bo's version; once Bo has her write, his side closes too.
        let shown = a.proj.heads(&sen, "gloss").unwrap();
        a.resolve_field(&sen, "gloss", Value::str("dwelling"), &shown).unwrap();
        assert!(a.proj.open_conflicts().unwrap().is_empty());
        b.ingest(theirs(&a)).unwrap();
        assert!(b.proj.open_conflicts().unwrap().is_empty());
        assert_eq!(b.proj.field_str(&sen, "gloss").unwrap().as_deref(), Some("dwelling"));
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn merged_text_shows_the_merged_value_and_a_deleted_entry_leaves_the_queue() {
        let open_m = |actor: &str| {
            let root = std::env::temp_dir().join(format!("wugs-resolve-test-{}", wugs_model::ulid()));
            let engines = crate::Engines { merger: Some(Box::new(Concat)), ..Default::default() };
            (root.clone(), Project::open_with(&root, actor, engines).unwrap())
        };
        let ((ra, mut a), (rb, mut b)) = (open_m("prs_Ana"), open_m("prs_Bo"));
        let ((lex, sen), _) = a.add_lexeme("ev", "house").unwrap();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        a.set_sense_text(&sen, "definition", "A").unwrap();
        b.set_sense_text(&sen, "definition", "B").unwrap();
        // A plain set on a third path: it conflicts with the merged pair.
        let mut t = b.intent("bulk");
        t.set("sense", &sen, "lexicon", "definition", Value::str("C")).unwrap();
        t.commit().unwrap();
        a.ingest(theirs(&b)).unwrap();
        let shown_now = a.proj.field_str(&sen, "definition").unwrap().unwrap();
        for c in a.proj.open_conflicts().unwrap() {
            assert_eq!(c.winner.value, Value::str(&shown_now), "the winner's value is what the field shows");
        }
        a.delete_lexeme(&lex).unwrap();
        assert!(a.proj.open_conflicts().unwrap().is_empty());
        assert_eq!(a.proj.open_conflict_count().unwrap(), 0);
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    /// Merges by joining every update's text in order: a function of the set.
    struct Concat;
    impl wugs_model::TextMerger for Concat {
        fn resolve(&self, updates: &[Vec<u8>]) -> Result<String, String> {
            let mut t: Vec<String> = updates.iter().map(|u| String::from_utf8_lossy(u).into_owned()).collect();
            t.sort();
            Ok(t.join(" "))
        }
        fn edit(&self, _: &[Vec<u8>], _: Option<(&str, u64)>, new: &str, _: u64) -> Result<(Vec<u8>, String), String> {
            Ok((new.as_bytes().to_vec(), new.to_string()))
        }
    }

    #[test]
    fn a_text_field_is_resolved_as_text() {
        let ((ra, mut a), (rb, mut b)) = pair();
        let txt = a.create_text("T").unwrap();
        let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "start").unwrap().0;
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        // No merger on either side: text-edits take the LWW path and conflict.
        a.edit_content(&txt, &seg, "alpha").unwrap();
        b.edit_content(&txt, &seg, "beta").unwrap();
        a.ingest(theirs(&b)).unwrap();
        let open = a.proj.open_conflicts().unwrap();
        assert_eq!(open.len(), 1);
        assert!(open[0].text);
        let shown = a.proj.heads(&seg, "content").unwrap();
        assert!(a.resolve_field(&seg, "content", Value::str("x"), &shown[..1]).unwrap_err().contains("changed again since"), "a stale screen is refused");
        let res = a.resolve_field(&seg, "content", Value::str("alpha beta"), &shown).unwrap();
        assert_eq!(res.ops, 1);
        assert_eq!(a.log.read_all().unwrap().ops.last().unwrap().verb, "text-edit");
        assert!(a.proj.open_conflicts().unwrap().is_empty());
        assert_eq!(a.proj.field_str(&seg, "content").unwrap().as_deref(), Some("alpha beta"));
        assert_eq!(a.log.read_all().unwrap().ops.last().unwrap().scope, txt, "written in the text's scope");
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }
}
