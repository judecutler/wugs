//! Glossing intents beyond the primitives in `lib.rs` (GLOSSING-MODEL.md).
//! Analyses live on each occurrence: Segment, Token, Morph. Everything here
//! writes ordinary per-occurrence ops under one txn per intent; nothing is
//! shared between occurrences.

use std::collections::BTreeMap;

use wugs_model::Value;

use crate::{gloss_field, CommitResult, Finding, Project};

/// One morph of an analysis: what `apply_analysis` writes and what a
/// suggestion offers. The derived order (surface, type, gloss, other
/// languages, sense) only makes suggestions sort deterministically.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct MorphSpec {
    pub surface: String,
    /// `root`, `prefix`, `suffix`, `infix`, `clitic`, `reduplication`, …
    pub morph_type: String,
    /// The primary-language gloss (`Morph.gloss`).
    pub gloss: Option<String>,
    /// Extra analysis languages, by lowercase tag (`Morph.gloss:<tag>`).
    pub glosses_by_lang: BTreeMap<String, String>,
    pub sense: Option<String>,
}

/// One way earlier occurrences of a form were analysed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub morphs: Vec<MorphSpec>,
    /// How many tokens in the project carry exactly this analysis.
    pub count: usize,
}

/// A lexicon sense whose lexeme matches a morph surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexiconMatch {
    pub lexeme: String,
    pub sense: String,
    pub lemma: String,
    pub gloss: String,
}

/// What `apply_analysis_to_unanalysed` did.
#[derive(Debug, Default)]
pub struct AppliedToUnanalysed {
    /// Tokens that received the analysis.
    pub applied: usize,
    /// Texts those tokens belong to (one txn each).
    pub texts: usize,
    /// Tokens with this form that already had morphs and were left alone.
    pub skipped_analysed: usize,
    pub findings: Vec<Finding>,
}

/// What `retokenize` did.
#[derive(Debug)]
pub struct Retokenized {
    /// The new tokens, in order.
    pub tokens: Vec<String>,
    /// New tokens that took over an old token's morphs, glosses and sense links.
    pub carried: usize,
    /// Old tokens that had morphs and matched no new form: their analysis is not carried.
    pub lost_analysed: usize,
    pub commit: CommitResult,
}

/// Longest common subsequence of two form sequences, as (old index, new
/// index) pairs in order. Ties resolve deterministically: matching works
/// from the end, so when a form repeats, the later occurrences are matched.
pub fn lcs_pairs<A: AsRef<str>, B: AsRef<str>>(old: &[A], new: &[B]) -> Vec<(usize, usize)> {
    let (n, m) = (old.len(), new.len());
    let mut len = vec![vec![0usize; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            len[i][j] = if old[i - 1].as_ref() == new[j - 1].as_ref() {
                len[i - 1][j - 1] + 1
            } else {
                len[i - 1][j].max(len[i][j - 1])
            };
        }
    }
    let (mut i, mut j) = (n, m);
    let mut pairs = Vec::with_capacity(len[n][m]);
    while i > 0 && j > 0 {
        if old[i - 1].as_ref() == new[j - 1].as_ref() {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if len[i - 1][j] >= len[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();
    pairs
}

impl Project {
    /// Re-split a segment into tokens after its transcription changed, in one
    /// txn. The old and new form sequences are matched by longest common
    /// subsequence of their folded forms (NFC, case-folded), so a typo fix
    /// elsewhere or a sentence-initial capital never costs a word its
    /// analysis. A matched token keeps its id, and with it its morphs,
    /// glosses, sense links and extension fields; if its exact form changed
    /// the new form is set on it. Unmatched new forms become new bare tokens.
    /// The segment's `tokens` list is then written once, so a concurrent
    /// retokenize on another device is a conflict on that one field. Tokens
    /// left out are hidden, not deleted (GLOSSING-MODEL, "Merge semantics").
    pub fn retokenize(&mut self, text: &str, segment: &str, forms: &[&str]) -> Result<Retokenized, String> {
        self.write_tokens("retokenize", text, segment, forms)
    }

    pub(crate) fn write_tokens(&mut self, intent: &str, text: &str, segment: &str, forms: &[&str]) -> Result<Retokenized, String> {
        self.require_in_text(segment, "segment", text)?;
        if forms.iter().any(|f| f.trim().is_empty()) {
            return Err("a token form must not be empty".into());
        }
        let new_forms: Vec<String> = forms.iter().map(|f| self.normalized("form", f)).collect();
        let old = self.proj.members(segment)?;
        let mut old_forms = Vec::with_capacity(old.len());
        let mut analysed = Vec::with_capacity(old.len());
        for tok in &old {
            old_forms.push(self.proj.field_str(tok, "form")?.unwrap_or_default());
            analysed.push(!self.proj.members(tok)?.is_empty());
        }
        let old_keys: Vec<String> = old_forms.iter().map(|f| self.form_key(f)).collect();
        let new_keys: Vec<String> = new_forms.iter().map(|f| self.form_key(f)).collect();
        let pairs = lcs_pairs(&old_keys, &new_keys);
        let matched = |j: usize| pairs.iter().find(|&&(_, nj)| nj == j).map(|&(oi, _)| oi);

        let mut t = self.intent(intent);
        let mut tokens = Vec::with_capacity(new_forms.len());
        for (j, form) in new_forms.iter().enumerate() {
            match matched(j) {
                Some(oi) => {
                    if old_forms[oi] != *form {
                        // The person retyped this word: a direct edit of its
                        // form, so it settles a conflict on `form` (the token
                        // list itself stays a plain, derived write).
                        t.set_resolving("token", &old[oi], text, "form", Value::str(form))?;
                    }
                    tokens.push(old[oi].clone());
                }
                None => tokens.push(t.create(
                    "token",
                    "tok",
                    text,
                    vec![
                        ("segment", Value::str(segment)),
                        ("pos", Value::str(&wugs_model::pos_seq(j))),
                        ("form", Value::str(form)),
                    ],
                )),
            }
        }
        t.set_members("segment", segment, text, &tokens)?;
        let commit = t.commit()?;
        let kept = |i: usize| pairs.iter().any(|&(oi, _)| oi == i);
        Ok(Retokenized {
            tokens,
            carried: (0..old.len()).filter(|&i| analysed[i] && kept(i)).count(),
            lost_analysed: (0..old.len()).filter(|&i| analysed[i] && !kept(i)).count(),
            commit,
        })
    }

    /// `form`-like text as entry would store it (NFC when a normalizer is installed).
    fn normalized(&self, field: &str, s: &str) -> String {
        match self.normalize(field, Value::str(s)) {
            Value::Str(n) => n,
            _ => s.to_string(),
        }
    }
}

/// What one morph of a new analysis becomes: an existing morph that keeps
/// its id and every field the analysis does not name (`form` ref, `x.*`),
/// with only the named fields that differ set, or a new morph.
enum MorphPlan {
    Keep { id: String, sets: Vec<(String, Value)> },
    New { fields: Vec<(String, Value)> },
}

/// The spec's fields as (field, value) pairs: what an analysis says about a
/// morph. An absent gloss or sense is the empty string.
fn spec_fields(m: &MorphSpec, extra: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = vec![
        ("surface".to_string(), m.surface.clone()),
        ("type".to_string(), m.morph_type.clone()),
        ("gloss".to_string(), m.gloss.clone().unwrap_or_default()),
        ("sense".to_string(), m.sense.clone().unwrap_or_default()),
    ];
    out.extend(extra.iter().cloned());
    out
}

/// Pair old and new morphs: longest common subsequence of folded surfaces,
/// then one-for-one by position inside each gap whose two sides are the same
/// length (a corrected surface keeps its morph). Returns, per new index, the
/// old index it keeps.
fn match_morphs(old_keys: &[String], new_keys: &[String]) -> Vec<Option<usize>> {
    let mut out = vec![None; new_keys.len()];
    let mut anchors = lcs_pairs(old_keys, new_keys);
    anchors.push((old_keys.len(), new_keys.len()));
    let (mut oi, mut nj) = (0, 0);
    for (ai, aj) in anchors {
        if ai - oi == aj - nj {
            for k in 0..ai - oi {
                out[nj + k] = Some(oi + k);
            }
        }
        if aj < new_keys.len() {
            out[aj] = Some(ai);
        }
        (oi, nj) = (ai + 1, aj + 1);
    }
    out
}

impl Project {
    /// How `token`'s current analysis becomes `analysis`, read before the txn opens.
    fn plan_analysis(&self, token: &str, analysis: &[MorphSpec], gloss_fields: &[Vec<(String, String)>]) -> Result<Vec<MorphPlan>, String> {
        let old = self.proj.members(token)?;
        let mut old_specs = Vec::with_capacity(old.len());
        for m in &old {
            old_specs.push(self.morph_spec(m)?);
        }
        let old_keys: Vec<String> = old_specs.iter().map(|m| self.form_key(&m.surface)).collect();
        let new_keys: Vec<String> = analysis.iter().map(|m| self.form_key(&m.surface)).collect();
        let matches = match_morphs(&old_keys, &new_keys);
        let mut plan = Vec::with_capacity(analysis.len());
        for (j, (m, extra)) in analysis.iter().zip(gloss_fields).enumerate() {
            let wanted: Vec<(String, String)> = spec_fields(m, extra)
                .into_iter()
                .map(|(f, v)| {
                    let v = self.normalized(&f, &v);
                    (f, v)
                })
                .collect();
            match matches[j] {
                Some(oi) => {
                    let had = &old_specs[oi];
                    let had_langs: Vec<(String, String)> =
                        had.glosses_by_lang.iter().map(|(lang, g)| (format!("{}{lang}", wugs_model::GLOSS_LANG_PREFIX), g.clone())).collect();
                    let had: BTreeMap<String, String> = spec_fields(had, &had_langs).into_iter().collect();
                    let wanted: BTreeMap<String, String> = wanted.into_iter().collect();
                    let value = |m: &BTreeMap<String, String>, f: &str| m.get(f).cloned().unwrap_or_default();
                    let names: std::collections::BTreeSet<&String> = wanted.keys().chain(had.keys()).collect();
                    let sets = names
                        .into_iter()
                        .filter(|f| value(&wanted, f) != value(&had, f))
                        .map(|f| (f.clone(), Value::str(&value(&wanted, f))))
                        .collect();
                    plan.push(MorphPlan::Keep { id: old[oi].clone(), sets });
                }
                None => {
                    let mut fields = vec![("token".to_string(), Value::str(token)), ("pos".to_string(), Value::str(&wugs_model::pos_seq(j)))];
                    fields.extend(wanted.into_iter().filter(|(_, v)| !v.is_empty()).map(|(f, v)| (f, Value::Str(v))));
                    plan.push(MorphPlan::New { fields });
                }
            }
        }
        Ok(plan)
    }

    /// Give a token exactly this analysis, in one txn under the intent
    /// `apply-analysis`. This is the accept-a-suggestion path, and re-editing
    /// a token's breakdown uses it too. Morphs are matched to the token's
    /// current ones (see `match_morphs`): a kept morph keeps its id, its
    /// `form` ref and extension fields, and gets a `set` only for a field the
    /// analysis changes, so a concurrent gloss of an untouched morph on
    /// another device survives. The token's `morphs` list is written once.
    /// Morphs left out are hidden, not deleted. An empty analysis clears the
    /// token. Returns the token's morph ids in order with the commit's findings.
    pub fn apply_analysis(
        &mut self,
        text: &str,
        token: &str,
        analysis: &[MorphSpec],
    ) -> Result<(Vec<String>, CommitResult), String> {
        self.require_in_text(token, "token", text)?;
        let gloss_fields = check_analysis(analysis)?;
        let plan = self.plan_analysis(token, analysis, &gloss_fields)?;
        let mut t = self.intent("apply-analysis").resolving();
        let ids = t.write_analysis(text, token, plan)?;
        let res = t.commit()?;
        Ok((ids, res))
    }

    /// Apply one analysis to every token in the project spelled `form`
    /// (NFC, case-folded) that has no morphs yet, one `apply-analysis` txn
    /// per text. A token that already has an analysis is never touched.
    /// A bulk apply lands on tokens that share the folded form but not the
    /// case («Köpekler» spread over «köpekler»), and validator #1 compares
    /// concatenation case-exactly, so each token takes the analysis with
    /// the first morph's first character matched to its own form. The
    /// token's character is copied, never computed, so what the person
    /// typed is what every surface carries; the Turkic pairs are named
    /// because char folding does not pair them.
    pub fn apply_analysis_to_unanalysed(
        &mut self,
        form: &str,
        analysis: &[MorphSpec],
    ) -> Result<AppliedToUnanalysed, String> {
        if analysis.is_empty() {
            return Err("nothing to apply: the analysis has no morphs".into());
        }
        let gloss_fields = check_analysis(analysis)?;
        let mut out = AppliedToUnanalysed::default();
        let mut by_text: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (text, token, has_morphs) in self.tokens_with_form(form)? {
            if has_morphs {
                out.skipped_analysed += 1;
            } else {
                by_text.entry(text).or_default().push(token);
            }
        }
        for (text, tokens) in by_text {
            let mut plans = Vec::with_capacity(tokens.len());
            for token in &tokens {
                let own = self.proj.field_str(token, "form")?.unwrap_or_default();
                plans.push(self.plan_analysis(token, &match_case(&own, analysis), &gloss_fields)?);
            }
            let mut t = self.intent("apply-analysis");
            for (token, plan) in tokens.iter().zip(plans) {
                t.write_analysis(&text, token, plan)?;
            }
            out.findings.extend(t.commit()?.findings);
            out.applied += tokens.len();
            out.texts += 1;
        }
        Ok(out)
    }

    /// The tokens spelled `form` (NFC, case-folded) that are unanalysed and
    /// visible in a live segment of a live text: (text, token) pairs. Its
    /// length is the N in "apply to N other unanalysed".
    pub fn unanalysed_with_form(&self, form: &str) -> Result<Vec<(String, String)>, String> {
        Ok(self
            .tokens_with_form(form)?
            .into_iter()
            .filter(|(_, _, has_morphs)| !has_morphs)
            .map(|(text, token, _)| (text, token))
            .collect())
    }

    /// For each form, how many unanalysed visible tokens are spelled like it
    /// (NFC, case-folded), in one query: the per-word counts of a view.
    pub fn unanalysed_counts(&self, forms: &[String]) -> Result<Vec<usize>, String> {
        let keys: Vec<String> = forms.iter().map(|f| self.form_key(f)).collect();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (key, _, _, analysed) in self.proj.tokens_by_keys(&keys)? {
            if !analysed {
                *counts.entry(key).or_default() += 1;
            }
        }
        Ok(keys.iter().map(|k| counts.get(k).copied().unwrap_or(0)).collect())
    }

    /// Visible tokens spelled `form` in live segments of live texts, as
    /// (text, token, has visible morphs). One query.
    fn tokens_with_form(&self, form: &str) -> Result<Vec<(String, String, bool)>, String> {
        Ok(self
            .proj
            .tokens_by_keys(&[self.form_key(form)])?
            .into_iter()
            .map(|(_, text, token, analysed)| (text, token, analysed))
            .collect())
    }

    /// Every existing analysis of tokens spelled `form` (NFC, case-folded)
    /// across the project, grouped by identical morph sequence and ranked by
    /// how many tokens use it (ties by morph order, so the list is stable).
    /// Derived on demand from the projection, never stored: accepting one
    /// writes ordinary per-occurrence ops through `apply_analysis`. Unanalysed
    /// tokens, and tokens in deleted segments or texts, do not count.
    pub fn analysis_suggestions(&self, form: &str) -> Result<Vec<Suggestion>, String> {
        let mut counts: BTreeMap<Vec<MorphSpec>, usize> = BTreeMap::new();
        for (_, token, has_morphs) in self.tokens_with_form(form)? {
            if !has_morphs {
                continue;
            }
            let mut morphs = Vec::new();
            for m in self.proj.members(&token)? {
                morphs.push(self.morph_spec(&m)?);
            }
            *counts.entry(morphs).or_default() += 1;
        }
        let mut out: Vec<Suggestion> = counts.into_iter().map(|(morphs, count)| Suggestion { morphs, count }).collect();
        out.sort_by(|a, b| b.count.cmp(&a.count));
        Ok(out)
    }

    /// Lexicon senses for a morph surface: lexemes whose lemma, or one of
    /// whose allomorph forms, equals it (NFC, case-folded, edge morph markers
    /// ignored so `ler` finds a lemma written `-ler`). One match per sense,
    /// lemma matches before allomorph matches.
    pub fn lexicon_lookup(&self, surface: &str) -> Result<Vec<LexiconMatch>, String> {
        let key = match self.normalize("lemma", Value::str(surface)) {
            Value::Str(s) => wugs_model::lexicon_key(&s),
            _ => wugs_model::lexicon_key(surface),
        };
        let mut lexemes = self.proj.by_lookup_key("lexeme", &key)?;
        for frm in self.proj.by_lookup_key("form", &key)? {
            if let Some(lex) = self.proj.field_str(&frm, "lexeme")? {
                if !lexemes.contains(&lex) && self.proj.entity_kind(&lex)?.as_deref() == Some("lexeme") && !self.proj.is_tombstoned(&lex)? {
                    lexemes.push(lex);
                }
            }
        }
        let mut out = Vec::new();
        for lex in lexemes {
            let lemma = self.proj.field_str(&lex, "lemma")?.unwrap_or_default();
            for sense in self.proj.ordered_children("lexeme", &lex, "sense")? {
                out.push(LexiconMatch {
                    lexeme: lex.clone(),
                    gloss: self.proj.field_str(&sense, "gloss")?.unwrap_or_default(),
                    sense,
                    lemma: lemma.clone(),
                });
            }
        }
        Ok(out)
    }

    /// Link a morph to a lexicon sense and take that sense's gloss, in one txn
    /// under `pick-sense`, so a sync never carries the link without the gloss.
    pub fn pick_sense(&mut self, text: &str, morph: &str, sense: &str, gloss: &str) -> Result<CommitResult, String> {
        self.require_in_text(morph, "morph", text)?;
        self.require(sense, "sense")?;
        let mut t = self.intent("pick-sense").resolving();
        t.set("morph", morph, text, "sense", Value::str(sense))?;
        t.set("morph", morph, text, "gloss", Value::str(gloss))?;
        t.commit()
    }

    /// Give a segment or token exactly this list of its own tokens or morphs,
    /// in one txn under `choose-member-list`: the "use the other analysis"
    /// answer to a membership-list conflict, where `members` is the losing
    /// list (`Projection::conflict_loser`). The write names both lists in its
    /// heads, so it settles the conflict on every device.
    pub fn choose_member_list(&mut self, text: &str, parent: &str, members: &[String]) -> Result<CommitResult, String> {
        let (parent_kind, ref_field) = self.member_parent(text, parent)?;
        for member in members {
            if self.proj.field_str(member, ref_field)?.as_deref() != Some(parent) || self.proj.is_tombstoned(member)? {
                return Err(format!("{member} is not a live part of {parent}"));
            }
        }
        let mut t = self.intent("choose-member-list").resolving();
        t.set_members(parent_kind, parent, text, members)?;
        t.commit()
    }

    /// A segment or token of `text`, as (its kind, the ref field its members carry).
    fn member_parent(&self, text: &str, parent: &str) -> Result<(&'static str, &'static str), String> {
        let kinds = match self.proj.entity_kind(parent)?.as_deref() {
            Some("segment") => ("segment", "segment"),
            Some("token") => ("token", "token"),
            _ => return Err(format!("{parent} is not a segment or a token")),
        };
        self.require_in_text(parent, kinds.0, text)?;
        Ok(kinds)
    }

    /// Put a hidden token or morph back into its parent's list (the
    /// "restore" of a hidden edit), in one txn under `restore-member`. It goes
    /// before the first listed sibling with a later `pos`, else last. The
    /// list is written with base = what this device sees, like any edit.
    pub fn restore_member(&mut self, text: &str, parent: &str, member: &str) -> Result<CommitResult, String> {
        let (parent_kind, ref_field) = self.member_parent(text, parent)?;
        if self.proj.field_str(member, ref_field)?.as_deref() != Some(parent) || self.proj.is_tombstoned(member)? {
            return Err(format!("{member} is not a live part of {parent}"));
        }
        let mut list = self.proj.members(parent)?;
        if list.contains(&member.to_string()) {
            return Err(format!("{member} is already shown"));
        }
        let pos = self.proj.field_str(member, "pos")?.unwrap_or_default();
        let mut at = list.len();
        for (i, sibling) in list.iter().enumerate() {
            if self.proj.field_str(sibling, "pos")?.unwrap_or_default() > pos {
                at = i;
                break;
            }
        }
        list.insert(at, member.to_string());
        let mut t = self.intent("restore-member").resolving();
        t.set_members(parent_kind, parent, text, &list)?;
        t.commit()
    }

    /// A morph as a spec. A sense link to a deleted sense is dropped, so an
    /// accepted suggestion never links to one.
    pub fn morph_spec(&self, morph: &str) -> Result<MorphSpec, String> {
        let mut spec = MorphSpec::default();
        for (field, value) in self.proj.fields_of(morph)? {
            let Some(v) = value.as_str().filter(|v| !v.is_empty()) else { continue };
            match field.as_str() {
                "surface" => spec.surface = v.to_string(),
                "type" => spec.morph_type = v.to_string(),
                "gloss" => spec.gloss = Some(v.to_string()),
                "sense" if !self.proj.is_tombstoned(v)? => spec.sense = Some(v.to_string()),
                _ => {
                    if let Some(lang) = field.strip_prefix(wugs_model::GLOSS_LANG_PREFIX) {
                        spec.glosses_by_lang.insert(lang.to_string(), v.to_string());
                    }
                }
            }
        }
        Ok(spec)
    }

    /// What the glossing validators (#1 morph concatenation, #2 gloss present)
    /// say about these tokens as they stand now: the checks a commit would
    /// have run, for a panel that opens a segment glossed earlier. Warn-only,
    /// like every local check. A finding's target is the token (#1) or the
    /// morph (#2).
    pub fn token_findings(&self, tokens: &[String]) -> Result<Vec<Finding>, String> {
        let ops: Vec<wugs_model::Op> = tokens
            .iter()
            .map(|t| wugs_model::Op {
                id: String::new(),
                hlc: String::new(),
                txn: String::new(),
                intent: None,
                actor: String::new(),
                device: String::new(),
                session: None,
                scope: String::new(),
                schema: wugs_model::SCHEMA_VERSION,
                verb: "set".to_string(),
                entity: "token".to_string(),
                target: t.clone(),
                field: None,
                value: Value::Null,
                base: None,
                heads: None,
            })
            .collect();
        let glossing: Vec<&Box<dyn wugs_validate::Validator>> =
            self.validators.iter().filter(|v| matches!(v.name(), "morph-concat" | "gloss-present")).collect();
        let mut out = Vec::new();
        for v in glossing {
            out.extend(v.check(&self.proj, &ops)?);
        }
        Ok(out)
    }

    /// The lookup key of a form as typed: normalized the way entry would
    /// store it, then case-folded.
    pub(crate) fn form_key(&self, form: &str) -> String {
        match self.normalize("form", Value::str(form)) {
            Value::Str(s) => wugs_model::fold_key(&s),
            _ => wugs_model::fold_key(form),
        }
    }
}

/// Validate an analysis and resolve each morph's extra-language field names.
/// The analysis with its first morph's first character taken from `form`,
/// when the two are case variants; otherwise an unchanged copy.
fn match_case(form: &str, analysis: &[MorphSpec]) -> Vec<MorphSpec> {
    let mut out = analysis.to_vec();
    let (Some(f), Some(first)) = (form.chars().next(), out.first_mut()) else {
        return out;
    };
    let Some(m) = first.surface.chars().next() else {
        return out;
    };
    let variants = f.to_lowercase().eq(m.to_lowercase()) || matches!((f, m), ('I', 'ı') | ('ı', 'I') | ('İ', 'i') | ('i', 'İ'));
    if f != m && variants {
        first.surface = f.to_string() + &first.surface[m.len_utf8()..];
    }
    out
}

fn check_analysis(analysis: &[MorphSpec]) -> Result<Vec<Vec<(String, String)>>, String> {
    let mut all = Vec::with_capacity(analysis.len());
    for m in analysis {
        if m.surface.is_empty() || m.morph_type.is_empty() {
            return Err("every morph needs a surface and a type".into());
        }
        let mut fields = Vec::new();
        for (lang, gloss) in &m.glosses_by_lang {
            fields.push((gloss_field(Some(lang))?, gloss.clone()));
        }
        all.push(fields);
    }
    Ok(all)
}

impl crate::Txn<'_> {
    /// Emit a planned analysis and the token's `morphs` list. Returns the ids in order.
    fn write_analysis(&mut self, text: &str, token: &str, plan: Vec<MorphPlan>) -> Result<Vec<String>, String> {
        let mut ids = Vec::with_capacity(plan.len());
        for step in plan {
            match step {
                MorphPlan::Keep { id, sets } => {
                    for (field, value) in sets {
                        self.set("morph", &id, text, &field, value)?;
                    }
                    ids.push(id);
                }
                MorphPlan::New { fields } => {
                    ids.push(self.create("morph", "mor", text, fields.iter().map(|(k, v)| (k.as_str(), v.clone())).collect()));
                }
            }
        }
        self.set_members("token", token, text, &ids)?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcs_matches_in_order_and_prefers_late_repeats() {
        assert_eq!(lcs_pairs(&["a", "b", "c"], &["a", "b", "c"]), [(0, 0), (1, 1), (2, 2)]);
        assert_eq!(lcs_pairs(&["a", "b", "c"], &["a", "x", "c"]), [(0, 0), (2, 2)], "a changed word matches nothing");
        assert_eq!(lcs_pairs(&["a", "c"], &["a", "b", "c"]), [(0, 0), (1, 2)], "insertion");
        assert_eq!(lcs_pairs(&["a", "b", "c"], &["a", "c"]), [(0, 0), (2, 1)], "deletion");
        assert_eq!(lcs_pairs(&["x", "a"], &["a", "a"]), [(1, 1)], "a repeated form matches the later occurrence");
        assert_eq!(lcs_pairs(&["a", "b"], &["b", "a"]), [(0, 1)], "a swap keeps one word, the earlier old one");
        assert!(lcs_pairs::<&str, &str>(&[], &["a"]).is_empty());
        assert!(lcs_pairs::<&str, &str>(&["a"], &[]).is_empty());
    }

    #[test]
    fn morphs_match_by_surface_then_one_for_one_in_equal_gaps() {
        let k = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(match_morphs(&k(&["köpek", "ler"]), &k(&["köpek", "ler", "∅"])), [Some(0), Some(1), None], "an added morph");
        assert_eq!(match_morphs(&k(&["köpek", "ler"]), &k(&["köpek", "lar"])), [Some(0), Some(1)], "a corrected surface keeps its morph");
        assert_eq!(match_morphs(&k(&["a", "b"]), &k(&["c", "d"])), [Some(0), Some(1)], "same count, nothing in common: by position");
        assert_eq!(match_morphs(&k(&["köpek", "ler"]), &k(&["köpekler"])), [None], "a merge is a new morph");
        assert_eq!(match_morphs(&k(&["ab"]), &k(&["a", "b"])), [None, None], "a split is new morphs");
        assert_eq!(match_morphs(&k(&["a", "x", "b"]), &k(&["a", "b"])), [Some(0), Some(2)], "a dropped middle morph");
        assert_eq!(match_morphs(&k(&[]), &k(&["a"])), [None]);
    }
}
