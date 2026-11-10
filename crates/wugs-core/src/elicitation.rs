//! Elicitation tracking (roadmap item 23, `docs/design/ELICITATION.md`):
//! concept lists to work through (Swadesh, Leipzig-Jakarta), which concepts
//! the lexicon already answers, and quick entry that grows the lexicon and
//! records the answer in one txn.
//!
//! A list and its concepts live in the project config (they are the plan,
//! shared by the team). Which sense answers which concept is a link entity
//! in the lexicon scope (`sense-concept`), like `sense-domain`, so coverage is
//! derived: a concept is covered while a live sense links to it. Ids derive
//! from the published list's own keys, so loading a list twice, or on two
//! devices, gives one list.

use wugs_model::Value;

use crate::lexicon::id_from_key;
use crate::{CommitResult, Project, SCOPE_CONFIG, SCOPE_LEXICON};

/// One concept of a published list (Concepticon conceptlist rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptRow {
    /// The row's permanent id in its list, e.g. `Swadesh-1955-100-1`.
    pub key: String,
    pub number: u32,
    /// The prompt shown to the linguist, in the list's language (English).
    pub gloss: String,
    /// Concepticon concept set id and gloss, when mapped ("" otherwise).
    pub concepticon: String,
    pub concepticon_gloss: String,
}

#[derive(Debug, Default)]
pub struct ImportedConcepts {
    pub list: String,
    pub created: usize,
    pub already: usize,
}

/// The id a published list gets in every project.
pub fn concept_list_id(source: &str) -> String {
    id_from_key("cpl", source)
}

/// The id a published list's concept gets in every project: from the
/// list's source and the row's key together, so lists whose row keys are
/// only unique within the list (a user's own "1", "2", …) never collide.
pub fn concept_id(source: &str, row_key: &str) -> String {
    id_from_key("cpt", &format!("{source}\u{0}{row_key}"))
}

impl Project {
    /// Load a concept list into the project config, one txn. `source` names
    /// the list for good (`concepticon:Swadesh-1955-100`). Concepts the
    /// project already holds, live or deleted, are left as they are.
    pub fn import_concept_list(&mut self, source: &str, name: &str, citation: &str, rows: &[ConceptRow]) -> Result<ImportedConcepts, String> {
        let list = concept_list_id(source);
        let mut out = ImportedConcepts { list: list.clone(), ..Default::default() };
        let mut t = self.intent("load-concept-list");
        if t.project.proj.entity_kind(&list)?.is_none() {
            t.push_create(
                "concept-list",
                &list,
                SCOPE_CONFIG,
                vec![
                    ("name", Value::str(name)),
                    ("source", Value::str(source)),
                    ("citation", Value::str(citation)),
                    ("origin", Value::str(source)),
                ],
            );
        }
        let mut seen = std::collections::HashSet::new();
        for row in rows {
            if !seen.insert(row.key.as_str()) {
                return Err(format!("concept {} appears twice", row.key));
            }
            let id = concept_id(source, &row.key);
            if t.project.proj.entity_kind(&id)?.is_some() {
                out.already += 1;
                continue;
            }
            let mut fields = vec![
                ("list", Value::str(&list)),
                ("pos", Value::str(&wugs_model::pos_seq(row.number as usize))),
                ("number", Value::Num(f64::from(row.number))),
                ("gloss", Value::str(&row.gloss)),
                ("origin", Value::str(&row.key)),
            ];
            if !row.concepticon.is_empty() {
                fields.push(("concepticon", Value::str(&row.concepticon)));
                fields.push(("concepticon-gloss", Value::str(&row.concepticon_gloss)));
            }
            t.push_create("concept", &id, SCOPE_CONFIG, fields);
            out.created += 1;
        }
        t.commit()?;
        Ok(out)
    }

    /// Every live link from a concept to a live sense of a live entry, as
    /// (link, sense). Two devices can each link the same sense, so a sense
    /// can appear twice; `concept_answers` lists each sense once.
    pub fn concept_senses(&self, concept: &str) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for link in self.proj.children_by_ref("concept", concept, "sense-concept")? {
            let Some(sense) = self.proj.field_str(&link, "sense")? else { continue };
            if self.proj.entity_kind(&sense)?.as_deref() != Some("sense") || self.proj.is_tombstoned(&sense)? {
                continue;
            }
            let Some(lexeme) = self.proj.field_str(&sense, "lexeme")? else { continue };
            if self.proj.entity_kind(&lexeme)?.as_deref() == Some("lexeme") && !self.proj.is_tombstoned(&lexeme)? {
                out.push((link, sense));
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        Ok(out)
    }

    /// The senses answering a concept, each once: the concept is answered
    /// while this is not empty.
    pub fn concept_answers(&self, concept: &str) -> Result<Vec<String>, String> {
        let mut senses: Vec<String> = self.concept_senses(concept)?.into_iter().map(|(_, s)| s).collect();
        senses.dedup();
        Ok(senses)
    }

    /// Record that a sense answers a concept. Nothing is written when the
    /// link exists.
    pub fn link_concept(&mut self, sense: &str, concept: &str) -> Result<CommitResult, String> {
        self.require(sense, "sense")?;
        self.require(concept, "concept")?;
        let mut t = self.intent("link-concept");
        if !t.project.concept_senses(concept)?.iter().any(|(_, s)| s == sense) {
            t.create("sense-concept", "scn", SCOPE_LEXICON, vec![("sense", Value::str(sense)), ("concept", Value::str(concept))]);
        }
        t.commit()
    }

    /// Undo `link_concept`: every live link between the two is tombstoned.
    pub fn unlink_concept(&mut self, sense: &str, concept: &str) -> Result<CommitResult, String> {
        self.require(concept, "concept")?;
        let links: Vec<String> =
            self.concept_senses(concept)?.into_iter().filter(|(_, s)| s == sense).map(|(l, _)| l).collect();
        let mut t = self.intent("unlink-concept");
        for link in links {
            t.delete("sense-concept", &link, SCOPE_LEXICON);
        }
        t.commit()
    }

    /// The answer to a concept as a new entry: a lexeme with one sense glossed
    /// `gloss` (the concept's own gloss when empty), linked to the concept, in
    /// one txn of the lexicon scope. `session` (an elicitation session id)
    /// rides on every op as provenance.
    pub fn elicit(&mut self, concept: &str, form: &str, gloss: &str, session: Option<&str>) -> Result<((String, String), CommitResult), String> {
        self.require(concept, "concept")?;
        let form = form.trim();
        if form.is_empty() {
            return Err("type the word you heard".into());
        }
        let gloss = match gloss.trim() {
            "" => self.proj.field_str(concept, "gloss")?.unwrap_or_default(),
            g => g.to_string(),
        };
        let mut t = self.intent("elicit");
        if let Some(s) = session {
            t = t.with_session(s);
        }
        let lex = t.create("lexeme", "lex", SCOPE_LEXICON, vec![("lemma", Value::str(form))]);
        let sen = t.create(
            "sense",
            "sen",
            SCOPE_LEXICON,
            vec![("lexeme", Value::str(&lex)), ("gloss", Value::str(&gloss)), ("pos", Value::str(&wugs_model::pos_seq(0)))],
        );
        t.create("sense-concept", "scn", SCOPE_LEXICON, vec![("sense", Value::str(&sen)), ("concept", Value::str(concept))]);
        let res = t.commit()?;
        Ok(((lex, sen), res))
    }

    /// Mark a concept as not applicable to this language (or clear the mark),
    /// with an optional note, so coverage counts it as done.
    pub fn set_concept_skipped(&mut self, concept: &str, skipped: bool, note: &str) -> Result<CommitResult, String> {
        self.require(concept, "concept")?;
        let mut t = self.intent("skip-concept").resolving();
        t.set("concept", concept, SCOPE_CONFIG, "skipped", Value::Bool(skipped))?;
        t.set("concept", concept, SCOPE_CONFIG, "skip-note", Value::str(note.trim()))?;
        t.commit()
    }

    /// A word for a semantic domain (rapid word collection): a new entry whose
    /// one sense is assigned to `domain`, in one txn of the lexicon scope.
    pub fn collect_for_domain(&mut self, domain: &str, form: &str, gloss: &str, session: Option<&str>) -> Result<((String, String), CommitResult), String> {
        self.require(domain, "domain")?;
        let form = form.trim();
        if form.is_empty() {
            return Err("type the word you heard".into());
        }
        let mut t = self.intent("collect-word");
        if let Some(s) = session {
            t = t.with_session(s);
        }
        let lex = t.create("lexeme", "lex", SCOPE_LEXICON, vec![("lemma", Value::str(form))]);
        let sen = t.create(
            "sense",
            "sen",
            SCOPE_LEXICON,
            vec![("lexeme", Value::str(&lex)), ("gloss", Value::str(gloss.trim())), ("pos", Value::str(&wugs_model::pos_seq(0)))],
        );
        t.create("sense-domain", "sdm", SCOPE_LEXICON, vec![("sense", Value::str(&sen)), ("domain", Value::str(domain))]);
        let res = t.commit()?;
        Ok(((lex, sen), res))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(actor: &str) -> (std::path::PathBuf, Project) {
        let root = std::env::temp_dir().join(format!("wugs-elicit-test-{}", wugs_model::ulid()));
        let p = Project::open(&root, actor).unwrap();
        (root, p)
    }

    const SWADESH: &str = "concepticon:Swadesh-1955-100";

    fn rows() -> Vec<ConceptRow> {
        let row = |n: u32, gloss: &str, cid: &str, cg: &str| ConceptRow {
            key: format!("Swadesh-1955-100-{n}"),
            number: n,
            gloss: gloss.into(),
            concepticon: cid.into(),
            concepticon_gloss: cg.into(),
        };
        vec![row(1, "all", "98", "ALL"), row(2, "ashes", "646", "ASH"), row(3, "bark", "1204", "BARK")]
    }

    #[test]
    fn a_list_loads_once_with_stable_ids() {
        let (root, mut p) = project("prs_Ana");
        let r = p.import_concept_list("concepticon:Swadesh-1955-100", "Swadesh 100", "Swadesh 1955", &rows()).unwrap();
        assert_eq!((r.created, r.already), (3, 0));
        assert!(wugs_model::looks_like_id(&r.list), "{}", r.list);
        assert_eq!(r.list, concept_list_id("concepticon:Swadesh-1955-100"));
        let c1 = concept_id(SWADESH, "Swadesh-1955-100-1");
        assert_eq!(p.proj.field_str(&c1, "gloss").unwrap().as_deref(), Some("all"));
        assert_eq!(p.proj.ordered_children("list", &r.list, "concept").unwrap().len(), 3);
        let again = p.import_concept_list("concepticon:Swadesh-1955-100", "Swadesh 100", "Swadesh 1955", &rows()).unwrap();
        assert_eq!((again.created, again.already), (0, 3));
        assert_eq!(p.proj.entities_of_kind("concept-list").unwrap().len(), 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn eliciting_grows_the_lexicon_and_covers_the_concept() {
        let (root, mut p) = project("prs_Ana");
        p.import_concept_list("concepticon:Swadesh-1955-100", "Swadesh 100", "", &rows()).unwrap();
        let ash = concept_id(SWADESH, "Swadesh-1955-100-2");
        let ((lex, sen), _) = p.elicit(&ash, " kül ", "", None).unwrap();
        assert_eq!(p.proj.field_str(&lex, "lemma").unwrap().as_deref(), Some("kül"));
        assert_eq!(p.proj.field_str(&sen, "gloss").unwrap().as_deref(), Some("ashes"), "the concept's gloss by default");
        assert_eq!(p.concept_senses(&ash).unwrap().len(), 1);
        p.delete_lexeme(&lex).unwrap();
        assert!(p.concept_senses(&ash).unwrap().is_empty(), "a deleted answer no longer covers");
        assert!(p.elicit(&ash, "  ", "", None).is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn an_existing_sense_can_answer_and_a_concept_can_be_skipped() {
        let (root, mut p) = project("prs_Ana");
        p.import_concept_list("concepticon:Swadesh-1955-100", "Swadesh 100", "", &rows()).unwrap();
        let bark = concept_id(SWADESH, "Swadesh-1955-100-3");
        let ((_, sen), _) = p.add_lexeme("kabuk", "bark").unwrap();
        p.link_concept(&sen, &bark).unwrap();
        assert_eq!(p.link_concept(&sen, &bark).unwrap().ops, 0);
        p.unlink_concept(&sen, &bark).unwrap();
        assert!(p.concept_senses(&bark).unwrap().is_empty());
        p.set_concept_skipped(&bark, true, "no word for tree bark").unwrap();
        assert_eq!(p.proj.field(&bark, "skipped").unwrap().map(|(v, _)| v), Some(Value::Bool(true)));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_word_for_a_domain_is_one_entry_assigned_to_it() {
        let (root, mut p) = project("prs_Ana");
        let rows = vec![crate::DomainRow { code: "1".into(), guid: "63403699-07C1-43F3-A47C-069D6E4316E5".into(), label: "Universe".into(), description: String::new() }];
        p.import_semantic_domains("semdom:v4", &rows).unwrap();
        let dom = crate::domain_id("63403699-07C1-43F3-A47C-069D6E4316E5").unwrap();
        let ((_, sen), res) = p.collect_for_domain(&dom, "evren", "universe", None).unwrap();
        assert_eq!(res.ops, 3, "lexeme, sense and the domain link in one txn");
        assert_eq!(p.sense_domains(&sen).unwrap().len(), 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_sense_two_devices_both_linked_answers_once_and_unlinks_fully() {
        let (ra, mut a) = project("prs_Ana");
        let (rb, mut b) = project("prs_Bo");
        a.import_concept_list(SWADESH, "Swadesh 100", "", &rows()).unwrap();
        let ((_, sen), _) = a.add_lexeme("kül", "ashes").unwrap();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        let ash = concept_id(SWADESH, "Swadesh-1955-100-2");
        a.link_concept(&sen, &ash).unwrap();
        b.link_concept(&sen, &ash).unwrap();
        let theirs: Vec<_> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect();
        a.ingest(theirs).unwrap();
        assert_eq!(a.concept_senses(&ash).unwrap().len(), 2, "two links");
        assert_eq!(a.concept_answers(&ash).unwrap(), [sen.clone()], "one answer");
        a.unlink_concept(&sen, &ash).unwrap();
        assert!(a.concept_answers(&ash).unwrap().is_empty());
        assert_ne!(concept_id("mine", "1"), concept_id("theirs", "1"), "row keys are scoped by list");
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn two_devices_loading_one_list_agree_on_its_ids() {
        let (ra, mut a) = project("prs_Ana");
        let (rb, mut b) = project("prs_Bo");
        a.import_concept_list("concepticon:Swadesh-1955-100", "Swadesh 100", "", &rows()).unwrap();
        b.import_concept_list("concepticon:Swadesh-1955-100", "Swadesh 100", "", &rows()).unwrap();
        let theirs: Vec<_> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect();
        a.ingest(theirs).unwrap();
        assert_eq!(a.proj.entities_of_kind("concept").unwrap().len(), 3);
        assert_eq!(a.proj.entities_of_kind("concept-list").unwrap().len(), 1);
        assert_eq!(a.proj.count("conflicts").unwrap(), 0, "equal twins are not a conflict");
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }
}
