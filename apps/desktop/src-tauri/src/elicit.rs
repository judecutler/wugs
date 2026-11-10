//! The Elicit panel's views and edits (roadmap item 23, docs/design/ELICITATION.md):
//! work through a concept list (Swadesh, Leipzig-Jakarta) or the semantic
//! domains' elicitation questions, entering each answer as a lexicon entry.
//! Plain functions over the open session; the tauri commands only forward.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;
use wugs_core::{CommitResult, ConceptRow, Project};

use crate::lexicon::{homograph, require, str_field, SIL_SEMDOM_V4};
use crate::session::{project, Session};

/// Concept lists from Concepticon (CC BY 4.0, Max Planck Institute for
/// Evolutionary Anthropology), one JSON object per line: a header, then
/// one concept per line. See `kits/tongues/concepts/README.md`.
const BUNDLED: &[&str] = &[
    include_str!("../../../../kits/tongues/concepts/Swadesh-1955-100.jsonl"),
    include_str!("../../../../kits/tongues/concepts/Swadesh-1952-200.jsonl"),
    include_str!("../../../../kits/tongues/concepts/Tadmor-2009-100.jsonl"),
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledList {
    /// `concepticon:<list id>`, the list's permanent name.
    pub source: String,
    pub name: String,
    pub citation: String,
    pub count: usize,
    /// The project's list entity when this list is loaded.
    pub list_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSummary {
    pub id: String,
    pub name: String,
    pub citation: String,
    pub total: usize,
    /// Concepts with at least one live answer.
    pub answered: usize,
    /// Concepts marked as not applicable (and not answered).
    pub skipped: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub sense_id: String,
    pub lexeme_id: String,
    pub lemma: String,
    pub homograph: Option<u32>,
    pub gloss: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptView {
    pub id: String,
    pub number: u32,
    pub gloss: String,
    pub concepticon_gloss: String,
    pub skipped: bool,
    pub skip_note: String,
    pub answers: Vec<Answer>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListView {
    pub summary: ListSummary,
    pub concepts: Vec<ConceptView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptEdit {
    pub concept: ConceptView,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub q: String,
    pub words: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainQuestions {
    pub id: String,
    pub code: String,
    pub label: String,
    pub description: String,
    /// The SIL elicitation questions, when the domain came from the SIL list.
    pub questions: Vec<Question>,
    /// Senses already assigned to the domain.
    pub words: Vec<Answer>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEdit {
    pub domain: DomainQuestions,
    pub notes: Vec<String>,
}

fn lines(res: &CommitResult) -> Vec<String> {
    res.findings.iter().map(|f| format!("{}: {}", f.code, f.message)).collect()
}

struct Bundled {
    source: String,
    name: String,
    citation: String,
    rows: Vec<ConceptRow>,
}

fn parse_bundled(text: &str) -> Result<Bundled, String> {
    let mut it = text.lines().filter(|l| !l.trim().is_empty());
    let head = wugs_model::json::parse(it.next().ok_or("empty concept list")?).map_err(|e| format!("concept list header: {e:?}"))?;
    let get = |v: &wugs_model::Value, k: &str| v.get_str(k).map(str::to_string).ok_or_else(|| format!("concept list: no {k}"));
    let mut rows = Vec::new();
    for line in it {
        let v = wugs_model::json::parse(line).map_err(|e| format!("concept list line: {e:?}"))?;
        let number = v.get("number").and_then(|n| n.as_f64()).ok_or("concept list: no number")? as u32;
        rows.push(ConceptRow {
            key: get(&v, "id")?,
            number,
            gloss: get(&v, "gloss")?,
            concepticon: v.get_str("concepticon").unwrap_or_default().to_string(),
            concepticon_gloss: v.get_str("concepticonGloss").unwrap_or_default().to_string(),
        });
    }
    Ok(Bundled { source: format!("concepticon:{}", get(&head, "list")?), name: get(&head, "name")?, citation: get(&head, "citation")?, rows })
}

fn bundled() -> Result<Vec<Bundled>, String> {
    BUNDLED.iter().map(|t| parse_bundled(t)).collect()
}

fn answer(p: &Project, sense: &str) -> Result<Answer, String> {
    let lexeme = str_field(p, sense, "lexeme")?;
    Ok(Answer {
        sense_id: sense.to_string(),
        lemma: str_field(p, &lexeme, "lemma")?,
        homograph: homograph(p, &lexeme)?,
        gloss: str_field(p, sense, "gloss")?,
        lexeme_id: lexeme,
    })
}

/// Live answers only: a sense whose entry was deleted no longer answers.
fn answers(p: &Project, senses: impl IntoIterator<Item = String>) -> Result<Vec<Answer>, String> {
    let mut out = Vec::new();
    for s in senses {
        let lex = str_field(p, &s, "lexeme")?;
        if p.proj.entity_kind(&lex)?.as_deref() == Some("lexeme") && !p.proj.is_tombstoned(&lex)? {
            out.push(answer(p, &s)?);
        }
    }
    Ok(out)
}

fn concept_view(p: &Project, id: &str) -> Result<ConceptView, String> {
    require(p, id, "concept")?;
    let senses = p.concept_answers(id)?;
    Ok(ConceptView {
        id: id.to_string(),
        number: p.proj.field_num(id, "number")?.unwrap_or(0.0) as u32,
        gloss: str_field(p, id, "gloss")?,
        concepticon_gloss: str_field(p, id, "concepticon-gloss")?,
        skipped: matches!(p.proj.field(id, "skipped")?, Some((wugs_model::Value::Bool(true), _))),
        skip_note: str_field(p, id, "skip-note")?,
        answers: answers(p, senses)?,
    })
}

fn summary(p: &Project, id: &str, concepts: &[ConceptView]) -> Result<ListSummary, String> {
    Ok(ListSummary {
        id: id.to_string(),
        name: str_field(p, id, "name")?,
        citation: str_field(p, id, "citation")?,
        total: concepts.len(),
        answered: concepts.iter().filter(|c| !c.answers.is_empty()).count(),
        skipped: concepts.iter().filter(|c| c.answers.is_empty() && c.skipped).count(),
    })
}

fn concepts_of(p: &Project, list: &str) -> Result<Vec<ConceptView>, String> {
    let mut out = Vec::new();
    for c in p.proj.ordered_children("list", list, "concept")? {
        out.push(concept_view(p, &c)?);
    }
    Ok(out)
}

pub fn concept_lists_bundled(session: &mut Session) -> Result<Vec<BundledList>, String> {
    let p = project(session)?;
    let mut out = Vec::new();
    for b in bundled()? {
        let id = wugs_core::concept_list_id(&b.source);
        let loaded = p.proj.entity_kind(&id)?.is_some() && !p.proj.is_tombstoned(&id)?;
        out.push(BundledList { count: b.rows.len(), list_id: loaded.then_some(id), source: b.source, name: b.name, citation: b.citation });
    }
    Ok(out)
}

pub fn concept_list_load(session: &mut Session, source: &str) -> Result<ListSummary, String> {
    let b = bundled()?.into_iter().find(|b| b.source == source).ok_or_else(|| format!("no bundled list {source}"))?;
    let p = project(session)?;
    let r = p.import_concept_list(&b.source, &b.name, &b.citation, &b.rows)?;
    let concepts = concepts_of(p, &r.list)?;
    summary(p, &r.list, &concepts)
}

pub fn concept_lists(session: &mut Session) -> Result<Vec<ListSummary>, String> {
    let p = project(session)?;
    let mut out = Vec::new();
    for id in p.proj.entities_of_kind("concept-list")? {
        let concepts = concepts_of(p, &id)?;
        out.push(summary(p, &id, &concepts)?);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn concept_list_view(session: &mut Session, list_id: &str) -> Result<ListView, String> {
    let p = project(session)?;
    require(p, list_id, "concept-list")?;
    let concepts = concepts_of(p, list_id)?;
    Ok(ListView { summary: summary(p, list_id, &concepts)?, concepts })
}

fn concept_edit(p: &Project, concept: &str, res: &CommitResult) -> Result<ConceptEdit, String> {
    Ok(ConceptEdit { concept: concept_view(p, concept)?, notes: lines(res) })
}

pub fn elicit_answer(session: &mut Session, concept: &str, form: &str, gloss: &str) -> Result<ConceptEdit, String> {
    let p = project(session)?;
    let (_, res) = p.elicit(concept, form, gloss, None)?;
    concept_edit(p, concept, &res)
}

pub fn elicit_link(session: &mut Session, concept: &str, sense: &str) -> Result<ConceptEdit, String> {
    let p = project(session)?;
    let res = p.link_concept(sense, concept)?;
    concept_edit(p, concept, &res)
}

pub fn elicit_unlink(session: &mut Session, concept: &str, sense: &str) -> Result<ConceptEdit, String> {
    let p = project(session)?;
    let res = p.unlink_concept(sense, concept)?;
    concept_edit(p, concept, &res)
}

pub fn elicit_skip(session: &mut Session, concept: &str, skipped: bool, note: &str) -> Result<ConceptEdit, String> {
    let p = project(session)?;
    let res = p.set_concept_skipped(concept, skipped, note)?;
    concept_edit(p, concept, &res)
}

/// The SIL elicitation questions by domain GUID (upper case), parsed once.
fn sil_questions() -> &'static HashMap<String, Vec<Question>> {
    static MAP: OnceLock<HashMap<String, Vec<Question>>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        for line in SIL_SEMDOM_V4.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(v) = wugs_model::json::parse(line) else { continue };
            let Some(guid) = v.get_str("guid") else { continue };
            let qs = match v.get("questions") {
                Some(wugs_model::Value::Arr(items)) => items
                    .iter()
                    .filter_map(|pair| match pair {
                        wugs_model::Value::Arr(qw) => Some(Question {
                            q: qw.first().and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                            words: qw.get(1).and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                        }),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            map.insert(guid.to_ascii_uppercase(), qs);
        }
        map
    })
}

fn domain_view(p: &Project, id: &str) -> Result<DomainQuestions, String> {
    require(p, id, "domain")?;
    let origin = str_field(p, id, "origin")?;
    let questions = origin
        .strip_prefix(&format!("{}:", crate::lexicon::SIL_ORIGIN))
        .and_then(|guid| sil_questions().get(&guid.to_ascii_uppercase()))
        .map(|qs| qs.iter().map(|q| Question { q: q.q.clone(), words: q.words.clone() }).collect())
        .unwrap_or_default();
    let mut senses = Vec::new();
    for link in p.proj.children_by_ref("domain", id, "sense-domain")? {
        if let Some(s) = p.proj.field_str(&link, "sense")? {
            if p.proj.entity_kind(&s)?.as_deref() == Some("sense") && !p.proj.is_tombstoned(&s)? && !senses.contains(&s) {
                senses.push(s);
            }
        }
    }
    let mut words = answers(p, senses)?;
    words.sort_by_key(|a| (wugs_model::fold_key(&a.lemma), a.homograph));
    Ok(DomainQuestions {
        id: id.to_string(),
        code: str_field(p, id, "code")?,
        label: str_field(p, id, "label")?,
        description: str_field(p, id, "description")?,
        questions,
        words,
    })
}

pub fn domain_questions(session: &mut Session, domain: &str) -> Result<DomainQuestions, String> {
    domain_view(project(session)?, domain)
}

pub fn domain_collect(session: &mut Session, domain: &str, form: &str, gloss: &str) -> Result<DomainEdit, String> {
    let p = project(session)?;
    let (_, res) = p.collect_for_domain(domain, form, gloss, None)?;
    Ok(DomainEdit { domain: domain_view(p, domain)?, notes: lines(&res) })
}
