//! Orthography profiles (Moran & Cysouw, *The Unicode Cookbook for
//! Linguists*, 2018; the `segments` package and CLDF's lexibank read the
//! same files): a tab-separated table whose `Grapheme` column lists the
//! letters and letter groups of a writing system and whose other columns
//! say what each is in another one, such as `IPA`. The Orthography panel
//! keeps one profile per project (the project setting
//! `orthography-profile`), starts one from a track's own letters, previews
//! a conversion and writes it as a converted track
//! (`wugs_core::converted`), so a practical orthography gets an IPA track
//! the Sounds panel can place on the chart.
//!
//! Rules, as `segments` reads a profile: graphemes are matched longest
//! first from the start of each word; `^` before a grapheme matches only at
//! a word's start and `$` after it only at its end; a cell `NULL` writes
//! nothing; an empty cell keeps the grapheme as written. Unlike `segments`,
//! a character no row matches is kept as written (and reported), never
//! replaced, and punctuation no row lists passes through quietly.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::concordance::{corpus_tiers, each_segment, Read, Scope};
use crate::session::{self, Session};
use wugs_core::converted::{ConvertReport, ConvertedRow};

/// The project setting that holds the profile.
const SETTING: &str = "orthography-profile";
/// The longest profile kept (bytes): what one project setting holds.
const MAX_PROFILE: usize = wugs_core::settings::MAX_VALUE;
/// Columns that describe a row rather than convert it.
const NOT_OUTPUT: &[&str] = &["grapheme", "frequency", "comment", "comments", "note", "notes", "left", "right", "class", "codepoints", "unicode", "example", "examples"];
/// Rows of before and after the preview lists.
const PREVIEW_ROWS: usize = 40;

#[derive(Debug, Clone, PartialEq)]
struct Rule {
    grapheme: Vec<char>,
    initial: bool,
    last: bool,
    /// One per output column: None keeps the grapheme as written.
    outputs: Vec<Option<String>>,
}

#[derive(Debug, Default)]
pub struct Profile {
    columns: Vec<String>,
    rules: Vec<Rule>,
    /// First character → rules starting with it, longest first.
    by_first: HashMap<char, Vec<usize>>,
    problems: Vec<String>,
}

/// What a profile says about itself, for the panel.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// Output columns, in the file's order.
    pub columns: Vec<String>,
    pub rules: usize,
    /// Problems in words, each with its line.
    pub problems: Vec<String>,
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

impl Profile {
    pub fn parse(text: &str) -> Profile {
        let mut p = Profile::default();
        let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
        let Some((_, header)) = lines.next() else {
            p.problems.push("The profile is empty: its first line names the columns, such as Grapheme and IPA.".into());
            return p;
        };
        let names: Vec<String> = header.trim_end_matches('\r').split('\t').map(|c| c.trim().to_string()).collect();
        let Some(gi) = names.iter().position(|n| n.eq_ignore_ascii_case("grapheme")) else {
            p.problems.push("Line 1 names no Grapheme column; the first line names the columns, such as Grapheme and IPA, separated by tabs.".into());
            return p;
        };
        let outputs: Vec<usize> = (0..names.len()).filter(|i| !names[*i].is_empty() && !NOT_OUTPUT.contains(&names[*i].to_lowercase().as_str())).collect();
        p.columns = outputs.iter().map(|i| names[*i].clone()).collect();
        if p.columns.is_empty() {
            p.problems.push("Line 1 has no column to convert into, such as IPA, beside Grapheme.".into());
        }
        let mut seen: HashMap<(Vec<char>, bool, bool), usize> = HashMap::new();
        for (n, line) in lines {
            let line_no = n + 1;
            let cells: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
            let raw = nfc(cells.get(gi).copied().unwrap_or("").trim());
            let initial = raw.starts_with('^');
            let last = raw.ends_with('$') && raw.chars().count() > 1;
            let core: String = raw.trim_start_matches('^').trim_end_matches('$').to_string();
            if core.is_empty() {
                if !(raw == "^" || raw == "$" || raw == "^$") {
                    p.problems.push(format!("Line {line_no} has no grapheme; it is left out."));
                }
                continue;
            }
            let grapheme: Vec<char> = core.chars().collect();
            if let Some(first) = seen.get(&(grapheme.clone(), initial, last)) {
                p.problems.push(format!("Line {line_no} lists «{raw}» again (first on line {first}); the first is used."));
                continue;
            }
            seen.insert((grapheme.clone(), initial, last), line_no);
            let outputs = outputs
                .iter()
                .map(|i| match cells.get(*i).map(|c| c.trim()) {
                    None | Some("") => None,
                    Some("NULL") => Some(String::new()),
                    Some(v) => Some(nfc(v)),
                })
                .collect();
            p.rules.push(Rule { grapheme, initial, last, outputs });
        }
        for (i, r) in p.rules.iter().enumerate() {
            p.by_first.entry(r.grapheme[0]).or_default().push(i);
        }
        for list in p.by_first.values_mut() {
            // Longest first; at one length an anchored rule before a plain one.
            list.sort_by(|a, b| {
                let (ra, rb) = (&p.rules[*a], &p.rules[*b]);
                rb.grapheme.len().cmp(&ra.grapheme.len()).then((rb.initial as u8 + rb.last as u8).cmp(&(ra.initial as u8 + ra.last as u8)))
            });
        }
        p
    }

    pub fn check(&self) -> Check {
        Check { columns: self.columns.clone(), rules: self.rules.len(), problems: self.problems.clone() }
    }

    fn column(&self, name: &str) -> Result<usize, String> {
        self.columns.iter().position(|c| c == name).ok_or_else(|| format!("The profile has no column «{name}»."))
    }

    /// One word through the profile: the converted word and the characters
    /// (with their combining marks) no row matched.
    fn word(&self, word: &str, column: usize, unknown: &mut Vec<String>) -> String {
        let chars: Vec<char> = word.chars().collect();
        let n = chars.len();
        let mut out = String::new();
        let mut i = 0;
        while i < n {
            let found = self.by_first.get(&chars[i]).and_then(|list| {
                list.iter().map(|k| &self.rules[*k]).find(|r| {
                    let len = r.grapheme.len();
                    i + len <= n && chars[i..i + len] == r.grapheme[..] && (!r.initial || i == 0) && (!r.last || i + len == n)
                })
            });
            if let Some(r) = found {
                match &r.outputs[column] {
                    Some(v) => out.push_str(v),
                    None => out.extend(r.grapheme.iter()),
                }
                i += r.grapheme.len();
                continue;
            }
            // No row: keep the character and its marks as written.
            let mut j = i + 1;
            while j < n && is_combining_mark(chars[j]) {
                j += 1;
            }
            let cluster: String = chars[i..j].iter().collect();
            if !cluster.chars().all(|c| c.is_ascii_punctuation() || is_other_punctuation(c)) {
                unknown.push(cluster.clone());
            }
            out.push_str(&cluster);
            i = j;
        }
        out
    }

    /// A word as sound segments through the profile: each matched
    /// grapheme's value in `column` (a value with spaces, as profiles write
    /// `t s` for a sequence, gives one segment each; NULL gives none), and
    /// an unmatched character with its marks as one segment.
    pub fn segments(&self, word: &str, column: usize, ignore_case: bool) -> Vec<String> {
        let word = nfc(word);
        let word = if ignore_case { word.to_lowercase() } else { word };
        let mut out = Vec::new();
        for w in word.split_whitespace() {
            let chars: Vec<char> = w.chars().collect();
            let n = chars.len();
            let mut i = 0;
            while i < n {
                let found = self.by_first.get(&chars[i]).and_then(|list| {
                    list.iter().map(|k| &self.rules[*k]).find(|r| {
                        let len = r.grapheme.len();
                        i + len <= n && chars[i..i + len] == r.grapheme[..] && (!r.initial || i == 0) && (!r.last || i + len == n)
                    })
                });
                if let Some(r) = found {
                    match &r.outputs[column] {
                        Some(v) => out.extend(v.split_whitespace().map(str::to_string)),
                        None => out.push(r.grapheme.iter().collect()),
                    }
                    i += r.grapheme.len();
                    continue;
                }
                let mut j = i + 1;
                while j < n && is_combining_mark(chars[j]) {
                    j += 1;
                }
                let cluster: String = chars[i..j].iter().collect();
                if !cluster.chars().all(|c| c.is_ascii_punctuation() || is_other_punctuation(c)) {
                    out.push(cluster);
                }
                i = j;
            }
        }
        out
    }

    /// A transcription through the profile, word by word (spaces as they
    /// were, collapsed to one), with the characters no row matched.
    pub fn convert(&self, text: &str, column: usize, ignore_case: bool) -> (String, Vec<String>) {
        let text = nfc(text);
        let mut unknown = Vec::new();
        let words: Vec<String> = text
            .split_whitespace()
            .map(|w| {
                let w = if ignore_case { w.to_lowercase() } else { w.to_string() };
                self.word(&w, column, &mut unknown)
            })
            .collect();
        (words.join(" "), unknown)
    }
}

/// How the project cuts a form into sound segments: through its
/// orthography profile's IPA column (or first column) when it has one, else
/// letter by letter with each letter's combining marks and any modifier
/// letters after it. Says which in its second value.
pub fn segmenter(p: &wugs_core::Project) -> Result<(Box<dyn Fn(&str) -> Vec<String>>, bool), String> {
    let text = p.setting(SETTING)?.unwrap_or_default();
    let profile = Profile::parse(&text);
    let column = profile.columns.iter().position(|c| c.eq_ignore_ascii_case("ipa")).or(if profile.columns.is_empty() { None } else { Some(0) });
    if let (Some(column), false) = (column, profile.rules.is_empty()) {
        return Ok((Box::new(move |w: &str| profile.segments(w, column, true)), true));
    }
    Ok((Box::new(|w: &str| letters(w)), false))
}

/// A word as letters, each with its combining marks and the modifier
/// letters (length, aspiration, palatalization) written after it.
pub fn letters(word: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in nfc(word).chars() {
        let modifier = matches!(c, 'ː' | 'ˑ' | 'ʰ' | 'ʲ' | 'ʷ' | 'ˠ' | 'ˤ' | 'ⁿ' | 'ˡ' | 'ʼ');
        if c.is_whitespace() || c.is_ascii_punctuation() || is_other_punctuation(c) {
            continue;
        }
        match out.last_mut() {
            Some(last) if is_combining_mark(c) || modifier => last.push(c),
            _ => out.push(c.to_string()),
        }
    }
    out
}

/// Punctuation outside ASCII a transcription commonly carries.
fn is_other_punctuation(c: char) -> bool {
    matches!(c, '«' | '»' | '“' | '”' | '‘' | '„' | '…' | '–' | '—' | '¿' | '¡' | '、' | '。' | '，' | '·' | '‹' | '›')
}

/// The project's profile as stored, and what it says.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub profile: String,
    pub check: Check,
    /// Transcription tracks with segments, fullest first: (id, name, segments).
    pub tracks: Vec<TrackRow>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct TrackRow {
    pub id: String,
    pub name: String,
    pub segments: usize,
}

fn tracks(p: &wugs_core::Project) -> Result<Vec<TrackRow>, String> {
    let (_, transcription) = corpus_tiers(p)?;
    let mut out = Vec::new();
    for id in transcription {
        if p.proj.is_tombstoned(&id)? {
            continue;
        }
        let segments = p.proj.children_by_ref("tier", &id, "segment")?.len();
        if segments > 0 {
            out.push(TrackRow { name: p.proj.field_str(&id, "name")?.unwrap_or_else(|| id.clone()), id, segments });
        }
    }
    out.sort_by(|a, b| b.segments.cmp(&a.segments).then(a.name.cmp(&b.name)));
    Ok(out)
}

pub fn stored(session: &mut Session) -> Result<Stored, String> {
    let p = session::project(session)?;
    let profile = p.setting(SETTING)?.unwrap_or_default();
    let check = if profile.is_empty() { Check { columns: Vec::new(), rules: 0, problems: Vec::new() } } else { Profile::parse(&profile).check() };
    Ok(Stored { profile, check, tracks: tracks(p)? })
}

/// Keep `profile` as the project's. Refused when it has no Grapheme column
/// or is too long; other problems are reported, not refused.
pub fn save(session: &mut Session, profile: &str) -> Result<Check, String> {
    if profile.len() > MAX_PROFILE {
        return Err(format!("A profile is at most {} KB.", MAX_PROFILE / 1024));
    }
    let parsed = Profile::parse(profile);
    if !profile.trim().is_empty() && parsed.columns.is_empty() && parsed.rules.is_empty() {
        return Err(parsed.problems.join(" "));
    }
    let p = session::project(session)?;
    if p.setting(SETTING)?.unwrap_or_default() != profile {
        p.set_setting(SETTING, profile)?;
    }
    Ok(parsed.check())
}

/// A starting profile from a track's own letters: every letter with its
/// combining marks, commonest first, with an IPA column that says the same
/// until someone corrects it, and how often each occurs.
pub fn draft(session: &mut Session, tier: &str, text: Option<String>, ignore_case: bool) -> Result<String, String> {
    let p = session::project(session)?;
    let mut counts: HashMap<String, usize> = HashMap::new();
    each_segment(p, Read::Words, &Scope::of(text, None), |seg| {
        if seg.tier != tier {
            return Ok(());
        }
        for w in &seg.words {
            let w = nfc(w);
            let w = if ignore_case { w.to_lowercase() } else { w };
            let chars: Vec<char> = w.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let mut j = i + 1;
                while j < chars.len() && is_combining_mark(chars[j]) {
                    j += 1;
                }
                let cluster: String = chars[i..j].iter().collect();
                if !cluster.chars().all(|c| c.is_ascii_punctuation() || is_other_punctuation(c)) {
                    *counts.entry(cluster).or_default() += 1;
                }
                i = j;
            }
        }
        Ok(())
    })?;
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut out = String::from("Grapheme\tIPA\tFrequency\n");
    for (g, n) in rows {
        out.push_str(&format!("{g}\t{g}\t{n}\n"));
    }
    Ok(out)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Change {
    pub before: String,
    pub after: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Unknown {
    pub text: String,
    pub count: usize,
    /// A word it occurs in, as written.
    pub example: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub check: Check,
    /// Segments read on the track.
    pub segments: usize,
    /// The first conversions, in text order.
    pub changes: Vec<Change>,
    /// Characters no row matched, commonest first.
    pub unknown: Vec<Unknown>,
}

/// The track's segments of one text or all, live and in text order:
/// (text id, segment id, content).
fn sources(p: &wugs_core::Project, tier: &str, text: Option<&str>) -> Result<Vec<(String, String, String)>, String> {
    let mut out = Vec::new();
    for seg in p.proj.children_by_ref("tier", tier, "segment")? {
        let t = p.proj.field_str(&seg, "text")?.unwrap_or_default();
        if text.is_some_and(|only| only != t) || p.proj.is_tombstoned(&t)? {
            continue;
        }
        let pos = p.proj.field_str(&seg, "pos")?.unwrap_or_default();
        let start = p.proj.field_num(&seg, "start")?.unwrap_or(f64::MAX);
        out.push((t, pos, start, seg));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.total_cmp(&b.2)).then(a.1.cmp(&b.1)).then(a.3.cmp(&b.3)));
    out.into_iter()
        .map(|(t, _, _, seg)| {
            let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
            Ok((t, seg, content))
        })
        .collect()
}

fn tally(unknown: &mut HashMap<String, (usize, String)>, found: Vec<String>, line: &str) {
    for u in found {
        let e = unknown.entry(u.clone()).or_insert_with(|| (0, line.split_whitespace().find(|w| w.contains(u.as_str())).unwrap_or(line).to_string()));
        e.0 += 1;
    }
}

fn unknown_list(unknown: HashMap<String, (usize, String)>) -> Vec<Unknown> {
    let mut list: Vec<Unknown> = unknown.into_iter().map(|(text, (count, example))| Unknown { text, count, example }).collect();
    list.sort_by(|a, b| b.count.cmp(&a.count).then(a.text.cmp(&b.text)));
    list
}

pub fn preview(session: &mut Session, profile: &str, tier: &str, text: Option<String>, column: &str, ignore_case: bool) -> Result<Preview, String> {
    let parsed = Profile::parse(profile);
    let col = parsed.column(column)?;
    let p = session::project(session)?;
    let rows = sources(p, tier, text.as_deref())?;
    let mut unknown = HashMap::new();
    let mut changes = Vec::new();
    for (_, _, content) in &rows {
        let (after, found) = parsed.convert(content, col, ignore_case);
        tally(&mut unknown, found, content);
        if changes.len() < PREVIEW_ROWS && !content.trim().is_empty() {
            changes.push(Change { before: content.clone(), after });
        }
    }
    Ok(Preview { check: parsed.check(), segments: rows.len(), changes, unknown: unknown_list(unknown) })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Converted {
    pub track: String,
    pub track_name: String,
    pub texts: usize,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub kept: usize,
    pub empty: usize,
    pub unknown: Vec<Unknown>,
}

/// Convert `tier` (one text or all) with `profile`'s `column` onto the
/// transcription track named `target` (made when there is none), keeping
/// the profile as the project's first.
pub fn convert(session: &mut Session, profile: &str, tier: &str, text: Option<String>, column: &str, ignore_case: bool, target: &str) -> Result<Converted, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("Name the track to write, such as IPA.".into());
    }
    save(session, profile)?;
    let parsed = Profile::parse(profile);
    let col = parsed.column(column)?;
    let p = session::project(session)?;
    let rows = sources(p, tier, text.as_deref())?;
    let mut unknown = HashMap::new();
    let mut converted = Vec::new();
    for (t, seg, content) in rows {
        let (after, found) = parsed.convert(&content, col, ignore_case);
        tally(&mut unknown, found, &content);
        converted.push((t, seg, after));
    }
    let mut out = write_track(p, tier, target, converted, &format!("orthography profile: {column}"))?;
    out.unknown = unknown_list(unknown);
    Ok(out)
}

/// Write converted rows `(text, source segment, content)`, in text order,
/// onto the transcription track named `target` (made when there is none,
/// refused when it is the source or holds segments not converted from it).
fn write_track(p: &mut wugs_core::Project, tier: &str, target: &str, rows: Vec<(String, String, String)>, by: &str) -> Result<Converted, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("Name the track to write, such as IPA.".into());
    }
    let source_name = p.proj.field_str(tier, "name")?.unwrap_or_default();
    if source_name.trim().eq_ignore_ascii_case(target) {
        return Err(format!("«{target}» is the track being converted; name another."));
    }
    let (_, transcription) = corpus_tiers(p)?;
    let mut track = None;
    for t in &transcription {
        if !p.proj.is_tombstoned(t)? && p.proj.field_str(t, "name")?.unwrap_or_default().trim().eq_ignore_ascii_case(target) {
            track = Some(t.clone());
            break;
        }
    }
    let track = match track {
        Some(t) => {
            // Never write into a track someone transcribed: only one this panel made.
            for seg in p.proj.children_by_ref("tier", &t, "segment")? {
                let from = p.proj.field_str(&seg, "x.converted-from")?;
                if from.is_none() || p.proj.field_str(from.as_deref().unwrap_or(""), "tier")?.as_deref() != Some(tier) {
                    return Err(format!("The «{target}» track holds segments that were not converted from this track; name another track to write."));
                }
            }
            t
        }
        None => p.add_tier(target, "time-aligned", Some("transcription"), None)?.0,
    };
    let mut per_text: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (t, seg, after) in rows {
        match per_text.last_mut() {
            Some((last, list)) if *last == t => list.push((seg, after)),
            _ => per_text.push((t, vec![(seg, after)])),
        }
    }
    let mut total = ConvertReport::default();
    for (t, list) in &per_text {
        let rows: Vec<ConvertedRow> = list.iter().map(|(s, c)| ConvertedRow { source: s, content: c }).collect();
        let (r, _) = p.write_converted(t, &track, &rows, by)?;
        total.created += r.created;
        total.updated += r.updated;
        total.unchanged += r.unchanged;
        total.kept += r.kept;
        total.empty += r.empty;
    }
    Ok(Converted {
        track_name: p.proj.field_str(&track, "name")?.unwrap_or_default(),
        track,
        texts: per_text.len(),
        created: total.created,
        updated: total.updated,
        unchanged: total.unchanged,
        kept: total.kept,
        empty: total.empty,
        unknown: Vec::new(),
    })
}

/// A segment of a track as another tool converts it.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackLine {
    pub segment: String,
    pub content: String,
}

/// Every segment of `tier` (one text or all) with its transcription, in
/// text and time order, for a tool that converts it itself (Sound changes).
pub fn track_sources(session: &mut Session, tier: &str, text: Option<String>) -> Result<Vec<TrackLine>, String> {
    let p = session::project(session)?;
    Ok(sources(p, tier, text.as_deref())?.into_iter().map(|(_, segment, content)| TrackLine { segment, content }).collect())
}

/// Write rows another tool converted from `tier` onto the track `target`,
/// as Write the track does, each kept with `by` (what made it).
pub fn write_rows(session: &mut Session, tier: &str, target: &str, rows: Vec<TrackLine>, by: &str) -> Result<Converted, String> {
    let p = session::project(session)?;
    let mut out = Vec::new();
    for r in rows {
        if p.proj.field_str(&r.segment, "tier")?.as_deref() != Some(tier) {
            return Err("A row is not a segment of the track being converted.".into());
        }
        let text = p.proj.field_str(&r.segment, "text")?.unwrap_or_default();
        out.push((text, r.segment, p.proj.nfc(r.content.split_whitespace().collect::<Vec<_>>().join(" ").as_str())));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    write_track(p, tier, target, out, by)
}

/// The lexicon's headwords through the profile, as written pronunciations.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pronounced {
    /// Entries that get a pronunciation (or got one, when written).
    pub entries: usize,
    /// Entries that already have a written pronunciation, left alone.
    pub already: usize,
    /// Entries whose headword has a letter the profile does not convert, left out.
    pub unconverted: usize,
    /// A few headwords and what they become, for the panel to show first.
    pub examples: Vec<Change>,
    pub unknown: Vec<Unknown>,
    pub written: bool,
}

/// Every live entry with a headword and no written pronunciation gets its
/// headword through the profile's `column` as one, in one txn, when `write`;
/// else only counted. A headword with a letter no row matches is left out,
/// so nothing half-converted lands in the dictionary.
pub fn pronounce(session: &mut Session, profile: &str, column: &str, ignore_case: bool, write: bool) -> Result<Pronounced, String> {
    let parsed = Profile::parse(profile);
    let col = parsed.column(column)?;
    if write {
        save(session, profile)?;
    }
    let p = session::project(session)?;
    let mut forms: Vec<(String, String)> = Vec::new();
    let mut unknown = HashMap::new();
    let mut out = Pronounced { entries: 0, already: 0, unconverted: 0, examples: Vec::new(), unknown: Vec::new(), written: false };
    let mut ids = p.proj.entities_of_kind("lexeme")?;
    ids.sort();
    for id in ids {
        if p.proj.is_tombstoned(&id)? {
            continue;
        }
        let lemma = p.proj.field_str(&id, "lemma")?.unwrap_or_default();
        if lemma.trim().is_empty() {
            continue;
        }
        if p.pronunciations(&id)?.iter().any(|x| !x.form.trim().is_empty()) {
            out.already += 1;
            continue;
        }
        let (after, found) = parsed.convert(&lemma, col, ignore_case);
        if !found.is_empty() {
            tally(&mut unknown, found, &lemma);
            out.unconverted += 1;
            continue;
        }
        if after.trim().is_empty() {
            continue;
        }
        if out.examples.len() < PREVIEW_ROWS {
            out.examples.push(Change { before: lemma.clone(), after: after.clone() });
        }
        forms.push((id, after));
    }
    out.entries = forms.len();
    out.unknown = unknown_list(unknown);
    if write && !forms.is_empty() {
        let (added, _) = p.add_pronunciation_forms(&forms, &format!("orthography profile: {column}"))?;
        out.entries = added;
        out.written = true;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headwords_become_written_pronunciations() {
        let dir = std::env::temp_dir().join(format!("wugs-pronounce-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "tester").unwrap();
        for (lemma, gloss) in [("shanga", "sand"), ("hay", "here"), ("aqa", "who")] {
            crate::lexicon::lexicon_add(&mut s, lemma, gloss).unwrap();
        }
        let said = crate::lexicon::lexicon_list(&mut s, "hay").unwrap()[0].id.clone();
        crate::lexicon::lexicon_add_pronunciation(&mut s, &said, "hai", None).unwrap();
        let first = pronounce(&mut s, PROFILE, "IPA", true, false).unwrap();
        assert_eq!((first.entries, first.already, first.unconverted, first.written), (1, 1, 1, false), "{first:?}");
        assert_eq!(first.examples[0].after, "ʃaŋa");
        assert_eq!(first.unknown[0].text, "q", "aqa has a q no row converts");
        let done = pronounce(&mut s, PROFILE, "IPA", true, true).unwrap();
        assert_eq!((done.entries, done.written), (1, true));
        let shanga = crate::lexicon::lexicon_list(&mut s, "shanga").unwrap()[0].id.clone();
        let view = crate::lexicon::entry_view(session::project(&mut s).unwrap(), &shanga).unwrap();
        assert_eq!(view.pronunciations.iter().map(|x| x.form.as_str()).collect::<Vec<_>>(), ["ʃaŋa"]);
        assert_eq!(pronounce(&mut s, PROFILE, "IPA", true, false).unwrap().entries, 0, "now it has one");
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    const PROFILE: &str = "# a practical orthography\nGrapheme\tIPA\tFrequency\tComment\nsh\tʃ\t3\t\na\ta\t9\t\nng\tŋ\t2\t\nn\t\t4\tas written\n^h\tʔ\t1\tword-initial h is a glottal stop\nh\th\t1\t\ny$\ti\t1\t\ny\tj\t1\t\nx\tNULL\t1\t\nj\tdʒ\t\t\n";

    #[test]
    fn a_profile_reads_its_columns_and_says_what_is_wrong() {
        let p = Profile::parse(PROFILE);
        assert_eq!(p.check(), Check { columns: vec!["IPA".into()], rules: 10, problems: vec![] });
        let bad = Profile::parse("IPA\tOrtho\nʃ\tsh\n");
        assert!(bad.problems[0].contains("no Grapheme column"));
        let twice = Profile::parse("Grapheme\tIPA\na\ta\na\tɑ\n\tx\n");
        assert_eq!(twice.rules.len(), 1);
        assert_eq!(twice.problems.len(), 2, "{:?}", twice.problems);
        assert!(twice.problems[0].contains("Line 3"));
    }

    #[test]
    fn forms_cut_into_segments_by_profile_or_by_letter() {
        let p = Profile::parse(PROFILE);
        let ipa = p.column("IPA").unwrap();
        assert_eq!(p.segments("Shangax", ipa, true), vec!["ʃ", "a", "ŋ", "a"]);
        assert_eq!(Profile::parse("Grapheme\tIPA\nts\tt s\n").segments("tsa", 0, false), vec!["t", "s", "a"]);
        assert_eq!(letters("tʰaːe\u{301}, ka"), vec!["tʰ", "aː", "é", "k", "a"]);
    }

    #[test]
    fn words_convert_longest_first_with_word_edges() {
        let p = Profile::parse(PROFILE);
        let ipa = p.column("IPA").unwrap();
        // sh before s+h, ng before n+g, ^h only word-initially, y$ only finally,
        // NULL writes nothing, an empty cell keeps the letter.
        assert_eq!(p.convert("shanga hah", ipa, false).0, "ʃaŋa ʔah");
        assert_eq!(p.convert("yay", ipa, false).0, "jai");
        assert_eq!(p.convert("naxa", ipa, false).0, "naa");
        assert_eq!(p.convert("Sha, jaz!", ipa, true), ("ʃa, dʒaz!".to_string(), vec!["z".to_string()]), "capitals read as small, punctuation passes, z is unknown");
        assert_eq!(p.convert("Sha", ipa, false).1, vec!["S".to_string()]);
        // A combining mark stays with an unknown letter; NFD input reads as NFC.
        assert_eq!(p.convert("e\u{301}", ipa, false), ("é".to_string(), vec!["é".to_string()]));
        assert_eq!(Profile::parse("Grapheme\tIPA\né\te\n").convert("e\u{301}", 0, false).0, "e");
    }

    #[test]
    fn a_track_converts_onto_an_ipa_track_the_sounds_panel_reads() {
        let dir = std::env::temp_dir().join(format!("wugs-orthography-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let st = stored(&mut session).unwrap();
        assert_eq!(st.profile, "");
        let source = st.tracks[0].id.clone();
        let draft = draft(&mut session, &source, None, true).unwrap();
        assert!(draft.starts_with("Grapheme\tIPA\tFrequency\n"));
        assert!(draft.lines().any(|l| l.starts_with("ʌ\tʌ\t")), "{draft}");
        // Write θ as th, as a practical spelling might.
        let profile = format!("{draft}θ\tth\t\n").replacen("θ\tθ\t", "ϑ\tϑ\t", 1);
        let pv = preview(&mut session, &profile, &source, None, "IPA", true).unwrap();
        assert!(pv.unknown.is_empty(), "{:?}", pv.unknown);
        assert!(pv.changes.iter().any(|c| c.after.contains("thɹ")), "{:?}", pv.changes);
        let done = convert(&mut session, &profile, &source, None, "IPA", true, "Practical").unwrap();
        assert!(done.created >= 2, "{done:?}");
        assert_eq!(stored(&mut session).unwrap().profile, profile, "the profile is kept");
        // A real profile runs to hundreds of rows; one past what a setting holds is refused in words.
        let long: String = std::iter::once("Grapheme\tIPA\tComment\n".to_string()).chain((0..600).map(|i| format!("x{i}\tx{i}\ta row with a note about it\n"))).collect();
        assert!(long.len() > 16_000);
        save(&mut session, &long).unwrap();
        let too_long = "Grapheme\tIPA\n".to_string() + &"a\ta\n".repeat(20_000);
        assert!(save(&mut session, &too_long).unwrap_err().contains("at most 64 KB"));
        save(&mut session, &profile).unwrap();
        let again = convert(&mut session, &profile, &source, None, "IPA", true, "practical").unwrap();
        assert_eq!((again.created, again.updated, again.unchanged), (0, 0, done.created), "the same track, nothing new");
        let words = crate::sounds::sound_words(&mut session, Some(&done.track), None).unwrap();
        assert!(words.words.iter().any(|w| w.word.contains("thɹ")), "{:?}", words.words);
        assert!(convert(&mut session, &profile, &source, None, "IPA", true, &st.tracks[0].name).is_err(), "not onto itself");
        // Nor into a track transcribed by hand.
        let hand = crate::session::add_tier(&mut session, "transcription", Some("Phonemic")).unwrap();
        let text = session::project(&mut session).unwrap().proj.entities_of_kind("text").unwrap()[0].clone();
        session::project(&mut session).unwrap().add_segment(&text, &hand.id, 0, Some(0.0), Some(500.0), "wan").unwrap();
        let refused = convert(&mut session, &profile, &source, None, "IPA", true, "phonemic").unwrap_err();
        assert!(refused.contains("not converted from this track"), "{refused}");
        // Another tool's conversion (Sound changes) goes onto a track the same way.
        let lines = track_sources(&mut session, &source, None).unwrap();
        assert!(lines.iter().any(|l| l.content == "wʌn tuː θɹiː"), "{lines:?}");
        let changed: Vec<TrackLine> = lines.iter().map(|l| TrackLine { segment: l.segment.clone(), content: l.content.replace('θ', "t") }).collect();
        let written = write_rows(&mut session, &source, "After rules", changed, "sound changes").unwrap();
        assert!(written.created >= 2, "{written:?}");
        let words = crate::sounds::sound_words(&mut session, Some(&written.track), None).unwrap();
        assert!(words.words.iter().any(|w| w.word == "tɹiː"), "{:?}", words.words);
        let stray = vec![TrackLine { segment: lines[0].segment.clone(), content: "x".into() }];
        assert!(write_rows(&mut session, &written.track, "Other", stray, "sound changes").unwrap_err().contains("not a segment of the track"));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
