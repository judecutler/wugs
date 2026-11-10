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
    pub count: usize,
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
    pub words: Vec<WordView>,
    pub hidden: Vec<HiddenView>,
    pub alternatives: Vec<AlternativeView>,
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

fn view(p: &Project, segment_id: &str) -> Result<GlossView, String> {
    let tokens = p.proj.members(segment_id)?;
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
            p.analysis_suggestions(&form)?.into_iter().take(SUGGESTIONS_PER_WORD).map(|s: Suggestion| SuggestionView { morphs: s.morphs.into_iter().map(Into::into).collect(), count: s.count }).collect()
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
    Ok(GlossView { segment_id: segment_id.to_string(), words, hidden, alternatives })
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
