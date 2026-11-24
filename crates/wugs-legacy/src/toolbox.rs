//! SIL Toolbox / Shoebox lexicons in MDF (Multi-Dictionary Formatter)
//! markup.
//!
//! Carried semantically (MDF meaning, see `MDF`): `\lx` lexeme, `\hm`
//! homograph number, `\lc` citation form, `\sn` sense number, `\ps` part of
//! speech, `\ge` English gloss, `\de` English definition, `\nt` note (a
//! sense's `notes`), `\va` variant form,
//! `\a` allomorph, `\xv` example sentence and the `\xe` (English
//! translation) right after it.
//! Every other line, and any repeat of a carried marker within the same
//! owner, is kept verbatim in its original position and counted in
//! `ToolboxReport::markers_kept_verbatim`.
//!
//! Each lexeme, and each sense opened by `\sn`, stores its block of lines as
//! `x.toolbox.fields`: `[marker, "value"]` for a verbatim line, `[marker,
//! {"of": id}]` for a carried line whose value export reads back from that
//! entity, so edits made in wugs win and lines keep their place.

use std::collections::HashSet;

use wugs_core::{Project, SCOPE_LEXICON};
use wugs_model::{pos_seq, Value};

use crate::common::*;
use crate::sfm::{self, Field, SfmFile};

pub const RECORD_MARKER: &str = "lx";
pub const DEFAULT_HEADER: &str = "\\_sh v3.0  400  MDF 4.0";

const F_FIELDS: &str = "x.toolbox.fields";
const F_RECORD: &str = "x.toolbox.record";
const F_SN: &str = "x.toolbox.sn";
const F_PS: &str = "x.toolbox.ps";
const F_HEADER: &str = "x.toolbox.header";
/// `\hm` as written (`01`), exported while it still parses to `homograph`.
const F_HM_TEXT: &str = "x.toolbox.hm";

#[derive(Clone, Copy, PartialEq)]
enum Carried {
    Lemma,
    Homograph,
    Citation,
    SenseNo,
    Pos,
    Gloss,
    Definition,
    Note,
    Variant,
    Allomorph,
    Example,
    ExampleTranslation,
    Phonetic,
    Synonym,
    Antonym,
    Compare,
    Etymology,
    Borrowed,
}

/// The MDF subset wugs carries: marker, meaning, and the field it lands in.
const MDF: &[(&str, Carried, &str)] = &[
    ("lx", Carried::Lemma, "lemma"),
    ("hm", Carried::Homograph, "homograph"),
    ("lc", Carried::Citation, "citation"),
    ("sn", Carried::SenseNo, F_SN),
    ("ps", Carried::Pos, F_PS),
    ("ge", Carried::Gloss, "gloss"),
    ("de", Carried::Definition, "definition"),
    ("nt", Carried::Note, "notes"),
    ("va", Carried::Variant, "form"),
    ("a", Carried::Allomorph, "form"),
    ("xv", Carried::Example, "sentence"),
    ("xe", Carried::ExampleTranslation, "translation"),
    ("ph", Carried::Phonetic, "form"),
    ("sy", Carried::Synonym, "relation"),
    ("an", Carried::Antonym, "relation"),
    ("cf", Carried::Compare, "relation"),
    ("et", Carried::Etymology, "etymology"),
    ("bw", Carried::Borrowed, "borrowed-from"),
];

/// A relation line's type, as wugs names it.
fn relation_type(c: Carried) -> Option<&'static str> {
    match c {
        Carried::Synonym => Some("synonym"),
        Carried::Antonym => Some("antonym"),
        Carried::Compare => Some("compare"),
        _ => None,
    }
}

/// `x.toolbox.written` on a relation: each entry's line as written, by lexeme id.
const F_WRITTEN: &str = "x.toolbox.written";

fn carried(marker: &str) -> Option<(Carried, &'static str)> {
    MDF.iter().find(|(m, _, _)| *m == marker).map(|(_, c, f)| (*c, *f))
}

fn marker_of(c: Carried) -> &'static str {
    MDF.iter().find(|(_, k, _)| *k == c).unwrap().0
}

#[derive(Debug, Default)]
pub struct ToolboxReport {
    pub entries_created: usize,
    /// Records already imported (same headword and homograph number as a
    /// record imported before): skipped, nothing of them is written.
    pub entries_skipped: usize,
    /// Records skipped because a lexeme made or renamed in wugs has their
    /// headword and homograph number. This is how our own export of such
    /// lexemes re-imports onto them, but a record from elsewhere that only
    /// shares the headword matches too, and its content was NOT imported.
    /// When this is not zero and the file is not our own export, the
    /// records need merging by hand.
    pub entries_skipped_matched_by_headword: usize,
    pub senses_created: usize,
    pub variants_created: usize,
    pub examples_created: usize,
    pub pronunciations_created: usize,
    /// `\sy`, `\an` and `\cf` lines that name an entry in the file or the
    /// project; a pair written on both entries is one relation. A line that
    /// names no entry stays verbatim.
    pub relations_created: usize,
    /// Lines kept only verbatim, by marker.
    pub markers_kept_verbatim: Counts,
    /// Records sharing headword and homograph number with an earlier record.
    pub duplicate_headwords: usize,
    pub commits: usize,
    pub findings: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Lexeme,
    Sense(usize),
    Variant(usize),
    Example(usize),
    Pronunciation(usize),
    Relation(usize),
}

enum Item {
    Verbatim(String, String),
    Slot(String, Owner),
}

#[derive(Default)]
struct SenseIn {
    sn: Option<String>,
    pos: Option<String>,
    gloss: Option<String>,
    definition: Option<String>,
    notes: Option<String>,
    block: Option<Vec<Item>>,
}

struct EntryIn {
    origin: String,
    lemma: String,
    homograph: Option<(f64, String)>,
    citation: Option<String>,
    /// `\et` and `\bw`, the first of each.
    etymology: Option<String>,
    borrowed: Option<String>,
    block: Vec<Item>,
    senses: Vec<SenseIn>,
    /// Other forms in record order: (surface, `form` kind: `variant` for `\va`, `allomorph` for `\a`).
    variants: Vec<(String, &'static str)>,
    /// Example sentences in record order: (sense index, sentence, translation).
    examples: Vec<(usize, String, Option<String>)>,
    /// `\ph` lines in record order.
    pronunciations: Vec<String>,
    /// `\sy` / `\an` / `\cf` lines: (type, the headword as written), and once
    /// resolved, the relation each line is (None: kept verbatim).
    relations: Vec<(&'static str, String, Option<String>)>,
    /// The lexeme id this entry gets when it is created.
    id: String,
}

/// Import a Toolbox MDF lexicon. One txn per entry, lexicon scope; the
/// database header goes on a document source in its own txn. Entries already
/// in the project (same headword and homograph number, or our own export of
/// them) are skipped, not updated; the report counts the two cases apart.
/// The whole file is parsed before anything is written, so invalid input
/// changes nothing.
pub fn import_toolbox(project: &mut Project, input: &str) -> Result<ToolboxReport, ImportError> {
    let mut report = ToolboxReport::default();
    let file = sfm::parse(input, RECORD_MARKER)?;
    if file.records.is_empty() && file.header.is_empty() {
        return Err(format_err("no SFM records"));
    }
    let mut occ = Occurrences::default();
    let mut entries = Vec::new();
    for rec in &file.records {
        entries.push(parse_entry(rec, &mut occ, &mut report)?);
    }

    // A header our export would write anyway (the default, for a project
    // without one) is not a source of its own: re-importing our export of a
    // headerless file must not invent one.
    let header = file.header.join("\n");
    let current = newest_header(project)?.unwrap_or_else(|| DEFAULT_HEADER.to_string());
    if !header.is_empty() && header != current {
        let key = format!("toolbox-header:{:016x}", fnv1a(&header));
        if !origin_index(project, "source")?.contains_key(&key) {
            let mut t = project.intent("import-toolbox");
            t.create_scope_root(
                "source",
                "src",
                vec![
                    ("kind", Value::str("document")),
                    (F_ORIGIN, Value::str(&key)),
                    (F_HEADER, Value::str(&header)),
                ],
            );
            t.commit()?;
            report.commits += 1;
        }
    }

    let origins: HashSet<String> = origin_index(project, "lexeme")?.into_keys().collect();
    let headwords = headword_keys(project)?;
    let mut fresh: Vec<(usize, EntryIn)> = Vec::new();
    for (i, e) in entries.into_iter().enumerate() {
        if origins.contains(&e.origin) {
            report.entries_skipped += 1;
            continue;
        }
        if headwords.contains(&e.origin) {
            report.entries_skipped_matched_by_headword += 1;
            continue;
        }
        fresh.push((i, e));
    }
    let pending = resolve_relations(project, &mut fresh, &mut report)?;
    for (i, e) in fresh {
        let res = create_entry(project, e, i, &mut report)?;
        report.entries_created += 1;
        report.commits += 1;
        report.findings += res;
    }
    // The relations go in once both their entries are.
    if !pending.is_empty() {
        let mut t = project.intent("import-toolbox");
        for r in &pending {
            let written = Value::Obj(r.written.iter().map(|(k, v)| (k.clone(), Value::str(v))).collect());
            t.create_with_id(
                "lexical-relation",
                &r.id,
                SCOPE_LEXICON,
                vec![("from", Value::str(&r.from)), ("to", Value::str(&r.to)), ("relation", Value::str(r.relation)), (F_WRITTEN, written)],
            );
            report.relations_created += 1;
        }
        report.findings += t.commit()?.findings.len();
        report.commits += 1;
    }
    Ok(report)
}

struct PendingRelation {
    id: String,
    from: String,
    to: String,
    relation: &'static str,
    written: std::collections::BTreeMap<String, String>,
}

/// A headword as written in a relation line: the lemma, and a homograph
/// number after it (`tela2`, `tela 2`, `tela₂`) when one is written.
fn split_homograph(written: &str) -> (String, Option<u32>) {
    let sub = |c: char| ('₀'..='₉').contains(&c);
    let digits: String = written.chars().rev().take_while(|c| c.is_ascii_digit() || sub(*c)).collect::<Vec<_>>().into_iter().rev().collect();
    if digits.is_empty() || digits.len() == written.len() {
        return (written.to_string(), None);
    }
    let n = digits.chars().map(|c| if sub(c) { char::from(b'0' + (c as u32 - '₀' as u32) as u8) } else { c }).collect::<String>().parse().ok();
    (written[..written.len() - digits.len()].trim_end().to_string(), n)
}

/// Point each relation line at the entry it names, among this file's new
/// records and the project's entries; a symmetric pair written on both
/// entries becomes one relation, and one the project already holds is reused.
fn resolve_relations(project: &Project, fresh: &mut [(usize, EntryIn)], report: &mut ToolboxReport) -> Result<Vec<PendingRelation>, String> {
    if fresh.iter().all(|(_, e)| e.relations.is_empty()) {
        return Ok(Vec::new());
    }
    // Every headword the lines may name: (lemma, homograph) → lexeme ids.
    let mut by_lemma: std::collections::HashMap<String, Vec<(Option<u32>, String)>> = std::collections::HashMap::new();
    for (_, e) in fresh.iter() {
        by_lemma.entry(project.proj.nfc(&e.lemma)).or_default().push((e.homograph.as_ref().map(|(h, _)| *h as u32), e.id.clone()));
    }
    for lex in project.proj.entities_of_kind("lexeme")? {
        if project.proj.is_tombstoned(&lex)? {
            continue;
        }
        let lemma = project.proj.nfc(&opt_str(project, &lex, "lemma")?.unwrap_or_default());
        let hm = project.proj.field_num(&lex, "homograph")?.map(|h| h as u32);
        by_lemma.entry(lemma).or_default().push((hm, lex));
    }
    let find = |written: &str| -> Option<String> {
        let whole = project.proj.nfc(written);
        if let Some([(_, id)]) = by_lemma.get(&whole).map(Vec::as_slice) {
            return Some(id.clone());
        }
        let (lemma, n) = split_homograph(&whole);
        let all = by_lemma.get(&lemma)?;
        let picked: Vec<&String> = all.iter().filter(|(h, _)| n.is_none() || *h == n).map(|(_, id)| id).collect();
        (picked.len() == 1).then(|| picked[0].clone())
    };
    let mut pending: Vec<PendingRelation> = Vec::new();
    for (_, e) in fresh.iter_mut() {
        let here = e.id.clone();
        for (relation, written, slot) in e.relations.iter_mut() {
            let Some(other) = find(written).filter(|o| *o != here) else { continue };
            let symmetric = wugs_core::relations::symmetric(relation);
            let twin = pending.iter_mut().find(|r| r.relation == *relation && ((r.from == here && r.to == other) || (symmetric && r.from == other && r.to == here)));
            if let Some(r) = twin {
                r.written.insert(here.clone(), written.clone());
                *slot = Some(r.id.clone());
                continue;
            }
            let existing = project
                .relations(&other)
                .unwrap_or_default()
                .into_iter()
                .find(|r| r.other == here && r.relation == *relation && symmetric);
            if let Some(r) = existing {
                *slot = Some(r.id);
                continue;
            }
            let id = wugs_model::new_id("rel");
            *slot = Some(id.clone());
            pending.push(PendingRelation { id, from: here.clone(), to: other, relation, written: [(here.clone(), written.clone())].into_iter().collect() });
        }
        // Lines that name no entry stay as written.
        let unresolved: Vec<usize> = e.relations.iter().enumerate().filter(|(_, r)| r.2.is_none()).map(|(i, _)| i).collect();
        if !unresolved.is_empty() {
            let back = |items: &mut Vec<Item>, report: &mut ToolboxReport| {
                for it in items.iter_mut() {
                    if let Item::Slot(m, Owner::Relation(i)) = it {
                        if unresolved.contains(i) {
                            report.markers_kept_verbatim.add(m);
                            *it = Item::Verbatim(m.clone(), e.relations[*i].1.clone());
                        }
                    }
                }
            };
            let mut block = std::mem::take(&mut e.block);
            back(&mut block, report);
            e.block = block;
            for s in 0..e.senses.len() {
                if let Some(mut b) = e.senses[s].block.take() {
                    back(&mut b, report);
                    e.senses[s].block = Some(b);
                }
            }
        }
    }
    Ok(pending)
}

fn parse_entry(rec: &[Field], occ: &mut Occurrences, report: &mut ToolboxReport) -> Result<EntryIn, ImportError> {
    let lemma = rec[0].value.clone();
    if lemma.is_empty() {
        return Err(format_err("\\lx with no headword"));
    }
    let has_sn = rec.iter().any(|f| f.marker == "sn");
    let mut e = EntryIn {
        origin: String::new(),
        lemma: lemma.clone(),
        homograph: None,
        citation: None,
        etymology: None,
        borrowed: None,
        block: Vec::new(),
        senses: Vec::new(),
        variants: Vec::new(),
        examples: Vec::new(),
        pronunciations: Vec::new(),
        relations: Vec::new(),
        id: wugs_model::new_id("lex"),
    };
    let mut current: Option<usize> = None;
    // Lines that belong to the sense the next \sn opens.
    let mut upcoming = SenseIn::default();

    for (i, f) in rec.iter().enumerate() {
        let next_is_sn = rec.get(i + 1).is_some_and(|n| n.marker == "sn");
        let item = match carried(&f.marker) {
            Some((Carried::Lemma, _)) if i == 0 => Some(Owner::Lexeme),
            Some((Carried::Homograph, _)) if e.homograph.is_none() && !f.value.is_empty() => {
                let n: u32 = f.value.parse().map_err(|_| {
                    format_err(format!("\\lx {lemma}: \\hm {:?} is not a homograph number", f.value))
                })?;
                e.homograph = Some((n as f64, f.value.clone()));
                Some(Owner::Lexeme)
            }
            Some((Carried::Citation, _)) if e.citation.is_none() => {
                e.citation = Some(f.value.clone());
                Some(Owner::Lexeme)
            }
            Some((Carried::Etymology, _)) if e.etymology.is_none() && !f.value.trim().is_empty() => {
                e.etymology = Some(f.value.clone());
                Some(Owner::Lexeme)
            }
            Some((Carried::Borrowed, _)) if e.borrowed.is_none() && !f.value.trim().is_empty() => {
                e.borrowed = Some(f.value.trim().to_string());
                Some(Owner::Lexeme)
            }
            Some((Carried::SenseNo, _)) => {
                e.senses.push(SenseIn {
                    sn: Some(f.value.clone()),
                    block: Some(Vec::new()),
                    ..std::mem::take(&mut upcoming)
                });
                current = Some(e.senses.len() - 1);
                Some(Owner::Sense(e.senses.len() - 1))
            }
            Some((c @ (Carried::Pos | Carried::Gloss | Carried::Definition | Carried::Note), _)) => {
                // MDF: a \ps directly above \sn opens that sense; anything
                // above the first \sn belongs to the first sense.
                let s = match (c, current) {
                    (Carried::Pos, _) if next_is_sn => e.senses.len(),
                    (_, Some(s)) => s,
                    (_, None) => 0,
                };
                if !has_sn && e.senses.is_empty() {
                    e.senses.push(SenseIn::default());
                }
                fn slot(sen: &mut SenseIn, c: Carried) -> &mut Option<String> {
                    match c {
                        Carried::Pos => &mut sen.pos,
                        Carried::Gloss => &mut sen.gloss,
                        Carried::Note => &mut sen.notes,
                        _ => &mut sen.definition,
                    }
                }
                let target = match e.senses.get_mut(s) {
                    Some(sen) => slot(sen, c),
                    None => slot(&mut upcoming, c),
                };
                if target.is_none() {
                    *target = Some(f.value.clone());
                    Some(Owner::Sense(s))
                } else {
                    None
                }
            }
            // An example belongs to the sense it is written under (the first
            // when there is no \sn); one above the first \sn stays verbatim.
            Some((Carried::Example, _)) if !f.value.is_empty() => {
                if !has_sn && e.senses.is_empty() {
                    e.senses.push(SenseIn::default());
                }
                match current.or((!has_sn).then_some(0)) {
                    Some(s) => {
                        e.examples.push((s, f.value.clone(), None));
                        Some(Owner::Example(e.examples.len() - 1))
                    }
                    None => None,
                }
            }
            // A translation goes with the example just above it, once.
            Some((Carried::ExampleTranslation, _)) if !f.value.is_empty() && rec.get(i.wrapping_sub(1)).is_some_and(|p| p.marker == "xv") => {
                match e.examples.last_mut() {
                    Some(x) if x.2.is_none() => {
                        x.2 = Some(f.value.clone());
                        Some(Owner::Example(e.examples.len() - 1))
                    }
                    _ => None,
                }
            }
            Some((Carried::Phonetic, _)) if !f.value.is_empty() => {
                e.pronunciations.push(f.value.clone());
                Some(Owner::Pronunciation(e.pronunciations.len() - 1))
            }
            // Resolved once every record is read; a line naming no entry goes back to verbatim.
            Some((c @ (Carried::Synonym | Carried::Antonym | Carried::Compare), _)) if !f.value.trim().is_empty() => {
                e.relations.push((relation_type(c).unwrap_or("compare"), f.value.trim().to_string(), None));
                Some(Owner::Relation(e.relations.len() - 1))
            }
            Some((c @ (Carried::Variant | Carried::Allomorph), _)) if !f.value.is_empty() => {
                e.variants.push((f.value.clone(), if c == Carried::Allomorph { "allomorph" } else { "variant" }));
                Some(Owner::Variant(e.variants.len() - 1))
            }
            _ => None,
        };
        let entry = match item {
            Some(owner) => Item::Slot(f.marker.clone(), owner),
            None => {
                report.markers_kept_verbatim.add(&f.marker);
                Item::Verbatim(f.marker.clone(), f.value.clone())
            }
        };
        match current.and_then(|s| e.senses[s].block.as_mut()) {
            Some(b) => b.push(entry),
            None => e.block.push(entry),
        }
    }

    // The origin is headword plus homograph number, by design: adding `\hm`
    // to an existing headword in Toolbox makes the record a new entry on the
    // next import, and a lexeme deleted in wugs comes back when the original
    // file is imported again (both as in wugs-flex).
    let base = format!(
        "toolbox:{lemma}|{}",
        e.homograph.as_ref().map(|(h, _)| (*h as u32).to_string()).unwrap_or_default()
    );
    e.origin = occ.key(base.clone());
    if e.origin != base {
        report.duplicate_headwords += 1;
    }
    Ok(e)
}

fn layout(items: &[Item], ids: &dyn Fn(Owner) -> String) -> Value {
    Value::Arr(
        items
            .iter()
            .map(|it| match it {
                Item::Verbatim(m, v) => Value::Arr(vec![Value::str(m), Value::str(v)]),
                Item::Slot(m, o) => Value::Arr(vec![Value::str(m), Value::obj(vec![("of", Value::str(&ids(*o)))])]),
            })
            .collect(),
    )
}

fn create_entry(project: &mut Project, e: EntryIn, record: usize, report: &mut ToolboxReport) -> Result<usize, String> {
    let mut t = project.intent("import-toolbox");
    let mut fields = vec![
        ("lemma", Value::str(&e.lemma)),
        (F_ORIGIN, Value::str(&e.origin)),
        (F_RECORD, Value::Num(record as f64)),
    ];
    if let Some((h, written)) = &e.homograph {
        fields.push(("homograph", Value::Num(*h)));
        fields.push((F_HM_TEXT, Value::str(written)));
    }
    if let Some(c) = &e.citation {
        fields.push(("citation", Value::str(c)));
    }
    if let Some(x) = &e.etymology {
        fields.push(("etymology", Value::str(x)));
    }
    if let Some(x) = &e.borrowed {
        fields.push(("borrowed-from", Value::str(x)));
    }
    let lex = e.id.clone();
    t.create_with_id("lexeme", &lex, SCOPE_LEXICON, fields);
    let mut pronunciation_ids = Vec::new();
    for (i, form) in e.pronunciations.iter().enumerate() {
        pronunciation_ids.push(t.create("pronunciation", "prn", SCOPE_LEXICON, vec![("lexeme", Value::str(&lex)), ("pos", Value::str(&pos_seq(i))), ("form", Value::str(form))]));
        report.pronunciations_created += 1;
    }

    let mut sense_ids = Vec::new();
    for (i, s) in e.senses.iter().enumerate() {
        let mut fields = vec![("lexeme", Value::str(&lex)), ("pos", Value::str(&pos_seq(i)))];
        for (k, v) in [(F_SN, &s.sn), (F_PS, &s.pos), ("gloss", &s.gloss), ("definition", &s.definition), ("notes", &s.notes)] {
            if let Some(v) = v {
                fields.push((k, Value::str(v)));
            }
        }
        sense_ids.push(t.create("sense", "sen", SCOPE_LEXICON, fields));
        report.senses_created += 1;
    }
    let mut variant_ids = Vec::new();
    for (i, (v, kind)) in e.variants.iter().enumerate() {
        variant_ids.push(t.create(
            "form",
            "frm",
            SCOPE_LEXICON,
            vec![
                ("lexeme", Value::str(&lex)),
                ("kind", Value::str(kind)),
                ("form", Value::str(v)),
                ("pos", Value::str(&pos_seq(i))),
            ],
        ));
        report.variants_created += 1;
    }

    let mut example_ids = Vec::new();
    for (i, (sense, sentence, translation)) in e.examples.iter().enumerate() {
        let mut fields = vec![("sense", Value::str(&sense_ids[*sense])), ("pos", Value::str(&pos_seq(i))), ("sentence", Value::str(sentence))];
        if let Some(t) = translation {
            fields.push(("translation", Value::str(t)));
        }
        example_ids.push(t.create("example", "exm", SCOPE_LEXICON, fields));
        report.examples_created += 1;
    }

    let ids = |o: Owner| match o {
        Owner::Lexeme => lex.clone(),
        Owner::Sense(i) => sense_ids[i].clone(),
        Owner::Variant(i) => variant_ids[i].clone(),
        Owner::Example(i) => example_ids[i].clone(),
        Owner::Pronunciation(i) => pronunciation_ids[i].clone(),
        Owner::Relation(i) => e.relations[i].2.clone().unwrap_or_default(),
    };
    t.set("lexeme", &lex, SCOPE_LEXICON, F_FIELDS, layout(&e.block, &ids))?;
    for (s, id) in e.senses.iter().zip(&sense_ids) {
        if let Some(b) = &s.block {
            t.set("sense", id, SCOPE_LEXICON, F_FIELDS, layout(b, &ids))?;
        }
    }
    Ok(t.commit()?.findings.len())
}

/// The key each live lexeme's current headword and homograph would get in
/// our own export (in export order), so born-digital and edited entries
/// re-import onto themselves.
fn headword_keys(project: &Project) -> Result<HashSet<String>, String> {
    let mut keys = HashSet::new();
    let mut occ = Occurrences::default();
    for lex in export_order(project)? {
        keys.insert(occ.key(export_key(project, &lex)?));
    }
    Ok(keys)
}

fn export_key(project: &Project, lex: &str) -> Result<String, String> {
    let lemma = opt_str(project, lex, "lemma")?.unwrap_or_default();
    let hm = project.proj.field_num(lex, "homograph")?.map(|h| format!("{}", h as u32)).unwrap_or_default();
    Ok(format!("toolbox:{lemma}|{hm}"))
}

fn export_order(project: &Project) -> Result<Vec<String>, String> {
    let p = &project.proj;
    let mut v = Vec::new();
    for id in p.entities_of_kind("lexeme")? {
        let rec = p.field_num(&id, F_RECORD)?.unwrap_or(f64::MAX);
        let lemma = opt_str(project, &id, "lemma")?.unwrap_or_default();
        let hm = p.field_num(&id, "homograph")?.unwrap_or(0.0);
        v.push((rec, lemma, hm, id));
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)).then_with(|| a.3.cmp(&b.3)));
    Ok(v.into_iter().map(|x| x.3).collect())
}

// ------------------------------------------------------------------ export

/// Render the project's whole lexicon as a Toolbox MDF database. Carried
/// lines take their value from the entities now; verbatim lines replay in
/// their original place. Reads the projection only.
pub fn export_toolbox(project: &Project) -> Result<String, String> {
    let header = newest_header(project)?.unwrap_or_else(|| DEFAULT_HEADER.to_string());
    let mut file = SfmFile { header: header.lines().map(str::to_string).collect(), records: Vec::new() };
    for lex in export_order(project)? {
        file.records.push(export_entry(project, &lex)?);
    }
    Ok(sfm::write(&file))
}

fn newest_header(project: &Project) -> Result<Option<String>, String> {
    let mut found = None;
    for src in project.proj.entities_of_kind("source")? {
        if let Some(h) = opt_str(project, &src, F_HEADER)? {
            found = Some(h);
        }
    }
    Ok(found)
}

struct Writer<'a> {
    project: &'a Project,
    /// The entry being written.
    lex: String,
    out: Vec<Field>,
    /// (owner, field) pairs that have a line in some block of this entry.
    placed: HashSet<(String, &'static str)>,
    emitted: HashSet<(String, &'static str)>,
}

fn block_items(block: &Option<Value>) -> Vec<(String, Result<String, String>)> {
    let Some(Value::Arr(items)) = block else { return Vec::new() };
    items
        .iter()
        .filter_map(|it| {
            let Value::Arr(pair) = it else { return None };
            let m = pair.first()?.as_str()?.to_string();
            match pair.get(1)? {
                Value::Str(v) => Some((m, Err(v.clone()))),
                v => Some((m, Ok(v.get_str("of")?.to_string()))),
            }
        })
        .collect()
}

impl Writer<'_> {
    fn value(&self, owner: &str, c: Carried, field: &str) -> Result<Option<String>, String> {
        let p = &self.project.proj;
        if p.entity_kind(owner)?.is_none() || p.is_tombstoned(owner)? {
            return Ok(None);
        }
        Ok(match c {
            Carried::Homograph => match (p.field_num(owner, field)?, opt_str(self.project, owner, F_HM_TEXT)?) {
                (Some(h), Some(w)) if w.parse::<u32>().ok() == Some(h as u32) => Some(w),
                (h, _) => h.map(|h| format!("{}", h as u32)),
            },
            // A cleared translation writes no empty \xe line.
            Carried::ExampleTranslation => opt_str(self.project, owner, field)?.filter(|t| !t.is_empty()),
            Carried::Phonetic => opt_str(self.project, owner, field)?.filter(|t| !t.is_empty()),
            // An emptied etymology or source language writes no line.
            Carried::Etymology | Carried::Borrowed => opt_str(self.project, owner, field)?.filter(|t| !t.trim().is_empty()),
            Carried::Synonym | Carried::Antonym | Carried::Compare => self.related(owner)?,
            _ => opt_str(self.project, owner, field)?,
        })
    }

    /// A relation line's value: the other end's headword, as this entry's
    /// line wrote it while that still names it, else its lemma with its
    /// homograph number.
    fn related(&self, relation: &str) -> Result<Option<String>, String> {
        let p = &self.project.proj;
        let entry_of = |id: &str| -> Result<Option<String>, String> {
            if p.entity_kind(id)?.is_none() || p.is_tombstoned(id)? {
                return Ok(None);
            }
            Ok(match p.entity_kind(id)?.as_deref() {
                Some("sense") => opt_str(self.project, id, "lexeme")?,
                _ => Some(id.to_string()),
            })
        };
        let (Some(from), Some(to)) = (opt_str(self.project, relation, "from")?, opt_str(self.project, relation, "to")?) else { return Ok(None) };
        let (Some(a), Some(b)) = (entry_of(&from)?, entry_of(&to)?) else { return Ok(None) };
        let other = if a == self.lex { b } else { a };
        if p.is_tombstoned(&other)? {
            return Ok(None);
        }
        let lemma = opt_str(self.project, &other, "lemma")?.unwrap_or_default();
        let hm = p.field_num(&other, "homograph")?.map(|h| h as u32);
        let written = opt_value(self.project, relation, F_WRITTEN)?.and_then(|w| w.get_str(&self.lex).map(str::to_string));
        if let Some(w) = written {
            let (l, n) = split_homograph(&w);
            if w == lemma || (l == lemma && n == hm) {
                return Ok(Some(w));
            }
        }
        Ok(Some(match hm {
            Some(n) => format!("{lemma}{n}"),
            None => lemma,
        }))
    }

    fn emit(&mut self, marker: &str, owner: &str) -> Result<(), String> {
        let Some((c, field)) = carried(marker) else { return Ok(()) };
        if !self.emitted.insert((owner.to_string(), field)) {
            return Ok(());
        }
        if let Some(v) = self.value(owner, c, field)? {
            self.out.push(Field { marker: marker.to_string(), value: v });
        }
        Ok(())
    }

    /// A carried value with no line of its own yet (added in wugs).
    fn emit_new(&mut self, c: Carried, owner: &str) -> Result<(), String> {
        let (marker, field) = (marker_of(c), carried(marker_of(c)).unwrap().1);
        if self.placed.contains(&(owner.to_string(), field)) {
            return Ok(());
        }
        self.emit(marker, owner)
    }

    fn walk(&mut self, items: Vec<(String, Result<String, String>)>) -> Result<(), String> {
        for (m, it) in items {
            match it {
                Err(verbatim) => self.out.push(Field { marker: m, value: verbatim }),
                Ok(of) => self.emit(&m, &of)?,
            }
        }
        Ok(())
    }
}

const SENSE_FIELDS: &[Carried] = &[Carried::Pos, Carried::Gloss, Carried::Definition, Carried::Note];

fn export_entry(project: &Project, lex: &str) -> Result<Vec<Field>, String> {
    let p = &project.proj;
    let senses = p.ordered_children("lexeme", lex, "sense")?;
    let mut variants = Vec::new();
    for f in p.ordered_children("lexeme", lex, "form")? {
        match opt_str(project, &f, "kind")?.as_deref() {
            Some("variant") => variants.push((f, Carried::Variant)),
            Some("allomorph") => variants.push((f, Carried::Allomorph)),
            _ => {}
        }
    }
    let lex_block = block_items(&opt_value(project, lex, F_FIELDS)?);
    let mut sense_blocks = Vec::new();
    for s in &senses {
        sense_blocks.push(opt_value(project, s, F_FIELDS)?.map(|b| block_items(&Some(b))));
    }
    let mut placed = HashSet::new();
    for (m, it) in lex_block.iter().chain(sense_blocks.iter().flatten().flatten()) {
        if let (Ok(of), Some((_, field))) = (it, carried(m)) {
            placed.insert((of.clone(), field));
        }
    }
    let mut w = Writer { project, lex: lex.to_string(), out: Vec::new(), placed, emitted: HashSet::new() };
    w.emit("lx", lex)?;
    w.walk(lex_block)?;
    w.emit_new(Carried::Homograph, lex)?;
    w.emit_new(Carried::Citation, lex)?;
    for x in p.ordered_children("lexeme", lex, "pronunciation")? {
        w.emit_new(Carried::Phonetic, &x)?;
    }
    for (v, c) in &variants {
        w.emit_new(*c, v)?;
    }
    let numbered = senses.len() > 1 || sense_blocks.iter().any(Option::is_some);
    for (i, (s, block)) in senses.iter().zip(sense_blocks).enumerate() {
        match block {
            Some(b) => w.walk(b)?,
            None if numbered => {
                let sn = opt_str(project, s, F_SN)?.unwrap_or_else(|| (i + 1).to_string());
                w.out.push(Field { marker: "sn".into(), value: sn });
                w.emitted.insert((s.clone(), F_SN));
            }
            None => {}
        }
        for c in SENSE_FIELDS {
            w.emit_new(*c, s)?;
        }
        // Examples added in wugs, after the sense's other lines.
        for x in p.ordered_children("sense", s, "example")? {
            w.emit_new(Carried::Example, &x)?;
            w.emit_new(Carried::ExampleTranslation, &x)?;
        }
    }
    // Relations made in wugs (an imported one keeps the lines it came with).
    for r in project.relations(lex)? {
        if opt_value(project, &r.id, F_WRITTEN)?.is_some() {
            continue;
        }
        let c = match r.relation.as_str() {
            "synonym" => Carried::Synonym,
            "antonym" => Carried::Antonym,
            "compare" => Carried::Compare,
            _ => continue,
        };
        w.emit_new(c, &r.id)?;
    }
    // An origin added in wugs, at the end of the record as MDF writes it.
    w.emit_new(Carried::Etymology, lex)?;
    w.emit_new(Carried::Borrowed, lex)?;
    Ok(w.out)
}
