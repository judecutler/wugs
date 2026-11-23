//! TEI in and out of a wugs project (roadmap 41, SCROLLS.md §1: "TEI and
//! the Tongues texts of the same project are read in one step").
//!
//! TEI (Text Encoding Initiative, P5) is how most curated corpora are
//! distributed: national and parliamentary corpora (ParlaMint), historical
//! collections, spoken corpora. This crate reads the part of TEI a corpus
//! linguist needs and leaves the rest of the document alone:
//!
//! - one text per `<TEI>` (P5, or P4's `<TEI.2>`), several from a
//!   `<teiCorpus>`; the title from `teiHeader/fileDesc/titleStmt/title`;
//! - a segment per `<s>`, or, where a block has no `<s>` inside, per `<p>`,
//!   `<u>` (spoken turns, `who` kept), `<l>`, `<ab>` or `<head>`;
//! - a token per `<w>` and `<pc>` (punctuation, `kind = "punct"`), with
//!   `lemma`, `pos` and `msd` stored in the sentence's `analysis` field in
//!   the shape `wugs-conllu` writes: a UD tag in `pos` (or ParlaMint's
//!   `msd="UPosTag=NOUN|Number=Sing"`) is UPOS, any other `pos` is XPOS,
//!   the rest of `msd` is FEATS. So a TEI corpus answers CQP queries and
//!   exports as CoNLL-U like a treebank.
//! - `<note>`, `<fw>`, `<figure>` and the header's other parts are not
//!   carried (counted in the report); `<lb/>`, `<pb/>` and `<cb/>` read as
//!   spaces.
//!
//! Export writes TEI P5 from the same sentences the CoNLL-U export makes,
//! so tags, lemmas and the glosses-to-lemmas rules are shared: one `<s>`
//! per segment, `<w>` and `<pc>` with `lemma`, `pos`, `msd` and
//! `join="right"` where no space follows, and each translation as a
//! `<note type="translation">` in the sentence.

use std::collections::{BTreeMap, HashMap};

use wugs_conllu::format::{self, Line};
use wugs_conllu::{ExportSentence, F_ANALYSIS, F_KIND};
use wugs_core::Project;
use wugs_model::{pos_seq, Value};
use wugs_xml::XmlNode;

/// UD's universal part-of-speech tags.
const UPOS: &[&str] = &["ADJ", "ADP", "ADV", "AUX", "CCONJ", "DET", "INTJ", "NOUN", "NUM", "PART", "PRON", "PROPN", "PUNCT", "SCONJ", "SYM", "VERB", "X"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub texts: Vec<String>,
    pub matched: Vec<String>,
    pub segments: usize,
    pub words: usize,
    /// Blocks with no `<s>` that came in a segment each.
    pub blocks_as_segments: usize,
    /// Elements left out, by name (`note` ×3).
    pub not_carried: BTreeMap<String, usize>,
}

/// One unit of a TEI text before it is written.
#[derive(Debug, Default)]
struct Unit {
    content: String,
    tokens: Vec<Tok>,
    id: Option<String>,
    who: Option<String>,
}

#[derive(Debug, Default)]
struct Tok {
    form: String,
    punct: bool,
    lemma: Option<String>,
    upos: Option<String>,
    xpos: Option<String>,
    feats: Option<String>,
    /// No space follows (`join="right"`, or the next token's `join="left"`).
    join_right: bool,
}

struct Doc {
    title: Option<String>,
    units: Vec<Unit>,
}

/// Elements whose text is not the text's own words.
const SKIPPED: &[&str] = &["note", "fw", "figure", "figDesc", "gap", "desc", "interp", "interpGrp", "linkGrp", "join", "spanGrp", "kinesic", "vocal", "incident", "pause", "anchor", "milestone"];
/// Elements that break a line or page: a space in the text.
const BREAKS: &[&str] = &["lb", "pb", "cb"];
/// Blocks that become a segment when no `<s>` is inside them.
const BLOCKS: &[&str] = &["p", "u", "l", "ab", "head", "seg", "quote", "item"];

fn local(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, l)| l)
}

fn attr<'a>(n: &'a XmlNode, key: &str) -> Option<&'a str> {
    n.attrs.iter().find(|(k, _)| local(k) == key || k == key).map(|(_, v)| v.as_str()).filter(|v| !v.trim().is_empty())
}

fn xml_id(n: &XmlNode) -> Option<String> {
    n.attr("xml:id").or_else(|| n.attr("id")).map(str::to_string)
}

/// The text of an element as words: breaks read as spaces, skipped
/// elements left out (counted), runs of white space as one space.
fn words_text(n: &XmlNode, skipped: &mut BTreeMap<String, usize>) -> String {
    fn go(n: &XmlNode, out: &mut String, skipped: &mut BTreeMap<String, usize>) {
        out.push_str(&n.head);
        for c in &n.children {
            let name = local(&c.name);
            if BREAKS.contains(&name) {
                out.push(' ');
            } else if SKIPPED.contains(&name) {
                *skipped.entry(name.to_string()).or_default() += 1;
            } else {
                go(c, out, skipped);
            }
            out.push_str(&c.tail);
        }
        if n.children.is_empty() && n.head.is_empty() {
            out.push_str(&n.text);
        }
    }
    let mut out = String::new();
    go(n, &mut out, skipped);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn has_descendant(n: &XmlNode, names: &[&str]) -> bool {
    n.children.iter().any(|c| names.contains(&local(&c.name)) || (!SKIPPED.contains(&local(&c.name)) && has_descendant(c, names)))
}

/// The tags of a `<w>`: lemma, then `pos` and `msd` as UPOS, XPOS and FEATS.
fn tags(w: &XmlNode, t: &mut Tok) {
    t.lemma = attr(w, "lemma").map(str::to_string);
    let pos = attr(w, "pos").map(str::to_string);
    let mut feats = Vec::new();
    if let Some(msd) = attr(w, "msd") {
        let mut plain = true;
        for item in msd.split('|') {
            match item.split_once('=') {
                Some(("UPosTag" | "UPOS" | "upos", v)) => {
                    t.upos = Some(v.to_string());
                    plain = false;
                }
                Some(_) => feats.push(item.to_string()),
                None => {}
            }
        }
        if plain && feats.is_empty() {
            // A tagset's own code (Multext-East `Ncmsn`), not features.
            t.xpos = Some(msd.to_string());
        }
    }
    match pos {
        Some(p) if UPOS.contains(&p.as_str()) && t.upos.is_none() => t.upos = Some(p),
        Some(p) if t.xpos.is_none() => t.xpos = Some(p),
        _ => {}
    }
    if !feats.is_empty() {
        t.feats = Some(feats.join("|"));
    }
}

/// The tokens under a unit, in document order.
fn tokens(n: &XmlNode, out: &mut Vec<Tok>, skipped: &mut BTreeMap<String, usize>) {
    for c in &n.children {
        let name = local(&c.name);
        if SKIPPED.contains(&name) {
            *skipped.entry(name.to_string()).or_default() += 1;
            continue;
        }
        if name == "w" || name == "pc" {
            let mut t = Tok { form: words_text(c, skipped), punct: name == "pc", ..Tok::default() };
            if t.form.is_empty() {
                continue;
            }
            if name == "w" {
                tags(c, &mut t);
            } else {
                t.upos = Some("PUNCT".into());
            }
            match attr(c, "join") {
                Some("right" | "both") => t.join_right = true,
                Some("left") => {
                    if let Some(prev) = out.last_mut() {
                        prev.join_right = true;
                    }
                }
                _ => {}
            }
            if attr(c, "join") == Some("both") {
                if let Some(prev) = out.last_mut() {
                    prev.join_right = true;
                }
            }
            out.push(t);
        } else {
            tokens(c, out, skipped);
        }
    }
}

/// A sentence read from its tokens: a space after each unless it joins the next.
fn joined(toks: &[Tok]) -> String {
    let mut s = String::new();
    for (i, t) in toks.iter().enumerate() {
        s.push_str(&t.form);
        if i + 1 < toks.len() && !t.join_right {
            s.push(' ');
        }
    }
    s
}

fn unit_of(n: &XmlNode, who: Option<String>, skipped: &mut BTreeMap<String, usize>) -> Unit {
    let mut toks = Vec::new();
    tokens(n, &mut toks, skipped);
    // A file with `join` marks spells out its spacing; without them the
    // text's own white space says where words meet.
    let explicit = toks.iter().any(|t| t.join_right);
    let content = if toks.is_empty() || !explicit { words_text(n, skipped) } else { joined(&toks) };
    let content = if content.is_empty() { joined(&toks) } else { content };
    Unit { content, tokens: toks, id: xml_id(n), who }
}

/// Every unit of a text body, in order.
fn walk(n: &XmlNode, who: Option<&str>, units: &mut Vec<Unit>, blocks: &mut usize, skipped: &mut BTreeMap<String, usize>) {
    for c in &n.children {
        let name = local(&c.name);
        if SKIPPED.contains(&name) {
            *skipped.entry(name.to_string()).or_default() += 1;
            continue;
        }
        let who_here = if name == "u" { attr(c, "who").map(|w| w.trim_start_matches('#').to_string()).or(who.map(str::to_string)) } else { who.map(str::to_string) };
        if name == "s" {
            let u = unit_of(c, who_here, skipped);
            if !u.content.is_empty() {
                units.push(u);
            }
        } else if BLOCKS.contains(&name) && !has_descendant(c, &["s"]) {
            let u = unit_of(c, who_here, skipped);
            if !u.content.is_empty() {
                *blocks += 1;
                units.push(u);
            }
        } else {
            walk(c, who_here.as_deref(), units, blocks, skipped);
        }
    }
}

fn title_of(tei: &XmlNode) -> Option<String> {
    let header = tei.children.iter().find(|c| local(&c.name) == "teiHeader")?;
    let file = header.children.iter().find(|c| local(&c.name) == "fileDesc")?;
    let stmt = file.children.iter().find(|c| local(&c.name) == "titleStmt")?;
    let title = stmt.children.iter().find(|c| local(&c.name) == "title")?;
    let mut ignore = BTreeMap::new();
    Some(words_text(title, &mut ignore)).filter(|t| !t.is_empty())
}

fn docs(root: &XmlNode, report: &mut ImportReport) -> Result<Vec<Doc>, String> {
    let mut out = Vec::new();
    let mut collect = |tei: &XmlNode, report: &mut ImportReport| {
        let mut units = Vec::new();
        for text in tei.children.iter().filter(|c| local(&c.name) == "text") {
            walk(text, None, &mut units, &mut report.blocks_as_segments, &mut report.not_carried);
        }
        out.push(Doc { title: title_of(tei), units });
    };
    match local(&root.name) {
        "TEI" | "TEI.2" => collect(root, report),
        "teiCorpus" | "teiCorpus.2" => {
            for c in root.children.iter().filter(|c| matches!(local(&c.name), "TEI" | "TEI.2")) {
                collect(c, report);
            }
        }
        other => return Err(format!("this is XML but not TEI: its root is <{other}>, where TEI starts with <TEI> or <teiCorpus>")),
    }
    Ok(out)
}

/// Import a TEI file: one text per `<TEI>`, one txn each. A text whose
/// title and segments match one already on `tier` adds nothing.
pub fn import_tei(p: &mut Project, input: &str, fallback_title: &str, tier: &str) -> Result<ImportReport, String> {
    let root = wugs_xml::parse(input).map_err(|e| format!("not well-formed XML ({e}), so nothing was imported"))?;
    let mut report = ImportReport::default();
    let documents = docs(&root, &mut report)?;
    if documents.iter().all(|d| d.units.is_empty()) {
        return Err("the TEI file has no text in its <body>; nothing was imported".into());
    }
    let several = documents.len() > 1;
    for (di, doc) in documents.into_iter().enumerate() {
        if doc.units.is_empty() {
            continue;
        }
        let title = p.proj.nfc(&doc.title.unwrap_or_else(|| if several { format!("{fallback_title} ({})", di + 1) } else { fallback_title.to_string() }));
        let contents: Vec<String> = doc.units.iter().map(|u| p.proj.nfc(&u.content)).collect();
        if let Some(t) = wugs_conllu::same_text(p, &title, tier, &contents)? {
            report.matched.push(t);
            continue;
        }
        let mut prepared = Vec::new();
        for (ui, (u, content)) in doc.units.iter().zip(&contents).enumerate() {
            let seg = wugs_model::new_id("seg");
            let ids: Vec<String> = u.tokens.iter().map(|_| wugs_model::new_id("tok")).collect();
            let mut fields: Vec<(&'static str, Value)> = vec![("tier", Value::str(tier)), ("pos", Value::str(&pos_seq(ui))), ("content", Value::str(content))];
            let tagged = u.tokens.iter().any(|t| t.lemma.is_some() || t.upos.is_some() || t.xpos.is_some() || t.feats.is_some() || t.join_right);
            if tagged {
                let rows = u
                    .tokens
                    .iter()
                    .zip(&ids)
                    .map(|(t, id)| {
                        let mut m = BTreeMap::new();
                        m.insert("tok".to_string(), Value::str(id));
                        let mut put = |k: &str, v: &Option<String>| {
                            if let Some(v) = v {
                                m.insert(k.to_string(), Value::Str(if k == "lemma" { p.proj.nfc(v) } else { format::column(v) }));
                            }
                        };
                        put("lemma", &t.lemma);
                        put("upos", &t.upos);
                        put("xpos", &t.xpos);
                        put("feats", &t.feats);
                        if t.join_right {
                            m.insert("misc".into(), Value::str("SpaceAfter=No"));
                        }
                        Value::Obj(m)
                    })
                    .collect();
                fields.push((F_ANALYSIS, Value::Arr(rows)));
            }
            if let Some(id) = &u.id {
                fields.push(("x.tei.id", Value::str(id)));
            }
            if let Some(who) = &u.who {
                fields.push(("x.tei.who", Value::str(who)));
            }
            let words: Vec<(String, String, bool)> = u.tokens.iter().zip(&ids).map(|(t, id)| (id.clone(), p.proj.nfc(&t.form), t.punct)).collect();
            prepared.push((seg, fields, words));
        }
        let mut t = p.intent("import-tei");
        let text = t.create_scope_root("text", "txt", vec![("title", Value::str(&title))]);
        for (seg, mut fields, words) in prepared {
            fields.insert(0, ("text", Value::str(&text)));
            t.create_with_id("segment", &seg, &text, fields);
            for (wi, (tok, form, punct)) in words.iter().enumerate() {
                let mut f = vec![("segment", Value::str(&seg)), ("pos", Value::str(&pos_seq(wi))), ("form", Value::str(form))];
                if *punct {
                    f.push((F_KIND, Value::str("punct")));
                }
                t.create_with_id("token", tok, &text, f);
                report.words += 1;
            }
            report.segments += 1;
        }
        t.commit()?;
        report.texts.push(text);
    }
    Ok(report)
}

/// Export sentences as one TEI P5 document. `language` is the project's
/// (name, code), written in `profileDesc/langUsage` and as the text's
/// `xml:lang`.
pub fn export_tei(p: &Project, title: &str, language: Option<(&str, &str)>, sentences: &[ExportSentence], generator: &str) -> Result<TeiExport, String> {
    let (parsed, mut notes) = wugs_conllu::export_sentences(p, title, sentences)?;
    let mut ranges = 0;
    let mut body = XmlNode::new("p");
    let mut ids: HashMap<String, usize> = HashMap::new();
    for (n_out, (si, s)) in parsed.iter().enumerate() {
        let es = &sentences[*si];
        let si = n_out;
        let id = p.proj.field_str(&es.segment, "x.tei.id")?.unwrap_or_else(|| format!("s{}", si + 1));
        let n = ids.entry(id.clone()).or_default();
        *n += 1;
        let id = if *n > 1 { format!("{id}-{n}") } else { id };
        let mut node = XmlNode::new("s").with_attr("xml:id", &id);
        if let Some(who) = p.proj.field_str(&es.segment, "x.tei.who")? {
            node = node.with_attr("who", &format!("#{who}"));
        }
        for line in &s.lines {
            match line {
                Line::Word { cols, .. } => {
                    let punct = cols[format::UPOS] == "PUNCT";
                    let mut w = XmlNode::new(if punct { "pc" } else { "w" });
                    if !punct && cols[format::LEMMA] != "_" {
                        w = w.with_attr("lemma", &cols[format::LEMMA]);
                    }
                    let pos = if cols[format::UPOS] != "_" { &cols[format::UPOS] } else { &cols[format::XPOS] };
                    if !punct && pos != "_" {
                        w = w.with_attr("pos", pos);
                    }
                    if cols[format::FEATS] != "_" {
                        w = w.with_attr("msd", &cols[format::FEATS]);
                    }
                    if format::misc_has(&cols[format::MISC], "SpaceAfter", "No") {
                        w = w.with_attr("join", "right");
                    }
                    node = node.with_child(w.with_text(&cols[format::FORM]));
                }
                Line::Range { .. } => ranges += 1,
                Line::Empty { .. } => {}
            }
        }
        for (lang, tr) in &es.translations {
            let mut note = XmlNode::new("note").with_attr("type", "translation");
            if let Some(l) = lang.as_deref().filter(|l| !l.is_empty()) {
                note = note.with_attr("xml:lang", l);
            }
            node = node.with_child(note.with_text(tr));
        }
        body = body.with_child(node);
    }
    if ranges > 0 {
        notes.push(format!("{ranges} multi-word {} written as {} syntactic words; TEI keeps no range.", if ranges == 1 { "token is" } else { "tokens are" }, if ranges == 1 { "its" } else { "their" }));
    }
    let mut header = XmlNode::new("teiHeader").with_child(
        XmlNode::new("fileDesc")
            .with_child(XmlNode::new("titleStmt").with_child(XmlNode::new("title").with_text(title)))
            .with_child(XmlNode::new("publicationStmt").with_child(XmlNode::new("p").with_text(&format!("Exported from wugs ({generator})."))))
            .with_child(XmlNode::new("sourceDesc").with_child(XmlNode::new("p").with_text("Born-digital; see the wugs project for recordings and history."))),
    );
    let mut text = XmlNode::new("text");
    if let Some((name, code)) = language {
        if !code.is_empty() {
            text = text.with_attr("xml:lang", code);
        }
        header = header.with_child(XmlNode::new("profileDesc").with_child(XmlNode::new("langUsage").with_child(XmlNode::new("language").with_attr("ident", code).with_text(name))));
    }
    let tei = XmlNode::new("TEI").with_attr("xmlns", "http://www.tei-c.org/ns/1.0").with_child(header).with_child(text.with_child(XmlNode::new("body").with_child(XmlNode::new("div").with_child(body))));
    Ok(TeiExport { tei: tei.to_xml(), notes })
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TeiExport {
    pub tei: String,
    /// What did not fit, one sentence each.
    pub notes: Vec<String>,
}
