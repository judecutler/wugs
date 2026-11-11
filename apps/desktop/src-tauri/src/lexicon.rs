//! The Lexicon panel's view of entries and its edits (roadmap item 22).
//! Plain functions over the open session; the tauri commands only forward.
//! Every edit returns the entry's fresh view, so the panel redraws from one
//! answer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wugs_core::paradigms::{Dimension, InflectedForm};
use wugs_core::{CommitResult, DomainRow, Project};
use wugs_model::fold_key;

use crate::session::{project, Session};

/// SIL Semantic Domains v4, English (© SIL Global, CC BY-SA 4.0), one JSON
/// object per line: code, guid, name, description, questions. Built from the
/// sillsdev/semdom repository; see `kits/tongues/semdom/README.md`. Kept as
/// separately licensed data, never mixed into MPL code.
pub(crate) const SIL_SEMDOM_V4: &str = include_str!("../../../../kits/tongues/semdom/semdom-v4-en.jsonl");
/// The `origin` prefix the SIL list's domains carry (`semdom:v4:<GUID>`).
pub const SIL_ORIGIN: &str = "semdom:v4";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryRow {
    pub id: String,
    pub lemma: String,
    pub homograph: Option<u32>,
    pub pos_of_speech: String,
    /// The primary glosses of the entry's senses, in order.
    pub glosses: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainRef {
    pub id: String,
    pub code: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenseView {
    pub id: String,
    pub gloss: String,
    /// `gloss:<tag>` fields, by tag.
    pub glosses_by_lang: BTreeMap<String, String>,
    pub definition: String,
    pub usage: String,
    pub domains: Vec<DomainRef>,
    /// Morphs in texts linked to this sense.
    pub occurrences: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: String,
    pub lemma: String,
    pub citation: String,
    pub pos_of_speech: String,
    pub homograph: Option<u32>,
    pub senses: Vec<SenseView>,
    /// Other live entries with exactly this lemma: (id, homograph).
    pub same_lemma: Vec<(String, Option<u32>)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryEdit {
    /// None after the entry was deleted.
    pub entry: Option<EntryView>,
    /// Validator findings of the commit (warnings, never blocks).
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainItem {
    pub id: String,
    pub code: String,
    pub label: String,
    pub description: String,
    /// 0 for a top-level domain.
    pub depth: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainsLoaded {
    pub created: usize,
    pub already: usize,
    pub notes: Vec<String>,
}

fn lines(res: &CommitResult) -> Vec<String> {
    res.findings.iter().map(|f| format!("{}: {}", f.code, f.message)).collect()
}

pub(crate) fn require(p: &Project, id: &str, kind: &str) -> Result<(), String> {
    if p.proj.entity_kind(id)?.as_deref() == Some(kind) && !p.proj.is_tombstoned(id)? {
        Ok(())
    } else {
        Err(format!("no such {kind}: {id}"))
    }
}

pub(crate) fn str_field(p: &Project, id: &str, field: &str) -> Result<String, String> {
    Ok(p.proj.field_str(id, field)?.unwrap_or_default())
}

pub(crate) fn homograph(p: &Project, id: &str) -> Result<Option<u32>, String> {
    Ok(p.proj.field_num(id, "homograph")?.filter(|n| *n >= 1.0).map(|n| n as u32))
}

fn senses(p: &Project, lexeme: &str) -> Result<Vec<String>, String> {
    p.proj.ordered_children("lexeme", lexeme, "sense")
}

/// Every live entry, sorted by folded lemma then homograph; `query` keeps
/// those whose lemma or a gloss contains it (case- and NFC-insensitive).
pub fn lexicon_list(session: &mut Session, query: &str) -> Result<Vec<EntryRow>, String> {
    let p = project(session)?;
    let q = fold_key(&p.proj.nfc(query.trim()));
    let mut rows = Vec::new();
    for id in p.proj.entities_of_kind("lexeme")? {
        let lemma = str_field(p, &id, "lemma")?;
        let mut glosses = Vec::new();
        for s in senses(p, &id)? {
            glosses.push(str_field(p, &s, "gloss")?);
        }
        let hit = q.is_empty()
            || fold_key(&p.proj.nfc(&lemma)).contains(&q)
            || glosses.iter().any(|g| fold_key(&p.proj.nfc(g)).contains(&q));
        if hit {
            rows.push(EntryRow { homograph: homograph(p, &id)?, pos_of_speech: str_field(p, &id, "pos-of-speech")?, id, lemma, glosses });
        }
    }
    rows.sort_by_cached_key(|r| (fold_key(&r.lemma), r.homograph, r.id.clone()));
    Ok(rows)
}

fn domain_ref(p: &Project, id: &str) -> Result<DomainRef, String> {
    Ok(DomainRef { id: id.to_string(), code: str_field(p, id, "code")?, label: str_field(p, id, "label")? })
}

fn entry_view(p: &Project, id: &str) -> Result<EntryView, String> {
    require(p, id, "lexeme")?;
    let lemma = str_field(p, id, "lemma")?;
    let mut sense_views = Vec::new();
    for s in senses(p, id)? {
        let mut glosses_by_lang = BTreeMap::new();
        for (field, value) in p.proj.fields_of(&s)? {
            if let (Some(tag), Some(v)) = (field.strip_prefix("gloss:"), value.as_str()) {
                if !v.is_empty() {
                    glosses_by_lang.insert(tag.to_string(), v.to_string());
                }
            }
        }
        let mut domains = Vec::new();
        for (_, dom) in p.sense_domains(&s)? {
            domains.push(domain_ref(p, &dom)?);
        }
        domains.sort_by(|a, b| code_order(&a.code).cmp(&code_order(&b.code)).then(a.label.cmp(&b.label)));
        domains.dedup_by(|a, b| a.id == b.id);
        sense_views.push(SenseView {
            gloss: str_field(p, &s, "gloss")?,
            glosses_by_lang,
            definition: str_field(p, &s, "definition")?,
            usage: str_field(p, &s, "usage")?,
            domains,
            occurrences: occurrences(p, &s)?,
            id: s,
        });
    }
    let mut same_lemma = Vec::new();
    let key = wugs_model::lexicon_key(&p.proj.nfc(&lemma));
    for other in p.proj.by_lookup_key("lexeme", &key)? {
        if other != id && p.proj.nfc(&str_field(p, &other, "lemma")?) == p.proj.nfc(&lemma) && !p.proj.is_tombstoned(&other)? {
            same_lemma.push((other.clone(), homograph(p, &other)?));
        }
    }
    same_lemma.sort();
    Ok(EntryView {
        id: id.to_string(),
        citation: str_field(p, id, "citation")?,
        pos_of_speech: str_field(p, id, "pos-of-speech")?,
        homograph: homograph(p, id)?,
        senses: sense_views,
        same_lemma,
        lemma,
    })
}

/// Morphs linked to a sense that a reader can see: shown in their word's
/// analysis, the word shown in its segment, the segment and its text live.
/// Hidden or orphaned morphs (a re-analysis, a deleted segment) do not count.
fn occurrences(p: &Project, sense: &str) -> Result<usize, String> {
    let mut n = 0;
    // A frequent sense shares segments and tokens: read each list once.
    let mut members: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut shown = |parent: &str, child: &str| -> Result<bool, String> {
        if !members.contains_key(parent) {
            members.insert(parent.to_string(), p.proj.members(parent)?);
        }
        Ok(members[parent].iter().any(|m| m == child))
    };
    for morph in p.proj.children_by_ref("sense", sense, "morph")? {
        let Some(token) = p.proj.field_str(&morph, "token")? else { continue };
        let Some(segment) = p.proj.field_str(&token, "segment")? else { continue };
        let Some(text) = p.proj.field_str(&segment, "text")? else { continue };
        let live = |id: &str| -> Result<bool, String> { Ok(p.proj.entity_kind(id)?.is_some() && !p.proj.is_tombstoned(id)?) };
        if live(&token)? && live(&segment)? && live(&text)? && shown(&token, &morph)? && shown(&segment, &token)? {
            n += 1;
        }
    }
    Ok(n)
}

/// Outline numbers sort part by part (1.2 before 1.10); anything else last.
fn code_order(code: &str) -> Vec<u64> {
    code.split('.').map(|part| part.parse().unwrap_or(u64::MAX)).collect()
}

fn edit(p: &Project, id: &str, res: &CommitResult) -> Result<EntryEdit, String> {
    Ok(EntryEdit { entry: Some(entry_view(p, id)?), notes: lines(res) })
}

pub fn lexicon_entry(session: &mut Session, id: &str) -> Result<EntryView, String> {
    entry_view(project(session)?, id)
}

pub fn lexicon_add(session: &mut Session, lemma: &str, gloss: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let ((lex, _), res) = p.add_lexeme(lemma, gloss)?;
    edit(p, &lex, &res)
}

pub fn lexicon_set_field(session: &mut Session, id: &str, field: &str, value: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let res = p.set_lexeme_field(id, field, value)?;
    edit(p, id, &res)
}

pub fn lexicon_set_homograph(session: &mut Session, id: &str, number: Option<u32>) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let res = p.set_homograph(id, number)?;
    edit(p, id, &res)
}

/// Number this entry and every other spelled the same as homographs.
pub fn lexicon_number_homographs(session: &mut Session, id: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let res = p.number_homographs(id)?;
    edit(p, id, &res)
}

pub fn lexicon_delete(session: &mut Session, id: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let res = p.delete_lexeme(id)?;
    Ok(EntryEdit { entry: None, notes: lines(&res) })
}

/// The live lexeme a sense belongs to.
fn owner(p: &Project, sense: &str) -> Result<String, String> {
    require(p, sense, "sense")?;
    p.proj.field_str(sense, "lexeme")?.ok_or_else(|| format!("sense {sense} has no entry"))
}

pub fn lexicon_add_sense(session: &mut Session, id: &str, gloss: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let (_, res) = p.add_sense(id, gloss)?;
    edit(p, id, &res)
}

pub fn lexicon_set_sense_gloss(session: &mut Session, sense: &str, gloss: &str, lang: Option<&str>) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let lex = owner(p, sense)?;
    let res = p.set_sense_gloss(sense, gloss, lang)?;
    edit(p, &lex, &res)
}

pub fn lexicon_set_sense_text(session: &mut Session, sense: &str, field: &str, text: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let lex = owner(p, sense)?;
    let res = p.set_sense_text(sense, field, text)?;
    edit(p, &lex, &res)
}

pub fn lexicon_delete_sense(session: &mut Session, sense: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let lex = owner(p, sense)?;
    let res = p.delete_sense(sense)?;
    edit(p, &lex, &res)
}

pub fn lexicon_assign_domain(session: &mut Session, sense: &str, domain: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let lex = owner(p, sense)?;
    let res = p.assign_domain(sense, domain)?;
    edit(p, &lex, &res)
}

pub fn lexicon_unassign_domain(session: &mut Session, sense: &str, domain: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let lex = owner(p, sense)?;
    let res = p.unassign_domain(sense, domain)?;
    edit(p, &lex, &res)
}

/// Every live domain in tree order (each parent before its children,
/// siblings by `pos`), with its depth. A domain whose parent is missing or
/// deleted shows at the top level.
pub fn domains_list(session: &mut Session) -> Result<Vec<DomainItem>, String> {
    let p = project(session)?;
    let ids = p.proj.entities_of_kind("domain")?;
    let live: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut children: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for id in &ids {
        let parent = p.proj.field_str(id, "parent")?.filter(|x| live.contains(x.as_str())).unwrap_or_default();
        let pos = str_field(p, id, "pos")?;
        children.entry(parent).or_default().push((pos, id.clone()));
    }
    for list in children.values_mut() {
        list.sort();
    }
    let mut out = Vec::with_capacity(ids.len());
    // Iterative walk; a parent cycle (validator #6 holds those) cannot loop
    // here because each id is emitted at most once.
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<(String, usize)> = children.get("").into_iter().flatten().rev().map(|(_, id)| (id.clone(), 0)).collect();
    while let Some((id, depth)) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        out.push(DomainItem {
            code: str_field(p, &id, "code")?,
            label: str_field(p, &id, "label")?,
            description: str_field(p, &id, "description")?,
            depth,
            id: id.clone(),
        });
        for (_, child) in children.get(&id).into_iter().flatten().rev() {
            stack.push((child.clone(), depth + 1));
        }
    }
    Ok(out)
}

/// The bundled SIL list as rows, parents first (the file is in outline order).
pub fn sil_domain_rows() -> Result<Vec<DomainRow>, String> {
    let mut rows = Vec::new();
    for (i, line) in SIL_SEMDOM_V4.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let v = wugs_model::json::parse(line).map_err(|e| format!("semantic domains line {}: {e:?}", i + 1))?;
        let get = |k: &str| v.get_str(k).map(str::to_string).ok_or_else(|| format!("semantic domains line {}: no {k}", i + 1));
        rows.push(DomainRow { code: get("code")?, guid: get("guid")?, label: get("name")?, description: get("description")? });
    }
    Ok(rows)
}

/// "Load SIL Semantic Domains": every domain of the bundled list the project
/// does not hold yet, into the project config, in one txn.
pub fn domains_load_sil(session: &mut Session) -> Result<DomainsLoaded, String> {
    let rows = sil_domain_rows()?;
    let p = project(session)?;
    let r = p.import_semantic_domains(SIL_ORIGIN, &rows)?;
    Ok(DomainsLoaded {
        created: r.created,
        already: r.already,
        notes: r.findings.iter().map(|f| format!("{}: {}", f.code, f.message)).collect(),
    })
}

// ---------------------------------------------------------------- paradigms (roadmap 50)

/// One axis of the grid the panel shows: a feature and its values in order.
/// The panel's templates (`src/lexicon/paradigms.ts`) decide them.
#[derive(Debug, Clone, Deserialize)]
pub struct Axis {
    pub feature: String,
    pub values: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormView {
    pub id: String,
    pub form: String,
    pub features: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellView {
    pub features: BTreeMap<String, String>,
    /// Usually one; two devices filling one cell before a sync leave two.
    pub forms: Vec<FormView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParadigmView {
    pub lexeme: String,
    /// Every combination of the axes' values, the first axis outermost.
    pub cells: Vec<CellView>,
    /// The entry's inflected forms that fit no cell of this grid.
    pub others: Vec<FormView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParadigmEdit {
    pub paradigm: ParadigmView,
    pub notes: Vec<String>,
}

fn form_view(f: InflectedForm) -> FormView {
    FormView { id: f.id, form: f.form, features: f.features }
}

fn paradigm_of(p: &Project, lexeme: &str, axes: &[Axis]) -> Result<ParadigmView, String> {
    let dims: Vec<Dimension> = axes.iter().map(|a| Dimension { feature: a.feature.clone(), values: a.values.clone() }).collect();
    let grid = p.paradigm(lexeme, &dims)?;
    Ok(ParadigmView {
        lexeme: lexeme.to_string(),
        cells: grid
            .cells
            .into_iter()
            .map(|c| CellView { features: c.features, forms: c.forms.into_iter().map(form_view).collect() })
            .collect(),
        others: grid.others.into_iter().map(form_view).collect(),
    })
}

/// The entry's live inflected form `form`, or an error naming what is wrong.
fn form_of(p: &Project, lexeme: &str, form: &str) -> Result<(), String> {
    require(p, form, "form")?;
    if p.proj.field_str(form, "lexeme")?.as_deref() != Some(lexeme) {
        return Err(format!("form {form} is not a form of {lexeme}"));
    }
    Ok(())
}

pub fn lexicon_paradigm(session: &mut Session, id: &str, axes: &[Axis]) -> Result<ParadigmView, String> {
    paradigm_of(project(session)?, id, axes)
}

/// Type into a cell: a new inflected form with the cell's features when
/// `form` is None, else a new surface for that form.
pub fn lexicon_fill_cell(
    session: &mut Session,
    id: &str,
    form: Option<&str>,
    features: &BTreeMap<String, String>,
    surface: &str,
    axes: &[Axis],
) -> Result<ParadigmEdit, String> {
    let p = project(session)?;
    let res = match form {
        Some(form) => {
            form_of(p, id, form)?;
            p.set_form_surface(form, surface)?
        }
        None => {
            let features: Vec<(&str, &str)> = features.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            p.add_inflected_form(id, surface, &features)?.1
        }
    };
    Ok(ParadigmEdit { paradigm: paradigm_of(p, id, axes)?, notes: lines(&res) })
}

pub fn lexicon_delete_form(session: &mut Session, id: &str, form: &str, axes: &[Axis]) -> Result<ParadigmEdit, String> {
    let p = project(session)?;
    form_of(p, id, form)?;
    let res = p.delete_form(form)?;
    Ok(ParadigmEdit { paradigm: paradigm_of(p, id, axes)?, notes: lines(&res) })
}
