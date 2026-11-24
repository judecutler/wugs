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
    include_str!("../../../../kits/tongues/concepts/Holman-2008-40.jsonl"),
    include_str!("../../../../kits/tongues/concepts/Starostin-1991-110.jsonl"),
    include_str!("../../../../kits/tongues/concepts/Dolgopolsky-1964-15.jsonl"),
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
    /// How the entry is said, heard in a recording here: its first
    /// pronunciation with a live recording.
    pub sound: Option<Sound>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sound {
    pub source_id: String,
    pub start_ms: f64,
    pub end_ms: f64,
}

fn sound_of(p: &Project, lexeme: &str) -> Result<Option<Sound>, String> {
    for pr in p.pronunciations(lexeme)? {
        if let Some(h) = pr.audio {
            if p.proj.entity_kind(&h.source)?.is_some() && !p.proj.is_tombstoned(&h.source)? {
                return Ok(Some(Sound { source_id: h.source, start_ms: h.start, end_ms: h.end }));
            }
        }
    }
    Ok(None)
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
        sound: sound_of(p, &lexeme)?,
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

/// The most concepts one imported list may hold.
const MAX_OWN_CONCEPTS: usize = 5000;

/// A table of concepts as a word list: the concept in a column named
/// gloss, english, concept, meaning, parameter, name, word or prompt (else
/// the first column that is not numbers), its number from number/no/nr/#/id
/// (else its row), its Concepticon concept set from a Concepticon_ID
/// column (digits only) and Concepticon_Gloss when present. Each row's key
/// is its number column's value, else its gloss, so loading the same table
/// again (or an edited copy) adds only rows it did not have.
pub(crate) fn own_list_rows(header: &[String], rows: &[Vec<String>]) -> Result<Vec<ConceptRow>, String> {
    let named = |names: &[&str]| header.iter().position(|h| names.contains(&h.trim().to_lowercase().replace(['_', '-'], " ").as_str()));
    let numeric = |i: usize| rows.iter().all(|r| r[i].trim().is_empty() || r[i].trim().trim_end_matches('.').parse::<f64>().is_ok());
    let concepticon = named(&["concepticon id", "concepticon"]);
    let concepticon_gloss = named(&["concepticon gloss"]);
    let number = named(&["number", "no", "no.", "nr", "#", "id", "index", "num"]).filter(|i| Some(*i) != concepticon);
    let gloss = named(&["gloss", "english", "concept", "meaning", "parameter", "name", "word", "prompt", "english gloss", "gloss en"])
        .or_else(|| (0..header.len()).find(|i| Some(*i) != number && Some(*i) != concepticon && Some(*i) != concepticon_gloss && !numeric(*i)))
        .ok_or("no column holds the concepts: name one gloss, english or concept")?;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (i, r) in rows.iter().enumerate() {
        let g = r[gloss].trim();
        if g.is_empty() {
            continue;
        }
        let raw_number = number.map(|n| r[n].trim().trim_end_matches('.').to_string()).unwrap_or_default();
        let n = raw_number.parse::<f64>().ok().filter(|n| *n >= 0.0 && n.fract() == 0.0).map(|n| n as u32).unwrap_or(i as u32 + 1);
        let mut key = if raw_number.is_empty() { g.to_lowercase() } else { raw_number.clone() };
        if !seen.insert(key.clone()) {
            let mut k = 2;
            while !seen.insert(format!("{key}-{k}")) {
                k += 1;
            }
            key = format!("{key}-{k}");
        }
        let cid = concepticon.map(|c| r[c].trim().to_string()).filter(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit())).unwrap_or_default();
        let cgloss = if cid.is_empty() { String::new() } else { concepticon_gloss.map(|c| r[c].trim().to_string()).unwrap_or_default() };
        out.push(ConceptRow { key, number: n, gloss: g.to_string(), concepticon: cid, concepticon_gloss: cgloss });
        if out.len() > MAX_OWN_CONCEPTS {
            return Err(format!("a word list holds at most {MAX_OWN_CONCEPTS} concepts"));
        }
    }
    if out.is_empty() {
        return Err("the table has no concepts: every row's concept cell is empty".into());
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnList {
    pub summary: ListSummary,
    pub created: usize,
    pub already: usize,
    pub with_concepticon: usize,
}

/// Load a word list of one's own from a CSV or tab-separated table; the
/// list is named for the file.
pub fn concept_list_import(session: &mut Session, path: &std::path::Path) -> Result<OwnList, String> {
    if std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len() > 5_000_000 {
        return Err("a word list file is at most 5 MB".into());
    }
    let (text, tabs) = crate::table_text::read_table(path, &["tsv", "tab", "txt"])?;
    let (header, rows, _) = crate::table_text::records(&text, tabs)?;
    let concepts = own_list_rows(&header, &rows)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "My word list".into());
    let p = project(session)?;
    let name = p.proj.nfc(&name);
    let source = format!("user:{}", name.to_lowercase());
    let with_concepticon = concepts.iter().filter(|c| !c.concepticon.is_empty()).count();
    let r = p.import_concept_list(&source, &name, "Loaded from a table", &concepts)?;
    let listed = concepts_of(p, &r.list)?;
    Ok(OwnList { summary: summary(p, &r.list, &listed)?, created: r.created, already: r.already, with_concepticon })
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

/// Give `lexeme` the sound of a timed segment (the answer heard there), as
/// a pronunciation. The answer stands either way: a segment that cannot be
/// heard is a note, not a failure.
fn take_sound(p: &mut Project, lexeme: &str, segment: Option<&str>, notes: &mut Vec<String>) -> Result<(), String> {
    let Some(seg) = segment else { return Ok(()) };
    match crate::lexicon::heard_in(p, seg) {
        Ok((heard, _)) => {
            if sound_of(p, lexeme)?.is_some_and(|s| s.source_id == heard.source && s.start_ms == heard.start && s.end_ms == heard.end) {
                return Ok(());
            }
            let (_, res) = p.add_pronunciation(lexeme, "", Some(&heard))?;
            notes.extend(lines(&res));
        }
        Err(e) => notes.push(format!("The answer is saved without a sound: {e}")),
    }
    Ok(())
}

/// A new entry answering the concept; with `segment`, the entry is also
/// heard in that segment of a recording (the word as it was said in the session).
pub fn elicit_answer(session: &mut Session, concept: &str, form: &str, gloss: &str, segment: Option<&str>) -> Result<ConceptEdit, String> {
    let p = project(session)?;
    let ((lexeme, _), res) = p.elicit(concept, form, gloss, None)?;
    let mut notes = lines(&res);
    take_sound(p, &lexeme, segment, &mut notes)?;
    Ok(ConceptEdit { concept: concept_view(p, concept)?, notes })
}

pub fn elicit_link(session: &mut Session, concept: &str, sense: &str, segment: Option<&str>) -> Result<ConceptEdit, String> {
    let p = project(session)?;
    let res = p.link_concept(sense, concept)?;
    let mut notes = lines(&res);
    let lexeme = str_field(p, sense, "lexeme")?;
    take_sound(p, &lexeme, segment, &mut notes)?;
    Ok(ConceptEdit { concept: concept_view(p, concept)?, notes })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_list_reads_whole() {
        let lists = bundled().unwrap();
        let sizes: Vec<(String, usize)> = lists.iter().map(|b| (b.source.clone(), b.rows.len())).collect();
        assert_eq!(
            sizes,
            [("concepticon:Swadesh-1955-100", 100), ("concepticon:Swadesh-1952-200", 200), ("concepticon:Tadmor-2009-100", 100), ("concepticon:Holman-2008-40", 40), ("concepticon:Starostin-1991-110", 110), ("concepticon:Dolgopolsky-1964-15", 15)]
                .map(|(s, n)| (s.to_string(), n))
        );
        for b in &lists {
            assert!(b.rows.iter().all(|r| !r.gloss.is_empty() && !r.concepticon.is_empty()), "{}", b.source);
        }
    }

    #[test]
    fn domains_count_the_words_collected_in_them() {
        let dir = std::env::temp_dir().join(format!("wugs-domain-count-{}", wugs_model::ulid()));
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir.join("project"), "tester").unwrap();
        crate::lexicon::domains_load_sil(&mut session).unwrap();
        let all = crate::lexicon::domains_list(&mut session).unwrap();
        assert!(all.iter().all(|d| d.words == 0));
        let sun = all.iter().find(|d| d.code == "1.1.1").unwrap().id.clone();
        domain_collect(&mut session, &sun, "sol", "sun").unwrap();
        domain_collect(&mut session, &sun, "lumo", "light").unwrap();
        let all = crate::lexicon::domains_list(&mut session).unwrap();
        assert_eq!(all.iter().find(|d| d.id == sun).unwrap().words, 2);
        assert_eq!(all.iter().filter(|d| d.words > 0).count(), 1, "a parent does not count its subdomains' words");
        let lumo = crate::lexicon::lexicon_list(&mut session, "lumo").unwrap()[0].id.clone();
        crate::lexicon::lexicon_delete(&mut session, &lumo).unwrap();
        let all = crate::lexicon::domains_list(&mut session).unwrap();
        assert_eq!(all.iter().find(|d| d.id == sun).unwrap().words, 1, "a deleted entry's sense no longer counts");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_table_of_ones_own_becomes_a_word_list_once() {
        let header: Vec<String> = ["No.", "English", "Concepticon_ID", "Concepticon_Gloss", "Notes"].iter().map(|s| s.to_string()).collect();
        let rows: Vec<Vec<String>> = [["1", "head", "1256", "HEAD", ""], ["2", "hair (of the head)", "1040", "HAIR", "ask twice"], ["3", "", "", "", ""], ["4", "rice paddy", "x", "", ""]]
            .iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect();
        let got = own_list_rows(&header, &rows).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!((got[0].key.as_str(), got[0].number, got[0].gloss.as_str(), got[0].concepticon.as_str()), ("1", 1, "head", "1256"));
        assert_eq!((got[2].number, got[2].concepticon.as_str(), got[2].concepticon_gloss.as_str()), (4, "", ""), "not digits: no Concepticon id");
        // No named column: the first one that is not numbers; repeats get their own keys.
        let bare: Vec<Vec<String>> = [["7", "water"], ["", "water"]].iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect();
        let got = own_list_rows(&["n".into(), "Kanu prompt".into()], &bare).unwrap();
        assert_eq!(got.iter().map(|c| c.gloss.as_str()).collect::<Vec<_>>(), vec!["water", "water"]);
        assert!(own_list_rows(&["n".into()], &[vec!["1".into()]]).is_err());

        let dir = std::env::temp_dir().join(format!("wugs-own-list-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Kanu body parts.csv");
        std::fs::write(&file, "No.,English,Concepticon_ID\n1,head,1256\n2,hair,1040\n").unwrap();
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir.join("project"), "tester").unwrap();
        let first = concept_list_import(&mut session, &file).unwrap();
        assert_eq!((first.summary.name.as_str(), first.summary.total, first.created, first.with_concepticon), ("Kanu body parts", 2, 2, 2));
        std::fs::write(&file, "No.,English,Concepticon_ID\n1,head,1256\n2,hair,1040\n3,eye,1248\n").unwrap();
        let again = concept_list_import(&mut session, &file).unwrap();
        assert_eq!((again.summary.id.clone(), again.created, again.already, again.summary.total), (first.summary.id.clone(), 1, 2, 3), "the same list, one new row");
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
