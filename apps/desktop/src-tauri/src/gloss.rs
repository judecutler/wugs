//! The Gloss panel's view of a segment and its edits (GLOSSING-MODEL.md).
//! Plain functions over the open session; the tauri commands only forward.
//! Every edit returns the segment's fresh view, so the panel redraws from one
//! answer and never guesses what the engine did.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wugs_core::{LexiconMatch, MorphSpec, Project, Suggestion};

use crate::session::{project, Session};

/// Suggestions shown per unanalysed word.
const SUGGESTIONS_PER_WORD: usize = 3;

thread_local! {
    /// The project's known morphs (`Project::morph_inventory`), kept while
    /// nothing has been written: keyed by project folder and newest op.
    static INVENTORY: std::cell::RefCell<Option<(std::path::PathBuf, Option<String>, std::rc::Rc<wugs_core::parse::Inventory>)>> = const { std::cell::RefCell::new(None) };
}

/// The known-morph inventory for guesses, built once per change to the project.
pub(crate) fn inventory(p: &Project) -> Result<std::rc::Rc<wugs_core::parse::Inventory>, String> {
    let newest = p.proj.max_hlc()?;
    if let Some(inv) = INVENTORY.with(|c| c.borrow().as_ref().filter(|(root, hlc, _)| *root == p.root && *hlc == newest).map(|(_, _, inv)| inv.clone())) {
        return Ok(inv);
    }
    let inv = std::rc::Rc::new(p.morph_inventory()?);
    INVENTORY.with(|c| *c.borrow_mut() = Some((p.root.clone(), newest, inv.clone())));
    Ok(inv)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MorphSpecView {
    pub surface: String,
    #[serde(rename = "type")]
    pub morph_type: String,
    pub gloss: Option<String>,
    #[serde(default)]
    pub glosses_by_lang: BTreeMap<String, String>,
    pub sense: Option<String>,
}

impl From<MorphSpec> for MorphSpecView {
    fn from(m: MorphSpec) -> Self {
        MorphSpecView { surface: m.surface, morph_type: m.morph_type, gloss: m.gloss, glosses_by_lang: m.glosses_by_lang, sense: m.sense }
    }
}

impl From<MorphSpecView> for MorphSpec {
    fn from(m: MorphSpecView) -> Self {
        MorphSpec { surface: m.surface, morph_type: m.morph_type, gloss: m.gloss, glosses_by_lang: m.glosses_by_lang, sense: m.sense }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MorphView {
    pub id: String,
    #[serde(flatten)]
    pub spec: MorphSpecView,
    /// `lemma – gloss` of the linked sense, when there is one.
    pub sense_label: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionView {
    pub morphs: Vec<MorphSpecView>,
    /// How many tokens carry it; 0 for a guess from known morphs (`guessed`).
    pub count: usize,
    /// Read from morphs and entries the project knows, not from this form's earlier analyses.
    pub guessed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordView {
    pub id: String,
    pub form: String,
    pub morphs: Vec<MorphView>,
    /// Earlier analyses of this form, best first. Empty once the word has an analysis.
    pub suggestions: Vec<SuggestionView>,
    /// Other unanalysed tokens spelled like this one, across the project.
    pub others_unanalysed: usize,
    /// Validator findings on this word or its morphs (warnings, never blocks).
    pub findings: Vec<String>,
}

/// Work from another device that the current analysis leaves out (a hidden
/// edit, GLOSSING-MODEL "Merge semantics"), one per hidden token or morph.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenView {
    /// The word whose morph list, or the segment whose word list, leaves `member` out.
    pub parent: String,
    pub member: String,
    /// The hidden word's form or morph's surface.
    pub label: String,
    /// The edited fields (`gloss`, `gloss:es`, `sense`, …).
    pub fields: Vec<String>,
}

/// The losing side of a conflict on a word's morph list or on the segment's
/// word list, offered as "use that instead" (a glossing slice of the conflict
/// UI, roadmap item 24).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeView {
    /// The word whose morphs, or the segment whose words, this would replace.
    pub parent: String,
    pub members: Vec<String>,
    /// Each member's morph surface or word form, in order.
    pub surfaces: Vec<String>,
    /// Each morph's gloss; empty for words.
    pub glosses: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossView {
    pub segment_id: String,
    /// The words to gloss: the segment's tokens without punctuation tokens
    /// (FLEx and CoNLL-U keep those; they take no gloss).
    pub words: Vec<WordView>,
    /// The words the transcription reads as, for telling whether it changed
    /// since it was broken out: the words, with a multi-word token
    /// (CoNLL-U's `del` = `de el`) read as the one word written.
    pub reads_as: Vec<String>,
    pub hidden: Vec<HiddenView>,
    pub alternatives: Vec<AlternativeView>,
    /// The sentence's notes (Annotations of kind `note`), oldest first.
    pub segment_notes: Vec<NoteView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteView {
    pub id: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossEdit {
    pub view: GlossView,
    /// Things worth a toast: findings of the commit, words that lost their analysis.
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyAllResult {
    pub applied: usize,
    pub texts: usize,
    pub skipped_analysed: usize,
    pub view: GlossView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LexiconMatchView {
    pub lexeme: String,
    pub sense: String,
    pub lemma: String,
    pub gloss: String,
}

impl From<LexiconMatch> for LexiconMatchView {
    fn from(m: LexiconMatch) -> Self {
        LexiconMatchView { lexeme: m.lexeme, sense: m.sense, lemma: m.lemma, gloss: m.gloss }
    }
}

fn lines(findings: &[wugs_core::Finding]) -> Vec<String> {
    findings.iter().map(|f| format!("{}: {}", f.code, f.message)).collect()
}

fn require_segment(p: &Project, text_id: &str, segment_id: &str) -> Result<(), String> {
    let ok = p.proj.entity_kind(segment_id)?.as_deref() == Some("segment")
        && !p.proj.is_tombstoned(segment_id)?
        && p.proj.field_str(segment_id, "text")?.as_deref() == Some(text_id);
    if ok { Ok(()) } else { Err(format!("no segment {segment_id} in text {text_id}")) }
}

fn sense_label(p: &Project, sense: &str) -> Result<Option<String>, String> {
    if p.proj.entity_kind(sense)?.as_deref() != Some("sense") || p.proj.is_tombstoned(sense)? {
        return Ok(None);
    }
    let gloss = p.proj.field_str(sense, "gloss")?.unwrap_or_default();
    let lemma = match p.proj.field_str(sense, "lexeme")? {
        Some(lex) => p.proj.field_str(&lex, "lemma")?.unwrap_or_default(),
        None => String::new(),
    };
    Ok(Some(match (lemma.as_str(), gloss.as_str()) {
        ("", g) => g.to_string(),
        (l, "") => l.to_string(),
        (l, g) => format!("{l} \u{2013} {g}"),
    }))
}

/// The words with each CoNLL-U multi-word token's range read as its own
/// written form (`mwt` on the segment: first and last token, and `form`).
fn reads_as(p: &Project, segment_id: &str, words: &[WordView]) -> Result<Vec<String>, String> {
    let mut ranges: Vec<(usize, usize, String)> = Vec::new();
    if let Some((wugs_model::Value::Arr(rs), _)) = p.proj.field(segment_id, "mwt")? {
        for r in rs {
            let at = |k: &str| r.get(k).and_then(wugs_model::Value::as_str).and_then(|t| words.iter().position(|w| w.id == t));
            if let (Some(a), Some(b), Some(form)) = (at("first"), at("last"), r.get("form").and_then(wugs_model::Value::as_str)) {
                if a < b && ranges.last().is_none_or(|l| l.1 < a) {
                    ranges.push((a, b, form.to_string()));
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        match ranges.iter().find(|r| r.0 == i) {
            Some((_, b, form)) => {
                out.push(form.clone());
                i = b + 1;
            }
            None => {
                out.push(words[i].form.clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

fn view(p: &Project, segment_id: &str) -> Result<GlossView, String> {
    let mut tokens = p.proj.members(segment_id)?;
    tokens.retain(|t| !p.proj.is_punct_token(t));
    let findings = p.token_findings(&tokens)?;
    let mut forms = Vec::with_capacity(tokens.len());
    for tok in &tokens {
        forms.push(p.proj.field_str(tok, "form")?.unwrap_or_default());
    }
    let unanalysed = p.unanalysed_counts(&forms)?;
    let mut words = Vec::with_capacity(tokens.len());
    for ((tok, form), unanalysed) in tokens.iter().zip(forms).zip(unanalysed) {
        let mut morphs = Vec::new();
        for id in p.proj.members(tok)? {
            let spec: MorphSpecView = p.morph_spec(&id)?.into();
            let sense_label = match spec.sense.as_deref() {
                Some(s) => sense_label(p, s)?,
                None => None,
            };
            morphs.push(MorphView { id, spec, sense_label });
        }
        let suggestions = if morphs.is_empty() {
            let earlier = p.analysis_suggestions(&form)?;
            // No earlier analysis of this form: guess one from the morphs the project knows.
            let (list, guessed) = if earlier.is_empty() { (p.guess_analysis(&*inventory(p)?, &form), true) } else { (earlier, false) };
            list.into_iter().take(SUGGESTIONS_PER_WORD).map(|s: Suggestion| SuggestionView { morphs: s.morphs.into_iter().map(Into::into).collect(), count: s.count, guessed }).collect()
        } else {
            Vec::new()
        };
        let others_unanalysed = unanalysed.saturating_sub(usize::from(morphs.is_empty()));
        let mine: Vec<String> = findings
            .iter()
            .filter(|f| f.target == *tok || morphs.iter().any(|m| m.id == f.target))
            .map(|f| format!("{}: {}", f.code, f.message))
            .collect();
        words.push(WordView { id: tok.clone(), form, morphs, suggestions, others_unanalysed, findings: mine });
    }
    let mut hidden: Vec<HiddenView> = Vec::new();
    for h in p.proj.hidden_edits(segment_id)? {
        match hidden.iter_mut().find(|v| v.member == h.member) {
            Some(v) if !v.fields.contains(&h.field) => v.fields.push(h.field),
            Some(_) => {}
            None => {
                let label = match p.proj.field_str(&h.member, "form")? {
                    Some(form) => form,
                    None => p.proj.field_str(&h.member, "surface")?.unwrap_or_default(),
                };
                hidden.push(HiddenView { parent: h.parent, member: h.member, label, fields: vec![h.field] });
            }
        }
    }
    let mut alternatives = Vec::new();
    for (parent, field) in std::iter::once((segment_id, "tokens")).chain(tokens.iter().map(|t| (t.as_str(), "morphs"))) {
        if let Some(alt) = alternative(p, parent, field)? {
            alternatives.push(alt);
        }
    }
    let reads_as = reads_as(p, segment_id, &words)?;
    let segment_notes = p.segment_notes(segment_id)?.into_iter().map(|(id, body)| NoteView { id, body }).collect();
    Ok(GlossView { segment_id: segment_id.to_string(), words, reads_as, hidden, alternatives, segment_notes })
}

/// The other list of an open conflict on `parent.field`, if it still names
/// only live members and differs from what is shown.
fn alternative(p: &Project, parent: &str, field: &str) -> Result<Option<AlternativeView>, String> {
    let Some(wugs_model::Value::Arr(ids)) = p.proj.conflict_loser(parent, field)? else { return Ok(None) };
    let (ref_field, label_field) = if field == "tokens" { ("segment", "form") } else { ("token", "surface") };
    let mut alt = AlternativeView { parent: parent.to_string(), members: Vec::new(), surfaces: Vec::new(), glosses: Vec::new() };
    for id in ids.iter().filter_map(|v| v.as_str()) {
        if p.proj.field_str(id, ref_field)?.as_deref() != Some(parent) || p.proj.is_tombstoned(id)? {
            return Ok(None);
        }
        alt.surfaces.push(p.proj.field_str(id, label_field)?.unwrap_or_default());
        if field == "morphs" {
            alt.glosses.push(p.proj.field_str(id, "gloss")?.unwrap_or_default());
        }
        alt.members.push(id.to_string());
    }
    if alt.members == p.proj.members(parent)? {
        return Ok(None);
    }
    Ok(Some(alt))
}

pub fn gloss_get(session: &mut Session, text_id: &str, segment_id: &str) -> Result<GlossView, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    view(p, segment_id)
}

fn edit(p: &Project, segment_id: &str, notes: Vec<String>) -> Result<GlossEdit, String> {
    Ok(GlossEdit { view: view(p, segment_id)?, notes })
}

/// Break a segment's transcription into tokens (the forms come from the panel's tokenizer).
pub fn gloss_tokenize(session: &mut Session, text_id: &str, segment_id: &str, forms: &[String]) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let refs: Vec<&str> = forms.iter().map(String::as_str).collect();
    let (_, res) = p.tokenize(text_id, segment_id, &refs)?;
    edit(p, segment_id, lines(&res.findings))
}

/// Re-split after the transcription changed; unchanged words keep their analysis.
pub fn gloss_retokenize(session: &mut Session, text_id: &str, segment_id: &str, forms: &[String]) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let refs: Vec<&str> = forms.iter().map(String::as_str).collect();
    let r = p.retokenize(text_id, segment_id, &refs)?;
    let mut notes = lines(&r.commit.findings);
    if r.lost_analysed > 0 {
        let plural = if r.lost_analysed == 1 { "word" } else { "words" };
        notes.insert(0, format!("{} analysed {plural} changed and lost the analysis; the others kept theirs.", r.lost_analysed));
    }
    edit(p, segment_id, notes)
}

/// Give one word exactly this analysis (accepting a suggestion, or re-typing the breakdown).
pub fn gloss_apply(session: &mut Session, text_id: &str, segment_id: &str, token_id: &str, morphs: Vec<MorphSpecView>) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let specs: Vec<MorphSpec> = morphs.into_iter().map(Into::into).collect();
    let (_, res) = p.apply_analysis(text_id, token_id, &specs)?;
    edit(p, segment_id, lines(&res.findings))
}

/// Apply one analysis to every other unanalysed token spelled the same.
pub fn gloss_apply_all(session: &mut Session, text_id: &str, segment_id: &str, form: &str, morphs: Vec<MorphSpecView>) -> Result<ApplyAllResult, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let specs: Vec<MorphSpec> = morphs.into_iter().map(Into::into).collect();
    let r = p.apply_analysis_to_unanalysed(form, &specs)?;
    Ok(ApplyAllResult { applied: r.applied, texts: r.texts, skipped_analysed: r.skipped_analysed, view: view(p, segment_id)? })
}

/// Set a morph's gloss in the primary language, or in `lang`.
/// Add a note to the sentence.
pub fn gloss_add_note(session: &mut Session, text_id: &str, segment_id: &str, body: &str) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    p.add_segment_note(text_id, segment_id, body)?;
    edit(p, segment_id, Vec::new())
}

/// Retype one of the sentence's notes; an empty one is deleted.
pub fn gloss_set_note(session: &mut Session, text_id: &str, segment_id: &str, note_id: &str, body: &str) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    if !p.segment_notes(segment_id)?.iter().any(|(id, _)| id == note_id) {
        return Err("That note is not on this sentence any more.".into());
    }
    p.set_note(text_id, note_id, body)?;
    edit(p, segment_id, Vec::new())
}

pub fn gloss_set(session: &mut Session, text_id: &str, segment_id: &str, morph_id: &str, gloss: &str, lang: Option<&str>) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let res = p.set_gloss(text_id, morph_id, gloss, lang)?;
    edit(p, segment_id, lines(&res.findings))
}

/// Pick a lexicon match: link the sense and set the gloss to the sense's.
pub fn gloss_pick_sense(session: &mut Session, text_id: &str, segment_id: &str, morph_id: &str, sense_id: &str, gloss: &str) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let res = p.pick_sense(text_id, morph_id, sense_id, gloss)?;
    edit(p, segment_id, lines(&res.findings))
}

/// "Add «surface» 'gloss' to the lexicon": a new lexeme with one sense, linked to the morph.
/// A lexeme with exactly this lemma and a sense with exactly this gloss is linked instead,
/// so a double click or a stale lexicon list never makes a duplicate entry.
pub fn gloss_add_lexeme(session: &mut Session, text_id: &str, segment_id: &str, morph_id: &str, lemma: &str, gloss: &str) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let same = |a: &str, b: &str| p.proj.nfc(a) == p.proj.nfc(b);
    let existing = p.lexicon_lookup(lemma)?.into_iter().find(|m| same(&m.lemma, lemma) && same(&m.gloss, gloss));
    let sense = match existing {
        Some(m) => m.sense,
        None => p.create_lexeme(lemma, gloss)?.1,
    };
    let res = p.link_sense(text_id, morph_id, &sense)?;
    edit(p, segment_id, lines(&res.findings))
}

/// Put a hidden word or morph back into view (its parent's list), so the edits on it show again.
pub fn gloss_restore(session: &mut Session, text_id: &str, segment_id: &str, parent_id: &str, member_id: &str) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let res = p.restore_member(text_id, parent_id, member_id)?;
    edit(p, segment_id, lines(&res.findings))
}

/// Use the other side of a conflict on a word's analysis or the segment's words.
pub fn gloss_choose_list(session: &mut Session, text_id: &str, segment_id: &str, parent_id: &str, members: &[String]) -> Result<GlossEdit, String> {
    let p = project(session)?;
    require_segment(p, text_id, segment_id)?;
    let res = p.choose_member_list(text_id, parent_id, members)?;
    edit(p, segment_id, lines(&res.findings))
}

pub fn gloss_lexicon(session: &mut Session, surface: &str) -> Result<Vec<LexiconMatchView>, String> {
    Ok(project(session)?.lexicon_lookup(surface)?.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;

    #[test]
    fn a_new_word_gets_a_guess_from_known_morphs() {
        let dir = std::env::temp_dir().join(format!("wugs-gloss-guess-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        crate::sample::add_morphology(&mut s).unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let seg = session::add_segment(&mut s, &text, None, Some(20_000.0), Some(21_000.0), "nitekama kanima").unwrap().id;
        let forms: Vec<String> = ["nitekama", "kanima"].iter().map(|f| f.to_string()).collect();
        let view = gloss_tokenize(&mut s, &text, &seg, &forms).unwrap().view;
        let line = |v: &SuggestionView| v.morphs.iter().map(|m| format!("{}:{}", m.surface, m.gloss.as_deref().unwrap_or("?"))).collect::<Vec<_>>().join(" ");
        let first = &view.words[0].suggestions[0];
        assert!(first.guessed && first.count == 0);
        assert_eq!(line(first), "ni:1SG teka:ten ma:FOC");
        let second = &view.words[1].suggestions[0];
        assert_eq!(line(second), "kani:count ma:FOC");
        // The root's guess keeps its lexicon link.
        assert!(second.morphs[0].sense.is_some());
        // Accepting one is an ordinary analysis; the next word spelled so gets it as an earlier analysis.
        gloss_apply(&mut s, &text, &seg, &view.words[1].id, second.morphs.clone()).unwrap();
        let again = session::add_segment(&mut s, &text, None, Some(22_000.0), Some(23_000.0), "kanima").unwrap().id;
        let v = gloss_tokenize(&mut s, &text, &again, &["kanima".to_string()]).unwrap().view;
        assert!(!v.words[0].suggestions[0].guessed);
        assert_eq!(v.words[0].suggestions[0].count, 1);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sentence_takes_notes() {
        let dir = std::env::temp_dir().join(format!("wugs-gloss-notes-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let seg = session::add_segment(&mut s, &text, None, Some(20_000.0), Some(21_000.0), "kani").unwrap().id;
        assert!(gloss_get(&mut s, &text, &seg).unwrap().segment_notes.is_empty());
        let v = gloss_add_note(&mut s, &text, &seg, "Said twice.").unwrap().view;
        assert_eq!(v.segment_notes.len(), 1);
        let id = v.segment_notes[0].id.clone();
        let v = gloss_set_note(&mut s, &text, &seg, &id, "Said twice, softly.").unwrap().view;
        assert_eq!(v.segment_notes[0].body, "Said twice, softly.");
        // Search finds the note as its sentence.
        let hits = session::search(&mut s, "softly", session::SearchScope::Texts).unwrap();
        assert!(hits.iter().any(|h| h.target == seg && h.kind == "segment" && h.field == "note"), "{hits:?}");
        let other = session::add_segment(&mut s, &text, None, Some(22_000.0), Some(23_000.0), "teka").unwrap().id;
        assert!(gloss_set_note(&mut s, &text, &other, &id, "x").is_err(), "a note is edited from its own sentence");
        assert!(gloss_set_note(&mut s, &text, &seg, &id, " ").unwrap().view.segment_notes.is_empty());
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_ud_sentence_reads_as_written_and_its_punctuation_takes_no_gloss() {
        let dir = std::env::temp_dir().join(format!("wugs-gloss-ud-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut s).unwrap();
        let tier = session::utterance_tier(p).unwrap();
        let ud = "# text = Vámonos al mar.\n1-2\tVámonos\t_\t_\t_\t_\t_\t_\t_\t_\n1\tVamos\tir\tVERB\t_\t_\t0\troot\t_\t_\n2\tnos\tnosotros\tPRON\t_\t_\t1\tobj\t_\t_\n3-4\tal\t_\t_\t_\t_\t_\t_\t_\t_\n3\ta\ta\tADP\t_\t_\t5\tcase\t_\t_\n4\tel\tel\tDET\t_\t_\t5\tdet\t_\t_\n5\tmar\tmar\tNOUN\t_\t_\t1\tobl\t_\tSpaceAfter=No\n6\t.\t.\tPUNCT\t_\t_\t1\tpunct\t_\t_\n\n";
        let text = wugs_conllu::import_conllu(p, ud, "playa", &tier).unwrap().texts[0].clone();
        let seg = p.proj.ordered_children("text", &text, "segment").unwrap()[0].clone();
        let v = gloss_get(&mut s, &text, &seg).unwrap();
        let forms: Vec<&str> = v.words.iter().map(|w| w.form.as_str()).collect();
        assert_eq!(forms, ["Vamos", "nos", "a", "el", "mar"], "syntactic words, no full stop");
        assert_eq!(v.reads_as, ["Vámonos", "al", "mar"], "what the Gloss panel compares with the transcription");
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
