//! Linguistic validators (DATA-MODEL.md §7). Validators run against the
//! projection AFTER a txn's ops are applied (inside a savepoint for ingest),
//! so they see the would-be merged state.
//!
//! Severity contract: `Warn` never blocks anything; `Hold` blocks only
//! incoming sync (quarantine). Local commits always proceed, findings attached.

use std::collections::BTreeSet;

use wugs_model::{looks_like_id, Op, Value};
use wugs_projection::Projection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Hold,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub target: String,
    pub message: String,
}

pub trait Validator {
    fn name(&self) -> &'static str;
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String>;
}

/// The default set (DATA-MODEL §7 #1–#8).
pub fn default_validators() -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(RefExists),
        Box::new(MorphConcat),
        Box::new(GlossPresent),
        Box::new(TierOverlap),
        Box::new(CharsetValid),
        Box::new(DomainAcyclic),
        Box::new(IntervalInSource),
        Box::new(DuplicateLemma),
    ]
}

pub fn run_all(
    validators: &[Box<dyn Validator>],
    p: &Projection,
    txn: &[Op],
) -> Result<Vec<Finding>, String> {
    let mut out = Vec::new();
    for v in validators {
        out.extend(v.check(p, txn)?);
    }
    Ok(out)
}

pub fn has_hold(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Hold)
}

// ---------------------------------------------------------------- #4
/// Referenced IDs exist (Hold); tombstoned references are a Warn.
/// Pending-dep aware: a reference created inside the same txn is fine, and a
/// reference to an entity the projection hasn't seen at all is *not* held
/// (cross-scope and cross-batch refs park as pending-dep instead, §5).
/// What IS held: references the projection knows and knows to be deleted —
/// plus same-kind sanity (a `sense` field must point at a sen_ id).
pub struct RefExists;

impl Validator for RefExists {
    fn name(&self) -> &'static str {
        "ref-exists"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        let created: BTreeSet<&str> = txn
            .iter()
            .filter(|o| o.verb == "create")
            .map(|o| o.target.as_str())
            .collect();
        let check_ref = |owner: &str, val: &Value, findings: &mut Vec<Finding>| -> Result<(), String> {
            let Some(s) = val.as_str() else { return Ok(()) };
            if !looks_like_id(s) || created.contains(s) {
                return Ok(());
            }
            if p.entity_kind(s)?.is_some() && p.is_tombstoned(s)? {
                findings.push(Finding {
                    code: "ref-tombstoned",
                    severity: Severity::Warn,
                    target: owner.to_string(),
                    message: format!("reference to deleted entity {s}"),
                });
            }
            Ok(())
        };
        for op in txn {
            match op.verb.as_str() {
                "create" => {
                    if let Some(fields) = op.value.as_obj() {
                        for v in fields.values() {
                            check_ref(&op.target, v, &mut findings)?;
                        }
                    }
                }
                "set" => check_ref(&op.target, &op.value, &mut findings)?,
                _ => {}
            }
        }
        Ok(findings)
    }
}

// ---------------------------------------------------------------- #1
/// Morph surfaces concatenate to the token form, ignoring the Leipzig
/// separators `- = ~ < >`, whitespace and zero morphs (`∅`, which add no
/// letters), after NFC when a normalizer is installed. Case counts: `Işık` split as `iş` + `ık` is a different word
/// in Turkish, and no case folding is right for every language. Warn.
pub struct MorphConcat;

fn strip_separators(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '-' | '=' | '~' | '<' | '>' | '∅') && !c.is_whitespace())
        .collect()
}

fn affected_tokens(p: &Projection, txn: &[Op]) -> Result<BTreeSet<String>, String> {
    let mut tokens = BTreeSet::new();
    for op in txn {
        if op.entity == "morph" {
            // token ref may be in this txn's value or already projected
            if let Some(t) = op.value.get_str("token") {
                tokens.insert(t.to_string());
            } else if let Some(t) = p.field_str(&op.target, "token")? {
                tokens.insert(t);
            }
        } else if op.entity == "token" {
            tokens.insert(op.target.clone());
        }
    }
    Ok(tokens)
}

impl Validator for MorphConcat {
    fn name(&self) -> &'static str {
        "morph-concat"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        for tok in affected_tokens(p, txn)? {
            let Some(form) = p.field_str(&tok, "form")? else {
                continue;
            };
            let morphs = p.members(&tok)?;
            if morphs.is_empty() {
                continue;
            }
            let mut concat = String::new();
            for m in &morphs {
                concat.push_str(&p.field_str(m, "surface")?.unwrap_or_default());
            }
            if strip_separators(&p.nfc(&concat)) != strip_separators(&p.nfc(&form)) {
                findings.push(Finding {
                    code: "morph-concat",
                    severity: Severity::Warn,
                    target: tok.clone(),
                    message: format!(
                        "morphs '{concat}' do not concatenate to token form '{form}'"
                    ),
                });
            }
        }
        Ok(findings)
    }
}

// ---------------------------------------------------------------- #2
/// Every morph of an affected token carries a gloss or a sense link. Warn.
/// (In this model each morph glosses itself, so "morph count = gloss count"
/// becomes "no ungrossed morph".)
pub struct GlossPresent;

impl Validator for GlossPresent {
    fn name(&self) -> &'static str {
        "gloss-present"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        for tok in affected_tokens(p, txn)? {
            for m in p.members(&tok)? {
                let gloss = p.field_str(&m, "gloss")?.unwrap_or_default();
                let sense = p.field_str(&m, "sense")?.unwrap_or_default();
                if gloss.is_empty() && sense.is_empty() {
                    findings.push(Finding {
                        code: "gloss-missing",
                        severity: Severity::Warn,
                        target: m.clone(),
                        message: "morph has neither gloss nor sense link".to_string(),
                    });
                }
            }
        }
        Ok(findings)
    }
}

// ---------------------------------------------------------------- #3
/// No overlapping intervals within one time-aligned tier of one text. Hold.
pub struct TierOverlap;

impl Validator for TierOverlap {
    fn name(&self) -> &'static str {
        "tier-overlap"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        let mut segments = BTreeSet::new();
        for op in txn {
            if op.entity == "segment" {
                segments.insert(op.target.clone());
            }
        }
        for seg in segments {
            if p.is_tombstoned(&seg)? {
                continue;
            }
            let (Some(start), Some(end)) =
                (p.field_num(&seg, "start")?, p.field_num(&seg, "end")?)
            else {
                continue;
            };
            let Some(text) = p.field_str(&seg, "text")? else {
                continue;
            };
            let tier = p.field_str(&seg, "tier")?.unwrap_or_default();
            for sib in p.children_by_ref("text", &text, "segment")? {
                if sib == seg {
                    continue;
                }
                if p.field_str(&sib, "tier")?.unwrap_or_default() != tier {
                    continue;
                }
                let (Some(s2), Some(e2)) =
                    (p.field_num(&sib, "start")?, p.field_num(&sib, "end")?)
                else {
                    continue;
                };
                if start < e2 && s2 < end {
                    findings.push(Finding {
                        code: "tier-overlap",
                        severity: Severity::Hold,
                        target: seg.clone(),
                        message: format!(
                            "segment [{start}..{end}]ms overlaps sibling {sib} [{s2}..{e2}]ms on the same tier"
                        ),
                    });
                }
            }
        }
        Ok(findings)
    }
}

// ---------------------------------------------------------------- #5
/// Characters of vernacular fields stay inside their writing system's
/// charset. Warn. Checked only when the owning entity carries a `ws` ref
/// whose WritingSystem declares a non-empty `charset`; a project without
/// writing systems is silent.
pub struct CharsetValid;

/// The fields that hold language data, per entity kind.
const VERNACULAR_FIELDS: &[(&str, &[&str])] = &[
    ("token", &["form"]),
    ("morph", &["surface"]),
    ("lexeme", &["lemma", "citation"]),
    ("form", &["form"]),
    ("segment", &["content"]),
];

impl Validator for CharsetValid {
    fn name(&self) -> &'static str {
        "charset-valid"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        for op in txn {
            let Some(fields) = VERNACULAR_FIELDS
                .iter()
                .find(|(kind, _)| *kind == op.entity)
                .map(|(_, f)| *f)
            else {
                continue;
            };
            let Some(ws) = p.field_str(&op.target, "ws")? else {
                continue;
            };
            let Some(charset) = p.field_str(&ws, "charset")? else {
                continue;
            };
            if charset.is_empty() {
                continue;
            }
            for field in fields {
                let Some(text) = p.field_str(&op.target, field)? else {
                    continue;
                };
                // touched in this txn? (create carries it in the value map)
                let touched = op.field.as_deref() == Some(field)
                    || (op.verb == "create" && op.value.get(field).is_some());
                if !touched {
                    continue;
                }
                let bad: BTreeSet<char> = text
                    .chars()
                    .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '=' | '~') && !charset.contains(*c))
                    .collect();
                if !bad.is_empty() {
                    findings.push(Finding {
                        code: "charset",
                        severity: Severity::Warn,
                        target: op.target.clone(),
                        message: format!(
                            "{field} contains characters outside the writing system: {}",
                            bad.iter().collect::<String>()
                        ),
                    });
                }
            }
        }
        Ok(findings)
    }
}

// ---------------------------------------------------------------- #6
/// The semantic-domain tree stays acyclic. Hold: a cycle breaks every
/// domain listing and sync must not import one.
pub struct DomainAcyclic;

impl Validator for DomainAcyclic {
    fn name(&self) -> &'static str {
        "domain-acyclic"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        for op in txn {
            if op.entity != "domain" {
                continue;
            }
            let mut seen = BTreeSet::new();
            let mut cur = op.target.clone();
            loop {
                if !seen.insert(cur.clone()) {
                    findings.push(Finding {
                        code: "domain-cycle",
                        severity: Severity::Hold,
                        target: op.target.clone(),
                        message: format!("semantic-domain parent chain loops at {cur}"),
                    });
                    break;
                }
                match p.field_str(&cur, "parent")? {
                    Some(parent) => cur = parent,
                    None => break,
                }
            }
        }
        Ok(findings)
    }
}

// ---------------------------------------------------------------- #7
/// Segment intervals stay within their source's duration. Warn.
pub struct IntervalInSource;

impl Validator for IntervalInSource {
    fn name(&self) -> &'static str {
        "interval-in-source"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        let mut segments = BTreeSet::new();
        for op in txn {
            if op.entity == "segment" {
                segments.insert(op.target.clone());
            }
        }
        for seg in segments {
            let Some(text) = p.field_str(&seg, "text")? else { continue };
            let Some(source) = p.field_str(&text, "source")? else { continue };
            let Some(duration) = p.field_num(&source, "duration")? else { continue };
            let start = p.field_num(&seg, "start")?;
            let end = p.field_num(&seg, "end")?;
            let low = start.unwrap_or(0.0);
            let high = end.unwrap_or(0.0);
            if low < 0.0 || high > duration {
                findings.push(Finding {
                    code: "interval-out-of-source",
                    severity: Severity::Warn,
                    target: seg.clone(),
                    message: format!(
                        "segment [{low}..{high}]ms outside source duration {duration}ms"
                    ),
                });
            }
        }
        Ok(findings)
    }
}

// ---------------------------------------------------------------- #8
/// A lemma written twice without homograph numbers. Warn: FLEx and print
/// dictionaries need the homograph number to tell the entries apart.
pub struct DuplicateLemma;

impl Validator for DuplicateLemma {
    fn name(&self) -> &'static str {
        "duplicate-lemma"
    }
    fn check(&self, p: &Projection, txn: &[Op]) -> Result<Vec<Finding>, String> {
        let mut findings = Vec::new();
        for op in txn {
            if op.entity != "lexeme" {
                continue;
            }
            let touched = op.verb == "create"
                || matches!(op.field.as_deref(), Some("lemma") | Some("homograph"));
            if !touched {
                continue;
            }
            let Some(lemma) = p.field_str(&op.target, "lemma")? else {
                continue;
            };
            let own_homograph = p.field_num(&op.target, "homograph")?.is_some();
            // indexed lookup: lemma values are stored like refs, so the
            // ref-children query answers "who else has this lemma"
            for other in p.children_by_ref("lemma", &lemma, "lexeme")? {
                if other == op.target {
                    continue;
                }
                if !own_homograph || p.field_num(&other, "homograph")?.is_none() {
                    findings.push(Finding {
                        code: "duplicate-lemma",
                        severity: Severity::Warn,
                        target: op.target.clone(),
                        message: format!(
                            "lemma '{lemma}' also on {other} and at least one of the two has no homograph number"
                        ),
                    });
                    break;
                }
            }
        }
        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wugs_model::{new_id, Clock};
    use wugs_projection::ApplyStats;

    struct Fx {
        p: Projection,
        clock: Clock,
        stats: ApplyStats,
    }

    impl Fx {
        fn new() -> Fx {
            Fx {
                p: Projection::open_memory().unwrap(),
                clock: Clock::new("dvc_T".into()),
                stats: ApplyStats::default(),
            }
        }
        fn create(&mut self, entity: &str, prefix: &str, fields: Vec<(&str, Value)>) -> (String, Op) {
            let id = new_id(prefix);
            let op = Op {
                id: new_id("op"),
                hlc: self.clock.now().encode(),
                txn: new_id("txn"),
                intent: None,
                actor: "prs_T".into(),
                device: "dvc_T".into(),
                session: None,
                scope: "s".into(),
                schema: 1,
                verb: "create".into(),
                entity: entity.into(),
                target: id.clone(),
                field: None,
                value: Value::obj(fields),
                base: None,
                heads: None,
            };
            self.p.apply(&op, &mut self.stats).unwrap();
            (id, op)
        }
    }

    #[test]
    fn morph_concat_detects_mismatch() {
        let mut fx = Fx::new();
        let (tok, _) = fx.create("token", "tok", vec![("form", Value::str("köpekler"))]);
        let (_, m1) = fx.create(
            "morph",
            "mor",
            vec![("token", Value::str(&tok)), ("pos", Value::str("a0")), ("surface", Value::str("köpek"))],
        );
        let (_, m2) = fx.create(
            "morph",
            "mor",
            vec![("token", Value::str(&tok)), ("pos", Value::str("a1")), ("surface", Value::str("lar"))], // wrong vowel
        );
        let txn = vec![m1, m2];
        let f = MorphConcat.check(&fx.p, &txn).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, "morph-concat");

        // fix the surface → passes
        let fix = Op {
            field: Some("surface".into()),
            value: Value::str("ler"),
            verb: "set".into(),
            id: new_id("op"),
            hlc: fx.clock.now().encode(),
            txn: new_id("txn"),
            intent: None,
            actor: "prs_T".into(),
            device: "dvc_T".into(),
            session: None,
            scope: "s".into(),
            schema: 1,
            entity: "morph".into(),
            target: txn[1].target.clone(),
            base: None,
            heads: None,
        };
        fx.p.apply(&fix, &mut fx.stats).unwrap();
        let f = MorphConcat.check(&fx.p, &[fix]).unwrap();
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn zero_morphs_do_not_break_concatenation() {
        let mut fx = Fx::new();
        let (tok, _) = fx.create("token", "tok", vec![("form", Value::str("köpek"))]);
        let morph = |fx: &mut Fx, pos: &str, surface: &str| {
            fx.create(
                "morph",
                "mor",
                vec![("token", Value::str(&tok)), ("pos", Value::str(pos)), ("surface", Value::str(surface))],
            )
            .1
        };
        let ops = vec![morph(&mut fx, "a0", "köpek"), morph(&mut fx, "a1", "∅")];
        let f = MorphConcat.check(&fx.p, &ops).unwrap();
        assert!(f.is_empty(), "a zero morph adds no letters: {f:?}");

        let extra = morph(&mut fx, "a2", "ler");
        let f = MorphConcat.check(&fx.p, &[extra]).unwrap();
        assert_eq!(f.len(), 1, "a real surplus morph still warns");
    }

    #[test]
    fn concatenation_compares_case_exactly() {
        let mut fx = Fx::new();
        let (tok, _) = fx.create("token", "tok", vec![("form", Value::str("Işık"))]);
        let morph = |fx: &mut Fx, pos: &str, surface: &str| {
            fx.create("morph", "mor", vec![("token", Value::str(&tok)), ("pos", Value::str(pos)), ("surface", Value::str(surface))]).1
        };
        let ops = vec![morph(&mut fx, "a0", "Iş"), morph(&mut fx, "a1", "-ık")];
        assert!(MorphConcat.check(&fx.p, &ops).unwrap().is_empty(), "Iş-ık spells Işık: no warning");

        let (tok2, _) = fx.create("token", "tok", vec![("form", Value::str("Işık"))]);
        let (_, op) = fx.create("morph", "mor", vec![("token", Value::str(&tok2)), ("pos", Value::str("a0")), ("surface", Value::str("işık"))]);
        assert_eq!(MorphConcat.check(&fx.p, &[op]).unwrap().len(), 1, "işık is another word than Işık");
    }

    #[test]
    fn tier_overlap_holds() {
        let mut fx = Fx::new();
        let (txt, _) = fx.create("text", "txt", vec![]);
        let (_, s1) = fx.create(
            "segment",
            "seg",
            vec![("text", Value::str(&txt)), ("tier", Value::str("tir_U")),
                 ("start", Value::Num(0.0)), ("end", Value::Num(1500.0))],
        );
        let (_, s2) = fx.create(
            "segment",
            "seg",
            vec![("text", Value::str(&txt)), ("tier", Value::str("tir_U")),
                 ("start", Value::Num(1200.0)), ("end", Value::Num(2000.0))],
        );
        let f = TierOverlap.check(&fx.p, &[s2.clone()]).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Hold);
        let _ = s1;
    }

    #[test]
    fn charset_warns_on_foreign_characters() {
        let mut fx = Fx::new();
        let (ws, _) = fx.create("ws", "ws", vec![("charset", Value::str("abcçdefgğhıijklmnoöprsştuüvyz"))]);
        let (_, op) = fx.create(
            "token",
            "tok",
            vec![("form", Value::str("köpekqx")), ("ws", Value::str(&ws))],
        );
        let f = CharsetValid.check(&fx.p, &[op]).unwrap();
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].code, "charset");
        assert!(f[0].message.contains('q') && f[0].message.contains('x'), "{}", f[0].message);

        // same form with no ws ref: silent
        let (_, op2) = fx.create("token", "tok", vec![("form", Value::str("köpekqx"))]);
        assert!(CharsetValid.check(&fx.p, &[op2]).unwrap().is_empty());
    }

    #[test]
    fn domain_cycle_is_held() {
        let mut fx = Fx::new();
        let (a, _) = fx.create("domain", "dom", vec![("label", Value::str("body"))]);
        let (b, _) = fx.create("domain", "dom", vec![("label", Value::str("head")), ("parent", Value::str(&a))]);
        // close the loop: a.parent = b
        let cycle = Op {
            id: new_id("op"),
            hlc: fx.clock.now().encode(),
            txn: new_id("txn"),
            intent: None,
            actor: "prs_T".into(),
            device: "dvc_T".into(),
            session: None,
            scope: "s".into(),
            schema: 1,
            verb: "set".into(),
            entity: "domain".into(),
            target: a.clone(),
            field: Some("parent".into()),
            value: Value::str(&b),
            base: None,
            heads: None,
        };
        // before the loop closes, a chain through b is fine
        let (c, cop) = fx.create("domain", "dom", vec![("parent", Value::str(&b))]);
        let _ = c;
        assert!(DomainAcyclic.check(&fx.p, &[cop]).unwrap().is_empty(), "straight chain passes");

        fx.p.apply(&cycle, &mut fx.stats).unwrap();
        let f = DomainAcyclic.check(&fx.p, &[cycle]).unwrap();
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].severity, Severity::Hold);
    }

    #[test]
    fn interval_outside_source_duration_warns() {
        let mut fx = Fx::new();
        let (src, _) = fx.create("source", "src", vec![("duration", Value::Num(60_000.0))]);
        let (txt, _) = fx.create("text", "txt", vec![("source", Value::str(&src))]);
        let (_, inside) = fx.create(
            "segment",
            "seg",
            vec![("text", Value::str(&txt)), ("start", Value::Num(0.0)), ("end", Value::Num(1500.0))],
        );
        assert!(IntervalInSource.check(&fx.p, &[inside]).unwrap().is_empty());
        let (_, outside) = fx.create(
            "segment",
            "seg",
            vec![("text", Value::str(&txt)), ("start", Value::Num(59_000.0)), ("end", Value::Num(61_000.0))],
        );
        let f = IntervalInSource.check(&fx.p, &[outside]).unwrap();
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].code, "interval-out-of-source");
    }

    #[test]
    fn duplicate_lemma_without_homograph_warns() {
        let mut fx = Fx::new();
        let (_, first) = fx.create("lexeme", "lex", vec![("lemma", Value::str("yüz"))]);
        assert!(DuplicateLemma.check(&fx.p, &[first]).unwrap().is_empty(), "first use is fine");
        let (_, second) = fx.create("lexeme", "lex", vec![("lemma", Value::str("yüz"))]);
        let f = DuplicateLemma.check(&fx.p, &[second]).unwrap();
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].code, "duplicate-lemma");

        // both numbered: fine
        let (_, n1) = fx.create("lexeme", "lex", vec![("lemma", Value::str("dil")), ("homograph", Value::Num(1.0))]);
        let _ = n1;
        let (_, n2) = fx.create("lexeme", "lex", vec![("lemma", Value::str("dil")), ("homograph", Value::Num(2.0))]);
        assert!(DuplicateLemma.check(&fx.p, &[n2]).unwrap().is_empty(), "numbered homographs are fine");
    }

    #[test]
    fn gloss_present_warns_then_clears() {
        let mut fx = Fx::new();
        let (tok, _) = fx.create("token", "tok", vec![("form", Value::str("wug"))]);
        let (_mor, mop) = fx.create(
            "morph",
            "mor",
            vec![("token", Value::str(&tok)), ("pos", Value::str("a0")), ("surface", Value::str("wug"))],
        );
        let f = GlossPresent.check(&fx.p, &[mop.clone()]).unwrap();
        assert_eq!(f.len(), 1);
    }
}
