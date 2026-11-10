//! LIFT 0.13 lexicons (FLEx's lexicon interchange format).
//!
//! Carried: entries (lexical-unit, citation, homograph `order`), senses
//! (gloss, definition, grammatical-info) and variants, in every writing
//! system, plus every attribute of entries, senses and variants. Everything
//! else is counted in `ImportReport::not_carried`.

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
/// All attributes of the source element (dates, order, ref…), re-emitted on
/// export under the ones wugs derives.
const F_ATTRS: &str = "x.lift.attrs";

#[derive(Debug, Default)]
pub struct ImportReport {
    pub entries_created: usize,
    pub entries_skipped: usize,
    pub senses_created: usize,
    pub variants_created: usize,
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
}

struct Sense {
    origin: String,
    id: Option<String>,
    guid: Option<String>,
    gloss: MultiText,
    definition: MultiText,
    grammatical_info: Option<String>,
    attrs: Value,
}

/// Import a whole LIFT lexicon. One txn per entry, all in the lexicon scope.
/// Entries already in the project (matched by guid, else id) are skipped, not
/// updated, so re-importing the same file or our own export creates nothing.
pub fn import_lift(project: &mut Project, lift: &str) -> Result<ImportReport, ImportError> {
    let mut report = ImportReport::default();
    let entries = parse_lift(lift, &mut report)?;
    let index = origin_index(project, "lexeme")?;

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
        let lex = t.create("lexeme", "lex", SCOPE_LEXICON, fields);

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
            t.create("sense", "sen", SCOPE_LEXICON, fields);
            report.senses_created += 1;
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
    Ok(report)
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
        attrs: attrs_value(node),
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
            other => report.not_carried.add(&format!("sense/{other}")),
        }
    }
    Ok(s)
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
            entry.children.push(sense);
        }
        for frm in ordered_children(project, "lexeme", lex, "form")? {
            if p.field_str(&frm, "kind")?.as_deref() != Some("variant") {
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
