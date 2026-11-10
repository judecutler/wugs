//! FlexText interlinear texts (FLEx's IGT export).
//!
//! interlinear-text -> Text · phrase -> Segment on the FLEx baseline tier ·
//! first phrase `gls` -> Segment on the free-translation tier · word -> Token
//! · morph -> Morph. Every `<item>` is kept verbatim in `x.flex.items`; the
//! mapped ones are also written to the semantic fields, and on export those
//! fields win, so edits made in wugs reach FLEx.
//!
//! Identity: a text or phrase is its FLEx guid. One without a guid gets a
//! positional key on import and a GUID derived from its wugs id on export,
//! so our own exports re-import onto the same entities. FLEx word and morph
//! guids name shared analyses, not occurrences, so they are kept for export
//! but never used as identity.

use std::collections::HashMap;

use wugs_core::{Project, SCOPE_CONFIG};
use wugs_model::{pos_seq, Value};
use wugs_xml::{self as xml, XmlNode};

use crate::common::*;
use crate::lift::ordered_children;

const F_GUID: &str = "x.flex.guid";
const F_ITEMS: &str = "x.flex.items";
const F_LANGUAGES: &str = "x.flex.languages";
const F_PARAGRAPH: &str = "x.flex.paragraph";
const F_PARAGRAPH_GUID: &str = "x.flex.paragraph_guid";
const F_WORD_KIND: &str = "x.flex.kind";
const F_MORPH_TYPE: &str = "x.flex.type";
/// All attributes of the source element, re-emitted on export under the ones
/// wugs derives (FLEx's `media-file`, `speaker`, dates…).
const F_ATTRS: &str = "x.flex.attrs";
const F_PARAGRAPH_ATTRS: &str = "x.flex.paragraph_attrs";
/// Children of `<interlinear-text>` the importer does not model (for
/// example `<media-files>`), carried verbatim.
const F_EXTRA: &str = "x.flex.extra";

pub const TIER_BASELINE: &str = "flextext:baseline";
pub const TIER_FREE: &str = "flextext:free";

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: String,
    pub lang: String,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct ImportReport {
    /// One wugs text per `<interlinear-text>`, in document order.
    pub texts: Vec<String>,
    pub texts_created: usize,
    pub phrases_created: usize,
    pub phrases_skipped: usize,
    pub tokens_created: usize,
    pub morphs_created: usize,
    /// Morphs linked to the one lexicon sense whose lemma and gloss match.
    pub senses_linked: usize,
    pub links_ambiguous: usize,
    pub links_unmatched: usize,
    pub not_carried: NotCarried,
    pub commits: usize,
    pub findings: usize,
}

struct IText {
    origin: String,
    guid: Option<String>,
    attrs: Value,
    extra: Vec<Value>,
    items: Vec<Item>,
    languages: Vec<Value>,
    paragraphs: Vec<Para>,
}

struct Para {
    guid: Option<String>,
    attrs: Value,
    phrases: Vec<Phrase>,
}

struct Phrase {
    origin: String,
    guid: Option<String>,
    attrs: Value,
    begin: Option<f64>,
    end: Option<f64>,
    items: Vec<Item>,
    words: Vec<Word>,
}

struct Word {
    origin: String,
    guid: Option<String>,
    attrs: Value,
    items: Vec<Item>,
    morphs: Vec<Morph>,
}

struct Morph {
    origin: String,
    guid: Option<String>,
    attrs: Value,
    flex_type: Option<String>,
    items: Vec<Item>,
}

fn first<'a>(items: &'a [Item], kind: &str) -> Option<&'a Item> {
    items.iter().find(|i| i.kind == kind)
}

/// FLEx morph types -> DATA-MODEL Morph.type. The original is kept in
/// `x.flex.type` for export.
fn morph_type(flex: Option<&str>) -> &'static str {
    match flex.unwrap_or("stem") {
        "stem" | "root" | "bound root" | "bound stem" | "particle" | "phrase" => "root",
        "prefix" | "prefixing interfix" => "prefix",
        "suffix" | "suffixing interfix" => "suffix",
        "infix" | "infixing interfix" => "infix",
        "circumfix" => "circumfix",
        "clitic" | "enclitic" | "proclitic" => "clitic",
        _ => "other",
    }
}

/// DATA-MODEL Morph.type -> the FLEx type to export: the original when the
/// type was not changed in wugs, else the nearest FLEx name.
fn flex_morph_type(wugs: Option<&str>, original: Option<&str>) -> Option<String> {
    match (wugs, original) {
        (Some(w), Some(o)) if morph_type(Some(o)) == w => Some(o.to_string()),
        (Some("root"), _) => Some("stem".to_string()),
        (Some("other"), o) => o.map(str::to_string),
        (Some(w), _) => Some(w.to_string()),
        (None, o) => o.map(str::to_string),
    }
}

/// Import every `<interlinear-text>` in a FlexText document. Import the LIFT
/// lexicon first: morphs link to senses at import time.
///
/// Txns: one for the shared tiers (config scope), one per text creation, one
/// per phrase (segment, free translation, tokens and morphs; text scope).
/// Phrases already in the project, matched by origin, are skipped with their
/// whole subtree.
pub fn import_flextext(project: &mut Project, input: &str, title_fallback: &str) -> Result<ImportReport, ImportError> {
    let mut report = ImportReport::default();
    let texts = parse_flextext(input, &mut report)?;
    let (baseline, free) = ensure_tiers(project, &mut report)?;
    let senses = SenseIndex::build(project)?;
    let mut text_index = origin_index(project, "text")?;
    for t in crate::lift::live_of_kind(project, "text")? {
        text_index.entry(derived_guid(&t)).or_insert(t);
    }

    for it in texts {
        let text = match resolve_origin(project, &text_index, "text", &it.origin)? {
            Some(id) => id,
            None => {
                let title = first(&it.items, "title").map(|i| i.text.clone()).unwrap_or_else(|| title_fallback.to_string());
                let mut fields = vec![
                    ("title", Value::str(&title)),
                    ("kind", Value::str("unspecified")),
                    (F_ORIGIN, Value::str(&it.origin)),
                    (F_ITEMS, items_value(&it.items)),
                    (F_LANGUAGES, Value::Arr(it.languages.clone())),
                    (F_ATTRS, it.attrs.clone()),
                    (F_EXTRA, Value::Arr(it.extra.clone())),
                ];
                if let Some(g) = &it.guid {
                    fields.push((F_GUID, Value::str(g)));
                }
                let mut t = project.intent("import-flextext");
                let id = t.create_scope_root("text", "txt", fields);
                t.commit()?;
                report.texts_created += 1;
                report.commits += 1;
                id
            }
        };
        report.texts.push(text.clone());

        let existing: HashMap<String, String> = {
            let mut m = HashMap::new();
            for seg in project.proj.children_by_ref("text", &text, "segment")? {
                m.insert(derived_guid(&seg), seg.clone());
                if let Some(o) = project.proj.field_str(&seg, F_ORIGIN)? {
                    m.insert(o, seg);
                }
            }
            m
        };

        let mut phrase_no = 0;
        for (pi, para) in it.paragraphs.iter().enumerate() {
            for ph in &para.phrases {
                let pos = pos_seq(phrase_no);
                phrase_no += 1;
                if existing.contains_key(&ph.origin) {
                    report.phrases_skipped += 1;
                    continue;
                }
                let mut t = project.intent("import-flextext");
                let mut fields = vec![
                    ("text", Value::str(&text)),
                    ("tier", Value::str(&baseline)),
                    ("pos", Value::str(&pos)),
                    ("content", Value::str(&phrase_content(ph))),
                    (F_ORIGIN, Value::str(&ph.origin)),
                    (F_ITEMS, items_value(&ph.items)),
                    (F_PARAGRAPH, Value::Num(pi as f64)),
                    (F_PARAGRAPH_ATTRS, para.attrs.clone()),
                    (F_ATTRS, ph.attrs.clone()),
                ];
                if let Some(b) = ph.begin {
                    fields.push(("start", Value::Num(b)));
                }
                if let Some(e) = ph.end {
                    fields.push(("end", Value::Num(e)));
                }
                if let Some(g) = &ph.guid {
                    fields.push((F_GUID, Value::str(g)));
                }
                if let Some(g) = &para.guid {
                    fields.push((F_PARAGRAPH_GUID, Value::str(g)));
                }
                let seg = t.create("segment", "seg", &text, fields);
                if let Some(gls) = first(&ph.items, "gls") {
                    t.create(
                        "segment",
                        "seg",
                        &text,
                        vec![
                            ("text", Value::str(&text)),
                            ("tier", Value::str(&free)),
                            ("pos", Value::str(&pos)),
                            ("parent", Value::str(&seg)),
                            ("content", Value::str(&gls.text)),
                        ],
                    );
                }
                for (wi, w) in ph.words.iter().enumerate() {
                    let (kind, form) = match first(&w.items, "txt").or_else(|| first(&w.items, "punct")) {
                        Some(i) => (i.kind.as_str(), i.text.as_str()),
                        None => ("txt", ""),
                    };
                    let mut fields = vec![
                        ("segment", Value::str(&seg)),
                        ("pos", Value::str(&pos_seq(wi))),
                        ("form", Value::str(form)),
                        (F_WORD_KIND, Value::str(if kind == "punct" { "punct" } else { "word" })),
                        (F_ORIGIN, Value::str(&w.origin)),
                        (F_ITEMS, items_value(&w.items)),
                        (F_ATTRS, w.attrs.clone()),
                    ];
                    if let Some(g) = &w.guid {
                        fields.push((F_GUID, Value::str(g)));
                    }
                    let tok = t.create("token", "tok", &text, fields);
                    report.tokens_created += 1;
                    for (mi, m) in w.morphs.iter().enumerate() {
                        let surface = first(&m.items, "txt").map(|i| i.text.as_str()).unwrap_or("");
                        let mut fields = vec![
                            ("token", Value::str(&tok)),
                            ("pos", Value::str(&pos_seq(mi))),
                            ("surface", Value::str(surface)),
                            ("type", Value::str(morph_type(m.flex_type.as_deref()))),
                            (F_ORIGIN, Value::str(&m.origin)),
                            (F_ITEMS, items_value(&m.items)),
                            (F_ATTRS, m.attrs.clone()),
                        ];
                        let gloss = first(&m.items, "gls").map(|i| i.text.clone());
                        if let Some(g) = &gloss {
                            fields.push(("gloss", Value::str(g)));
                        }
                        if let Some(ft) = &m.flex_type {
                            fields.push((F_MORPH_TYPE, Value::str(ft)));
                        }
                        if let Some(g) = &m.guid {
                            fields.push((F_GUID, Value::str(g)));
                        }
                        let cf = first(&m.items, "cf").map(|i| i.text.as_str()).unwrap_or(surface);
                        match senses.lookup(cf, gloss.as_deref()) {
                            Link::One(sen) => {
                                fields.push(("sense", Value::str(&sen)));
                                report.senses_linked += 1;
                            }
                            Link::Many => report.links_ambiguous += 1,
                            Link::None => report.links_unmatched += 1,
                        }
                        t.create("morph", "mor", &text, fields);
                        report.morphs_created += 1;
                    }
                }
                let res = t.commit()?;
                report.phrases_created += 1;
                report.commits += 1;
                report.findings += res.findings.len();
            }
        }
    }
    Ok(report)
}

/// A phrase's own `txt` item if it has one, else its words joined the way
/// FLEx displays them: spaces between words, punctuation attached.
fn phrase_content(ph: &Phrase) -> String {
    if let Some(i) = first(&ph.items, "txt") {
        return i.text.clone();
    }
    let mut out = String::new();
    for w in &ph.words {
        if let Some(i) = first(&w.items, "txt") {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&i.text);
        } else if let Some(p) = first(&w.items, "punct") {
            out.push_str(&p.text);
        }
    }
    out
}

fn ensure_tiers(project: &mut Project, report: &mut ImportReport) -> Result<(String, String), ImportError> {
    let index = origin_index(project, "tier")?;
    if let (Some(b), Some(f)) = (index.get(TIER_BASELINE), index.get(TIER_FREE)) {
        return Ok((b.clone(), f.clone()));
    }
    let mut t = project.intent("import-flextext");
    let baseline = match index.get(TIER_BASELINE) {
        Some(b) => b.clone(),
        None => t.create(
            "tier",
            "tir",
            SCOPE_CONFIG,
            vec![
                ("name", Value::str("Baseline (FLEx)")),
                ("kind", Value::str("time-aligned")),
                (F_ORIGIN, Value::str(TIER_BASELINE)),
            ],
        ),
    };
    let free = match index.get(TIER_FREE) {
        Some(f) => f.clone(),
        None => t.create(
            "tier",
            "tir",
            SCOPE_CONFIG,
            vec![
                ("name", Value::str("Free translation (FLEx)")),
                ("kind", Value::str("symbolic-association")),
                ("parent", Value::str(&baseline)),
                (F_ORIGIN, Value::str(TIER_FREE)),
            ],
        ),
    };
    t.commit()?;
    report.commits += 1;
    Ok((baseline, free))
}

enum Link {
    One(String),
    Many,
    None,
}

/// (form, gloss) -> senses, where form is a lemma or citation form with and
/// without its affix markers.
struct SenseIndex(HashMap<(String, String), Vec<String>>);

fn strip_markers(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, '-' | '=' | '~'))
}

impl SenseIndex {
    fn build(project: &Project) -> Result<SenseIndex, String> {
        let p = &project.proj;
        let mut m: HashMap<(String, String), Vec<String>> = HashMap::new();
        for lex in crate::lift::live_of_kind(project, "lexeme")? {
            let mut forms: Vec<String> = Vec::new();
            for f in ["lemma", "citation"] {
                if let Some(v) = p.field_str(&lex, f)? {
                    forms.push(strip_markers(&v).to_string());
                }
            }
            forms.sort();
            forms.dedup();
            for sen in p.children_by_ref("lexeme", &lex, "sense")? {
                let gloss = p.field_str(&sen, "gloss")?.unwrap_or_default();
                for f in &forms {
                    m.entry((f.clone(), gloss.clone())).or_default().push(sen.clone());
                }
            }
        }
        Ok(SenseIndex(m))
    }

    fn lookup(&self, form: &str, gloss: Option<&str>) -> Link {
        let Some(g) = gloss else { return Link::None };
        match self.0.get(&(strip_markers(form).to_string(), g.to_string())).map(Vec::as_slice) {
            Some([one]) => Link::One(one.clone()),
            Some([_, _, ..]) => Link::Many,
            _ => Link::None,
        }
    }
}

// ------------------------------------------------------------------ parse

fn parse_flextext(input: &str, report: &mut ImportReport) -> Result<Vec<IText>, ImportError> {
    let root = xml::parse(input)?;
    if root.name != "document" {
        return Err(format_err(format!("root element is <{}>, expected <document>", root.name)));
    }
    let doc_key = format!("flextext-fnv:{:016x}", fnv1a(input));
    let mut out = Vec::new();
    for (ti, it) in root.children.iter().enumerate() {
        if it.name != "interlinear-text" {
            report.not_carried.add(&it.name);
            continue;
        }
        let guid = it.attr("guid").map(str::to_string);
        let origin = guid.clone().unwrap_or_else(|| format!("{doc_key}#{ti}"));
        let mut text = IText {
            origin,
            guid,
            attrs: attrs_value(it),
            extra: Vec::new(),
            items: Vec::new(),
            languages: Vec::new(),
            paragraphs: Vec::new(),
        };
        for c in &it.children {
            match c.name.as_str() {
                "item" => text.items.push(item(c, report)?),
                "paragraphs" => {
                    for (pi, p) in c.children_named("paragraph").enumerate() {
                        text.paragraphs.push(parse_para(p, &text.origin, pi, report)?);
                    }
                }
                "languages" => {
                    for l in c.children_named("language") {
                        text.languages.push(Value::Obj(
                            l.attrs.iter().map(|(k, v)| (k.clone(), Value::str(v))).collect(),
                        ));
                    }
                }
                _ => text.extra.push(node_value(c)),
            }
        }
        out.push(text);
    }
    let mut seen = std::collections::HashSet::new();
    for o in out
        .iter()
        .flat_map(|t| std::iter::once(&t.origin).chain(t.paragraphs.iter().flat_map(|p| p.phrases.iter().map(|ph| &ph.origin))))
    {
        if !seen.insert(o.clone()) {
            return Err(format_err(format!("duplicate text or phrase guid {o}")));
        }
    }
    Ok(out)
}

fn parse_para(p: &XmlNode, text: &str, pi: usize, report: &mut ImportReport) -> Result<Para, ImportError> {
    let guid = p.attr("guid").map(str::to_string);
    let mut para = Para {
        guid,
        attrs: attrs_value(p),
        phrases: Vec::new(),
    };
    for c in &p.children {
        match c.name.as_str() {
            "phrases" => {
                for (phi, ph) in c.children_named("phrase").enumerate() {
                    para.phrases.push(parse_phrase(ph, &format!("{text}#p{pi}.{phi}"), report)?);
                }
            }
            other => report.not_carried.add(&format!("paragraph/{other}")),
        }
    }
    Ok(para)
}

fn parse_phrase(ph: &XmlNode, fallback: &str, report: &mut ImportReport) -> Result<Phrase, ImportError> {
    let guid = ph.attr("guid").map(str::to_string);
    let origin = guid.clone().unwrap_or_else(|| fallback.to_string());
    let mut phrase = Phrase {
        begin: opt_ms(ph, "begin-time-offset")?,
        end: opt_ms(ph, "end-time-offset")?,
        origin,
        guid,
        attrs: attrs_value(ph),
        items: Vec::new(),
        words: Vec::new(),
    };
    for c in &ph.children {
        match c.name.as_str() {
            "item" => phrase.items.push(item(c, report)?),
            "words" => {
                for (wi, w) in c.children_named("word").enumerate() {
                    phrase.words.push(parse_word(w, &format!("{}#w{wi}", phrase.origin), report)?);
                }
            }
            other => report.not_carried.add(&format!("phrase/{other}")),
        }
    }
    Ok(phrase)
}

fn parse_word(w: &XmlNode, origin: &str, report: &mut ImportReport) -> Result<Word, ImportError> {
    let mut word = Word {
        origin: origin.to_string(),
        guid: w.attr("guid").map(str::to_string),
        attrs: attrs_value(w),
        items: Vec::new(),
        morphs: Vec::new(),
    };
    for c in &w.children {
        match c.name.as_str() {
            "item" => word.items.push(item(c, report)?),
            "morphemes" => {
                for (mi, m) in c.children_named("morph").enumerate() {
                    let mut morph = Morph {
                        origin: format!("{}#m{mi}", word.origin),
                        guid: m.attr("guid").map(str::to_string),
                        attrs: attrs_value(m),
                        flex_type: m.attr("type").map(str::to_string),
                        items: Vec::new(),
                    };
                    for mc in &m.children {
                        match mc.name.as_str() {
                            "item" => morph.items.push(item(mc, report)?),
                            other => report.not_carried.add(&format!("morph/{other}")),
                        }
                    }
                    word.morphs.push(morph);
                }
            }
            other => report.not_carried.add(&format!("word/{other}")),
        }
    }
    Ok(word)
}

fn item(node: &XmlNode, report: &mut ImportReport) -> Result<Item, ImportError> {
    for (k, _) in node.attrs.iter().filter(|(k, _)| k != "type" && k != "lang") {
        report.not_carried.add(&format!("item@{k}"));
    }
    for c in &node.children {
        report.not_carried.add(&format!("item/{}", c.name));
    }
    Ok(Item {
        kind: req(node, "type")?.to_string(),
        lang: node.attr("lang").unwrap_or("").to_string(),
        text: node.deep_text(),
    })
}

fn items_value(items: &[Item]) -> Value {
    Value::Arr(
        items
            .iter()
            .map(|i| Value::Arr(vec![Value::str(&i.kind), Value::str(&i.lang), Value::str(&i.text)]))
            .collect(),
    )
}

fn items_from(v: Option<Value>) -> Vec<Item> {
    let Some(Value::Arr(a)) = v else { return Vec::new() };
    a.iter()
        .filter_map(|x| match x {
            Value::Arr(p) if p.len() == 3 => Some(Item {
                kind: p[0].as_str()?.to_string(),
                lang: p[1].as_str()?.to_string(),
                text: p[2].as_str()?.to_string(),
            }),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------ export

/// How an entity's current semantic value overrides its stored items.
struct Primary<'a> {
    kind: &'a str,
    value: Option<String>,
    /// Owned: the semantic field is the authority, so a value with no item
    /// adds one and a cleared value removes the item. Not owned (a phrase
    /// whose text FLEx rebuilds from its words): only replace an existing item.
    owned: bool,
    default_lang: &'a str,
}

fn render_items(mut items: Vec<Item>, primaries: &[Primary]) -> Vec<Item> {
    for p in primaries {
        let at = items.iter().position(|i| i.kind == p.kind);
        match (p.value.as_deref().filter(|v| !v.is_empty()), at) {
            (Some(v), Some(i)) => items[i].text = v.to_string(),
            (Some(v), None) if p.owned => items.push(Item {
                kind: p.kind.to_string(),
                lang: p.default_lang.to_string(),
                text: v.to_string(),
            }),
            (None, Some(i)) if p.owned => {
                items.remove(i);
            }
            _ => {}
        }
    }
    items
}

fn item_node(i: &Item) -> XmlNode {
    XmlNode::new("item")
        .with_attr("type", &i.kind)
        .with_attr("lang", &i.lang)
        .with_text(&i.text)
}

/// Render one text as a FlexText document. Phrases are the text's segments
/// on the FLEx baseline tier, or, for a text that never came from FLEx, its
/// top-level segments. Reads the projection only.
pub fn export_flextext(project: &Project, text: &str) -> Result<String, String> {
    let p = &project.proj;
    if p.entity_kind(text)?.as_deref() != Some("text") {
        return Err(format!("{text} is not a text"));
    }
    let tiers = origin_index(project, "tier")?;
    let baseline = tiers.get(TIER_BASELINE);
    let free_tier = tiers.get(TIER_FREE);

    let all = ordered_children(project, "text", text, "segment")?;
    let on_baseline: Vec<String> = all
        .iter()
        .filter(|s| p.field_str(s, "tier").ok().flatten().as_ref() == baseline)
        .cloned()
        .collect();
    let phrases = if baseline.is_some() && !on_baseline.is_empty() {
        on_baseline
    } else {
        let mut v = Vec::new();
        for s in &all {
            if p.field_str(s, "parent")?.is_none() {
                v.push(s.clone());
            }
        }
        v
    };

    let vern = |items: &[Item]| items.iter().find(|i| i.kind == "txt").map(|i| i.lang.clone());

    let mut it = with_attrs(
        XmlNode::new("interlinear-text"),
        field_value(project, text, F_ATTRS)?,
        &[("guid", Some(p.field_str(text, F_GUID)?.unwrap_or_else(|| derived_guid(text))))],
    );
    for i in render_items(
        items_from(field_value(project, text, F_ITEMS)?),
        &[Primary { kind: "title", value: p.field_str(text, "title")?, owned: true, default_lang: "en" }],
    ) {
        it.children.push(item_node(&i));
    }

    let mut paragraphs = XmlNode::new("paragraphs");
    let mut current: Option<(i64, XmlNode)> = None;
    for seg in &phrases {
        let para_no = p.field_num(seg, F_PARAGRAPH)?.unwrap_or(0.0) as i64;
        if current.as_ref().map(|(n, _)| *n) != Some(para_no) {
            if let Some((_, node)) = current.take() {
                paragraphs.children.push(node);
            }
            let para = with_attrs(
                XmlNode::new("paragraph"),
                field_value(project, seg, F_PARAGRAPH_ATTRS)?,
                &[("guid", p.field_str(seg, F_PARAGRAPH_GUID)?)],
            );
            current = Some((para_no, para.with_child(XmlNode::new("phrases"))));
        }

        let mut ph = with_attrs(
            XmlNode::new("phrase"),
            field_value(project, seg, F_ATTRS)?,
            &[
                ("guid", Some(p.field_str(seg, F_GUID)?.unwrap_or_else(|| derived_guid(seg)))),
                ("begin-time-offset", p.field_num(seg, "start")?.map(ms)),
                ("end-time-offset", p.field_num(seg, "end")?.map(ms)),
            ],
        );
        let free = match free_tier {
            Some(ft) => {
                let mut f = None;
                for c in p.children_by_ref("parent", seg, "segment")? {
                    if p.field_str(&c, "tier")?.as_ref() == Some(ft) {
                        f = p.field_str(&c, "content")?;
                    }
                }
                f
            }
            None => None,
        };
        let stored = items_from(field_value(project, seg, F_ITEMS)?);
        let tokens = p.members(seg)?;
        let content = p.field_str(seg, "content")?;
        let items = render_items(
            stored,
            &[
                Primary { kind: "txt", value: content.clone(), owned: tokens.is_empty(), default_lang: "und" },
                Primary { kind: "gls", value: free, owned: true, default_lang: "en" },
            ],
        );
        let (before, after): (Vec<&Item>, Vec<&Item>) = items.iter().partition(|i| i.kind == "segnum");
        for i in before {
            ph.children.push(item_node(i));
        }

        let mut words = XmlNode::new("words");
        for tok in &tokens {
            let mut w = with_attrs(
                XmlNode::new("word"),
                field_value(project, tok, F_ATTRS)?,
                &[("guid", p.field_str(tok, F_GUID)?)],
            );
            let kind = if p.field_str(tok, F_WORD_KIND)?.as_deref() == Some("punct") { "punct" } else { "txt" };
            let stored = items_from(field_value(project, tok, F_ITEMS)?);
            let lang = vern(&stored).unwrap_or_else(|| "und".into());
            let items = render_items(
                stored,
                &[Primary { kind, value: p.field_str(tok, "form")?, owned: true, default_lang: &lang }],
            );
            let (head, tail): (Vec<&Item>, Vec<&Item>) = items.iter().partition(|i| i.kind == "txt" || i.kind == "punct");
            for i in head {
                w.children.push(item_node(i));
            }
            let morphs = p.members(tok)?;
            if !morphs.is_empty() {
                let mut ms_node = XmlNode::new("morphemes");
                for m in &morphs {
                    let flex_type = flex_morph_type(p.field_str(m, "type")?.as_deref(), p.field_str(m, F_MORPH_TYPE)?.as_deref());
                    let mut mn = with_attrs(
                        XmlNode::new("morph"),
                        field_value(project, m, F_ATTRS)?,
                        &[("type", flex_type), ("guid", p.field_str(m, F_GUID)?)],
                    );
                    for i in render_items(
                        items_from(field_value(project, m, F_ITEMS)?),
                        &[
                            Primary { kind: "txt", value: p.field_str(m, "surface")?, owned: true, default_lang: &lang },
                            Primary { kind: "gls", value: p.field_str(m, "gloss")?, owned: true, default_lang: "en" },
                        ],
                    ) {
                        mn.children.push(item_node(&i));
                    }
                    ms_node.children.push(mn);
                }
                w.children.push(ms_node);
            }
            for i in tail {
                w.children.push(item_node(i));
            }
            words.children.push(w);
        }
        if !words.children.is_empty() {
            ph.children.push(words);
        }
        for i in after {
            ph.children.push(item_node(i));
        }
        let (_, para) = current.as_mut().expect("paragraph opened above");
        para.children[0].children.push(ph);
    }
    if let Some((_, node)) = current.take() {
        paragraphs.children.push(node);
    }
    it.children.push(paragraphs);

    if let Some(Value::Arr(langs)) = field_value(project, text, F_LANGUAGES)? {
        if !langs.is_empty() {
            let mut ln = XmlNode::new("languages");
            for l in langs {
                let mut n = XmlNode::new("language");
                if let Value::Obj(m) = l {
                    for (k, v) in m {
                        if let Some(s) = v.as_str() {
                            n = n.with_attr(&k, s);
                        }
                    }
                }
                ln.children.push(n);
            }
            it.children.push(ln);
        }
    }
    if let Some(Value::Arr(extra)) = field_value(project, text, F_EXTRA)? {
        it.children.extend(extra.iter().filter_map(value_node));
    }

    Ok(XmlNode::new("document").with_attr("version", "2").with_child(it).to_xml())
}

fn ms(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        n.to_string()
    }
}
