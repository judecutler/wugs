//! SIL Toolbox / Shoebox interlinear texts: the `\ref` records Toolbox's
//! Interlinearize writes, decades of field linguists' glossed texts.
//!
//! ```text
//! \id Story 1
//! \ref Story1.001
//! \ELANBegin 00:00:01.250
//! \ELANEnd 00:00:03.900
//! \tx  nikani      ŋai
//! \mb  ni-   kani  ŋai
//! \ge  1SG-  count one
//! \ft  I count one.
//! \nt  Speaker counts on her fingers.
//! ```
//!
//! Carried: `\id` starts a text (its title), each `\ref` a segment (the
//! reference kept as `x.toolbox.ref`), `\tx` the words, `\mb` the morphs
//! and `\ge` their glosses, aligned by column under the words as Toolbox
//! writes them (several `\tx` bundles in one record follow each other),
//! `\ft` the free translation (a segment on the translation track naming the
//! sentence as `parent`), `\nt` a note on the sentence, `\ELANBegin` /
//! `\ELANEnd` its times. Morph types come from their markers: `ni-` a
//! prefix, `-ler` a suffix, `=ma` or `ma=` a clitic, `<um>` an infix, else a
//! root; surfaces are stored without markers, as wugs stores them. A root
//! whose form and gloss match exactly one sense in the lexicon is linked to
//! it. Every other marker is counted in `not_carried`. One txn per text; a
//! text whose title and sentences are already here adds nothing.

use std::collections::HashMap;

use wugs_core::Project;
use wugs_model::{lexicon_key, new_id, pos_seq, Value};

use crate::common::Counts;

#[derive(Debug, Default)]
pub struct TextReport {
    pub texts: Vec<String>,
    pub matched: Vec<String>,
    pub sentences: usize,
    pub words: usize,
    pub morphs: usize,
    /// Roots linked to the one lexicon sense with their form and gloss.
    pub linked: usize,
    pub translations: usize,
    pub notes: usize,
    pub timed: usize,
    /// Markers read but not carried, by marker.
    pub not_carried: Counts,
}

/// Does this SFM file hold interlinear text (`\tx` lines) rather than a lexicon?
pub fn is_interlinear(input: &str) -> bool {
    let mut lx = 0;
    let mut tx = 0;
    for line in input.lines().take(2000) {
        match marker(line).map(|(m, _)| m) {
            Some("tx") => tx += 1,
            Some("lx") => lx += 1,
            _ => {}
        }
    }
    tx > 0 && tx >= lx
}

/// Does the file carry free translations, so the importer needs a translation track?
pub fn has_free_translation(input: &str) -> bool {
    input.lines().any(|l| marker(l).is_some_and(|(m, v)| m == "ft" && !v.trim().is_empty()))
}

/// `\marker value` → (marker, the rest of the line after the marker).
fn marker(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix('\\')?;
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some((&rest[..end], &rest[end..]))
}

/// A combining mark takes no column of its own (Toolbox aligns what it shows).
fn width(c: char) -> usize {
    let u = c as u32;
    let combining = (0x300..=0x36F).contains(&u) || (0x1AB0..=0x1AFF).contains(&u) || (0x1DC0..=0x1DFF).contains(&u) || (0x20D0..=0x20FF).contains(&u) || (0xFE20..=0xFE2F).contains(&u);
    usize::from(!combining)
}

/// The tokens of an aligned line with the column each starts at, counted
/// from the start of the whole line so lines with markers of different
/// lengths still line up.
fn columns(line: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut col = 0;
    let mut cur: Option<(usize, String)> = None;
    let skip = line.find(|c: char| c.is_whitespace()).unwrap_or(line.len());
    for (i, c) in line.char_indices() {
        if i < skip {
            col += width(c);
            continue;
        }
        if c.is_whitespace() {
            if let Some(t) = cur.take() {
                out.push(t);
            }
            col += if c == '\t' { 8 - col % 8 } else { 1 };
        } else {
            match &mut cur {
                Some((_, s)) => s.push(c),
                None => cur = Some((col, c.to_string())),
            }
            col += width(c);
        }
    }
    if let Some(t) = cur {
        out.push(t);
    }
    out
}

/// Which of `starts` (sorted) a token at `col` falls under: the last one at or before it.
fn under(starts: &[usize], col: usize) -> usize {
    starts.iter().rposition(|s| *s <= col).unwrap_or(0)
}

pub(crate) struct Morph {
    pub(crate) surface: String,
    pub(crate) kind: &'static str,
    pub(crate) gloss: String,
    /// The one lexicon sense with this root's form and gloss, found before the txn.
    pub(crate) sense: Option<String>,
    /// Its part of speech from a `\ps` line under the morphs, kept as `x.toolbox.ps`.
    pub(crate) ps: String,
}

pub(crate) struct Sentence {
    pub(crate) reference: String,
    pub(crate) words: Vec<(String, Vec<Morph>)>,
    pub(crate) translation: String,
    pub(crate) notes: Vec<String>,
    pub(crate) start: Option<f64>,
    pub(crate) end: Option<f64>,
}

pub(crate) struct Text {
    pub(crate) title: String,
    pub(crate) sentences: Vec<Sentence>,
}

fn morph_of(written: &str) -> (String, &'static str) {
    let w = written.trim();
    let kind = if w.starts_with('<') && w.ends_with('>') {
        "infix"
    } else if w.starts_with('=') || w.ends_with('=') {
        "clitic"
    } else if w.ends_with('-') && !w.starts_with('-') {
        "prefix"
    } else if w.starts_with('-') && !w.ends_with('-') {
        "suffix"
    } else {
        "root"
    };
    (w.trim_matches(|c| matches!(c, '-' | '=' | '<' | '>')).to_string(), kind)
}

/// `00:01:02.500`, `1:02.5` or `62.5` (seconds) → milliseconds.
fn ms(v: &str) -> Option<f64> {
    let mut secs = 0.0;
    for part in v.trim().split(':') {
        secs = secs * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    (secs.is_finite() && secs >= 0.0).then_some(secs * 1000.0)
}

/// One bundle: a `\tx` line and the `\mb` / `\ge` / `\ps` lines under it.
fn bundle(tx: &str, mb: Option<&str>, ge: Option<&str>, ps: Option<&str>) -> Vec<(String, Vec<Morph>)> {
    let words = columns(tx);
    let word_starts: Vec<usize> = words.iter().map(|w| w.0).collect();
    let mut out: Vec<(String, Vec<Morph>)> = words.iter().map(|w| (w.1.clone(), Vec::new())).collect();
    let Some(mb) = mb else { return out };
    let morphs = columns(mb);
    let morph_starts: Vec<usize> = morphs.iter().map(|m| m.0).collect();
    let mut glosses: Vec<Vec<String>> = vec![Vec::new(); morphs.len()];
    if let Some(ge) = ge {
        for (col, g) in columns(ge) {
            if !morphs.is_empty() {
                glosses[under(&morph_starts, col)].push(g);
            }
        }
    }
    let mut parts: Vec<Vec<String>> = vec![Vec::new(); morphs.len()];
    if let Some(ps) = ps {
        for (col, x) in columns(ps) {
            if !morphs.is_empty() {
                parts[under(&morph_starts, col)].push(x);
            }
        }
    }
    for (((col, written), gloss), ps) in morphs.iter().zip(glosses).zip(parts) {
        if words.is_empty() {
            break;
        }
        let (surface, kind) = morph_of(written);
        // Toolbox writes *** for a morph it could not gloss.
        let gloss = gloss.into_iter().filter(|g| g != "***").collect::<Vec<_>>().join(" ");
        let gloss = gloss.trim_matches(|c| matches!(c, '-' | '=')).to_string();
        let ps = ps.into_iter().filter(|x| x != "***").collect::<Vec<_>>().join(" ");
        out[under(&word_starts, *col)].1.push(Morph { surface, kind, gloss, sense: None, ps });
    }
    out
}

fn parse(input: &str, fallback_title: &str, report: &mut TextReport) -> Vec<Text> {
    let mut texts: Vec<Text> = Vec::new();
    let mut lines: Vec<(&str, &str, &str)> = Vec::new(); // (marker, value, whole line)
    for line in input.lines() {
        if let Some((m, v)) = marker(line.trim_end_matches('\r')) {
            lines.push((m, v, line.trim_end_matches('\r')));
        }
    }
    let mut current: Option<Sentence> = None;
    let mut pending_tx: Option<&str> = None;
    let mut pending_mb: Option<&str> = None;
    let mut pending_ge: Option<&str> = None;
    let mut pending_ps: Option<&str> = None;
    let flush_bundle = |s: &mut Option<Sentence>, tx: &mut Option<&str>, mb: &mut Option<&str>, ge: &mut Option<&str>, ps: &mut Option<&str>| {
        if let (Some(sent), Some(t)) = (s.as_mut(), tx.take()) {
            sent.words.extend(bundle(t, mb.take(), ge.take(), ps.take()));
        }
        *mb = None;
        *ge = None;
        *ps = None;
    };
    let finish = |s: Option<Sentence>, texts: &mut Vec<Text>| {
        if let Some(s) = s {
            if texts.is_empty() {
                texts.push(Text { title: fallback_title.to_string(), sentences: Vec::new() });
            }
            texts.last_mut().unwrap().sentences.push(s);
        }
    };
    for (m, v, line) in lines {
        let value = v.trim();
        match m {
            "_sh" | "_DateStampHasFourDigitYear" => {}
            "id" => {
                flush_bundle(&mut current, &mut pending_tx, &mut pending_mb, &mut pending_ge, &mut pending_ps);
                finish(current.take(), &mut texts);
                texts.push(Text { title: if value.is_empty() { fallback_title.to_string() } else { value.to_string() }, sentences: Vec::new() });
            }
            "ref" => {
                flush_bundle(&mut current, &mut pending_tx, &mut pending_mb, &mut pending_ge, &mut pending_ps);
                finish(current.take(), &mut texts);
                current = Some(Sentence { reference: value.to_string(), words: Vec::new(), translation: String::new(), notes: Vec::new(), start: None, end: None });
            }
            _ if current.is_none() => report.not_carried.add(m),
            "tx" => {
                flush_bundle(&mut current, &mut pending_tx, &mut pending_mb, &mut pending_ge, &mut pending_ps);
                pending_tx = Some(line);
            }
            "mb" if pending_tx.is_some() && pending_mb.is_none() => pending_mb = Some(line),
            "ge" if pending_mb.is_some() && pending_ge.is_none() => pending_ge = Some(line),
            "ps" if pending_mb.is_some() && pending_ps.is_none() => pending_ps = Some(line),
            "ft" => {
                let s = current.as_mut().unwrap();
                if !value.is_empty() {
                    if !s.translation.is_empty() {
                        s.translation.push(' ');
                    }
                    s.translation.push_str(value);
                }
            }
            "nt" if !value.is_empty() => current.as_mut().unwrap().notes.push(value.to_string()),
            "ELANBegin" | "ELANEnd" if ms(value).is_some() => {
                let s = current.as_mut().unwrap();
                if m == "ELANBegin" {
                    s.start = ms(value);
                } else {
                    s.end = ms(value);
                }
            }
            other => report.not_carried.add(other),
        }
    }
    flush_bundle(&mut current, &mut pending_tx, &mut pending_mb, &mut pending_ge, &mut pending_ps);
    finish(current.take(), &mut texts);
    texts.retain(|t| !t.sentences.is_empty());
    texts
}

/// Live roots' senses by (form key, folded gloss), for linking.
fn sense_index(p: &Project) -> Result<HashMap<(String, String), Vec<String>>, String> {
    let mut index: HashMap<(String, String), Vec<String>> = HashMap::new();
    for lex in p.proj.entities_of_kind("lexeme")? {
        if p.proj.is_tombstoned(&lex)? {
            continue;
        }
        let lemma = p.proj.field_str(&lex, "lemma")?.unwrap_or_default();
        for sen in p.proj.ordered_children("lexeme", &lex, "sense")? {
            let gloss = p.proj.field_str(&sen, "gloss")?.unwrap_or_default();
            if !gloss.trim().is_empty() {
                index.entry((lexicon_key(&p.proj.nfc(&lemma)), wugs_model::fold_key(gloss.trim()))).or_default().push(sen);
            }
        }
    }
    Ok(index)
}

/// A live text with this title whose sentences on `tier` read the same.
fn already_here(p: &Project, title: &str, tier: &str, contents: &[String]) -> Result<Option<String>, String> {
    for t in p.proj.entities_of_kind("text")? {
        if p.proj.is_tombstoned(&t)? {
            continue;
        }
        if p.proj.field_str(&t, "title")?.map(|x| p.proj.nfc(&x)).as_deref() != Some(title) {
            continue;
        }
        let mut here = Vec::new();
        for seg in p.proj.ordered_children("text", &t, "segment")? {
            if p.proj.field_str(&seg, "tier")?.as_deref() == Some(tier) {
                here.push(p.proj.nfc(&p.proj.field_str(&seg, "content")?.unwrap_or_default()));
            }
        }
        if here == contents {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

/// Import a Toolbox interlinear text file: one text per `\id` (or one,
/// titled `fallback_title`, when there is none), sentences on `tier`,
/// free translations on `translation_tier` (a symbolic association of
/// `tier`; `\ft` lines are counted as not carried without one).
pub fn import_toolbox_text(p: &mut Project, input: &str, fallback_title: &str, tier: &str, translation_tier: Option<&str>) -> Result<TextReport, String> {
    let mut report = TextReport::default();
    let texts = parse(input, fallback_title, &mut report);
    if texts.is_empty() {
        return Err("the file has no \\ref records with text; nothing was imported".into());
    }
    write_texts(p, texts, tier, translation_tier, Writing { intent: "import-toolbox-text", reference: "x.toolbox.ref" }, &mut report)?;
    Ok(report)
}

/// How a set of parsed texts is written: the txn's intent and the field that
/// keeps each sentence's reference.
pub(crate) struct Writing {
    pub(crate) intent: &'static str,
    pub(crate) reference: &'static str,
}

/// Write parsed interlinear texts, one txn each; roots are linked to the one
/// lexicon sense with their form and gloss, and a text already here is matched.
pub(crate) fn write_texts(p: &mut Project, texts: Vec<Text>, tier: &str, translation_tier: Option<&str>, how: Writing, report: &mut TextReport) -> Result<(), String> {
    let senses = sense_index(p)?;
    for mut text in texts {
        for m in text.sentences.iter_mut().flat_map(|s| s.words.iter_mut()).flat_map(|w| w.1.iter_mut()) {
            if m.kind == "root" && !m.gloss.is_empty() {
                if let Some([only]) = senses.get(&(lexicon_key(&p.proj.nfc(&m.surface)), wugs_model::fold_key(&m.gloss))).map(Vec::as_slice) {
                    m.sense = Some(only.clone());
                }
            }
        }
        let title = p.proj.nfc(text.title.trim());
        let contents: Vec<String> = text.sentences.iter().map(|s| p.proj.nfc(&s.words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(" "))).collect();
        if let Some(t) = already_here(p, &title, tier, &contents)? {
            report.matched.push(t);
            continue;
        }
        let mut t = p.intent(how.intent);
        let tid = t.create_scope_root("text", "txt", vec![("title", Value::str(&title))]);
        for (si, (s, content)) in text.sentences.iter().zip(&contents).enumerate() {
            let seg = new_id("seg");
            let mut fields = vec![("text", Value::str(&tid)), ("tier", Value::str(tier)), ("pos", Value::str(&pos_seq(si))), ("content", Value::str(content))];
            if !s.reference.is_empty() {
                fields.push((how.reference, Value::str(&s.reference)));
            }
            if let (Some(a), Some(b)) = (s.start, s.end) {
                if b > a {
                    fields.push(("start", Value::Num(a.round())));
                    fields.push(("end", Value::Num(b.round())));
                    report.timed += 1;
                }
            }
            t.create_with_id("segment", &seg, &tid, fields);
            report.sentences += 1;
            for (wi, (form, morphs)) in s.words.iter().enumerate() {
                let tok = t.create("token", "tok", &tid, vec![("segment", Value::str(&seg)), ("pos", Value::str(&pos_seq(wi))), ("form", Value::str(form))]);
                report.words += 1;
                for (mi, m) in morphs.iter().enumerate() {
                    let mut fields = vec![("token", Value::str(&tok)), ("pos", Value::str(&pos_seq(mi))), ("surface", Value::str(&m.surface)), ("type", Value::str(m.kind))];
                    if !m.gloss.is_empty() {
                        fields.push(("gloss", Value::str(&m.gloss)));
                        if let Some(sense) = &m.sense {
                            fields.push(("sense", Value::str(sense)));
                            report.linked += 1;
                        }
                    }
                    if !m.ps.is_empty() {
                        fields.push(("x.toolbox.ps", Value::str(&m.ps)));
                    }
                    t.create("morph", "mor", &tid, fields);
                    report.morphs += 1;
                }
            }
            match (translation_tier, s.translation.is_empty()) {
                (Some(tt), false) => {
                    t.create("segment", "seg", &tid, vec![("text", Value::str(&tid)), ("tier", Value::str(tt)), ("pos", Value::str(&pos_seq(si))), ("parent", Value::str(&seg)), ("content", Value::str(&s.translation))]);
                    report.translations += 1;
                }
                (None, false) => report.not_carried.add("ft"),
                _ => {}
            }
            for note in &s.notes {
                t.create("annotation", "ann", &tid, vec![("target", Value::str(&seg)), ("kind", Value::str("note")), ("body", Value::str(note))]);
                report.notes += 1;
            }
        }
        t.commit()?;
        report.texts.push(tid);
    }
    Ok(())
}

// ------------------------------------------------------------------ export

fn shown(s: &str) -> usize {
    s.chars().map(width).sum()
}

fn marked(surface: &str, kind: &str) -> String {
    match kind {
        "prefix" => format!("{surface}-"),
        "suffix" => format!("-{surface}"),
        "clitic" => format!("={surface}"),
        "infix" => format!("<{surface}>"),
        _ => surface.to_string(),
    }
}

fn hms(ms: f64) -> String {
    let total = (ms.max(0.0)).round() as u64;
    format!("{:02}:{:02}:{:02}.{:03}", total / 3_600_000, total / 60_000 % 60, total / 1000 % 60, total % 1000)
}

/// Pad `cells` to their widths in one aligned line after `\marker`.
fn aligned(marker: &str, cells: &[(String, usize)]) -> String {
    let mut line = format!("\\{marker} ");
    for (i, (text, w)) in cells.iter().enumerate() {
        line.push_str(text);
        if i + 1 < cells.len() {
            line.push_str(&" ".repeat(w.saturating_sub(shown(text)).max(1)));
        }
    }
    line.trim_end().to_string()
}

/// One text as a Toolbox interlinear file: a `\ref` per sentence (its
/// imported reference, else `<title>.NNN`), times, the words with their
/// morphs and glosses aligned under them, the free translation and notes.
/// `sentences` are the segments in order, each with its translation.
pub fn export_toolbox_text(p: &Project, title: &str, sentences: &[(String, Option<String>)]) -> Result<String, String> {
    let mut out = String::from("\\_sh v3.0  400  Text\n\n");
    out.push_str(&format!("\\id {title}\n"));
    let stem: String = title.split_whitespace().collect::<Vec<_>>().join("_");
    for (n, (seg, translation)) in sentences.iter().enumerate() {
        out.push('\n');
        let reference = p.proj.field_str(seg, "x.toolbox.ref")?.filter(|r| !r.is_empty()).unwrap_or_else(|| format!("{stem}.{:03}", n + 1));
        out.push_str(&format!("\\ref {reference}\n"));
        if let (Some(a), Some(b)) = (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?) {
            out.push_str(&format!("\\ELANBegin {}\n\\ELANEnd {}\n", hms(a), hms(b)));
        }
        let tokens = p.proj.members(seg)?;
        let (mut tx, mut mb, mut ge, mut ps) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut analysed = false;
        let mut any_ps = false;
        if tokens.is_empty() {
            for w in p.proj.field_str(seg, "content")?.unwrap_or_default().split_whitespace() {
                tx.push((w.to_string(), shown(w) + 1));
            }
        }
        for tok in &tokens {
            let form = p.proj.field_str(tok, "form")?.unwrap_or_default();
            let mut cells = Vec::new();
            for m in p.proj.members(tok)? {
                let kind = p.proj.field_str(&m, "type")?.unwrap_or_default();
                let surface = marked(&p.proj.field_str(&m, "surface")?.unwrap_or_default(), &kind);
                let gloss = p.proj.field_str(&m, "gloss")?.unwrap_or_default();
                let gloss = if gloss.is_empty() { "***".to_string() } else { marked(&gloss, &kind) };
                let part = p.proj.field_str(&m, "x.toolbox.ps")?.unwrap_or_default();
                any_ps |= !part.is_empty();
                let w = shown(&surface).max(shown(&gloss)).max(shown(&part)) + 1;
                cells.push((surface, gloss, part, w));
                analysed = true;
            }
            let inner: usize = cells.iter().map(|c| c.3).sum();
            let word_w = (shown(&form) + 1).max(inner);
            tx.push((form, word_w));
            // The last morph takes up the slack, so the next word's morphs start under it.
            let n = cells.len();
            for (i, (surface, gloss, part, w)) in cells.into_iter().enumerate() {
                let w = if i + 1 == n { w + word_w - inner } else { w };
                mb.push((surface, w));
                ge.push((gloss, w));
                ps.push((if part.is_empty() { "***".to_string() } else { part }, w));
            }
            if n == 0 {
                mb.push((String::new(), word_w));
                ge.push((String::new(), word_w));
                ps.push((String::new(), word_w));
            }
        }
        out.push_str(&aligned("tx", &tx));
        out.push('\n');
        if analysed {
            out.push_str(&aligned("mb", &mb));
            out.push('\n');
            out.push_str(&aligned("ge", &ge));
            out.push('\n');
            if any_ps {
                out.push_str(&aligned("ps", &ps));
                out.push('\n');
            }
        }
        if let Some(t) = translation.as_deref().filter(|t| !t.trim().is_empty()) {
            out.push_str(&format!("\\ft {}\n", t.trim()));
        }
        for (_, note) in p.segment_notes(seg)? {
            out.push_str(&format!("\\nt {}\n", note.replace('\n', " ")));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_count_from_the_line_start_and_ignore_combining_marks() {
        assert_eq!(columns("\\tx  ab  cd"), vec![(5, "ab".to_string()), (9, "cd".to_string())]);
        // é written as e + U+0301 is one column wide.
        assert_eq!(columns("\\tx  e\u{301}b  cd")[1], (9, "cd".to_string()));
        assert_eq!(under(&[5, 9], 7), 0);
        assert_eq!(under(&[5, 9], 9), 1);
    }

    #[test]
    fn markers_set_the_morph_type() {
        assert_eq!(morph_of("ni-"), ("ni".to_string(), "prefix"));
        assert_eq!(morph_of("-ler"), ("ler".to_string(), "suffix"));
        assert_eq!(morph_of("=ma"), ("ma".to_string(), "clitic"));
        assert_eq!(morph_of("<um>"), ("um".to_string(), "infix"));
        assert_eq!(morph_of("kani"), ("kani".to_string(), "root"));
        assert_eq!(ms("00:00:01.250"), Some(1250.0));
        assert_eq!(ms("62.5"), Some(62500.0));
        assert_eq!(ms("x"), None);
    }
}
