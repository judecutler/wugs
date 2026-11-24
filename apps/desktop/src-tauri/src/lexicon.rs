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
    /// Some sense is linked from a morph of a live text (the list's "Not in
    /// any text" picks the entries where it is false).
    pub used: bool,
    /// The language a loanword came from ("" for none).
    pub borrowed_from: String,
    /// A pronunciation of it is heard in a live recording.
    pub heard: bool,
    /// Every sense has an example sentence (false with no senses).
    pub examples: bool,
    /// Every sense is in a semantic domain (false with no senses).
    pub domains: bool,
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
    /// The linguist's notes on this meaning.
    pub notes: String,
    pub domains: Vec<DomainRef>,
    /// Morphs in texts linked to this sense.
    pub occurrences: usize,
    /// Example sentences chosen for this sense, in order.
    pub examples: Vec<ExampleView>,
    /// A picture of what it means, for a picture dictionary.
    pub picture: Option<PictureView>,
}

/// A sense's picture: its blob, the name it was chosen under, and where the
/// file is in this project's blob store (absent when it is not on this computer).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureView {
    pub blob: String,
    pub name: String,
    pub path: Option<String>,
}

/// The kinds of picture a sense takes: what every browser, Word and Anki show.
pub const PICTURE_TYPES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];
/// A dictionary picture is a photo or drawing, not a scan of a book.
pub const PICTURE_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// The MIME type of a picture by its file name's extension.
pub(crate) fn picture_mime(name: &str) -> &'static str {
    match name.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

/// The file extension a picture is written under (its own, else `jpg`).
pub(crate) fn picture_ext(name: &str) -> String {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if PICTURE_TYPES.contains(&ext.as_str()) { ext } else { "jpg".into() }
}

fn picture_view(p: &Project, sense: &str) -> Result<Option<PictureView>, String> {
    Ok(p.sense_picture(sense)?.map(|(blob, name)| {
        let path = wugs_blob::parse_ref(&blob).map(|hex| p.root.join("blobs").join(hex)).filter(|f| f.is_file()).map(|f| f.display().to_string());
        PictureView { blob, name, path }
    }))
}

/// An example sentence of a sense (`wugs_core::examples`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleView {
    pub id: String,
    pub sentence: String,
    pub translation: String,
    pub reference: String,
    /// The sentence it was taken from, while that sentence is still there.
    pub from: Option<ExampleSource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleSource {
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
    pub start_ms: Option<f64>,
    pub end_ms: Option<f64>,
}

fn example_views(p: &Project, sense: &str) -> Result<Vec<ExampleView>, String> {
    let live = |id: &str| -> Result<bool, String> { Ok(p.proj.entity_kind(id)?.is_some() && !p.proj.is_tombstoned(id)?) };
    let mut out = Vec::new();
    for e in p.examples(sense)? {
        let mut from = None;
        if let Some(seg) = e.source.as_deref().filter(|s| live(s).unwrap_or(false)) {
            let text = p.proj.field_str(seg, "text")?.unwrap_or_default();
            if live(&text)? {
                from = Some(ExampleSource {
                    text_title: str_field(p, &text, "title")?,
                    tier_id: str_field(p, seg, "tier")?,
                    segment_id: seg.to_string(),
                    start_ms: p.proj.field_num(seg, "start")?,
                    end_ms: p.proj.field_num(seg, "end")?,
                    text_id: text,
                });
            }
        }
        out.push(ExampleView { id: e.id, sentence: e.sentence, translation: e.translation, reference: e.reference.unwrap_or_default(), from });
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: String,
    pub lemma: String,
    pub citation: String,
    pub pos_of_speech: String,
    pub homograph: Option<u32>,
    /// The linguist's notes on the whole entry.
    pub notes: String,
    /// Where the word comes from (collaborative text), and a loanword's source language.
    pub etymology: String,
    pub borrowed_from: String,
    pub senses: Vec<SenseView>,
    /// Other live entries with exactly this lemma: (id, homograph).
    pub same_lemma: Vec<(String, Option<u32>)>,
    /// Other spellings of the entry (allomorphs): (form id, surface).
    pub allomorphs: Vec<(String, String)>,
    /// How the entry is said, each with where to hear it.
    pub pronunciations: Vec<PronunciationView>,
    /// Synonyms, antonyms and the like, read from this entry's side.
    pub relations: Vec<RelationView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationView {
    pub id: String,
    /// Its type from this entry's side (`synonym`, `whole`, or as a file wrote it).
    pub relation: String,
    /// The sense of this entry it is attached to (1-based), when not the entry.
    pub here_sense: Option<usize>,
    pub other_id: String,
    pub other_lemma: String,
    pub other_homograph: Option<u32>,
    pub other_sense: Option<usize>,
}

/// Where a lexeme-or-sense sits: its entry, and its sense number when it is a sense.
fn entry_and_sense(p: &Project, id: &str) -> Result<(String, Option<usize>), String> {
    if p.proj.entity_kind(id)?.as_deref() == Some("sense") {
        let lex = str_field(p, id, "lexeme")?;
        let n = p.proj.ordered_children("lexeme", &lex, "sense")?.iter().position(|x| x == id).map(|i| i + 1);
        return Ok((lex, n));
    }
    Ok((id.to_string(), None))
}

fn relation_views(p: &Project, lexeme: &str) -> Result<Vec<RelationView>, String> {
    let mut out = Vec::new();
    for r in p.relations(lexeme)? {
        let (_, here_sense) = entry_and_sense(p, &r.here)?;
        let (other, other_sense) = entry_and_sense(p, &r.other)?;
        out.push(RelationView {
            id: r.id,
            relation: r.relation,
            here_sense,
            other_lemma: str_field(p, &other, "lemma")?,
            other_homograph: homograph(p, &other)?,
            other_id: other,
            other_sense,
        });
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PronunciationView {
    pub id: String,
    pub form: String,
    /// The recording and span it is heard in, while that recording is live.
    pub audio: Option<HeardView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeardView {
    pub source_id: String,
    pub label: String,
    pub start_ms: f64,
    pub end_ms: f64,
}

fn pronunciation_views(p: &Project, lexeme: &str) -> Result<Vec<PronunciationView>, String> {
    let mut out = Vec::new();
    for x in p.pronunciations(lexeme)? {
        let audio = match x.audio {
            Some(h) if p.proj.entity_kind(&h.source)?.as_deref() == Some("source") && !p.proj.is_tombstoned(&h.source)? => {
                Some(HeardView { label: str_field(p, &h.source, "label")?, source_id: h.source, start_ms: h.start, end_ms: h.end })
            }
            _ => None,
        };
        out.push(PronunciationView { id: x.id, form: x.form, audio });
    }
    Ok(out)
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
    /// Live senses of live entries assigned to it (not its subdomains').
    pub words: usize,
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
        let mut used = false;
        let mut examples = true;
        let mut domains = true;
        let sense_ids = senses(p, &id)?;
        for s in &sense_ids {
            glosses.push(str_field(p, s, "gloss")?);
            used = used || linked_in_a_text(p, s)?;
            examples = examples && !p.examples(s)?.is_empty();
            domains = domains && !p.sense_domains(s)?.is_empty();
        }
        let examples = examples && !sense_ids.is_empty();
        let domains = domains && !sense_ids.is_empty();
        let hit = q.is_empty()
            || fold_key(&p.proj.nfc(&lemma)).contains(&q)
            || glosses.iter().any(|g| fold_key(&p.proj.nfc(g)).contains(&q));
        if hit {
            let heard = p.pronunciations(&id)?.iter().filter_map(|pr| pr.audio.as_ref()).any(|a| p.proj.is_tombstoned(&a.source).map(|dead| !dead).unwrap_or(false));
            rows.push(EntryRow { homograph: homograph(p, &id)?, pos_of_speech: str_field(p, &id, "pos-of-speech")?, borrowed_from: str_field(p, &id, "borrowed-from")?, id, lemma, glosses, used, heard, examples, domains });
        }
    }
    rows.sort_by_cached_key(|r| (fold_key(&r.lemma), r.homograph, r.id.clone()));
    Ok(rows)
}

fn domain_ref(p: &Project, id: &str) -> Result<DomainRef, String> {
    Ok(DomainRef { id: id.to_string(), code: str_field(p, id, "code")?, label: str_field(p, id, "label")? })
}

pub(crate) fn entry_view(p: &Project, id: &str) -> Result<EntryView, String> {
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
            notes: str_field(p, &s, "notes")?,
            domains,
            occurrences: occurrences(p, &s)?,
            examples: example_views(p, &s)?,
            picture: picture_view(p, &s)?,
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
        notes: str_field(p, id, "notes")?,
        etymology: str_field(p, id, "etymology")?,
        borrowed_from: str_field(p, id, "borrowed-from")?,
        senses: sense_views,
        same_lemma,
        allomorphs: p.allomorphs(id)?,
        pronunciations: pronunciation_views(p, id)?,
        relations: relation_views(p, id)?,
        lemma,
    })
}

/// Morphs linked to a sense that a reader can see: shown in their word's
/// analysis, the word shown in its segment, the segment and its text live.
/// Hidden or orphaned morphs (a re-analysis, a deleted segment) do not count.
/// Whether a live morph of a live token, segment and text links this sense:
/// the list's cheap form of `occurrences` (it stops at the first and does not
/// ask whether a membership list hides the morph).
fn linked_in_a_text(p: &Project, sense: &str) -> Result<bool, String> {
    let live = |id: &str| -> Result<bool, String> { Ok(p.proj.entity_kind(id)?.is_some() && !p.proj.is_tombstoned(id)?) };
    for morph in p.proj.children_by_ref("sense", sense, "morph")? {
        let Some(token) = p.proj.field_str(&morph, "token")? else { continue };
        let Some(segment) = p.proj.field_str(&token, "segment")? else { continue };
        let Some(text) = p.proj.field_str(&segment, "text")? else { continue };
        if live(&token)? && live(&segment)? && live(&text)? {
            return Ok(true);
        }
    }
    Ok(false)
}

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

/// The most words one call makes entries of.
pub const MAX_WORDS: usize = 5000;

/// Each word not spelled as any entry or form yet made an entry with one
/// empty sense to gloss, in one txn (`import_lexicon_rows`); repeats (by
/// lookup key) once. Returns how many entries were made.
pub fn lexicon_add_words(session: &mut Session, words: &[String]) -> Result<usize, String> {
    if words.len() > MAX_WORDS {
        return Err(format!("At most {MAX_WORDS} words can be added at once; filter the list first."));
    }
    let p = project(session)?;
    let mut seen = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for w in words {
        let lemma = w.trim();
        let key = wugs_model::lexicon_key(lemma);
        if lemma.is_empty() || key.is_empty() || !seen.insert(key.clone()) || crate::consistency::known(p, &key)? {
            continue;
        }
        rows.push(wugs_core::LexiconRow {
            lemma: lemma.to_string(),
            homograph: None,
            category: String::new(),
            citation: String::new(),
            gloss: String::new(),
            glosses: Vec::new(),
            definition: String::new(),
            usage: String::new(),
            domains: Vec::new(),
            example: String::new(),
            example_translation: String::new(),
        });
    }
    if rows.is_empty() {
        return Ok(0);
    }
    Ok(p.import_lexicon_rows(&rows)?.0.entries)
}

pub fn lexicon_set_field(session: &mut Session, id: &str, field: &str, value: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    // An entry's notes and etymology are collaborative text, like a sense's definition.
    let res = if field == "notes" || field == "etymology" { p.set_lexeme_text(id, field, value)? } else { p.set_lexeme_field(id, field, value)? };
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

/// Another spelling of the entry (an allomorph), found by glossing lookups and guesses like the lemma.
pub fn lexicon_add_allomorph(session: &mut Session, id: &str, form: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    require(p, id, "lexeme")?;
    let (_, res) = p.add_allomorph(id, form)?;
    edit(p, id, &res)
}

/// Where a timed segment is heard: its text's recording and its span.
pub(crate) fn heard_in(p: &Project, segment: &str) -> Result<(wugs_core::pronunciations::Heard, String), String> {
    require(p, segment, "segment")?;
    let text = str_field(p, segment, "text")?;
    let (start, end) = match (p.proj.field_num(segment, "start")?, p.proj.field_num(segment, "end")?) {
        (Some(a), Some(b)) if b > a => (a, b),
        _ => return Err("This segment has no times, so there is nothing to hear in it.".into()),
    };
    let Some((source, _)) = p.text_recording(&text)? else {
        return Err("This segment's text has no recording yet: pick one for it on the Timeline.".into());
    };
    let content = str_field(p, segment, "content")?;
    Ok((wugs_core::pronunciations::Heard { source, start, end }, content))
}

/// A pronunciation of the entry: typed, heard in a timed segment, or both.
/// Taken from a segment with no form typed, a one-word transcription is its form.
pub fn lexicon_add_pronunciation(session: &mut Session, id: &str, form: &str, segment: Option<&str>) -> Result<EntryEdit, String> {
    let p = project(session)?;
    require(p, id, "lexeme")?;
    let heard = segment.map(|g| heard_in(p, g)).transpose()?;
    let form = match &heard {
        Some((_, content)) if form.trim().is_empty() && content.split_whitespace().count() == 1 => content.trim().to_string(),
        _ => form.to_string(),
    };
    let (_, res) = p.add_pronunciation(id, &form, heard.as_ref().map(|(h, _)| h))?;
    edit(p, id, &res)
}

fn own_pronunciation(p: &Project, id: &str, pronunciation: &str) -> Result<(), String> {
    if !p.pronunciations(id)?.iter().any(|x| x.id == pronunciation) {
        return Err("that pronunciation is not one of this entry's".into());
    }
    Ok(())
}

fn edit_maybe(p: &Project, id: &str, res: Option<CommitResult>) -> Result<EntryEdit, String> {
    match res {
        Some(res) => edit(p, id, &res),
        None => Ok(EntryEdit { entry: Some(entry_view(p, id)?), notes: Vec::new() }),
    }
}

/// Retype a pronunciation; emptied, it is deleted unless it has a recording.
pub fn lexicon_set_pronunciation(session: &mut Session, id: &str, pronunciation: &str, form: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    own_pronunciation(p, id, pronunciation)?;
    let heard = p.pronunciations(id)?.into_iter().find(|x| x.id == pronunciation).and_then(|x| x.audio);
    if form.trim().is_empty() && heard.is_none() {
        let res = p.delete_pronunciation(pronunciation)?;
        return edit(p, id, &res);
    }
    let res = p.set_pronunciation_form(pronunciation, form)?;
    edit_maybe(p, id, res)
}

/// Hear a pronunciation in a timed segment, or (None) in nothing.
pub fn lexicon_pronunciation_audio(session: &mut Session, id: &str, pronunciation: &str, segment: Option<&str>) -> Result<EntryEdit, String> {
    let p = project(session)?;
    own_pronunciation(p, id, pronunciation)?;
    let heard = segment.map(|g| heard_in(p, g)).transpose()?;
    let res = p.set_pronunciation_audio(pronunciation, heard.as_ref().map(|(h, _)| h))?;
    edit_maybe(p, id, res)
}

pub fn lexicon_delete_pronunciation(session: &mut Session, id: &str, pronunciation: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    own_pronunciation(p, id, pronunciation)?;
    let res = p.delete_pronunciation(pronunciation)?;
    edit(p, id, &res)
}

/// The live entry a typed headword names: its lemma, with a homograph
/// number after it (`tela2`, `tela 2`, `tela₂`) when several share it.
fn entry_named(p: &Project, typed: &str) -> Result<String, String> {
    let typed = p.proj.nfc(typed.trim());
    let sub = |c: char| ('₀'..='₉').contains(&c);
    let digits: String = typed.chars().rev().take_while(|c| c.is_ascii_digit() || sub(*c)).collect::<Vec<_>>().into_iter().rev().collect();
    let wanted: Option<u32> = (!digits.is_empty())
        .then(|| digits.chars().map(|c| if sub(c) { char::from(b'0' + (c as u32 - '₀' as u32) as u8) } else { c }).collect::<String>().parse().ok())
        .flatten();
    let bare = typed[..typed.len() - digits.len()].trim_end().to_string();
    let mut found = Vec::new();
    for name in [typed.clone(), bare.clone()] {
        if name.is_empty() {
            continue;
        }
        for lex in p.proj.by_lookup_key("lexeme", &wugs_model::lexicon_key(&name))? {
            if !p.proj.is_tombstoned(&lex)? && p.proj.nfc(&str_field(p, &lex, "lemma")?) == name && !found.iter().any(|(l, _, _)| *l == lex) {
                found.push((lex.clone(), homograph(p, &lex)?, name == bare && name != typed));
            }
        }
        if !found.is_empty() {
            break;
        }
    }
    let numbered: Vec<&(String, Option<u32>, bool)> = found.iter().filter(|(_, h, by_number)| !by_number || *h == wanted).collect();
    match numbered.as_slice() {
        [] if found.is_empty() => Err(format!("No entry is spelled «{typed}». Add it first, or check the spelling.")),
        [] => Err(format!("No «{bare}» has homograph number {}.", wanted.unwrap_or(0))),
        [(lex, _, _)] => Ok(lex.clone()),
        _ => Err(format!("Several entries are spelled «{bare}»: add the homograph number, such as «{bare}2».")),
    }
}

/// Relate the entry to another, named by its headword.
pub fn lexicon_add_relation(session: &mut Session, id: &str, relation: &str, target: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    require(p, id, "lexeme")?;
    let other = entry_named(p, target)?;
    let (_, res) = p.add_relation(id, relation, &other)?;
    edit(p, id, &res)
}

pub fn lexicon_delete_relation(session: &mut Session, id: &str, relation: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    if !p.relations(id)?.iter().any(|r| r.id == relation) {
        return Err("that relation is not one of this entry's".into());
    }
    let res = p.delete_relation(relation)?;
    edit(p, id, &res)
}

/// Remove one of the entry's allomorphs (a tombstone).
pub fn lexicon_delete_allomorph(session: &mut Session, id: &str, form: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    if !p.allomorphs(id)?.iter().any(|(f, _)| f == form) {
        return Err("that form is not one of this entry's".into());
    }
    let res = p.delete_form(form)?;
    edit(p, id, &res)
}

/// Copy a picture into the blob store and show it with a sense.
pub fn lexicon_set_picture(session: &mut Session, sense: &str, path: &str) -> Result<EntryEdit, String> {
    let file = std::path::Path::new(path);
    let name = file.file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("{path} has no usable file name"))?.to_string();
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if !PICTURE_TYPES.contains(&ext.as_str()) {
        return Err(format!("{name} is not a picture wugs can show: choose a PNG, JPEG, GIF or WebP file."));
    }
    let size = std::fs::metadata(file).map_err(|e| format!("{name}: {e}"))?.len();
    if size > PICTURE_MAX_BYTES {
        return Err(format!("{name} is {} MB; a dictionary picture can be at most 20 MB. Save a smaller copy and choose that.", size / (1024 * 1024)));
    }
    let p = project(session)?;
    require(p, sense, "sense")?;
    let entry = str_field(p, sense, "lexeme")?;
    let store = wugs_blob::BlobStore::open(&p.root).map_err(|e| format!("open blob store: {e}"))?;
    let blob = store.put_file(file).map_err(|e| format!("store {name}: {e}"))?;
    let res = p.set_sense_picture(sense, Some((&blob, &name)))?;
    edit(p, &entry, &res)
}

pub fn lexicon_clear_picture(session: &mut Session, sense: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    require(p, sense, "sense")?;
    let entry = str_field(p, sense, "lexeme")?;
    let res = p.set_sense_picture(sense, None)?;
    edit(p, &entry, &res)
}

pub fn lexicon_delete(session: &mut Session, id: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let res = p.delete_lexeme(id)?;
    Ok(EntryEdit { entry: None, notes: lines(&res) })
}

/// Merge entry `other` into `keep` (`Project::merge_lexemes`); `keep`'s view back.
pub fn lexicon_merge(session: &mut Session, keep: &str, other: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let res = p.merge_lexemes(keep, other)?;
    edit(p, keep, &res)
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

/// A new example typed for a sense.
pub fn lexicon_add_example(session: &mut Session, sense: &str, sentence: &str, translation: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let lex = owner(p, sense)?;
    let (_, res) = p.add_example(sense, sentence, translation, None, None)?;
    edit(p, &lex, &res)
}

/// A sentence from a text as an example of a sense, with its first
/// translation and a link back to it. The lexicon is shared with everyone
/// the project reaches, so a sentence of a text that is not open is not
/// copied into it (DECISIONS 2026-11-23).
pub fn lexicon_example_from(session: &mut Session, sense: &str, segment: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    let lex = owner(p, sense)?;
    require(p, segment, "segment")?;
    let text = p.proj.field_str(segment, "text")?.unwrap_or_default();
    if p.text_access(&text)? != "open" {
        let title = str_field(p, &text, "title")?;
        return Err(format!("«{title}» is not open to everyone, and the lexicon is: type an example that may be shared instead."));
    }
    if p.examples(sense)?.iter().any(|e| e.source.as_deref() == Some(segment)) {
        return Err("That sentence is already an example of this sense.".into());
    }
    let content = str_field(p, segment, "content")?;
    let translation = crate::interchange::translations_of(p, segment)?.into_iter().map(|(_, t)| t).next().unwrap_or_default();
    let (_, res) = p.add_example(sense, &content, &translation, Some(segment), None)?;
    edit(p, &lex, &res)
}

/// A sense's first use in a text the lexicon may quote (open to everyone),
/// as a segment with content; one with a translation is preferred.
fn first_use(p: &Project, sense: &str) -> Result<Option<String>, String> {
    let live = |id: &str| -> Result<bool, String> { Ok(p.proj.entity_kind(id)?.is_some() && !p.proj.is_tombstoned(id)?) };
    let mut morphs = p.proj.children_by_ref("sense", sense, "morph")?;
    morphs.sort();
    let mut untranslated = None;
    for morph in morphs {
        let Some(token) = p.proj.field_str(&morph, "token")? else { continue };
        let Some(segment) = p.proj.field_str(&token, "segment")? else { continue };
        let Some(text) = p.proj.field_str(&segment, "text")? else { continue };
        if !(live(&morph)? && live(&token)? && live(&segment)? && live(&text)?) || !p.proj.members(&token)?.contains(&morph) || !p.proj.members(&segment)?.contains(&token) {
            continue;
        }
        if p.text_access(&text)? != "open" || str_field(p, &segment, "content")?.trim().is_empty() {
            continue;
        }
        if crate::interchange::translations_of(p, &segment)?.into_iter().any(|(_, t)| !t.trim().is_empty()) {
            return Ok(Some(segment));
        }
        untranslated.get_or_insert(segment);
    }
    Ok(untranslated)
}

/// Senses of live entries with no example sentence yet: how many, and how
/// many of them are used in a text that is open, so could take one.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleGaps {
    pub without: usize,
    pub fillable: usize,
}

fn senses_without_examples(p: &Project) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for lex in p.proj.entities_of_kind("lexeme")? {
        if p.proj.is_tombstoned(&lex)? {
            continue;
        }
        for sense in senses(p, &lex)? {
            if p.examples(&sense)?.is_empty() {
                out.push(sense);
            }
        }
    }
    Ok(out)
}

pub fn example_gaps(session: &mut Session) -> Result<ExampleGaps, String> {
    let p = project(session)?;
    let without = senses_without_examples(p)?;
    let mut fillable = 0;
    for sense in &without {
        if first_use(p, sense)?.is_some() {
            fillable += 1;
        }
    }
    Ok(ExampleGaps { without: without.len(), fillable })
}

/// Give every sense with no example its first use in an open text (with its
/// translation, preferring a translated sentence), as **Use as example**
/// does one at a time. Returns how many were added.
pub fn examples_from_texts(session: &mut Session) -> Result<usize, String> {
    let p = project(session)?;
    let mut added = 0;
    for sense in senses_without_examples(p)? {
        let Some(segment) = first_use(p, &sense)? else { continue };
        let content = str_field(p, &segment, "content")?;
        let translation = crate::interchange::translations_of(p, &segment)?.into_iter().map(|(_, t)| t).find(|t| !t.trim().is_empty()).unwrap_or_default();
        p.add_example(&sense, &content, &translation, Some(&segment), None)?;
        added += 1;
    }
    Ok(added)
}

/// Retype an example's `sentence` or `translation`.
pub fn lexicon_set_example(session: &mut Session, example: &str, field: &str, text: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    require(p, example, "example")?;
    let sense = p.proj.field_str(example, "sense")?.unwrap_or_default();
    let lex = owner(p, &sense)?;
    let res = p.set_example_text(example, field, text)?;
    match res {
        Some(res) => edit(p, &lex, &res),
        None => Ok(EntryEdit { entry: Some(entry_view(p, &lex)?), notes: Vec::new() }),
    }
}

pub fn lexicon_delete_example(session: &mut Session, example: &str) -> Result<EntryEdit, String> {
    let p = project(session)?;
    require(p, example, "example")?;
    let sense = p.proj.field_str(example, "sense")?.unwrap_or_default();
    let lex = owner(p, &sense)?;
    let res = p.delete_example(example)?;
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
    // How many live senses of live entries each domain holds, each pair once.
    let mut words: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
    for link in p.proj.entities_of_kind("sense-domain")? {
        if p.proj.is_tombstoned(&link)? {
            continue;
        }
        let (Some(sense), Some(dom)) = (p.proj.field_str(&link, "sense")?, p.proj.field_str(&link, "domain")?) else { continue };
        if p.proj.is_tombstoned(&sense)? {
            continue;
        }
        match p.proj.field_str(&sense, "lexeme")? {
            Some(lex) if !p.proj.is_tombstoned(&lex)? => {
                words.entry(dom).or_default().insert(sense);
            }
            _ => {}
        }
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
            words: words.get(&id).map_or(0, |s| s.len()),
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

/// The bundled SIL list loaded into a project that holds no domains yet, so
/// an imported file's domain codes have something to link to. How many
/// domains it made (0 when the project had some already).
pub fn sil_domains_if_none(p: &mut wugs_core::Project) -> Result<usize, String> {
    for id in p.proj.entities_of_kind("domain")? {
        if !p.proj.is_tombstoned(&id)? {
            return Ok(0);
        }
    }
    Ok(p.import_semantic_domains(SIL_ORIGIN, &sil_domain_rows()?)?.created)
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

#[cfg(test)]
mod tests {
    #[test]
    fn words_become_entries_once_and_known_ones_are_skipped() {
        let dir = std::env::temp_dir().join(format!("wugs-add-words-{}", wugs_model::ulid()));
        let mut s: crate::session::Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let before = super::lexicon_list(&mut s, "").unwrap().len();
        let words = ["wʌn", "ndizi", "Ndizi", "  ", "kisu"].map(String::from);
        assert_eq!(super::lexicon_add_words(&mut s, &words).unwrap(), 2);
        assert_eq!(super::lexicon_list(&mut s, "").unwrap().len(), before + 2);
        let row = super::lexicon_list(&mut s, "ndizi").unwrap().remove(0);
        assert_eq!(row.lemma, "ndizi");
        assert_eq!(super::lexicon_add_words(&mut s, &words).unwrap(), 0, "a second run adds nothing");
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn senses_take_their_first_use_as_an_example() {
        let dir = std::env::temp_dir().join(format!("wugs-ex-from-texts-{}", wugs_model::ulid()));
        let mut s: crate::session::Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let gaps = super::example_gaps(&mut s).unwrap();
        assert!(gaps.without >= gaps.fillable && gaps.fillable >= 5, "{gaps:?}");
        let added = super::examples_from_texts(&mut s).unwrap();
        assert_eq!(added, gaps.fillable);
        let wan = super::lexicon_list(&mut s, "wʌn").unwrap()[0].id.clone();
        let e = super::entry_view(crate::session::project(&mut s).unwrap(), &wan).unwrap();
        let ex = &e.senses[0].examples[0];
        assert_eq!(ex.sentence, "wʌn tuː θɹiː");
        assert!(!ex.translation.is_empty() && ex.from.is_some(), "{ex:?}");
        assert_eq!(super::example_gaps(&mut s).unwrap().fillable, 0, "a second run adds nothing");
        // The list now says the entry has its examples, and that it is not heard yet.
        let row = super::lexicon_list(&mut s, "wʌn").unwrap().remove(0);
        assert!(row.examples && !row.heard, "{row:?}");
        let text = crate::session::list_texts(&mut s).unwrap()[0].id.clone();
        let seg = crate::session::get_text(&mut s, &text).unwrap().segments.into_iter().find(|g| g.content.starts_with("wʌn")).unwrap().id;
        super::lexicon_add_pronunciation(&mut s, &wan, "", Some(&seg)).unwrap();
        assert!(super::lexicon_list(&mut s, "wʌn").unwrap()[0].heard, "a pronunciation heard in the recording counts");
        assert_eq!(super::examples_from_texts(&mut s).unwrap(), 0);
        // In a semantic domain once its one sense is.
        assert!(!super::lexicon_list(&mut s, "wʌn").unwrap()[0].domains);
        let dom = {
            let p = crate::session::project(&mut s).unwrap();
            super::sil_domains_if_none(p).unwrap();
            p.proj.entities_of_kind("domain").unwrap().remove(0)
        };
        super::lexicon_assign_domain(&mut s, &e.senses[0].id, &dom).unwrap();
        assert!(super::lexicon_list(&mut s, "wʌn").unwrap()[0].domains);
        std::fs::remove_dir_all(&dir).ok();
    }

    use super::*;

    #[test]
    fn the_list_says_which_entries_a_text_uses() {
        let dir = std::env::temp_dir().join(format!("wugs-lexicon-list-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let before = lexicon_list(&mut session, "").unwrap();
        assert!(before.iter().any(|r| r.used));
        for r in &before {
            let entry = lexicon_entry(&mut session, &r.id).unwrap();
            assert_eq!(r.used, entry.senses.iter().any(|s| s.occurrences > 0), "{} agrees with used in N words", r.lemma);
        }
        let id = lexicon_add(&mut session, "zuzu", "").unwrap().entry.unwrap().id;
        let row = lexicon_list(&mut session, "zuzu").unwrap().into_iter().find(|r| r.id == id).unwrap();
        assert!(!row.used);
        assert_eq!(row.glosses, vec![String::new()], "an entry added with no gloss has one sense with an empty gloss");
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
