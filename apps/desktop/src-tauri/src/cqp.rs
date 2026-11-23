//! CQP-style queries for the Concordance (roadmap 41, SCROLLS.md §1 step 3):
//! the subset of the Corpus Query Processor's language that CQPweb, Sketch
//! Engine and NoSketch users write every day, run over the project's
//! sentences rather than a CWB index.
//!
//! ```text
//! "dog"                          a word, as a regular expression over the whole word
//! [lemma="go"] [upos="ADP"]      a sequence of tokens
//! [word="ni.*" %c]               %c ignores case, %d ignores diacritics
//! [upos="ADJ" & !lemma="big"]    & (and), | (or), ! (not), ( )
//! [upos!="PUNCT"]                != is "does not match"
//! [] [upos="NOUN"]{1,3}          [] is any token; ? * + {n} {n,m} {n,} repeat
//! [feats=".*Number=Plur.*"]      FEATS as one string
//! [gloss=".*PL.*"]               a word's morpheme glosses, joined (1SG-count)
//! ```
//!
//! Attributes: `word`, `lemma`, `upos` (also `pos`, `tag`), `xpos`,
//! `feats`, `gloss`. Lemmas and tags come from a CoNLL-U sentence's
//! `analysis`; for a word glossed in wugs, the lemma is its root's lexicon
//! entry and XPOS that entry's part of speech (UPOS when it names a UD tag).
//! As in CQP, a regular expression must match the whole value and case
//! matters unless `%c` is given. Matches stay inside a sentence; from each
//! token the longest match is taken, and the next search starts after it.

use std::collections::HashMap;

use regex::Regex;
use serde::Serialize;
use wugs_core::Project;
use wugs_model::Value;

use crate::session::{project, Session};

/// The longest a repeat may run, so `[]*` stays bounded.
const MAX_REPEAT: usize = 20;
/// Hits returned (the total counts every match).
pub const LIMIT: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attr {
    Word,
    Lemma,
    Upos,
    Xpos,
    Feats,
    Gloss,
}

impl Attr {
    fn named(name: &str) -> Option<Attr> {
        Some(match name {
            "word" | "form" => Attr::Word,
            "lemma" => Attr::Lemma,
            "upos" | "pos" | "tag" => Attr::Upos,
            "xpos" => Attr::Xpos,
            "feats" => Attr::Feats,
            "gloss" => Attr::Gloss,
            _ => return None,
        })
    }
}

#[derive(Debug)]
enum Cond {
    Any,
    Test { attr: Attr, negate: bool, re: Regex, diacritics: bool },
    Not(Box<Cond>),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
}

#[derive(Debug)]
struct Item {
    cond: Cond,
    min: usize,
    max: usize,
}

/// A query that did not parse: what is wrong, and where (a character index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    pub at: usize,
    pub message: String,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at character {})", self.message, self.at + 1)
    }
}

struct Parser {
    chars: Vec<char>,
    i: usize,
}

impl Parser {
    fn err<T>(&self, message: impl Into<String>) -> Result<T, QueryError> {
        Err(QueryError { at: self.i.min(self.chars.len()), message: message.into() })
    }

    fn skip(&mut self) {
        while self.chars.get(self.i).is_some_and(|c| c.is_whitespace()) {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip();
        self.chars.get(self.i).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn items(&mut self) -> Result<Vec<Item>, QueryError> {
        let mut items = Vec::new();
        while self.peek().is_some() {
            let cond = match self.peek() {
                Some('[') => {
                    self.i += 1;
                    if self.eat(']') {
                        Cond::Any
                    } else {
                        let c = self.or()?;
                        if !self.eat(']') {
                            return self.err("a token's conditions end with ]");
                        }
                        c
                    }
                }
                Some('"' | '\'') => {
                    let (text, at) = self.string()?;
                    let (case, diacritics) = self.flags()?;
                    Cond::Test { attr: Attr::Word, negate: false, re: compile(&text, case, diacritics, at)?, diacritics }
                }
                Some(':') if self.chars.get(self.i + 1) == Some(&':') => break,
                Some(c) => return self.err(format!("«{c}» cannot start a token; write [word=\"…\"], \"…\" or []")),
                None => break,
            };
            let (min, max) = self.repeat()?;
            items.push(Item { cond, min, max });
        }
        if items.is_empty() {
            return self.err("an empty query");
        }
        if items.iter().all(|i| i.min == 0) {
            return Err(QueryError { at: 0, message: "this query can match nothing at all; make at least one token required".into() });
        }
        Ok(items)
    }

    fn repeat(&mut self) -> Result<(usize, usize), QueryError> {
        // Repeats sit right after the token, with no space between.
        match self.chars.get(self.i) {
            Some('?') => {
                self.i += 1;
                Ok((0, 1))
            }
            Some('*') => {
                self.i += 1;
                Ok((0, MAX_REPEAT))
            }
            Some('+') => {
                self.i += 1;
                Ok((1, MAX_REPEAT))
            }
            Some('{') => {
                self.i += 1;
                let n = self.number()?;
                let m = if self.eat(',') { if self.peek() == Some('}') { MAX_REPEAT } else { self.number()? } } else { n };
                if !self.eat('}') {
                    return self.err("a repeat is written {n}, {n,m} or {n,}");
                }
                if m < n || m > MAX_REPEAT {
                    return self.err(format!("a repeat runs from a smaller number to a larger one, at most {MAX_REPEAT}"));
                }
                Ok((n, m))
            }
            _ => Ok((1, 1)),
        }
    }

    fn number(&mut self) -> Result<usize, QueryError> {
        self.skip();
        let start = self.i;
        while self.chars.get(self.i).is_some_and(char::is_ascii_digit) {
            self.i += 1;
        }
        if start == self.i {
            return self.err("a number goes here");
        }
        self.chars[start..self.i].iter().collect::<String>().parse().map_err(|_| QueryError { at: start, message: "that number is too large".into() })
    }

    fn or(&mut self) -> Result<Cond, QueryError> {
        let mut c = self.and()?;
        while self.eat('|') {
            c = Cond::Or(Box::new(c), Box::new(self.and()?));
        }
        Ok(c)
    }

    fn and(&mut self) -> Result<Cond, QueryError> {
        let mut c = self.unary()?;
        while self.eat('&') {
            c = Cond::And(Box::new(c), Box::new(self.unary()?));
        }
        Ok(c)
    }

    fn unary(&mut self) -> Result<Cond, QueryError> {
        if self.eat('!') {
            return Ok(Cond::Not(Box::new(self.unary()?)));
        }
        if self.eat('(') {
            let c = self.or()?;
            if !self.eat(')') {
                return self.err("a ( needs its )");
            }
            return Ok(c);
        }
        self.skip();
        let start = self.i;
        while self.chars.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') {
            self.i += 1;
        }
        let name: String = self.chars[start..self.i].iter().collect();
        if name.is_empty() {
            return self.err("an attribute goes here: word, lemma, upos, xpos, feats or gloss");
        }
        let Some(attr) = Attr::named(&name.to_ascii_lowercase()) else {
            return Err(QueryError { at: start, message: format!("«{name}» is not an attribute wugs knows; use word, lemma, upos, xpos, feats or gloss") });
        };
        let negate = if self.eat('!') {
            if !self.eat('=') {
                return self.err("write != for \"does not match\"");
            }
            true
        } else if self.eat('=') {
            false
        } else {
            return self.err(format!("{name} is followed by = or != and a value in quotes"));
        };
        let (text, at) = self.string()?;
        let (case, diacritics) = self.flags()?;
        Ok(Cond::Test { attr, negate, re: compile(&text, case, diacritics, at)?, diacritics })
    }

    fn string(&mut self) -> Result<(String, usize), QueryError> {
        let Some(q @ ('"' | '\'')) = self.peek() else {
            return self.err("a value goes in quotes, like \"dog\"");
        };
        let start = self.i;
        self.i += 1;
        let mut s = String::new();
        loop {
            match self.chars.get(self.i) {
                None => return Err(QueryError { at: start, message: "this quote is never closed".into() }),
                Some(&c) if c == q => {
                    self.i += 1;
                    return Ok((s, start));
                }
                // \" and \' are quotes; any other backslash stays for the regular expression.
                Some('\\') if self.chars.get(self.i + 1) == Some(&q) => {
                    s.push(q);
                    self.i += 2;
                }
                Some(&c) => {
                    s.push(c);
                    self.i += 1;
                }
            }
        }
    }

    /// `%c`, `%d` or `%cd` after a value.
    fn flags(&mut self) -> Result<(bool, bool), QueryError> {
        let (mut case, mut diacritics) = (false, false);
        while self.eat('%') {
            let mut any = false;
            while let Some(c @ ('c' | 'd' | 'l')) = self.chars.get(self.i).copied() {
                match c {
                    'c' => case = true,
                    'd' => diacritics = true,
                    _ => {}
                }
                any = true;
                self.i += 1;
            }
            if !any {
                return self.err("after % comes c (ignore case) or d (ignore diacritics)");
            }
        }
        Ok((case, diacritics))
    }
}

fn compile(pattern: &str, ignore_case: bool, ignore_diacritics: bool, at: usize) -> Result<Regex, QueryError> {
    let pattern = if ignore_diacritics { strip_marks(pattern) } else { pattern.to_string() };
    let flags = if ignore_case { "(?i)" } else { "" };
    regex::RegexBuilder::new(&format!("{flags}^(?:{pattern})$"))
        .size_limit(1 << 20)
        .build()
        .map_err(|e| QueryError { at, message: format!("«{pattern}» is not a regular expression wugs can read: {}", e.to_string().lines().last().unwrap_or("")) })
}

/// The text with combining marks taken off (NFD, marks dropped): `é` → `e`.
fn strip_marks(s: &str) -> String {
    use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
    s.nfd().filter(|c| !is_combining_mark(*c)).collect()
}

/// A parsed query.
pub struct Query {
    items: Vec<Item>,
    /// Conditions on the text's metadata after `::`, all of which must hold.
    meta: Vec<MetaTest>,
}

/// `match.text_<key>="re"` (or `!=`) after `::`: a condition on the
/// metadata of the text a match is in, as CQPweb writes it.
#[derive(Debug)]
struct MetaTest {
    /// The metadata keys it may name, tried in order: `genre`, then
    /// `text_genre` for `match.text_genre`.
    keys: Vec<String>,
    negate: bool,
    re: Regex,
    diacritics: bool,
}

impl MetaTest {
    fn holds(&self, meta: &[(String, String)]) -> bool {
        let value = self.keys.iter().find_map(|k| meta.iter().find(|(m, _)| m == k).map(|(_, v)| v.as_str())).unwrap_or("");
        let value = if self.diacritics { strip_marks(value) } else { value.to_string() };
        self.re.is_match(&value) != self.negate
    }
}

impl Parser {
    /// After `::`: `match.text_<key>` conditions joined by `&`.
    fn meta(&mut self) -> Result<Vec<MetaTest>, QueryError> {
        let mut tests = Vec::new();
        loop {
            self.skip();
            let start = self.i;
            while self.chars.get(self.i).is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.')) {
                self.i += 1;
            }
            let name: String = self.chars[start..self.i].iter().collect::<String>().to_lowercase();
            let rest = name.strip_prefix("match.").unwrap_or(&name);
            if rest.is_empty() {
                return Err(QueryError { at: start, message: "after :: comes a condition on the text, like match.text_genre=\"news\"".into() });
            }
            let mut keys = Vec::new();
            if let Some(k) = rest.strip_prefix("text_").filter(|k| !k.is_empty()) {
                keys.push(k.to_string());
            }
            keys.push(rest.to_string());
            let negate = if self.eat('!') {
                if !self.eat('=') {
                    return self.err("write != for \"does not match\"");
                }
                true
            } else if self.eat('=') {
                false
            } else {
                return self.err(format!("{name} is followed by = or != and a value in quotes"));
            };
            let (text, at) = self.string()?;
            let (case, diacritics) = self.flags()?;
            tests.push(MetaTest { keys, negate, re: compile(&text, case, diacritics, at)?, diacritics });
            if !self.eat('&') {
                break;
            }
        }
        if self.peek().is_some() {
            return self.err("a query ends after its text conditions; join more with &");
        }
        Ok(tests)
    }
}

/// Read a query, or say what is wrong with it and where.
pub fn parse(query: &str) -> Result<Query, QueryError> {
    let mut p = Parser { chars: query.chars().collect(), i: 0 };
    let items = p.items()?;
    let meta = if p.eat(':') {
        p.i += 1;
        p.meta()?
    } else {
        Vec::new()
    };
    Ok(Query { items, meta })
}

/// One token as a query sees it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tok {
    pub word: String,
    pub lemma: String,
    pub upos: String,
    pub xpos: String,
    pub feats: String,
    pub gloss: String,
}

impl Tok {
    fn get(&self, a: Attr) -> &str {
        match a {
            Attr::Word => &self.word,
            Attr::Lemma => &self.lemma,
            Attr::Upos => &self.upos,
            Attr::Xpos => &self.xpos,
            Attr::Feats => &self.feats,
            Attr::Gloss => &self.gloss,
        }
    }
}

fn holds(c: &Cond, t: &Tok) -> bool {
    match c {
        Cond::Any => true,
        Cond::Test { attr, negate, re, diacritics } => {
            let v = t.get(*attr);
            let m = if *diacritics { re.is_match(&strip_marks(v)) } else { re.is_match(v) };
            m != *negate
        }
        Cond::Not(c) => !holds(c, t),
        Cond::And(a, b) => holds(a, t) && holds(b, t),
        Cond::Or(a, b) => holds(a, t) || holds(b, t),
    }
}

impl Query {
    /// Every match in a sentence as (start, length): from each start, the
    /// longest match; the next search starts after it.
    pub fn matches(&self, toks: &[Tok]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            match self.longest(0, i, toks) {
                Some(end) if end > i => {
                    out.push((i, end - i));
                    i = end;
                }
                _ => i += 1,
            }
        }
        out
    }

    /// The furthest position items[k..] can reach from `at`, trying longer
    /// repeats first.
    fn longest(&self, k: usize, at: usize, toks: &[Tok]) -> Option<usize> {
        let Some(item) = self.items.get(k) else { return Some(at) };
        // How many tokens in a row this item could take from `at`.
        let mut run = 0;
        while run < item.max && at + run < toks.len() && holds(&item.cond, &toks[at + run]) {
            run += 1;
        }
        let mut best: Option<usize> = None;
        for n in (item.min..=run).rev() {
            if let Some(end) = self.longest(k + 1, at + n, toks) {
                best = Some(best.map_or(end, |b: usize| b.max(end)));
            }
        }
        best
    }
}

/// The tokens of a segment for a query: every token, punctuation included,
/// with lemma, tags and features from its CoNLL-U `analysis`, or from its
/// root's lexicon entry when glossed in wugs. A segment not yet broken into
/// words reads as its words, with nothing but `word`.
fn tokens_of(p: &Project, seg: &str) -> Result<Vec<Tok>, String> {
    let ids: Vec<String> = p.proj.members(seg)?.into_iter().filter(|t| !p.proj.is_tombstoned(t).unwrap_or(true)).collect();
    if ids.is_empty() {
        let content = p.proj.field_str(seg, "content")?.unwrap_or_default();
        return Ok(wugs_model::words_of(&content).into_iter().map(|w| Tok { word: w, ..Tok::default() }).collect());
    }
    let mut rows: HashMap<String, Value> = HashMap::new();
    if let Some((Value::Arr(rs), _)) = p.proj.field(seg, "analysis")? {
        for r in rs {
            if let Some(t) = r.get("tok").and_then(Value::as_str) {
                rows.insert(t.to_string(), r.clone());
            }
        }
    }
    let col = |r: Option<&Value>, k: &str| r.and_then(|r| r.get(k)).and_then(Value::as_str).unwrap_or("").to_string();
    let mut out = Vec::with_capacity(ids.len());
    for id in &ids {
        let r = rows.get(id);
        let mut t = Tok {
            word: p.proj.field_str(id, "form")?.unwrap_or_default(),
            lemma: col(r, "lemma"),
            upos: col(r, "upos"),
            xpos: col(r, "xpos"),
            feats: col(r, "feats"),
            gloss: String::new(),
        };
        if t.upos.is_empty() && p.proj.is_punct_token(id) {
            t.upos = "PUNCT".into();
        }
        let mut glosses = Vec::new();
        for m in crate::concordance::morphs_of(p, id)? {
            glosses.push((if m.gloss.is_empty() { "?".to_string() } else { m.gloss.clone() }, m.kind.clone()));
            if t.lemma.is_empty() && m.kind == "root" {
                if let Some(sense) = &m.sense {
                    if let Some(lex) = p.proj.field_str(sense, "lexeme")? {
                        if !p.proj.is_tombstoned(&lex)? {
                            t.lemma = p.proj.field_str(&lex, "lemma")?.unwrap_or_default();
                            let pos = p.proj.field_str(&lex, "pos-of-speech")?.unwrap_or_default();
                            if t.xpos.is_empty() {
                                t.xpos = pos.clone();
                            }
                            if t.upos.is_empty() {
                                t.upos = wugs_conllu::upos_of(&pos).unwrap_or("").to_string();
                            }
                        }
                    }
                }
            }
        }
        t.gloss = crate::concordance::joined(&glosses);
        out.push(t);
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryProblem {
    pub at: usize,
    pub message: String,
}

/// Run a query over every sentence (or one text's): hits in the
/// Concordance's shape, each with the matched span.
pub fn run(session: &mut Session, query: &str, only_text: Option<&str>) -> Result<crate::concordance::Concordance, String> {
    let q = parse(query).map_err(|e| e.to_string())?;
    let p = project(session)?;
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    let mut hits = Vec::new();
    let mut total = 0;
    let mut texts = p.proj.entities_of_kind("text")?;
    texts.sort();
    for text in texts {
        if only_text.is_some_and(|t| t != text) || p.proj.is_tombstoned(&text)? {
            continue;
        }
        if !q.meta.is_empty() {
            let meta = p.text_metadata(&text)?;
            if !q.meta.iter().all(|m| m.holds(&meta)) {
                continue;
            }
        }
        let title = p.proj.field_str(&text, "title")?.unwrap_or_default();
        for seg in p.proj.ordered_children("text", &text, "segment")? {
            let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
            if !transcription.contains(&tier) || p.proj.is_tombstoned(&seg)? {
                continue;
            }
            let toks = tokens_of(p, &seg)?;
            for (at, len) in q.matches(&toks) {
                total += 1;
                if hits.len() < LIMIT {
                    hits.push(crate::concordance::Hit {
                        text_id: text.clone(),
                        text_title: title.clone(),
                        segment_id: seg.clone(),
                        tier_id: tier.clone(),
                        start_ms: p.proj.field_num(&seg, "start")?,
                        end_ms: p.proj.field_num(&seg, "end")?,
                        words: toks.iter().map(|t| t.word.clone()).collect(),
                        at,
                        len,
                        breakdown: None,
                        glosses: None,
                    });
                }
            }
        }
    }
    Ok(crate::concordance::Concordance { hits, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(spec: &[(&str, &str, &str)]) -> Vec<Tok> {
        spec.iter().map(|(w, l, u)| Tok { word: w.to_string(), lemma: l.to_string(), upos: u.to_string(), ..Tok::default() }).collect()
    }

    fn found(q: &str, t: &[Tok]) -> Vec<(usize, usize)> {
        parse(q).unwrap_or_else(|e| panic!("{q}: {e}")).matches(t)
    }

    #[test]
    fn the_everyday_queries_match_as_cqp_does() {
        let s = toks(&[("The", "the", "DET"), ("big", "big", "ADJ"), ("dogs", "dog", "NOUN"), ("went", "go", "VERB"), ("to", "to", "ADP"), ("the", "the", "DET"), ("park", "park", "NOUN"), (".", ".", "PUNCT")]);
        assert_eq!(found("\"dogs\"", &s), [(2, 1)]);
        assert_eq!(found("\"the\"", &s), [(5, 1)], "case matters, as in CQP");
        assert_eq!(found("\"the\" %c", &s), [(0, 1), (5, 1)]);
        assert_eq!(found("\"do\"", &s), [], "the whole word must match");
        assert_eq!(found("[lemma=\"go\"] [upos=\"ADP\"]", &s), [(3, 2)]);
        assert_eq!(found("[upos=\"DET\"] [upos=\"ADJ\"]? [upos=\"NOUN\"]", &s), [(0, 3), (5, 2)], "optional tokens, longest match");
        assert_eq!(found("[upos=\"ADJ\" & !lemma=\"big\"]", &s), []);
        assert_eq!(found("[upos=\"ADJ\" | upos=\"NOUN\"]", &s), [(1, 1), (2, 1), (6, 1)], "matches do not overlap or merge");
        assert_eq!(found("[upos!=\"PUNCT\"]{3}", &s), [(0, 3), (3, 3)]);
        assert_eq!(found("[lemma=\"go\"] []{0,2} [upos=\"NOUN\"]", &s), [(3, 4)], "a gap of up to two tokens");
        assert_eq!(found("[word=\"d.*\"]", &s), [(2, 1)], "a regular expression over the word");
    }

    #[test]
    fn diacritics_can_be_ignored() {
        let s = toks(&[("café", "café", "NOUN"), ("cafe", "cafe", "NOUN")]);
        assert_eq!(found("\"cafe\"", &s), [(1, 1)]);
        assert_eq!(found("\"cafe\" %d", &s), [(0, 1), (1, 1)]);
        assert_eq!(found("\"CAFÉ\" %cd", &s), [(0, 1), (1, 1)]);
    }

    #[test]
    fn mistakes_say_what_and_where() {
        let e = |q: &str| parse(q).err().unwrap();
        assert!(e("[lema=\"go\"]").message.contains("«lema» is not an attribute"));
        assert_eq!(e("[lema=\"go\"]").at, 1);
        assert!(e("[word=\"go]").message.contains("never closed"));
        assert!(e("[word=\"go\"").message.contains("end with ]"));
        assert!(e("[word=\"(\"]").message.contains("regular expression"));
        assert!(e("[]*").message.contains("match nothing"));
        assert!(e("[]{3,1}").message.contains("smaller number"));
        assert!(e("dog").message.contains("cannot start a token"));
        assert!(e("").message.contains("empty"));
    }

    #[test]
    fn a_glossed_and_a_ud_sentence_answer_the_same_attributes() {
        let dir = std::env::temp_dir().join(format!("wugs-cqp-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        crate::sample::add_morphology(&mut s).unwrap();
        let p = project(&mut s).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let ud = "# text = Vámonos al mar.\n1-2\tVámonos\t_\t_\t_\t_\t_\t_\t_\t_\n1\tVamos\tir\tVERB\t_\tMood=Imp\t0\troot\t_\t_\n2\tnos\tnosotros\tPRON\t_\t_\t1\tobj\t_\t_\n3-4\tal\t_\t_\t_\t_\t_\t_\t_\t_\n3\ta\ta\tADP\t_\t_\t5\tcase\t_\t_\n4\tel\tel\tDET\t_\t_\t5\tdet\t_\t_\n5\tmar\tmar\tNOUN\t_\t_\t1\tobl\t_\tSpaceAfter=No\n6\t.\t.\tPUNCT\t_\t_\t1\tpunct\t_\t_\n\n";
        let playa = wugs_conllu::import_conllu(p, ud, "playa", &tier).unwrap().texts[0].clone();
        p.set_text_metadata(&playa, "Genre", "beach talk").unwrap();
        let r = run(&mut s, "[upos=\"ADP\"] [upos=\"DET\"] [upos=\"NOUN\"]", None).unwrap();
        assert_eq!(r.total, 1);
        let h = &r.hits[0];
        assert_eq!(&h.words[h.at..h.at + h.len], ["a", "el", "mar"]);
        assert_eq!(h.words.last().map(String::as_str), Some("."), "punctuation is a token to a query");
        assert_eq!(run(&mut s, "[feats=\".*Mood=Imp.*\"]", None).unwrap().total, 1);
        // Glossed words: lemmas come from the lexicon, glosses from the morphs.
        assert!(run(&mut s, "[gloss=\".*1SG.*\"]", None).unwrap().total >= 1);
        assert!(run(&mut s, "[lemma=\"kani\"]", None).unwrap().total >= 1);
        assert!(run(&mut s, "[lema=\"x\"]", None).unwrap_err().contains("not an attribute"));
        // Conditions on the text's metadata, as CQPweb writes them.
        assert_eq!(run(&mut s, "[upos=\"NOUN\"] :: match.text_genre=\"beach.*\"", None).unwrap().total, 1);
        assert_eq!(run(&mut s, "[upos=\"NOUN\"] :: match.text_genre!=\"beach.*\"", None).unwrap().total, 0);
        assert_eq!(run(&mut s, "[upos=\"NOUN\"] :: match.text_genre=\"BEACH TALK\" %c & match.text_year=\"\"", None).unwrap().total, 1, "a missing key reads as empty");
        assert!(run(&mut s, "[upos=\"NOUN\"] :: genre", None).unwrap_err().contains("followed by = or !="));
        assert!(run(&mut s, "[upos=\"NOUN\"] :: match.text_genre=\"x\" [word=\"y\"]", None).unwrap_err().contains("ends after"));
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
