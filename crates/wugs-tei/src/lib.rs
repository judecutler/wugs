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
    /// `<surface>`s of a `<facsimile>` that became page images of their
    /// text (each without its picture, which a TEI file does not hold), and
    /// segments whose `facs` named a zone on one.
    pub pages: usize,
    pub regions: usize,
}

/// One unit of a TEI text before it is written.
#[derive(Debug, Default)]
struct Unit {
    content: String,
    tokens: Vec<Tok>,
    id: Option<String>,
    who: Option<String>,
    /// The `facs` pointer of the element: the zone it stands on.
    facs: Option<String>,
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
    /// What the header says about the document, as metadata keys.
    meta: Vec<(String, String)>,
    units: Vec<Unit>,
    surfaces: Vec<Surface>,
}

/// A page of a `<facsimile>`: its image's address and its zones, each a
/// rectangle as fractions of the surface.
#[derive(Debug, Default)]
struct Surface {
    url: String,
    zones: Vec<(String, [f64; 4])>,
}

/// The pages of a TEI document's `<facsimile>`: each `<surface>` whose
/// extent is given (`ulx`/`uly`/`lrx`/`lry`, or a `<graphic>`'s `width` and
/// `height` in pixels) with its rectangular `<zone>`s, nested ones too.
fn surfaces_of(tei: &XmlNode) -> Vec<Surface> {
    let num = |n: &XmlNode, k: &str| attr(n, k).and_then(|v| v.trim().trim_end_matches("px").parse::<f64>().ok());
    let mut out = Vec::new();
    for f in tei.children.iter().filter(|c| local(&c.name) == "facsimile") {
        for s in f.children.iter().filter(|c| local(&c.name) == "surface") {
            let graphic = s.children.iter().find(|c| local(&c.name) == "graphic");
            let (x0, y0) = (num(s, "ulx").unwrap_or(0.0), num(s, "uly").unwrap_or(0.0));
            let size = match (num(s, "lrx"), num(s, "lry")) {
                (Some(x1), Some(y1)) => Some((x1 - x0, y1 - y0)),
                _ => graphic.and_then(|g| Some((num(g, "width")?, num(g, "height")?))),
            };
            let Some((w, h)) = size.filter(|(w, h)| *w > 0.0 && *h > 0.0) else { continue };
            let mut zones = Vec::new();
            let mut stack: Vec<&XmlNode> = s.children.iter().collect();
            while let Some(z) = stack.pop() {
                if local(&z.name) != "zone" {
                    continue;
                }
                stack.extend(z.children.iter());
                // A rectangle, or the box around a polygon's `points` (Transkribus's lines).
                let corners = match (num(z, "ulx"), num(z, "uly"), num(z, "lrx"), num(z, "lry")) {
                    (Some(a), Some(b), Some(c), Some(d)) => Some((a, b, c, d)),
                    _ => attr(z, "points").and_then(|pts| {
                        let xy: Vec<(f64, f64)> = pts.split_whitespace().filter_map(|p| p.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
                        let min = |f: fn(&(f64, f64)) -> f64| xy.iter().map(f).fold(f64::INFINITY, f64::min);
                        let max = |f: fn(&(f64, f64)) -> f64| xy.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
                        (xy.len() >= 2).then(|| (min(|p| p.0), min(|p| p.1), max(|p| p.0), max(|p| p.1)))
                    }),
                };
                let (Some(id), Some((a, b, c, d))) = (xml_id(z), corners) else { continue };
                let clamp = |v: f64| v.clamp(0.0, 1.0);
                let (l, t) = (clamp((a.min(c) - x0) / w), clamp((b.min(d) - y0) / h));
                let (r, bo) = (clamp((a.max(c) - x0) / w), clamp((b.max(d) - y0) / h));
                if r > l && bo > t {
                    zones.push((id, [l, t, r - l, bo - t]));
                }
            }
            let url = graphic.and_then(|g| attr(g, "url")).unwrap_or("").to_string();
            out.push(Surface { url, zones });
        }
    }
    out
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
    Unit { content, tokens: toks, id: xml_id(n), who, facs: attr(n, "facs").map(|f| f.trim().trim_start_matches('#').to_string()) }
}

/// A block's lines when its line breaks point at facsimile zones
/// (`<lb facs="#…"/>`, as Transkribus writes a page): a unit per line with
/// that zone, and the text before the first such break a unit of its own.
/// None when the block has no such breaks or holds tagged words.
fn facs_lines(n: &XmlNode, who: Option<&str>, skipped: &mut BTreeMap<String, usize>) -> Option<Vec<Unit>> {
    let is_lb = |c: &XmlNode| local(&c.name) == "lb" && attr(c, "facs").is_some();
    if !n.children.iter().any(is_lb) || has_descendant(n, &["w", "pc"]) {
        return None;
    }
    let mut out = Vec::new();
    let mut current = (None::<String>, n.head.clone());
    let finish = |line: (Option<String>, String), out: &mut Vec<Unit>| {
        let content = line.1.split_whitespace().collect::<Vec<_>>().join(" ");
        if !content.is_empty() {
            out.push(Unit { content, tokens: Vec::new(), id: None, who: who.map(str::to_string), facs: line.0 });
        }
    };
    for c in &n.children {
        if is_lb(c) {
            let next = (attr(c, "facs").map(|f| f.trim().trim_start_matches('#').to_string()), String::new());
            finish(std::mem::replace(&mut current, next), &mut out);
        } else {
            let name = local(&c.name);
            if BREAKS.contains(&name) {
                current.1.push(' ');
            } else if SKIPPED.contains(&name) {
                *skipped.entry(name.to_string()).or_default() += 1;
            } else {
                current.1.push(' ');
                current.1.push_str(&words_text(c, skipped));
                current.1.push(' ');
            }
        }
        current.1.push_str(&c.tail);
    }
    finish(current, &mut out);
    Some(out)
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
            if let Some(lines) = facs_lines(c, who_here.as_deref(), skipped) {
                *blocks += 1;
                units.extend(lines);
                continue;
            }
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

/// Children of `n` with this local name.
fn kids<'a>(n: &'a XmlNode, name: &'a str) -> impl Iterator<Item = &'a XmlNode> + 'a {
    n.children.iter().filter(move |c| local(&c.name) == name)
}

/// The first node down a path of local names.
fn at<'a>(mut n: &'a XmlNode, path: &[&str]) -> Option<&'a XmlNode> {
    for name in path {
        n = n.children.iter().find(|c| local(&c.name) == *name)?;
    }
    Some(n)
}

fn text_of(n: &XmlNode) -> String {
    words_text(n, &mut BTreeMap::new())
}

/// A date element: its `when` (or `from`–`to`), else its text.
fn date_of(n: &XmlNode) -> Option<String> {
    let v = match (attr(n, "when"), attr(n, "from"), attr(n, "to")) {
        (Some(w), _, _) => w.to_string(),
        (None, Some(f), Some(t)) => format!("{f}/{t}"),
        _ => text_of(n),
    };
    Some(v).filter(|v| !v.is_empty())
}

/// Document metadata from a `teiHeader`, under the keys corpus tools use:
/// author, editor, date (the text's creation date, else its source's, else
/// publication), publisher, licence, idno, source (the source's title),
/// language (`langUsage` idents), genre (a `catRef` or `term` whose scheme
/// or type names a genre), keywords and category (other terms and
/// categories), speakers (`particDesc` names) and place (`settingDesc`).
/// Several values of one key are joined with "; ".
fn header_meta(tei: &XmlNode) -> Vec<(String, String)> {
    let Some(h) = kids(tei, "teiHeader").next() else { return Vec::new() };
    let mut m: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    fn put(m: &mut BTreeMap<&'static str, Vec<String>>, k: &'static str, v: Option<String>) {
        if let Some(v) = v.map(|v| v.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|v| !v.is_empty()) {
            let e = m.entry(k).or_default();
            if !e.contains(&v) {
                e.push(v);
            }
        }
    }
    let file = at(h, &["fileDesc"]);
    if let Some(stmt) = file.and_then(|f| at(f, &["titleStmt"])) {
        kids(stmt, "author").for_each(|a| put(&mut m, "author", Some(text_of(a))));
        kids(stmt, "editor").for_each(|a| put(&mut m, "editor", Some(text_of(a))));
    }
    let mut extra: Vec<(String, String)> = Vec::new();
    if let Some(ns) = file.and_then(|f| at(f, &["notesStmt"])) {
        for n in kids(ns, "note").filter(|n| attr(n, "type") == Some("meta")) {
            if let Some(k) = attr(n, "n") {
                extra.push((k.to_string(), text_of(n)));
            }
        }
    }
    let publication = file.and_then(|f| at(f, &["publicationStmt"]));
    if let Some(ps) = publication {
        kids(ps, "publisher").for_each(|n| put(&mut m, "publisher", Some(text_of(n))));
        kids(ps, "idno").for_each(|n| put(&mut m, "idno", Some(text_of(n))));
        if let Some(av) = at(ps, &["availability"]) {
            for l in kids(av, "licence") {
                put(&mut m, "licence", attr(l, "target").map(str::to_string).or_else(|| Some(text_of(l))));
            }
            if attr(av, "status").is_some() && kids(av, "licence").next().is_none() {
                put(&mut m, "licence", attr(av, "status").map(str::to_string));
            }
        }
    }
    let bibl = file.and_then(|f| at(f, &["sourceDesc"])).and_then(|s| kids(s, "bibl").chain(kids(s, "biblStruct")).chain(kids(s, "biblFull")).next());
    if let Some(b) = bibl {
        if !m.contains_key("author") {
            kids(b, "author").for_each(|a| put(&mut m, "author", Some(text_of(a))));
        }
        kids(b, "title").take(1).for_each(|t| put(&mut m, "source", Some(text_of(t))));
    }
    let profile = at(h, &["profileDesc"]);
    let dates = [
        profile.and_then(|p| at(p, &["creation", "date"])),
        bibl.and_then(|b| kids(b, "date").next()),
        publication.and_then(|p| kids(p, "date").next()),
    ];
    if let Some(d) = dates.into_iter().flatten().find_map(date_of) {
        put(&mut m, "date", Some(d));
    }
    if let Some(p) = profile {
        if let Some(lu) = at(p, &["langUsage"]) {
            kids(lu, "language").for_each(|l| put(&mut m, "language", attr(l, "ident").map(str::to_string).or_else(|| Some(text_of(l)))));
        }
        if let Some(tc) = at(p, &["textClass"]) {
            let genre = |n: &XmlNode| [attr(n, "scheme"), attr(n, "type")].into_iter().flatten().any(|s| s.to_lowercase().contains("genre"));
            for c in kids(tc, "catRef") {
                let targets: Vec<String> = attr(c, "target").unwrap_or_default().split_whitespace().map(|t| t.trim_start_matches('#').to_string()).collect();
                for v in targets {
                    put(&mut m, if genre(c) { "genre" } else { "category" }, Some(v));
                }
            }
            for k in kids(tc, "keywords") {
                for term in kids(k, "term") {
                    put(&mut m, if genre(k) || genre(term) { "genre" } else { "keywords" }, Some(text_of(term)));
                }
            }
            kids(tc, "classCode").for_each(|c| put(&mut m, "category", Some(text_of(c))));
        }
        if let Some(pd) = at(p, &["particDesc"]) {
            for person in kids(pd, "person").chain(kids(pd, "listPerson").flat_map(|l| kids(l, "person"))) {
                put(&mut m, "speakers", kids(person, "persName").next().map(text_of).or_else(|| attr(person, "id").map(str::to_string)));
            }
        }
        if let Some(sd) = at(p, &["settingDesc"]) {
            for s in kids(sd, "setting") {
                kids(s, "placeName").chain(kids(s, "place")).chain(kids(s, "name")).for_each(|n| put(&mut m, "place", Some(text_of(n))));
            }
        }
    }
    let mut out: Vec<(String, String)> = m.into_iter().map(|(k, v)| (k.to_string(), v.join("; "))).collect();
    for (k, v) in extra {
        if !v.is_empty() && !out.iter().any(|(o, _)| *o == k) {
            out.push((k, v));
        }
    }
    out
}

fn docs(root: &XmlNode, report: &mut ImportReport) -> Result<Vec<Doc>, String> {
    let mut out = Vec::new();
    let mut collect = |tei: &XmlNode, report: &mut ImportReport| {
        let mut units = Vec::new();
        for text in tei.children.iter().filter(|c| local(&c.name) == "text") {
            walk(text, None, &mut units, &mut report.blocks_as_segments, &mut report.not_carried);
        }
        out.push(Doc { title: title_of(tei), meta: header_meta(tei), units, surfaces: surfaces_of(tei) });
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
        let meta: Vec<(String, Value)> = doc.meta.iter().filter_map(|(k, v)| Some((format!("{}{}", wugs_core::metadata::PREFIX, wugs_core::metadata::key_of(k)?), Value::Str(p.proj.nfc(v))))).collect();
        if let Some(t) = wugs_conllu::same_text(p, &title, tier, &contents)? {
            // The same document again: its header may have grown, and only what changed is written.
            let rows: Vec<(String, String, String)> = doc.meta.iter().map(|(k, v)| (t.clone(), k.clone(), v.clone())).collect();
            p.import_text_metadata(&rows)?;
            report.matched.push(t);
            continue;
        }
        // The facsimile's pages, when units point at their zones: an image
        // source per surface (made before the text, as sources are scopes of
        // their own) and each unit's zone as its region.
        let zone_at: HashMap<&str, (usize, [f64; 4])> = doc.surfaces.iter().enumerate().flat_map(|(si, s)| s.zones.iter().map(move |(id, r)| (id.as_str(), (si, *r)))).collect();
        let used = doc.units.iter().any(|u| u.facs.as_deref().is_some_and(|f| zone_at.contains_key(f)));
        let mut page_ids = Vec::new();
        if used {
            for (si, s) in doc.surfaces.iter().enumerate() {
                let name = s.url.rsplit(['/', '\\']).next().filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| format!("page {}", si + 1));
                page_ids.push(p.add_source(&name, None, "image", None)?);
            }
            report.pages += page_ids.len();
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
            if let Some((si, [x, y, w, h])) = u.facs.as_deref().and_then(|f| zone_at.get(f)).filter(|_| used) {
                let round = |v: f64| (v * 1e5).round() / 1e5;
                let (x, y) = (round(*x), round(*y));
                let (w, h) = (round(*w).min(1.0 - x), round(*h).min(1.0 - y));
                fields.push(("region", Value::obj(vec![("source", Value::str(&page_ids[*si])), ("x", Value::Num(x)), ("y", Value::Num(y)), ("w", Value::Num(w)), ("h", Value::Num(h))])));
                report.regions += 1;
            }
            let words: Vec<(String, String, bool)> = u.tokens.iter().zip(&ids).map(|(t, id)| (id.clone(), p.proj.nfc(&t.form), t.punct)).collect();
            prepared.push((seg, fields, words));
        }
        let mut t = p.intent("import-tei");
        let mut text_fields = vec![("title", Value::str(&title))];
        if !page_ids.is_empty() {
            text_fields.push(("pages", Value::Arr(page_ids.iter().map(|s| Value::str(s)).collect())));
        }
        text_fields.extend(meta.iter().map(|(k, v)| (k.as_str(), v.clone())));
        let text = t.create_scope_root("text", "txt", text_fields);
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

/// The side of a page in a `<facsimile>`'s coordinates: zones are written
/// as shares of the page in ten-thousandths.
const FACS_SCALE: f64 = 10_000.0;

/// Export sentences as one TEI P5 document. `language` is the project's
/// (name, code), written in `profileDesc/langUsage` and as the text's
/// `xml:lang`.
pub fn export_tei(p: &Project, title: &str, language: Option<(&str, &str)>, sentences: &[ExportSentence], generator: &str) -> Result<TeiExport, String> {
    let (parsed, mut notes) = wugs_conllu::export_sentences(p, title, sentences)?;
    let mut ranges = 0;
    let mut body = XmlNode::new("p");
    let mut ids: HashMap<String, usize> = HashMap::new();
    // The text's page images: a sentence marked on one points at its zone.
    let text_id = match sentences.first() {
        Some(s) => p.proj.field_str(&s.segment, "text")?,
        None => None,
    };
    let pages = match &text_id {
        Some(t) => p.text_pages(t)?,
        None => Vec::new(),
    };
    let mut zones: Vec<Vec<XmlNode>> = vec![Vec::new(); pages.len()];
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
        if let Some(r) = p.segment_region(&es.segment)? {
            if let Some(pi) = pages.iter().position(|g| *g == r.source) {
                let zone = format!("z-{id}");
                let at = |v: f64| format!("{}", (v * FACS_SCALE).round() as i64);
                zones[pi].push(XmlNode::new("zone").with_attr("xml:id", &zone).with_attr("ulx", &at(r.x)).with_attr("uly", &at(r.y)).with_attr("lrx", &at(r.x + r.w)).with_attr("lry", &at(r.y + r.h)));
                node = node.with_attr("facs", &format!("#{zone}"));
            }
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
    // The text's own metadata, where TEI keeps each kind (and what TEI has
    // no place for as `notesStmt` notes named by key), so it comes back in.
    let meta = match &text_id {
        Some(t) => p.text_metadata(t)?,
        None => Vec::new(),
    };
    let get = |k: &str| meta.iter().find(|(m, _)| m == k).map(|(_, v)| v.as_str());
    let many = |k: &str| get(k).map(|v| v.split("; ").map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
    let mut title_stmt = XmlNode::new("titleStmt").with_child(XmlNode::new("title").with_text(title));
    for a in many("author") {
        title_stmt = title_stmt.with_child(XmlNode::new("author").with_text(&a));
    }
    for a in many("editor") {
        title_stmt = title_stmt.with_child(XmlNode::new("editor").with_text(&a));
    }
    let mut publication = XmlNode::new("publicationStmt").with_child(XmlNode::new("p").with_text(&format!("Exported from wugs ({generator}).")));
    if let Some(l) = get("licence") {
        let licence = if l.starts_with("http://") || l.starts_with("https://") { XmlNode::new("licence").with_attr("target", l) } else { XmlNode::new("licence").with_text(l) };
        publication = publication.with_child(XmlNode::new("availability").with_child(licence));
    }
    const PLACED: [&str; 10] = ["author", "editor", "licence", "date", "genre", "keywords", "language", "speakers", "place", "source"];
    let mut file = XmlNode::new("fileDesc").with_child(title_stmt).with_child(publication);
    let others: Vec<&(String, String)> = meta.iter().filter(|(k, _)| !PLACED.contains(&k.as_str())).collect();
    if !others.is_empty() {
        let mut notes_stmt = XmlNode::new("notesStmt");
        for (k, v) in others {
            notes_stmt = notes_stmt.with_child(XmlNode::new("note").with_attr("type", "meta").with_attr("n", k).with_text(v));
        }
        file = file.with_child(notes_stmt);
    }
    let mut source = XmlNode::new("sourceDesc").with_child(XmlNode::new("p").with_text("Born-digital; see the wugs project for recordings and history."));
    if let Some(s) = get("source") {
        source = XmlNode::new("sourceDesc").with_child(XmlNode::new("bibl").with_child(XmlNode::new("title").with_text(s)));
    }
    let mut header = XmlNode::new("teiHeader").with_child(file.with_child(source));
    let mut text = XmlNode::new("text");
    let mut profile = XmlNode::new("profileDesc");
    let mut has_profile = false;
    if let Some(d) = get("date") {
        let iso = d.len() >= 4 && d.chars().all(|c| c.is_ascii_digit() || c == '-') && d.chars().take(4).all(|c| c.is_ascii_digit());
        let date = if iso { XmlNode::new("date").with_attr("when", d) } else { XmlNode::new("date").with_text(d) };
        profile = profile.with_child(XmlNode::new("creation").with_child(date));
        has_profile = true;
    }
    if let Some((name, code)) = language {
        if !code.is_empty() {
            text = text.with_attr("xml:lang", code);
        }
        profile = profile.with_child(XmlNode::new("langUsage").with_child(XmlNode::new("language").with_attr("ident", code).with_text(name)));
        has_profile = true;
    }
    let (genre, keywords) = (many("genre"), many("keywords"));
    if !genre.is_empty() || !keywords.is_empty() {
        let mut class = XmlNode::new("textClass");
        for (scheme, terms) in [("genre", genre), ("", keywords)] {
            if terms.is_empty() {
                continue;
            }
            let mut k = XmlNode::new("keywords");
            if !scheme.is_empty() {
                k = k.with_attr("scheme", "#genre");
            }
            for term in terms {
                k = k.with_child(XmlNode::new("term").with_text(&term));
            }
            class = class.with_child(k);
        }
        profile = profile.with_child(class);
        has_profile = true;
    }
    let speakers = many("speakers");
    if !speakers.is_empty() {
        let mut list = XmlNode::new("listPerson");
        for sp in speakers {
            list = list.with_child(XmlNode::new("person").with_child(XmlNode::new("persName").with_text(&sp)));
        }
        profile = profile.with_child(XmlNode::new("particDesc").with_child(list));
        has_profile = true;
    }
    if let Some(pl) = get("place") {
        profile = profile.with_child(XmlNode::new("settingDesc").with_child(XmlNode::new("setting").with_child(XmlNode::new("placeName").with_text(pl))));
        has_profile = true;
    }
    if has_profile {
        header = header.with_child(profile);
    }
    let mut tei = XmlNode::new("TEI").with_attr("xmlns", "http://www.tei-c.org/ns/1.0").with_child(header);
    if zones.iter().any(|z| !z.is_empty()) {
        // Each page as a surface FACS_SCALE units square, so zones are
        // shares of the page whatever the image's size.
        let mut facs = XmlNode::new("facsimile");
        let side = format!("{}", FACS_SCALE as i64);
        for (i, (page, zs)) in pages.iter().zip(zones).enumerate() {
            let label = p.proj.field_str(page, "label")?.unwrap_or_default();
            let mut surface = XmlNode::new("surface").with_attr("xml:id", &format!("page-{}", i + 1)).with_attr("n", &format!("{}", i + 1)).with_attr("ulx", "0").with_attr("uly", "0").with_attr("lrx", &side).with_attr("lry", &side);
            surface = surface.with_child(XmlNode::new("graphic").with_attr("url", &label));
            for z in zs {
                surface = surface.with_child(z);
            }
            facs = facs.with_child(surface);
        }
        tei = tei.with_child(facs);
    }
    let tei = tei.with_child(text.with_child(XmlNode::new("body").with_child(XmlNode::new("div").with_child(body))));
    Ok(TeiExport { tei: tei.to_xml(), notes })
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TeiExport {
    pub tei: String,
    /// What did not fit, one sentence each.
    pub notes: Vec<String>,
}
