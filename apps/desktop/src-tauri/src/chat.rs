//! CHAT, the transcription format of CLAN and the TalkBank corpora
//! (CHILDES for child language, AphasiaBank, CABank and the rest).
//!
//! - **In:** one text per file. Each speaker's utterances go on a
//!   transcription track named by the speaker's code (`CHI`, `MOT`), so the
//!   turn-taking, agreement and corpus tools see who said what. An
//!   utterance's time bullet (`·1234_5678·`) times it. The line wugs shows
//!   and counts is read as CLAN reads words: codes in brackets, overlap and
//!   prosody marks, pauses, events (`&=laughs`) and omitted words (`0is`)
//!   left out; fillers and fragments (`&-uh`, `&+fr`) kept as words; a
//!   shortened form written whole (`(be)cause` → `because`); a form's
//!   `@` marker dropped (`dada@b` → `dada`). The main line as written is
//!   kept (`x.chat.main`), and so is every dependent tier (`%mor`, `%gra`,
//!   `%com` … as `x.chat.mor` …). Header lines become the text's details:
//!   languages, participants, media, the date (as YYYY-MM-DD), the place,
//!   the situation and others, and each speaker's `@ID` line (`id_chi`) with
//!   their age and sex as details of their own (`age_chi`, `sex_chi`), so
//!   texts can be picked by a child's age. An utterance with no words in it
//!   (`0 [=! laughs] .`) is left out and counted.
//! - **Out:** every transcription track of a text, as one transcript in
//!   time order, a speaker per track. A sentence whose text still reads as
//!   its kept main line is written as it came, dependent tiers and all;
//!   any other is written from what wugs holds, with a terminator, and its
//!   dependent tiers are left out, since they described the old line.

use std::fmt::Write as _;

/// One `*SPK:` line with what hangs under it.
#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    pub speaker: String,
    /// The main line as written, continuation lines joined, the time bullet taken out.
    pub main: String,
    /// The words as wugs shows them (`clean`).
    pub text: String,
    /// Start and end in milliseconds, from the line's time bullets.
    pub span: Option<(f64, f64)>,
    /// Dependent tiers by name without `%` (`mor`, `gra`, `com`), as written.
    pub tiers: Vec<(String, String)>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Chat {
    /// Header lines (`@Languages`, `@Date` …) by name without `@`, in order.
    pub headers: Vec<(String, String)>,
    pub utterances: Vec<Utterance>,
    /// Header and gem lines between utterances (`@Comment`, `@G`), not carried.
    pub between: usize,
}

/// CLAN marks a time bullet with U+0015 on each side: `·1234_5678·`.
const BULLET: char = '\u{15}';

/// Every bullet's `start_end` in a line, and the line without them.
fn bullets(line: &str) -> (String, Option<(f64, f64)>) {
    let mut out = String::new();
    let mut span: Option<(f64, f64)> = None;
    let mut rest = line;
    while let Some(at) = rest.find(BULLET) {
        out.push_str(&rest[..at]);
        let after = &rest[at + BULLET.len_utf8()..];
        let Some(end) = after.find(BULLET) else {
            rest = after;
            break;
        };
        let inner = &after[..end];
        // `%snd:"file"_1_2` and `1_2` both end in two numbers.
        let nums: Vec<&str> = inner.rsplitn(3, '_').collect();
        if nums.len() >= 2 {
            if let (Ok(e), Ok(s)) = (nums[0].trim().parse::<f64>(), nums[1].trim().parse::<f64>()) {
                span = Some(match span {
                    Some((a, b)) => (a.min(s), b.max(e)),
                    None => (s, e),
                });
            }
        }
        rest = &after[end + BULLET.len_utf8()..];
    }
    out.push_str(rest);
    (out.trim().to_string(), span.filter(|(s, e)| e > s))
}

/// Read a CHAT file. Errors only when there is no utterance at all.
pub fn read(input: &str) -> Result<Chat, String> {
    let mut chat = Chat::default();
    // Logical lines: a line starting with a tab continues the one before.
    let mut lines: Vec<String> = Vec::new();
    for raw in input.lines() {
        let raw = raw.trim_end_matches('\r');
        if raw.starts_with('\t') || (raw.starts_with(' ') && !lines.is_empty()) {
            if let Some(last) = lines.last_mut() {
                last.push(' ');
                last.push_str(raw.trim());
                continue;
            }
        }
        lines.push(raw.to_string());
    }
    let mut begun = false;
    for line in &lines {
        if let Some(rest) = line.strip_prefix('*') {
            let Some((code, main)) = rest.split_once(':') else {
                continue;
            };
            let (main, span) = bullets(main);
            begun = true;
            chat.utterances.push(Utterance {
                speaker: code.trim().to_string(),
                text: clean(&main),
                main,
                span,
                tiers: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix('%') {
            let Some((name, value)) = rest.split_once(':') else {
                continue;
            };
            if let Some(u) = chat.utterances.last_mut() {
                // A dependent tier's own bullet (`%snd`) is kept as written.
                u.tiers
                    .push((name.trim().to_string(), value.trim().to_string()));
            }
        } else if let Some(rest) = line.strip_prefix('@') {
            let (name, value) = match rest.split_once(':') {
                Some((n, v)) => (n.trim(), v.trim()),
                None => (rest.trim(), ""),
            };
            if matches!(name, "UTF8" | "Begin" | "End" | "Blank" | "New Episode") {
                continue;
            }
            if begun {
                chat.between += 1;
            } else {
                chat.headers.push((name.to_string(), value.to_string()));
            }
        }
    }
    if chat.utterances.is_empty() {
        return Err(
            "it has no utterances (lines starting with *), so it is not a CHAT transcript".into(),
        );
    }
    Ok(chat)
}

/// Marks of overlap, prosody and the like that CA transcripts in CHAT
/// write inside words; none is part of a word.
const MARKS: &[char] = &[
    '⌈', '⌉', '⌊', '⌋', '↑', '↓', '°', '∆', '∇', '▁', '▔', '☺', '♋', '⁇', '∬', 'Ϋ', '∲', '§', '∾',
    '⁑', '⤇', '∙', 'Ἡ', '↻', '≈', '≋', '〔', '〕', '⇗', '↗', '→', '↘', '⇘', '∞', '≡', '‡', '„',
    '<', '>', '^', '≠',
];

/// The words of a main line, as CLAN's word counts read them.
pub fn clean(main: &str) -> String {
    // Codes in square brackets go first: `[//]`, `[: want to]`, `[=! laughs]`.
    let mut s = String::new();
    let mut depth = 0usize;
    for c in main.chars() {
        match c {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            _ => s.push(c),
        }
    }
    let mut words: Vec<String> = Vec::new();
    for token in s.split_whitespace() {
        let token: String = token.chars().filter(|c| !MARKS.contains(c)).collect();
        if token.is_empty() {
            continue;
        }
        if token == "+..." {
            words.push("…".into());
            continue;
        }
        if token == "+..?" {
            words.push("…?".into());
            continue;
        }
        // Other `+` codes are linkers and special terminators (`+/.`, `+<`).
        if token.starts_with('+') {
            continue;
        }
        // Events and the like: `&=laughs`, `&{l=…`, `&}l=…`.
        if token.starts_with("&=") || token.starts_with("&{") || token.starts_with("&}") {
            continue;
        }
        // Pauses: `(.)`, `(..)`, `(...)`, `(1.5)`, `(2:13.4)`.
        if token.starts_with('(')
            && token.ends_with(')')
            && token[1..token.len() - 1]
                .chars()
                .all(|c| c == '.' || c == ':' || c.is_ascii_digit())
        {
            continue;
        }
        // An omitted word: `0is`, or a lone `0` for an action with no words.
        if token == "0"
            || (token.starts_with('0') && token[1..].starts_with(|c: char| c.is_alphabetic()))
        {
            continue;
        }
        let token = token
            .strip_prefix("&-")
            .or_else(|| token.strip_prefix("&+"))
            .or_else(|| token.strip_prefix("&~"))
            .unwrap_or(&token)
            .to_string();
        // A form's marker: `dada@b`, `gato@s:spa`.
        let token = match token.find('@') {
            Some(i) if i > 0 => token[..i].to_string(),
            _ => token,
        };
        // Sounds left out are written in parentheses (`(be)cause`), and a
        // colon inside a word lengthens a sound (`no:`).
        let word: String = if token.chars().any(char::is_alphabetic) {
            token
                .chars()
                .filter(|c| !matches!(c, '(' | ')' | ':'))
                .collect()
        } else {
            token
        };
        if !word.is_empty() {
            words.push(word);
        }
    }
    let mut out = String::new();
    for w in words {
        let punct = matches!(w.as_str(), "." | "?" | "!" | "," | "…" | "…?");
        if !out.is_empty() && !punct {
            out.push(' ');
        }
        out.push_str(&w);
    }
    if out.chars().any(char::is_alphanumeric) {
        out
    } else {
        String::new()
    }
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// CHAT's `12-JAN-1990` as `1990-01-12`; None when it is not one.
pub fn iso_date(chat: &str) -> Option<String> {
    let mut parts = chat.trim().split('-');
    let (d, m, y) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || d.is_empty()
        || d.len() > 2
        || y.len() != 4
        || !d.chars().chain(y.chars()).all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let m = MONTHS.iter().position(|x| x.eq_ignore_ascii_case(m))? + 1;
    let d: u32 = d.parse().ok()?;
    (1..=31).contains(&d).then(|| format!("{y}-{m:02}-{d:02}"))
}

/// `1990-01-12` as CHAT writes it, `12-JAN-1990`.
pub fn chat_date(iso: &str) -> Option<String> {
    let mut parts = iso.trim().split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let month = MONTHS.get(m.parse::<usize>().ok()?.checked_sub(1)?)?;
    Some(format!("{d}-{month}-{y}"))
}

/// Header names kept as the text's details under a name of their own;
/// any other header is kept under its own name.
fn detail_key(header: &str) -> String {
    match header {
        "Location" => "place".into(),
        "Date" => "date".into(),
        other => other.to_lowercase(),
    }
}

/// The text's details from the header: `(key, value)`, keys as written
/// before `key_of` tidies them (`Recording Quality` → `recording_quality`).
pub fn details(chat: &Chat) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |k: String, v: &str| {
        let v = v.trim();
        if v.is_empty() {
            return;
        }
        match out.iter_mut().find(|(have, _)| *have == k) {
            Some((_, old)) => {
                old.push_str("; ");
                old.push_str(v);
            }
            None => out.push((k, v.to_string())),
        }
    };
    for (name, value) in &chat.headers {
        if name == "ID" {
            // lang|corpus|code|age|sex|group|SES|role|education|custom|
            let f: Vec<&str> = value.split('|').collect();
            let code = f
                .get(2)
                .map(|c| c.trim().to_lowercase())
                .unwrap_or_default();
            if code.is_empty() {
                continue;
            }
            push(format!("id {code}"), value);
            if let Some(age) = f.get(3) {
                push(format!("age {code}"), age);
            }
            if let Some(sex) = f.get(4) {
                push(format!("sex {code}"), sex);
            }
            continue;
        }
        if name == "Date" {
            if let Some(d) = iso_date(value) {
                push("date".into(), &d);
                continue;
            }
        }
        push(detail_key(name), value);
    }
    out
}

/// A track's name as a CHAT speaker code: letters and digits, upper case,
/// at most seven; `SPK` when nothing is left.
pub fn speaker_code(name: &str) -> String {
    let code: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(7)
        .collect::<String>()
        .to_ascii_uppercase();
    if code.is_empty() {
        "SPK".into()
    } else {
        code
    }
}

/// One sentence to write: its speaker's code, what it reads now, its time
/// and what was kept from the file it came from.
pub struct Line {
    pub code: String,
    pub content: String,
    pub span: Option<(f64, f64)>,
    pub main: Option<String>,
    pub tiers: Vec<(String, String)>,
}

/// A main line from what wugs holds: spaces before CHAT's punctuation and a
/// terminator at the end.
fn main_of(content: &str) -> String {
    let mut out = String::new();
    for w in content.split_whitespace() {
        let mut w = w;
        let mut trail = Vec::new();
        loop {
            if let Some(rest) = w.strip_suffix("…?") {
                trail.push("+..?");
                w = rest;
            } else if let Some(rest) = w.strip_suffix('…') {
                trail.push("+...");
                w = rest;
            } else if let Some(c) = w
                .chars()
                .last()
                .filter(|c| matches!(c, '.' | '?' | '!' | ','))
            {
                trail.push(match c {
                    '.' => ".",
                    '?' => "?",
                    '!' => "!",
                    _ => ",",
                });
                w = &w[..w.len() - 1];
            } else {
                break;
            }
        }
        if !w.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(w);
        }
        // Only the last of a run of marks stands (`?!` → `!`), since CHAT takes one.
        if let Some(t) = trail.first() {
            out.push(' ');
            out.push_str(t);
        }
    }
    let ends = [" .", " ?", " !", " +...", " +..?"];
    if out.ends_with(" ,") {
        out.truncate(out.len() - 2);
    }
    if !ends.iter().any(|e| out.ends_with(e)) {
        out.push_str(" .");
    }
    out
}

/// The transcript: `headers` are what to write above the utterances, in order.
pub fn write(headers: &[(String, String)], lines: &[Line]) -> String {
    let mut out = String::from("@UTF8\n@Begin\n");
    for (name, value) in headers {
        let _ = writeln!(out, "@{name}:\t{value}");
    }
    for l in lines {
        let kept = l.main.as_deref().filter(|m| clean(m) == l.content);
        let main = kept
            .map(str::to_string)
            .unwrap_or_else(|| main_of(&l.content));
        let _ = write!(out, "*{}:\t{main}", l.code);
        if let Some((s, e)) = l.span {
            let _ = write!(
                out,
                " {BULLET}{}_{}{BULLET}",
                s.round() as i64,
                e.round() as i64
            );
        }
        out.push('\n');
        if kept.is_some() {
            for (name, value) in &l.tiers {
                let _ = writeln!(out, "%{name}:\t{value}");
            }
        }
    }
    out.push_str("@End\n");
    out
}

/// Header lines about the display in CLAN, not the recording; never details.
const DISPLAY_HEADERS: [&str; 4] = ["Font", "ColorWords", "Window", "Options"];

/// Import a CHAT file as one text named `title`; the text's id (None only
/// when nothing in it had words) and the report's lines.
pub fn import(
    p: &mut wugs_core::Project,
    input: &str,
    title: &str,
) -> Result<(Option<String>, Vec<String>), String> {
    use crate::interchange::plural;
    let chat = read(input)?;
    let title = p.proj.nfc(title);
    let spoken: Vec<&Utterance> = chat
        .utterances
        .iter()
        .filter(|u| !u.text.is_empty())
        .collect();
    if spoken.is_empty() {
        return Err("no utterance in it has words, so nothing was imported".into());
    }
    let units: Vec<String> = spoken.iter().map(|u| p.proj.nfc(&u.text)).collect();
    let mut lines = Vec::new();
    if let Some(t) = crate::interchange::same_text(p, &title, &units)? {
        lines.push("It matched a text already here; nothing was added.".into());
        return Ok((Some(t), lines));
    }
    // A transcription track per speaker, found by name or made.
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    let mut tracks: Vec<(String, String)> = Vec::new();
    let mut made = Vec::new();
    for u in &spoken {
        if tracks.iter().any(|(code, _)| *code == u.speaker) {
            continue;
        }
        let mut found = None;
        for t in &transcription {
            if !p.proj.is_tombstoned(t)?
                && p.proj
                    .field_str(t, "name")?
                    .unwrap_or_default()
                    .trim()
                    .eq_ignore_ascii_case(&u.speaker)
            {
                found = Some(t.clone());
                break;
            }
        }
        let id = match found {
            Some(t) => t,
            None => {
                made.push(u.speaker.clone());
                p.add_tier(&u.speaker, "time-aligned", Some("transcription"), None)?
                    .0
            }
        };
        tracks.push((u.speaker.clone(), id));
    }
    let extras: Vec<Vec<(String, String)>> = spoken
        .iter()
        .map(|u| {
            let mut x = vec![("x.chat.main".to_string(), p.proj.nfc(&u.main))];
            for (name, value) in &u.tiers {
                x.push((format!("x.chat.{name}"), p.proj.nfc(value)));
            }
            if !u.tiers.is_empty() {
                x.push((
                    "x.chat.order".into(),
                    u.tiers
                        .iter()
                        .map(|(n, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                ));
            }
            x
        })
        .collect();
    let track_of = |code: &str| {
        tracks
            .iter()
            .find(|(c, _)| c == code)
            .map(|(_, t)| t.as_str())
    };
    let rows: Vec<wugs_core::WrittenRow> = spoken
        .iter()
        .enumerate()
        .map(|(i, u)| wugs_core::WrittenRow {
            text: &units[i],
            translation: "",
            extras: extras[i].clone(),
            span: u.span,
            tier: track_of(&u.speaker),
        })
        .collect();
    let first = tracks[0].1.clone();
    let (text, _) = p.import_written_rows(&title, &first, None, &rows)?;
    let meta: Vec<(String, String, String)> = details(&chat)
        .into_iter()
        .filter(|(k, _)| !DISPLAY_HEADERS.iter().any(|d| d.eq_ignore_ascii_case(k)))
        .map(|(k, v)| {
            (
                text.clone(),
                k,
                p.proj.nfc(&v.chars().take(2000).collect::<String>()),
            )
        })
        .collect();
    p.import_text_metadata(&meta)?;
    let codes: Vec<&str> = tracks.iter().map(|(c, _)| c.as_str()).collect();
    lines.push(format!(
        "A new text with {}, a track per speaker ({}).",
        plural(units.len(), "utterance", "utterances"),
        codes.join(", ")
    ));
    if !made.is_empty() {
        lines.push(format!(
            "New {}: {}.",
            if made.len() == 1 { "track" } else { "tracks" },
            made.join(", ")
        ));
    }
    let untimed = spoken.iter().filter(|u| u.span.is_none()).count();
    if untimed > 0 && untimed < spoken.len() {
        lines.push(format!(
            "{} had no time bullet, so {} untimed.",
            plural(untimed, "utterance", "utterances"),
            if untimed == 1 {
                "it came in"
            } else {
                "they came in"
            }
        ));
    }
    let silent = chat.utterances.len() - spoken.len();
    if silent > 0 {
        lines.push(format!(
            "{} with no words (only an action or a sound) {} left out.",
            plural(silent, "utterance", "utterances"),
            if silent == 1 { "was" } else { "were" }
        ));
    }
    if chat.between > 0 {
        lines.push(format!(
            "{} between utterances (comments, gems) {} not carried.",
            plural(chat.between, "header line", "header lines"),
            if chat.between == 1 { "was" } else { "were" }
        ));
    }
    if !meta.is_empty() {
        lines.push("The header (languages, participants, ages, date, place…) is kept as the text's details; dependent tiers such as %mor stay with each utterance.".into());
    }
    if spoken.iter().any(|u| u.span.is_some()) {
        lines.push("Pick the recording in the Timeline (or the Media panel) to hear it.".into());
    }
    Ok((Some(text), lines))
}

/// A text as a CHAT transcript: every transcription track, a speaker per
/// track, in time order (untimed sentences after the timed ones, in text
/// order). Returns the file and notes for the report.
pub fn export(p: &wugs_core::Project, text_id: &str) -> Result<(String, Vec<String>), String> {
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    let meta = p.text_metadata(text_id)?;
    let get = |k: &str| {
        meta.iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    let mut rows: Vec<(Option<f64>, usize, Line, String)> = Vec::new();
    let mut codes: Vec<(String, String)> = Vec::new();
    for (i, seg) in p
        .proj
        .ordered_children("text", text_id, "segment")?
        .into_iter()
        .enumerate()
    {
        let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
        if !transcription.contains(&tier) {
            continue;
        }
        let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }
        if !codes.iter().any(|(t, _)| *t == tier) {
            let base = speaker_code(&p.proj.field_str(&tier, "name")?.unwrap_or_default());
            let mut code = base.clone();
            let mut n = 2;
            while codes.iter().any(|(_, c)| *c == code) {
                code = format!("{}{n}", &base[..base.len().min(6)]);
                n += 1;
            }
            codes.push((tier.clone(), code));
        }
        let code = codes
            .iter()
            .find(|(t, _)| *t == tier)
            .map(|(_, c)| c.clone())
            .unwrap_or_default();
        let (start, end) = (
            p.proj.field_num(&seg, "start")?,
            p.proj.field_num(&seg, "end")?,
        );
        let span = match (start, end) {
            (Some(s), Some(e)) if e > s => Some((s, e)),
            _ => None,
        };
        let main = p.proj.field_str(&seg, "x.chat.main")?;
        let mut tiers = Vec::new();
        let order = p.proj.field_str(&seg, "x.chat.order")?.unwrap_or_default();
        for name in order.split_whitespace() {
            if let Some(v) = p.proj.field_str(&seg, &format!("x.chat.{name}"))? {
                tiers.push((name.to_string(), v));
            }
        }
        rows.push((
            span.map(|s| s.0),
            i,
            Line {
                code,
                content: content.trim().to_string(),
                span,
                main,
                tiers,
            },
            tier,
        ));
    }
    if rows.is_empty() {
        return Err(
            "This text has no transcribed sentences, so there is nothing to write as CHAT.".into(),
        );
    }
    rows.sort_by(|a, b| match (a.0, b.0) {
        (Some(x), Some(y)) => x.total_cmp(&y).then(a.1.cmp(&b.1)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.1.cmp(&b.1),
    });
    let language = p.project_language()?;
    let lang = get("languages").unwrap_or_else(|| {
        if language.iso.is_empty() {
            "und".into()
        } else {
            language.iso.clone()
        }
    });
    // A participant entry from the file (`CHI Ross Target_Child`), else one with no role known.
    let given = get("participants").unwrap_or_default();
    let entries: Vec<String> = codes
        .iter()
        .map(|(_, code)| {
            given
                .split(',')
                .map(str::trim)
                .find(|e| e.split_whitespace().next() == Some(code.as_str()))
                .map(str::to_string)
                .unwrap_or_else(|| format!("{code} Unidentified"))
        })
        .collect();
    let mut headers: Vec<(String, String)> = vec![
        ("Languages".into(), lang.clone()),
        ("Participants".into(), entries.join(", ")),
    ];
    let corpus = speaker_code(&p.proj.field_str(text_id, "title")?.unwrap_or_default());
    for ((_, code), entry) in codes.iter().zip(&entries) {
        let role = entry.split_whitespace().last().unwrap_or("Unidentified");
        let id = get(&format!("id_{}", code.to_lowercase())).unwrap_or_else(|| {
            format!(
                "{}|{}|{code}|||||{role}|||",
                lang.split(',').next().unwrap_or("und").trim(),
                corpus.to_lowercase()
            )
        });
        headers.push(("ID".into(), id));
    }
    if let Some(m) = get("media") {
        headers.push(("Media".into(), m));
    }
    if let Some(d) = get("date").and_then(|d| chat_date(&d)) {
        headers.push(("Date".into(), d));
    }
    for (key, name) in [
        ("place", "Location"),
        ("situation", "Situation"),
        ("activities", "Activities"),
        ("transcriber", "Transcriber"),
        ("comment", "Comment"),
    ] {
        if let Some(v) = get(key) {
            headers.push((name.into(), v));
        }
    }
    let lines: Vec<Line> = rows.into_iter().map(|r| r.2).collect();
    let fresh = lines
        .iter()
        .filter(|l| l.main.as_deref().is_none_or(|m| clean(m) != l.content))
        .count();
    let mut notes = Vec::new();
    if fresh > 0 && lines.iter().any(|l| l.main.is_some()) {
        notes.push(format!("{} changed in wugs {} written from the text as it reads now, without {} dependent tiers.", crate::interchange::plural(fresh, "utterance", "utterances"), if fresh == 1 { "was" } else { "were" }, if fresh == 1 { "its" } else { "their" }));
    }
    Ok((write(&headers, &lines), notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child, MOT Mother\n@ID:\teng|Synth|CHI|2;03.12|female|||Target_Child|||\n@ID:\teng|Synth|MOT|||||Mother|||\n@Media:\tsynth, audio\n@Date:\t12-JAN-1990\n@Location:\tKitchen\n*MOT:\tlook at the doggie [!] . \u{15}0_1200\u{15}\n%mor:\tv|look prep|at det:art|the n|doggie .\n%gra:\t1|0|ROOT 2|1|JCT 3|4|DET 4|2|POBJ 5|1|PUNCT\n*CHI:\t&-uh (be)cause <I want> [/] I want\n\tdada@b +...  \u{15}1300_2900\u{15}\n%com:\tpoints\n@Comment:\tdog barks\n*CHI:\t0 [=! laughs] .\n*MOT:\tno: &=sighs 0is that a xxx ? \u{15}3000_3800\u{15}\n@End\n";

    #[test]
    fn reads_speakers_times_words_and_tiers() {
        let c = read(SAMPLE).unwrap();
        assert_eq!(c.utterances.len(), 4);
        let u = &c.utterances[0];
        assert_eq!(
            (u.speaker.as_str(), u.text.as_str(), u.span),
            ("MOT", "look at the doggie.", Some((0.0, 1200.0)))
        );
        assert_eq!(u.main, "look at the doggie [!] .");
        assert_eq!(
            u.tiers.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["mor", "gra"]
        );
        let u = &c.utterances[1];
        assert_eq!(
            u.text, "uh because I want I want dada…",
            "a retracing is still said, a filler is a word, a continuation line joins"
        );
        assert_eq!(u.span, Some((1300.0, 2900.0)));
        assert_eq!(c.utterances[2].text, "", "no words: only an event");
        assert_eq!(
            c.utterances[3].text, "no that a xxx?",
            "lengthening, events and an omitted word"
        );
        assert_eq!(c.between, 1, "the comment between utterances");
        assert!(read("@Begin\n@End\n").is_err());
    }

    #[test]
    fn header_lines_become_details() {
        let d = details(&read(SAMPLE).unwrap());
        let get = |k: &str| d.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
        assert_eq!(get("date"), Some("1990-01-12"));
        assert_eq!(get("place"), Some("Kitchen"));
        assert_eq!(get("languages"), Some("eng"));
        assert_eq!(get("age chi"), Some("2;03.12"));
        assert_eq!(get("sex chi"), Some("female"));
        assert_eq!(get("age mot"), None, "an empty field is no detail");
        assert_eq!(get("id mot"), Some("eng|Synth|MOT|||||Mother|||"));
        assert_eq!(get("media"), Some("synth, audio"));
    }

    #[test]
    fn dates_go_both_ways() {
        assert_eq!(iso_date("3-feb-2001").as_deref(), Some("2001-02-03"));
        assert_eq!(iso_date("2001-02-03"), None);
        assert_eq!(chat_date("2001-02-03").as_deref(), Some("03-FEB-2001"));
        assert_eq!(chat_date("2001"), None);
    }

    #[test]
    fn an_unchanged_line_goes_back_as_it_came() {
        let c = read(SAMPLE).unwrap();
        let lines: Vec<Line> = c
            .utterances
            .iter()
            .filter(|u| !u.text.is_empty())
            .map(|u| Line {
                code: u.speaker.clone(),
                content: u.text.clone(),
                span: u.span,
                main: Some(u.main.clone()),
                tiers: u.tiers.clone(),
            })
            .collect();
        let out = write(&[("Languages".into(), "eng".into())], &lines);
        assert!(out.starts_with("@UTF8\n@Begin\n@Languages:\teng\n*MOT:\tlook at the doggie [!] . \u{15}0_1200\u{15}\n%mor:\tv|look"));
        assert!(out.ends_with("@End\n"));
        let again = read(&out).unwrap();
        assert_eq!(
            again
                .utterances
                .iter()
                .map(|u| u.text.clone())
                .collect::<Vec<_>>(),
            lines.iter().map(|l| l.content.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_edited_line_is_written_fresh_without_its_tiers() {
        let line = Line {
            code: "MOT".into(),
            content: "look at the dog, then!".into(),
            span: None,
            main: Some("look at the doggie [!] .".into()),
            tiers: vec![("mor".into(), "old".into())],
        };
        assert_eq!(
            write(&[], &[line]),
            "@UTF8\n@Begin\n*MOT:\tlook at the dog , then !\n@End\n"
        );
        let plain = Line {
            code: "A".into(),
            content: "so then…".into(),
            span: Some((10.4, 20.0)),
            main: None,
            tiers: Vec::new(),
        };
        assert_eq!(
            write(&[], &[plain]),
            "@UTF8\n@Begin\n*A:\tso then +... \u{15}10_20\u{15}\n@End\n"
        );
        assert_eq!(main_of("no words"), "no words .");
        assert_eq!(
            clean(&main_of("look at the dog, then!")),
            "look at the dog, then!"
        );
    }

    #[test]
    fn track_names_become_codes() {
        assert_eq!(speaker_code("chi"), "CHI");
        assert_eq!(speaker_code("Mother (Ana)"), "MOTHERA");
        assert_eq!(speaker_code("ʔə"), "SPK");
    }
}
