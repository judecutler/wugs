//! LIFT 0.13 lexicons (FLEx's lexicon interchange format).
//!
//! Carried: entries (lexical-unit, citation, homograph `order`,
//! pronunciations with their media file names, notes), senses (gloss,
//! definition, grammatical-info, notes, examples with their translations),
//! variants, and
//! relations between entries and senses, in every writing system, plus
//! every attribute of entries, pronunciations, senses, examples, variants
//! and relations. Everything else is counted in
//! `ImportReport::not_carried`.

use wugs_core::{Project, SCOPE_LEXICON};
use wugs_model::{pos_seq, Value};
use wugs_xml::{self as xml, XmlNode};

use crate::common::*;

const F_ID: &str = "x.lift.id";
const F_GUID: &str = "x.lift.guid";
const F_LEXICAL_UNIT: &str = "x.lift.lexical_unit";
const F_CITATION: &str = "x.lift.citation";
const F_GLOSS: &str = "x.lift.gloss";
const F_DEFINITION: &str = "x.lift.definition";
const F_GRAMMATICAL_INFO: &str = "x.lift.grammatical_info";
const F_FORMS: &str = "x.lift.forms";
/// An example's `<translation>`s: `[{type?, forms}]`, the first one's first
/// form being the example's `translation`.
const F_TRANSLATIONS: &str = "x.lift.translations";
/// All attributes of the source element (dates, order, ref…), re-emitted on
/// export under the ones wugs derives.
const F_ATTRS: &str = "x.lift.attrs";
/// A pronunciation's `<media>` elements, each its attributes (`href`…), kept
/// for export: wugs hears a pronunciation in a project recording instead.
const F_MEDIA: &str = "x.lift.media";
/// A sense's `<illustration>`s whose picture is not in the project, each
/// `{attrs, label}`, written back as they came.
const F_ILLUSTRATIONS: &str = "x.lift.illustrations";
/// The `<label>` of the illustration that became the sense's picture.
const F_PICTURE_LABEL: &str = "x.lift.picture-label";
/// An entry's or sense's `<note>`s, each `{attrs, forms}`; the first one's
/// first form is its `notes`, the rest written back as they came.
const F_NOTES: &str = "x.lift.notes";

#[derive(Debug, Default)]
pub struct ImportReport {
    pub entries_created: usize,
    pub entries_skipped: usize,
    pub senses_created: usize,
    pub variants_created: usize,
    pub examples_created: usize,
    pub pronunciations_created: usize,
    pub relations_created: usize,
    /// `<note>`s on entries and senses.
    pub notes: usize,
    /// Each `<illustration>`: the sense it belongs to and its `href`, for the
    /// caller to look for beside the file (`attach_illustration`).
    pub illustrations: Vec<(String, String)>,
    /// `<span>` runs flattened into plain text (their text is kept in order;
    /// the markup is not).
    pub spans_flattened: usize,
    pub not_carried: NotCarried,
    pub commits: usize,
    pub findings: usize,
}

struct Entry {
    origin: String,
    id: Option<String>,
    guid: Option<String>,
    order: Option<f64>,
    attrs: Value,
    lexical_unit: MultiText,
    citation: MultiText,
    senses: Vec<Sense>,
    variants: Vec<(MultiText, Value)>,
    /// Each `<pronunciation>`: its attributes, forms and `<media>` attributes.
    pronunciations: Vec<(Value, MultiText, Vec<Value>)>,
    /// Each `<relation>`'s attributes (`type`, `ref`…).
    relations: Vec<Value>,
    /// Each `<note>`: `{attrs, forms}`.
    notes: Vec<Value>,
}

struct Sense {
    origin: String,
    id: Option<String>,
    guid: Option<String>,
    gloss: MultiText,
    definition: MultiText,
    grammatical_info: Option<String>,
    examples: Vec<Example>,
    attrs: Value,
    relations: Vec<Value>,
    /// Each `<illustration>`: `{attrs, label}`.
    illustrations: Vec<Value>,
    notes: Vec<Value>,
}

struct Example {
    attrs: Value,
    forms: MultiText,
    translations: Vec<(Option<String>, MultiText)>,
}

fn translations_value(ts: &[(Option<String>, MultiText)]) -> Value {
    Value::Arr(
        ts.iter()
            .map(|(kind, forms)| {
                let mut o = vec![("forms", multi_value(forms))];
                if let Some(k) = kind {
                    o.push(("type", Value::str(k)));
                }
                Value::obj(o)
            })
            .collect(),
    )
}

fn translations_from(v: Option<Value>) -> Vec<(Option<String>, MultiText)> {
    match v {
        Some(Value::Arr(items)) => items
            .iter()
            .map(|t| (t.get_str("type").map(str::to_string), t.get("forms").and_then(multi_from).unwrap_or_default()))
            .collect(),
        _ => Vec::new(),
    }
}

/// An imported `<illustration>` whose file was found and stored as `blob`:
/// it becomes the sense's picture under `name`, its label is kept for
/// export, and it leaves the illustrations still waiting for a file.
pub fn attach_illustration(project: &mut Project, sense: &str, href: &str, blob: &str, name: &str) -> Result<(), String> {
    let items = match field_value(project, sense, F_ILLUSTRATIONS)? {
        Some(Value::Arr(items)) => items,
        _ => Vec::new(),
    };
    let is_it = |v: &Value| v.get("attrs").and_then(|a| a.get_str("href")) == Some(href);
    let label = items.iter().find(|v| is_it(v)).and_then(|v| v.get("label").cloned());
    let rest: Vec<Value> = items.iter().filter(|v| !is_it(v)).cloned().collect();
    project.set_sense_picture(sense, Some((blob, name)))?;
    let mut t = project.intent("import-lift-picture");
    t.set("sense", sense, SCOPE_LEXICON, F_ILLUSTRATIONS, if rest.is_empty() { Value::Null } else { Value::Arr(rest) })?;
    if let Some(l) = label {
        t.set("sense", sense, SCOPE_LEXICON, F_PICTURE_LABEL, l)?;
    }
    t.commit()?;
    Ok(())
}

/// Import a whole LIFT lexicon. One txn per entry, all in the lexicon scope.
/// Entries already in the project (matched by guid, else id) are skipped, not
/// updated, so re-importing the same file or our own export creates nothing.
pub fn import_lift(project: &mut Project, lift: &str) -> Result<ImportReport, ImportError> {
    let mut report = ImportReport::default();
    let entries = parse_lift(lift, &mut report)?;
    let index = origin_index(project, "lexeme")?;
    // Relations wait until every entry is in, since they point at each other.
    let mut pending: Vec<(String, Value)> = Vec::new();

    for e in entries {
        if resolve_origin(project, &index, "lexeme", &e.origin)?.is_some() {
            report.entries_skipped += 1;
            continue;
        }
        let mut t = project.intent("import-lift");
        let lemma = e.lexical_unit.first().map(|(_, t)| t.clone()).unwrap_or_default();
        let mut fields = vec![
            ("lemma", Value::str(&lemma)),
            (F_ORIGIN, Value::str(&e.origin)),
            (F_LEXICAL_UNIT, multi_value(&e.lexical_unit)),
            (F_ATTRS, e.attrs.clone()),
        ];
        if let Some((_, c)) = e.citation.first() {
            fields.push(("citation", Value::str(c)));
            fields.push((F_CITATION, multi_value(&e.citation)));
        }
        if let Some(o) = e.order {
            fields.push(("homograph", Value::Num(o)));
        }
        push_opt(&mut fields, F_ID, &e.id);
        push_opt(&mut fields, F_GUID, &e.guid);
        push_notes(&mut fields, &e.notes, &mut report);
        let lex = t.create("lexeme", "lex", SCOPE_LEXICON, fields);
        pending.extend(e.relations.iter().map(|r| (lex.clone(), r.clone())));

        for (i, s) in e.senses.iter().enumerate() {
            let mut fields = vec![
                ("lexeme", Value::str(&lex)),
                ("pos", Value::str(&pos_seq(i))),
                (F_ORIGIN, Value::str(&s.origin)),
                (F_GLOSS, multi_value(&s.gloss)),
                (F_ATTRS, s.attrs.clone()),
            ];
            if let Some((_, g)) = s.gloss.first() {
                fields.push(("gloss", Value::str(g)));
            }
            if let Some((_, d)) = s.definition.first() {
                fields.push(("definition", Value::str(d)));
                fields.push((F_DEFINITION, multi_value(&s.definition)));
            }
            push_opt(&mut fields, F_GRAMMATICAL_INFO, &s.grammatical_info);
            push_opt(&mut fields, F_ID, &s.id);
            push_opt(&mut fields, F_GUID, &s.guid);
            if !s.illustrations.is_empty() {
                fields.push((F_ILLUSTRATIONS, Value::Arr(s.illustrations.clone())));
            }
            push_notes(&mut fields, &s.notes, &mut report);
            let sen = t.create("sense", "sen", SCOPE_LEXICON, fields);
            for ill in &s.illustrations {
                if let Some(href) = ill.get("attrs").and_then(|a| a.get_str("href")) {
                    report.illustrations.push((sen.clone(), href.to_string()));
                }
            }
            report.senses_created += 1;
            pending.extend(s.relations.iter().map(|r| (sen.clone(), r.clone())));
            for (j, x) in s.examples.iter().enumerate() {
                let mut fields = vec![
                    ("sense", Value::str(&sen)),
                    ("pos", Value::str(&pos_seq(j))),
                    ("sentence", Value::str(x.forms.first().map(|(_, t)| t.as_str()).unwrap_or(""))),
                    (F_FORMS, multi_value(&x.forms)),
                    (F_TRANSLATIONS, translations_value(&x.translations)),
                    (F_ATTRS, x.attrs.clone()),
                ];
                if let Some((_, tr)) = x.translations.first().and_then(|(_, f)| f.first()) {
                    fields.push(("translation", Value::str(tr)));
                }
                if let Some(src) = x.attrs.get_str("source") {
                    fields.push(("reference", Value::str(src)));
                }
                t.create("example", "exm", SCOPE_LEXICON, fields);
                report.examples_created += 1;
            }
        }
        for (i, (attrs, forms, media)) in e.pronunciations.iter().enumerate() {
            let mut fields = vec![
                ("lexeme", Value::str(&lex)),
                ("pos", Value::str(&pos_seq(i))),
                (F_FORMS, multi_value(forms)),
                (F_ATTRS, attrs.clone()),
            ];
            if let Some((_, f)) = forms.first().filter(|(_, f)| !f.is_empty()) {
                fields.push(("form", Value::str(f)));
            }
            if !media.is_empty() {
                fields.push((F_MEDIA, Value::Arr(media.clone())));
            }
            t.create("pronunciation", "prn", SCOPE_LEXICON, fields);
            report.pronunciations_created += 1;
        }
        for (i, (v, attrs)) in e.variants.iter().enumerate() {
            t.create(
                "form",
                "frm",
                SCOPE_LEXICON,
                vec![
                    ("lexeme", Value::str(&lex)),
                    ("kind", Value::str("variant")),
                    ("form", Value::str(v.first().map(|(_, t)| t.as_str()).unwrap_or(""))),
                    ("pos", Value::str(&pos_seq(i))),
                    (F_FORMS, multi_value(v)),
                    (F_ATTRS, attrs.clone()),
                ],
            );
            report.variants_created += 1;
        }
        let res = t.commit()?;
        report.entries_created += 1;
        report.commits += 1;
        report.findings += res.findings.len();
    }
    if !pending.is_empty() {
        let ids = lift_ids(project)?;
        // A symmetric relation FLEx lists on both its ends becomes one relation.
        let mut made: Vec<(String, String, String, Value)> = Vec::new();
        for (from, attrs) in pending {
            let Some(to) = attrs.get_str("ref").and_then(|r| ids.get(r)).cloned() else {
                report.not_carried.add("relation (points at no entry in the file or project)");
                continue;
            };
            let lift_type = attrs.get_str("type").unwrap_or("").to_string();
            let relation = relation_of(&lift_type);
            if relation.is_empty() || to == from {
                report.not_carried.add("relation (no type, or to itself)");
                continue;
            }
            let symmetric = wugs_core::relations::symmetric(&relation);
            let twin = made.iter().any(|(a, b, r, _)| *r == relation && ((*a == from && *b == to) || (symmetric && *a == to && *b == from)))
                || project.relations_between(&from, &to, &relation, symmetric)?;
            if !twin {
                made.push((from, to, relation, attrs));
            }
        }
        if !made.is_empty() {
            let mut t = project.intent("import-lift");
            for (from, to, relation, attrs) in &made {
                t.create(
                    "lexical-relation",
                    "rel",
                    SCOPE_LEXICON,
                    vec![("from", Value::str(from)), ("to", Value::str(to)), ("relation", Value::str(relation)), (F_ATTRS, attrs.clone())],
                );
                report.relations_created += 1;
            }
            let res = t.commit()?;
            report.commits += 1;
            report.findings += res.findings.len();
        }
    }
    Ok(report)
}

/// `notes` from the first `<note>`'s first form, every note kept for export.
fn push_notes<'a>(fields: &mut Vec<(&'a str, Value)>, notes: &[Value], report: &mut ImportReport) {
    let Some(first) = notes.first() else { return };
    if let Some((_, text)) = first.get("forms").and_then(multi_from).and_then(|m| m.into_iter().next()) {
        fields.push(("notes", Value::str(&text)));
    }
    fields.push((F_NOTES, Value::Arr(notes.to_vec())));
    report.notes += notes.len();
}

/// `<note type><form lang><text>…</text></form></note>`: `{attrs, forms}`.
fn parse_note(node: &XmlNode, report: &mut ImportReport) -> Value {
    for c in node.children.iter().filter(|c| c.name != "form") {
        report.not_carried.add(&format!("note/{}", c.name));
    }
    Value::Obj(vec![("attrs".to_string(), attrs_value(node)), ("forms".to_string(), multi_value(&read_multi(node, report)))].into_iter().collect())
}

/// The `<note>`s to write: the stored ones, the first carrying `notes` as
/// it reads now (a note typed in wugs goes out in `lang`, untyped).
fn note_nodes(project: &Project, id: &str, lang: &str) -> Result<Vec<XmlNode>, String> {
    let mut notes: Vec<(Option<Value>, MultiText)> = match field_value(project, id, F_NOTES)? {
        Some(Value::Arr(items)) => items.iter().map(|n| (n.get("attrs").cloned(), n.get("forms").and_then(multi_from).unwrap_or_default())).collect(),
        _ => Vec::new(),
    };
    let now = project.proj.field_str(id, "notes")?.filter(|n| !n.trim().is_empty());
    match notes.first_mut() {
        Some((_, forms)) => *forms = with_primary(Some(std::mem::take(forms)), now, lang),
        None => {
            if let Some(n) = now {
                notes.push((None, vec![(lang.to_string(), n)]));
            }
        }
    }
    Ok(notes
        .into_iter()
        .filter(|(_, forms)| !forms.is_empty())
        .map(|(attrs, forms)| with_attrs(multi_node("note", &forms), attrs, &[]))
        .collect())
}

fn push_opt<'a>(fields: &mut Vec<(&'a str, Value)>, key: &'a str, v: &Option<String>) {
    if let Some(v) = v {
        fields.push((key, Value::str(v)));
    }
}

fn parse_lift(input: &str, report: &mut ImportReport) -> Result<Vec<Entry>, ImportError> {
    let root = xml::parse(input)?;
    if root.name != "lift" {
        return Err(format_err(format!("root element is <{}>, expected <lift>", root.name)));
    }
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for child in &root.children {
        match child.name.as_str() {
            "entry" => {
                // FLEx marks deleted entries with dateDeleted; they are not data.
                if child.attr("dateDeleted").is_some() {
                    report.not_carried.add("entry[dateDeleted]");
                    continue;
                }
                let e = parse_entry(child, report)?;
                if !seen.insert(e.origin.clone()) {
                    return Err(format_err(format!("duplicate entry {}", e.origin)));
                }
                entries.push(e);
            }
            "header" => report.not_carried.add("header"),
            other => report.not_carried.add(other),
        }
    }
    Ok(entries)
}

fn parse_entry(node: &XmlNode, report: &mut ImportReport) -> Result<Entry, ImportError> {
    let id = node.attr("id").map(str::to_string);
    let guid = node.attr("guid").map(str::to_string);
    let origin = guid
        .clone()
        .or_else(|| id.clone())
        .ok_or_else(|| format_err("<entry> has neither guid nor id"))?;
    let order = match node.attr("order") {
        Some(o) => Some(
            o.trim()
                .parse::<f64>()
                .map_err(|_| format_err(format!("entry {origin} has non-numeric order {o:?}")))?,
        ),
        None => None,
    };
    let mut e = Entry {
        origin,
        id,
        guid,
        order,
        attrs: attrs_value(node),
        lexical_unit: Vec::new(),
        citation: Vec::new(),
        senses: Vec::new(),
        variants: Vec::new(),
        pronunciations: Vec::new(),
        relations: Vec::new(),
        notes: Vec::new(),
    };
    for c in &node.children {
        match c.name.as_str() {
            "lexical-unit" => e.lexical_unit = read_multi(c, report),
            "citation" => e.citation = read_multi(c, report),
            "sense" => {
                let s = parse_sense(c, e.senses.len(), &e.origin, report)?;
                e.senses.push(s);
            }
            "variant" => {
                e.variants.push((read_multi(c, report), attrs_value(c)));
                for vc in c.children.iter().filter(|vc| vc.name != "form") {
                    report.not_carried.add(&format!("variant/{}", vc.name));
                }
            }
            "pronunciation" => {
                let mut media = Vec::new();
                for pc in &c.children {
                    match pc.name.as_str() {
                        "form" => {}
                        "media" => {
                            media.push(attrs_value(pc));
                            for mc in &pc.children {
                                report.not_carried.add(&format!("media/{}", mc.name));
                            }
                        }
                        other => report.not_carried.add(&format!("pronunciation/{other}")),
                    }
                }
                e.pronunciations.push((attrs_value(c), read_multi(c, report), media));
            }
            "relation" => e.relations.push(parse_relation(c, report)),
            "note" => e.notes.push(parse_note(c, report)),
            other => report.not_carried.add(&format!("entry/{other}")),
        }
    }
    if e.lexical_unit.is_empty() {
        return Err(format_err(format!("entry {} has no lexical-unit form", e.origin)));
    }
    Ok(e)
}

fn parse_sense(node: &XmlNode, index: usize, entry: &str, report: &mut ImportReport) -> Result<Sense, ImportError> {
    let id = node.attr("id").map(str::to_string);
    let guid = node.attr("guid").map(str::to_string);
    let origin = guid
        .clone()
        .or_else(|| id.clone())
        .unwrap_or_else(|| format!("{entry}#sense{index}"));
    let mut s = Sense {
        origin,
        id,
        guid,
        gloss: Vec::new(),
        definition: Vec::new(),
        grammatical_info: None,
        examples: Vec::new(),
        attrs: attrs_value(node),
        relations: Vec::new(),
        illustrations: Vec::new(),
        notes: Vec::new(),
    };
    for c in &node.children {
        match c.name.as_str() {
            // LIFT puts one <gloss lang> per writing system, text directly inside.
            "gloss" => {
                for gc in c.children.iter().filter(|gc| gc.name != "text") {
                    report.not_carried.add(&format!("gloss/{}", gc.name));
                }
                s.gloss.push((
                    c.attr("lang").unwrap_or("und").to_string(),
                    c.child("text").map(|t| flat_text(t, report)).unwrap_or_default(),
                ))
            }
            "definition" => s.definition = read_multi(c, report),
            "grammatical-info" => {
                s.grammatical_info = c.attr("value").map(str::to_string);
                for gc in &c.children {
                    report.not_carried.add(&format!("grammatical-info/{}", gc.name));
                }
            }
            "example" => s.examples.push(parse_example(c, report)),
            "relation" => s.relations.push(parse_relation(c, report)),
            "note" => s.notes.push(parse_note(c, report)),
            "illustration" => {
                let label = c.child("label").map(|l| read_multi(l, report)).unwrap_or_default();
                for ic in c.children.iter().filter(|ic| ic.name != "label") {
                    report.not_carried.add(&format!("illustration/{}", ic.name));
                }
                s.illustrations.push(Value::Obj(vec![("attrs".to_string(), attrs_value(c)), ("label".to_string(), multi_value(&label))].into_iter().collect()));
            }
            other => report.not_carried.add(&format!("sense/{other}")),
        }
    }
    Ok(s)
}

/// `<relation type ref>`: its attributes; what it holds is not carried.
fn parse_relation(node: &XmlNode, report: &mut ImportReport) -> Value {
    for c in &node.children {
        report.not_carried.add(&format!("relation/{}", c.name));
    }
    attrs_value(node)
}

/// FLEx's relation type names as wugs's, where they mean the same.
fn relation_of(lift_type: &str) -> String {
    match lift_type.trim().to_lowercase().as_str() {
        "synonym" | "synonyms" => "synonym".into(),
        "antonym" | "antonyms" => "antonym".into(),
        "compare" | "confer" | "cf" => "compare".into(),
        _ => lift_type.trim().to_string(),
    }
}

/// wugs's relation types as FLEx names them.
fn lift_type_of(relation: &str) -> String {
    match relation {
        "synonym" => "Synonyms".into(),
        "antonym" => "Antonym".into(),
        "compare" => "Compare".into(),
        "part" => "Part".into(),
        "whole" => "Whole".into(),
        other => other.into(),
    }
}

/// Every live lexeme and sense by the LIFT `id` it came with, else its own.
fn lift_ids(project: &Project) -> Result<std::collections::HashMap<String, String>, String> {
    let mut out = std::collections::HashMap::new();
    for kind in ["lexeme", "sense"] {
        for id in project.proj.entities_of_kind(kind)? {
            if project.proj.is_tombstoned(&id)? {
                continue;
            }
            if let Some(lift) = project.proj.field_str(&id, F_ID)? {
                out.insert(lift, id.clone());
            }
            out.insert(id.clone(), id);
        }
    }
    Ok(out)
}

/// `<example source><form>…</form><translation type><form>…</form></translation></example>`
fn parse_example(node: &XmlNode, report: &mut ImportReport) -> Example {
    let mut translations = Vec::new();
    for c in &node.children {
        match c.name.as_str() {
            "form" => {}
            "translation" => {
                for tc in c.children.iter().filter(|tc| tc.name != "form") {
                    report.not_carried.add(&format!("translation/{}", tc.name));
                }
                translations.push((c.attr("type").map(str::to_string), read_multi(c, report)));
            }
            other => report.not_carried.add(&format!("example/{other}")),
        }
    }
    Example { attrs: attrs_value(node), forms: read_multi(node, report), translations }
}

/// `<x><form lang><text>…</text></form>…</x>`
fn read_multi(node: &XmlNode, report: &mut ImportReport) -> MultiText {
    node.children_named("form")
        .map(|f| {
            for fc in f.children.iter().filter(|fc| fc.name != "text") {
                report.not_carried.add(&format!("form/{}", fc.name));
            }
            (
                f.attr("lang").unwrap_or("und").to_string(),
                f.child("text").map(|t| flat_text(t, report)).unwrap_or_default(),
            )
        })
        .collect()
}

fn flat_text(text: &XmlNode, report: &mut ImportReport) -> String {
    report.spans_flattened += count_spans(text);
    text.deep_text()
}

fn count_spans(n: &XmlNode) -> usize {
    n.children.iter().map(|c| (c.name == "span") as usize + count_spans(c)).sum()
}

// ------------------------------------------------------------------ export

/// Render the project's whole lexicon as LIFT. Reads the projection only.
pub fn export_lift(project: &Project) -> Result<String, String> {
    let p = &project.proj;
    let mut lexemes: Vec<(String, f64, String)> = Vec::new();
    for id in live_of_kind(project, "lexeme")? {
        let lemma = p.field_str(&id, "lemma")?.unwrap_or_default();
        let hn = p.field_num(&id, "homograph")?.unwrap_or(0.0);
        lexemes.push((lemma, hn, id));
    }
    lexemes.sort_by(|a, b| (&a.0, a.1, &a.2).partial_cmp(&(&b.0, b.1, &b.2)).unwrap());

    let mut root = XmlNode::new("lift").with_attr("producer", "wugs").with_attr("version", "0.13");
    for (lemma, _, lex) in &lexemes {
        let lang = default_lang(project, lex, F_LEXICAL_UNIT)?;
        let mut entry = with_attrs(
            XmlNode::new("entry"),
            field_value(project, lex, F_ATTRS)?,
            &[
                ("id", Some(p.field_str(lex, F_ID)?.unwrap_or_else(|| lex.clone()))),
                ("guid", p.field_str(lex, F_GUID)?),
                ("order", p.field_num(lex, "homograph")?.map(num)),
            ],
        );
        let lu = with_primary(field_multi(project, lex, F_LEXICAL_UNIT)?, Some(lemma.clone()), &lang);
        entry.children.push(multi_node("lexical-unit", &lu));
        let cit = with_primary(field_multi(project, lex, F_CITATION)?, p.field_str(lex, "citation")?, &lang);
        if !cit.is_empty() {
            entry.children.push(multi_node("citation", &cit));
        }
        for prn in ordered_children(project, "lexeme", lex, "pronunciation")? {
            // A pronunciation typed in wugs is written in the entry's language, as an IPA form.
            let forms = with_primary(field_multi(project, &prn, F_FORMS)?, p.field_str(&prn, "form")?.filter(|f| !f.is_empty()), &format!("{lang}-fonipa"));
            let mut x = with_attrs(multi_node("pronunciation", &forms), field_value(project, &prn, F_ATTRS)?, &[]);
            if let Some(Value::Arr(media)) = field_value(project, &prn, F_MEDIA)? {
                for m in media {
                    x.children.push(with_attrs(XmlNode::new("media"), Some(m), &[]));
                }
            }
            entry.children.push(x);
        }
        for sen in ordered_children(project, "lexeme", lex, "sense")? {
            let mut sense = with_attrs(
                XmlNode::new("sense"),
                field_value(project, &sen, F_ATTRS)?,
                &[
                    ("id", Some(p.field_str(&sen, F_ID)?.unwrap_or_else(|| sen.clone()))),
                    ("guid", p.field_str(&sen, F_GUID)?),
                ],
            );
            if let Some(gi) = p.field_str(&sen, F_GRAMMATICAL_INFO)? {
                sense.children.push(XmlNode::new("grammatical-info").with_attr("value", &gi));
            }
            let gl = with_primary(field_multi(project, &sen, F_GLOSS)?, p.field_str(&sen, "gloss")?, "en");
            for (l, t) in &gl {
                sense.children.push(
                    XmlNode::new("gloss")
                        .with_attr("lang", l)
                        .with_child(XmlNode::new("text").with_text(t)),
                );
            }
            let def = with_primary(field_multi(project, &sen, F_DEFINITION)?, p.field_str(&sen, "definition")?, "en");
            if !def.is_empty() {
                sense.children.push(multi_node("definition", &def));
            }
            sense.children.extend(note_nodes(project, &sen, "en")?);
            for exm in ordered_children(project, "sense", &sen, "example")? {
                let forms = with_primary(field_multi(project, &exm, F_FORMS)?, p.field_str(&exm, "sentence")?, &lang);
                let mut x = with_attrs(multi_node("example", &forms), field_value(project, &exm, F_ATTRS)?, &[("source", p.field_str(&exm, "reference")?.filter(|r| !r.is_empty()))]);
                let mut translations = translations_from(field_value(project, &exm, F_TRANSLATIONS)?);
                let first = translations.first().map(|(_, f)| f.clone());
                let now = with_primary(first, p.field_str(&exm, "translation")?, "en");
                match translations.first_mut() {
                    Some(t) => t.1 = now,
                    None if !now.is_empty() => translations.push((None, now)),
                    None => {}
                }
                for (kind, forms) in translations.iter().filter(|(_, f)| !f.is_empty()) {
                    let mut tr = multi_node("translation", forms);
                    if let Some(k) = kind {
                        tr = tr.with_attr("type", k);
                    }
                    x.children.push(tr);
                }
                sense.children.push(x);
            }
            relation_nodes(project, &sen, &mut sense)?;
            // The sense's picture first, by the name it was chosen under (FLEx
            // looks for it in a `pictures` folder beside the file), then any
            // illustration whose picture never reached the project.
            if let (Some(_), Some(name)) = (p.field_str(&sen, "picture")?.filter(|b| !b.is_empty()), p.field_str(&sen, "x.picture-name")?) {
                let mut ill = XmlNode::new("illustration").with_attr("href", &name);
                if let Some(label) = field_multi(project, &sen, F_PICTURE_LABEL)?.filter(|l| !l.is_empty()) {
                    ill.children.push(multi_node("label", &label));
                }
                sense.children.push(ill);
            }
            if let Some(Value::Arr(items)) = field_value(project, &sen, F_ILLUSTRATIONS)? {
                for item in items {
                    let mut ill = with_attrs(XmlNode::new("illustration"), item.get("attrs").cloned(), &[]);
                    if let Some(label) = item.get("label").and_then(multi_from).filter(|l| !l.is_empty()) {
                        ill.children.push(multi_node("label", &label));
                    }
                    sense.children.push(ill);
                }
            }
            entry.children.push(sense);
        }
        entry.children.extend(note_nodes(project, lex, "en")?);
        relation_nodes(project, lex, &mut entry)?;
        for frm in ordered_children(project, "lexeme", lex, "form")? {
            // FLEx writes an entry's allomorphs as <variant>, so ours go out the same way.
            if !matches!(p.field_str(&frm, "kind")?.as_deref(), Some("variant" | "allomorph")) {
                continue;
            }
            let forms = with_primary(field_multi(project, &frm, F_FORMS)?, p.field_str(&frm, "form")?, &lang);
            let mut v = multi_node("variant", &forms);
            v = with_attrs(v, field_value(project, &frm, F_ATTRS)?, &[]);
            entry.children.push(v);
        }
        root.children.push(entry);
    }
    Ok(root.to_xml())
}

/// An entry's or sense's `<relation>`s: those it points from, and the
/// symmetric ones pointing at it, which FLEx lists on both ends.
fn relation_nodes(project: &Project, holder: &str, node: &mut XmlNode) -> Result<(), String> {
    let p = &project.proj;
    let mut rels: Vec<(String, String, bool)> = Vec::new();
    for (field, other_field) in [("from", "to"), ("to", "from")] {
        for id in p.children_by_ref(field, holder, "lexical-relation")? {
            let relation = p.field_str(&id, "relation")?.unwrap_or_default();
            if field == "to" && !wugs_core::relations::symmetric(&relation) {
                continue;
            }
            if let Some(other) = p.field_str(&id, other_field)? {
                rels.push((id, other, field == "from"));
            }
        }
    }
    rels.sort();
    for (id, other, own) in rels {
        if p.entity_kind(&other)?.is_none() || p.is_tombstoned(&other)? {
            continue;
        }
        let relation = p.field_str(&id, "relation")?.unwrap_or_default();
        let stored = field_value(project, &id, F_ATTRS)?;
        // The other end gets the type as the file wrote it, without the first end's other attributes.
        let lift_type = stored.as_ref().and_then(|a| a.get_str("type")).filter(|t| relation_of(t) == relation).map(str::to_string).unwrap_or_else(|| lift_type_of(&relation));
        let target = p.field_str(&other, F_ID)?.unwrap_or_else(|| other.clone());
        let base = XmlNode::new("relation");
        node.children.push(with_attrs(base, if own { stored } else { None }, &[("type", Some(lift_type)), ("ref", Some(target))]));
    }
    Ok(())
}

fn multi_node(name: &str, m: &MultiText) -> XmlNode {
    let mut n = XmlNode::new(name);
    for (l, t) in m {
        n.children.push(
            XmlNode::new("form")
                .with_attr("lang", l)
                .with_child(XmlNode::new("text").with_text(t)),
        );
    }
    n
}

fn default_lang(project: &Project, id: &str, field: &str) -> Result<String, String> {
    Ok(field_multi(project, id, field)?
        .and_then(|m| m.first().map(|(l, _)| l.clone()))
        .unwrap_or_else(|| "und".to_string()))
}

fn num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        n.to_string()
    }
}

pub(crate) fn live_of_kind(project: &Project, kind: &str) -> Result<Vec<String>, String> {
    project.proj.entities_of_kind(kind)
}

/// Children by ref, ordered by their `pos` then id.
pub(crate) fn ordered_children(project: &Project, field: &str, parent: &str, kind: &str) -> Result<Vec<String>, String> {
    project.proj.ordered_children(field, parent, kind)
}
