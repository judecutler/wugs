//! CoNLL-U in and out of a wugs project (roadmap 41, `docs/design/SCROLLS.md` §2).
//!
//! A document is a `text`; each sentence is a segment on the tier the caller
//! names, its syntactic words are `token`s (`form`, and `kind = "punct"` for
//! UPOS PUNCT, so the corpus tools do not count them as words). Everything
//! else stays with the sentence so a file comes back as it went in:
//!
//! - `analysis`: one JSON array per sentence, a row per word or empty node
//!   in file order. A word's row names its token (`tok`); an empty node's
//!   carries its `id` (`8.1`) and `form`. Columns are keyed by
//!   [`format::NAMES`] and left out when `_`. HEAD is the head's token id
//!   (or `0`), so a retokenize that keeps token ids keeps the tree.
//! - `mwt`: multi-word tokens as `{first, last}` token ids plus their columns.
//! - `comments`: every `#` line as written.
//!
//! One annotation op per sentence keeps a 10M-token corpus near one op per
//! sentence plus one per token (design §2, "Scale first").

pub mod format;

use std::collections::{BTreeMap, HashMap};

use format::{Cols, Line, Sentence};
use wugs_core::Project;
use wugs_model::{pos_seq, Value};

/// Fields this crate writes on a segment.
pub const F_ANALYSIS: &str = "analysis";
pub const F_MWT: &str = "mwt";
pub const F_COMMENTS: &str = "comments";
/// A token's kind: `punct` for punctuation, absent for a word.
pub const F_KIND: &str = "kind";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// The texts made, in file order.
    pub texts: Vec<String>,
    /// Documents that matched a text already here (same title and sentences).
    pub matched: Vec<String>,
    pub sentences: usize,
    pub words: usize,
    pub ranges: usize,
    pub empty_nodes: usize,
}

/// A document of the file: its title and sentences.
struct Doc {
    title: String,
    sentences: Vec<Sentence>,
}

/// Split at `# newdoc` (the title is its `id`, else the file's name with a
/// number), or the whole file as one document named `fallback`.
fn documents(sentences: Vec<Sentence>, fallback: &str) -> Vec<Doc> {
    let mut docs: Vec<Doc> = Vec::new();
    for s in sentences {
        let newdoc = s.comments.iter().find_map(|c| {
            let t = c.trim_start();
            t.strip_prefix("newdoc").filter(|r| r.is_empty() || r.starts_with([' ', '='])).map(|_| format::comment_value(c, "newdoc id").map(str::to_string))
        });
        match (newdoc, docs.last_mut()) {
            (None, Some(d)) => d.sentences.push(s),
            (newdoc, _) => {
                let n = docs.len() + 1;
                let title = newdoc.flatten().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| if n == 1 { fallback.to_string() } else { format!("{fallback} ({n})") });
                docs.push(Doc { title, sentences: vec![s] });
            }
        }
    }
    docs
}

/// What a sentence's segment shows: its `# text`, else its surface.
fn content_of(s: &Sentence) -> String {
    s.comment("text").map(str::to_string).unwrap_or_else(|| s.surface())
}

/// Import a CoNLL-U file: one text per document, one txn each. A document
/// whose title and sentences match a text already on `tier` adds nothing.
pub fn import_conllu(p: &mut Project, input: &str, fallback_title: &str, tier: &str) -> Result<ImportReport, String> {
    let sentences = format::parse(input).map_err(|problems| {
        let shown: Vec<String> = problems.iter().map(ToString::to_string).collect();
        format!("this is not valid CoNLL-U, so nothing was imported: {}", shown.join("; "))
    })?;
    if sentences.is_empty() {
        return Err("the file has no sentences; nothing was imported".into());
    }
    let mut report = ImportReport::default();
    for doc in documents(sentences, fallback_title) {
        let title = p.proj.nfc(&doc.title);
        let contents: Vec<String> = doc.sentences.iter().map(|s| p.proj.nfc(content_of(s).trim())).collect();
        if let Some(t) = same_text(p, &title, tier, &contents)? {
            report.matched.push(t);
            continue;
        }
        // Everything is worked out first; the txn only writes.
        let mut prepared = Vec::new();
        for (si, (s, content)) in doc.sentences.iter().zip(&contents).enumerate() {
            let seg = wugs_model::new_id("seg");
            let tok_of: HashMap<u32, String> = s.words().map(|(id, _)| (id, wugs_model::new_id("tok"))).collect();
            let mut rows = Vec::new();
            let mut ranges = Vec::new();
            for line in &s.lines {
                match line {
                    Line::Word { id, cols } => rows.push(row(Some(&tok_of[id]), None, cols, &tok_of, p)),
                    Line::Empty { major, minor, cols } => {
                        rows.push(row(None, Some(&format!("{major}.{minor}")), cols, &tok_of, p));
                        report.empty_nodes += 1;
                    }
                    Line::Range { first, last, cols } => {
                        let mut m = columns(cols, &[], p);
                        m.insert("first".into(), Value::str(&tok_of[first]));
                        m.insert("last".into(), Value::str(&tok_of[last]));
                        ranges.push(Value::Obj(m));
                        report.ranges += 1;
                    }
                }
            }
            let mut fields: Vec<(&'static str, Value)> = vec![("tier", Value::str(tier)), ("pos", Value::str(&pos_seq(si))), ("content", Value::str(content)), (F_ANALYSIS, Value::Arr(rows))];
            if !ranges.is_empty() {
                fields.push((F_MWT, Value::Arr(ranges)));
            }
            if !s.comments.is_empty() {
                fields.push((F_COMMENTS, Value::Arr(s.comments.iter().map(|c| Value::str(c)).collect())));
            }
            let words: Vec<(String, String, bool)> = s.words().map(|(id, cols)| (tok_of[&id].clone(), cols[format::FORM].clone(), cols[format::UPOS] == "PUNCT")).collect();
            prepared.push((seg, fields, words));
        }
        let mut t = p.intent("import-conllu");
        let text = t.create_scope_root("text", "txt", vec![("title", Value::str(&title))]);
        for (seg, mut fields, words) in prepared {
            fields.insert(0, ("text", Value::str(&text)));
            t.create_with_id("segment", &seg, &text, fields);
            for (wi, (tok, form, punct)) in words.iter().enumerate() {
                let mut fields = vec![("segment", Value::str(&seg)), ("pos", Value::str(&pos_seq(wi))), ("form", Value::str(form))];
                if *punct {
                    fields.push((F_KIND, Value::str("punct")));
                }
                t.create_with_id("token", tok, &text, fields);
                report.words += 1;
            }
            report.sentences += 1;
        }
        t.commit()?;
        report.texts.push(text);
    }
    Ok(report)
}

/// A row of `analysis`: the columns that are not `_` (FORM only for empty
/// nodes; a word's form is its token's), HEAD as a token id.
fn row(tok: Option<&str>, empty_id: Option<&str>, cols: &Cols, tok_of: &HashMap<u32, String>, p: &Project) -> Value {
    let skip: &[usize] = if tok.is_some() { &[format::FORM, format::HEAD] } else { &[format::HEAD] };
    let mut m = columns(cols, skip, p);
    if let Some(t) = tok {
        m.insert("tok".into(), Value::str(t));
    }
    if let Some(id) = empty_id {
        m.insert("id".into(), Value::str(id));
    }
    let head = &cols[format::HEAD];
    if head != "_" {
        let id = head.parse::<u32>().ok().and_then(|h| if h == 0 { Some("0".to_string()) } else { tok_of.get(&h).cloned() });
        m.insert("head".into(), Value::str(&id.unwrap_or_else(|| head.clone())));
    }
    Value::Obj(m)
}

fn columns(cols: &Cols, skip: &[usize], p: &Project) -> BTreeMap<String, Value> {
    let mut m = BTreeMap::new();
    for (k, c) in cols.iter().enumerate() {
        if c != "_" && !skip.contains(&k) {
            // Lemmas and forms are language data: NFC like every form in wugs.
            let v = if k == format::LEMMA || k == format::FORM { p.proj.nfc(c) } else { c.clone() };
            m.insert(format::NAMES[k].to_string(), Value::Str(v));
        }
    }
    m
}

/// A live text titled `title` whose segments on `tier` read `contents` (an
/// importer's "the same file again adds nothing"; TEI uses it too).
pub fn same_text(p: &Project, title: &str, tier: &str, contents: &[String]) -> Result<Option<String>, String> {
    for t in p.proj.entities_of_kind("text")? {
        if p.proj.is_tombstoned(&t)? || p.proj.field_str(&t, "title")?.as_deref() != Some(title) {
            continue;
        }
        let mut held = Vec::new();
        for seg in p.proj.ordered_children("text", &t, "segment")? {
            if p.proj.field_str(&seg, "tier")?.as_deref() == Some(tier) {
                held.push(p.proj.field_str(&seg, "content")?.unwrap_or_default());
            }
        }
        if held == contents {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

/// One sentence to export: its segment, and its translations
/// (`(language code, text)`; written as `# text_<code>`, or
/// `# translation` when the code is unknown).
pub struct ExportSentence {
    pub segment: String,
    pub translations: Vec<(Option<String>, String)>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Exported {
    pub conllu: String,
    /// What did not fit, one sentence each, for the person to read.
    pub notes: Vec<String>,
}

/// Export sentences as CoNLL-U. A sentence imported from CoNLL-U comes back
/// as it went in, with any edits made since: changed words, a `# text` that
/// follows the transcription. A sentence written in wugs gets `# sent_id`
/// and `# text`; its glossed words carry the analysis in MISC the way UD
/// treebanks do (`Gloss`, `MSeg`, `MGloss`), and a root linked to the
/// lexicon gives the LEMMA and, through the entry's category, XPOS (and
/// UPOS when the category names a UD tag).
pub fn export(p: &Project, title: &str, sentences: &[ExportSentence]) -> Result<Exported, String> {
    let (out, notes) = export_sentences(p, title, sentences)?;
    let out: Vec<Sentence> = out.into_iter().map(|(_, s)| s).collect();
    Ok(Exported { conllu: format::write(&out), notes })
}

/// The sentences [`export`] writes, each with the index of the
/// [`ExportSentence`] it came from (segments with no words are left out),
/// and the notes; for writers of other formats (TEI) that share the rules.
pub fn export_sentences(p: &Project, title: &str, sentences: &[ExportSentence]) -> Result<(Vec<(usize, Sentence)>, Vec<String>), String> {
    let mut out = Vec::new();
    let mut notes = Notes::default();
    for (si, es) in sentences.iter().enumerate() {
        let s = sentence(p, title, si, es, &mut notes)?;
        if !s.lines.is_empty() {
            out.push((si, s));
        } else {
            notes.empty += 1;
        }
    }
    Ok((out, notes.lines()))
}

#[derive(Default)]
struct Notes {
    empty: usize,
    untokenized: usize,
    ranges_dropped: usize,
    heads_dropped: usize,
}

impl Notes {
    fn lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
        if self.empty > 0 {
            v.push(format!("{} with no words left out.", n(self.empty, "segment", "segments")));
        }
        if self.untokenized > 0 {
            v.push(format!("{} not yet broken into words in the Gloss panel were split at spaces.", n(self.untokenized, "segment", "segments")));
        }
        if self.ranges_dropped > 0 {
            v.push(format!("{} whose words changed since import were left out.", n(self.ranges_dropped, "multi-word token", "multi-word tokens")));
        }
        if self.heads_dropped > 0 {
            v.push(format!("{} pointed at a word no longer in its sentence and were written as _.", n(self.heads_dropped, "head", "heads")));
        }
        v
    }
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn sentence(p: &Project, title: &str, si: usize, es: &ExportSentence, notes: &mut Notes) -> Result<Sentence, String> {
    let seg = &es.segment;
    let content = p.proj.field_str(seg, "content")?.unwrap_or_default();
    let mut tokens = p.proj.members(seg)?;
    tokens.retain(|t| !p.proj.is_tombstoned(t).unwrap_or(true));
    let forms: Vec<String> = if tokens.is_empty() {
        if !content.trim().is_empty() {
            notes.untokenized += 1;
        }
        content.split_whitespace().map(str::to_string).collect()
    } else {
        tokens.iter().map(|t| p.proj.field_str(t, "form").map(Option::unwrap_or_default)).collect::<Result<_, _>>()?
    };
    let index: HashMap<&str, u32> = tokens.iter().enumerate().map(|(i, t)| (t.as_str(), i as u32 + 1)).collect();
    let analysis = p.proj.field(seg, F_ANALYSIS)?.map(|(v, _)| v);
    let rows: Vec<Value> = match &analysis {
        Some(Value::Arr(rows)) => rows.clone(),
        _ => Vec::new(),
    };
    let by_tok: HashMap<String, &Value> = rows.iter().filter_map(|r| str_of(r, "tok").map(|t| (t, r))).collect();
    let mut s = Sentence::default();

    // Comments: as imported, with `# text` following the transcription; else made here.
    match p.proj.field(seg, F_COMMENTS)? {
        Some((Value::Arr(cs), _)) => {
            for c in cs.iter().filter_map(Value::as_str) {
                let text_line = format::comment_value(c, "text").is_some();
                s.comments.push(if text_line && format::comment_value(c, "text") != Some(content.as_str()) { format!(" text = {}", one_line(&content)) } else { c.to_string() });
            }
        }
        _ => {
            if si == 0 {
                s.comments.push(format!(" newdoc id = {}", one_line(title)));
            }
            s.comments.push(format!(" sent_id = {}", si + 1));
            s.comments.push(format!(" text = {}", one_line(&content)));
            for (lang, tr) in &es.translations {
                let key = lang.as_deref().filter(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).map(|l| format!("text_{l}")).unwrap_or_else(|| "translation".into());
                s.comments.push(format!(" {key} = {}", one_line(tr)));
            }
        }
    }

    // Ranges, keyed by the word they start at.
    let mut ranges: BTreeMap<u32, (u32, Cols)> = BTreeMap::new();
    if let Some((Value::Arr(rs), _)) = p.proj.field(seg, F_MWT)? {
        for r in rs {
            let (Some(a), Some(b)) = (str_of(&r, "first").and_then(|t| index.get(t.as_str()).copied()), str_of(&r, "last").and_then(|t| index.get(t.as_str()).copied())) else {
                notes.ranges_dropped += 1;
                continue;
            };
            if b <= a || ranges.range(..=b).next_back().is_some_and(|(_, (end, _))| *end >= a) {
                notes.ranges_dropped += 1;
                continue;
            }
            ranges.insert(a, (b, cols_of(&r, None, &index, notes)));
        }
    }
    // Empty nodes, after the word they followed (by that word's token).
    let mut empties: BTreeMap<u32, Vec<Cols>> = BTreeMap::new();
    let mut after = 0u32;
    for r in &rows {
        if let Some(t) = str_of(r, "tok") {
            if let Some(&i) = index.get(t.as_str()) {
                after = i;
            }
        } else if str_of(r, "id").is_some() {
            empties.entry(after).or_default().push(cols_of(r, None, &index, notes));
        }
    }
    let push_empties = |s: &mut Sentence, major: u32, empties: &BTreeMap<u32, Vec<Cols>>| {
        for (k, cols) in empties.get(&major).into_iter().flatten().enumerate() {
            s.lines.push(Line::Empty { major, minor: k as u32 + 1, cols: cols.clone() });
        }
    };
    push_empties(&mut s, 0, &empties);
    for (i, form) in forms.iter().enumerate() {
        let id = i as u32 + 1;
        if let Some((last, cols)) = ranges.get(&id) {
            s.lines.push(Line::Range { first: id, last: *last, cols: cols.clone() });
        }
        let tok = tokens.get(i);
        let mut cols = match tok.and_then(|t| by_tok.get(t.as_str())) {
            Some(r) => cols_of(r, Some(form), &index, notes),
            None => {
                let mut c: Cols = std::array::from_fn(|_| "_".to_string());
                c[format::FORM] = format::column(form);
                if let Some(t) = tok {
                    from_glossing(p, t, &mut c)?;
                    if p.proj.field_str(t, F_KIND)?.as_deref() == Some("punct") || p.proj.field_str(t, "x.flex.kind")?.as_deref() == Some("punct") {
                        c[format::UPOS] = "PUNCT".into();
                    }
                }
                c
            }
        };
        cols[format::FORM] = format::column(form);
        s.lines.push(Line::Word { id, cols });
        push_empties(&mut s, id, &empties);
    }
    Ok(s)
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A row's columns back as CoNLL-U text; HEAD from token id to word number.
fn cols_of(r: &Value, form: Option<&str>, index: &HashMap<&str, u32>, notes: &mut Notes) -> Cols {
    let mut c: Cols = std::array::from_fn(|k| str_of(r, format::NAMES[k]).map(|v| format::column(&v)).unwrap_or_else(|| "_".into()));
    if let Some(f) = form {
        c[format::FORM] = format::column(f);
    }
    if let Some(h) = str_of(r, "head") {
        c[format::HEAD] = if h == "0" {
            h
        } else if let Some(i) = index.get(h.as_str()) {
            i.to_string()
        } else if h.starts_with("tok_") {
            notes.heads_dropped += 1;
            "_".into()
        } else {
            h
        };
    }
    c
}

/// UD's universal tags for what a lexicon category is commonly called.
pub fn upos_of(category: &str) -> Option<&'static str> {
    let c = category.trim().trim_end_matches('.').to_lowercase();
    Some(match c.as_str() {
        "noun" | "n" => "NOUN",
        "proper noun" | "propn" | "pn" | "name" => "PROPN",
        "verb" | "v" | "vb" => "VERB",
        "auxiliary" | "aux" => "AUX",
        "adjective" | "adj" | "a" => "ADJ",
        "adverb" | "adv" => "ADV",
        "pronoun" | "pron" | "pro" => "PRON",
        "determiner" | "det" => "DET",
        "adposition" | "preposition" | "postposition" | "adp" | "prep" | "postp" => "ADP",
        "numeral" | "num" | "number" => "NUM",
        "conjunction" | "coordinating conjunction" | "cconj" | "conj" => "CCONJ",
        "subordinating conjunction" | "sconj" | "subordinator" => "SCONJ",
        "particle" | "part" | "prt" => "PART",
        "interjection" | "intj" | "interj" => "INTJ",
        _ => return None,
    })
}

/// A word glossed in wugs: MISC gets `Gloss` (one morph) or `MSeg` and
/// `MGloss` (several, joined at their markers); a root linked to an entry
/// gives LEMMA, XPOS (the entry's category) and UPOS when it names a UD tag.
fn from_glossing(p: &Project, tok: &str, c: &mut Cols) -> Result<(), String> {
    let morphs: Vec<String> = p.proj.members(tok)?.into_iter().filter(|m| !p.proj.is_tombstoned(m).unwrap_or(true)).collect();
    if morphs.is_empty() {
        return Ok(());
    }
    let mut seg = String::new();
    let mut gl = String::new();
    let mut lemma = None;
    for (i, m) in morphs.iter().enumerate() {
        let surface = p.proj.field_str(m, "surface")?.unwrap_or_default();
        let gloss = p.proj.field_str(m, "gloss")?.unwrap_or_default();
        let kind = p.proj.field_str(m, "type")?.unwrap_or_default();
        let bare = surface.trim_matches(|ch| matches!(ch, '-' | '=' | '~' | '<' | '>'));
        let joint = match kind.as_str() {
            "clitic" => "=",
            "reduplication" => "~",
            _ => "-",
        };
        if i > 0 {
            seg.push_str(joint);
            gl.push_str(joint);
        }
        seg.push_str(bare);
        gl.push_str(if gloss.is_empty() { "?" } else { &gloss });
        if lemma.is_none() && kind == "root" {
            if let Some(sense) = p.proj.field_str(m, "sense")? {
                if let Some(lex) = p.proj.field_str(&sense, "lexeme")? {
                    if !p.proj.is_tombstoned(&lex)? {
                        lemma = Some((p.proj.field_str(&lex, "lemma")?.unwrap_or_default(), p.proj.field_str(&lex, "pos-of-speech")?.unwrap_or_default()));
                    }
                }
            }
        }
    }
    let misc = if morphs.len() == 1 { format!("Gloss={}", format::misc_value(&gl)) } else { format!("MSeg={}|MGloss={}", format::misc_value(&seg), format::misc_value(&gl)) };
    c[format::MISC] = misc;
    if let Some((lemma, category)) = lemma {
        if !lemma.is_empty() {
            c[format::LEMMA] = format::column(&lemma);
        }
        if !category.trim().is_empty() {
            c[format::XPOS] = format::column(&category);
            if let Some(u) = upos_of(&category) {
                c[format::UPOS] = u.into();
            }
        }
    }
    Ok(())
}
