//! Praat TextGrids, long ("ooTextFile") and short form.
//!
//! Praat's own text reader keeps only numbers, quoted strings and `<flags>`
//! and skips everything else (`xmin =`, `item [1]:`, `! comments`), which is
//! why one token stream reads both forms. Import: one text per file, one tier
//! per TextGrid tier, one segment per labelled interval and per point, empty
//! marks included (times in ms, rounded; the original seconds text is kept
//! so an unedited boundary exports exactly as it came in). Export writes the long form with
//! contiguous intervals from tier xmin to xmax, as Praat requires, filling
//! gaps between segments with empty intervals.

use std::collections::HashMap;

use wugs_core::{Project, SCOPE_CONFIG};
use wugs_model::{pos_seq, Value};

use crate::common::*;

const F_TIER_TEXT: &str = "x.praat.text";
const F_CLASS: &str = "x.praat.class";
const F_ORDER: &str = "x.praat.order";
/// Seconds as written in the source, on texts, tiers and segments.
const F_XMIN: &str = "x.praat.xmin";
const F_XMAX: &str = "x.praat.xmax";

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Class {
    Interval,
    Text,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Interval => "IntervalTier",
            Class::Text => "TextTier",
        }
    }
}

/// A time in seconds with the text it is written as.
#[derive(Clone, Debug)]
pub(crate) struct Time {
    pub(crate) secs: f64,
    pub(crate) text: String,
}

impl Time {
    pub(crate) fn ms(&self) -> f64 {
        (self.secs * 1000.0).round()
    }

    fn from_ms(ms: f64, written: Option<String>) -> Time {
        if let Some(w) = written {
            if let Ok(secs) = w.parse::<f64>() {
                if (secs * 1000.0).round() == ms {
                    return Time { secs, text: w };
                }
            }
        }
        let secs = ms / 1000.0;
        Time { secs, text: format!("{secs}") }
    }
}

pub(crate) struct Item {
    pub(crate) start: Time,
    pub(crate) end: Time,
    pub(crate) label: String,
}

pub(crate) struct TierIn {
    pub(crate) class: Class,
    pub(crate) name: String,
    xmin: Time,
    xmax: Time,
    pub(crate) items: Vec<Item>,
}

pub(crate) struct Grid {
    xmin: Time,
    xmax: Time,
    pub(crate) tiers: Vec<TierIn>,
}

// ------------------------------------------------------------------ reader

#[derive(Debug)]
enum Tok {
    Num(Time),
    Str(String),
    Flag(String),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, ImportError> {
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => {
                let mut v = String::new();
                loop {
                    match it.next() {
                        Some((_, '"')) if it.peek().map(|p| p.1) == Some('"') => {
                            it.next();
                            v.push('"');
                        }
                        Some((_, '"')) => break,
                        Some((_, ch)) => v.push(ch),
                        None => return Err(format_err("unterminated string")),
                    }
                }
                out.push(Tok::Str(v));
            }
            '!' => {
                while it.next_if(|p| p.1 != '\n').is_some() {}
            }
            '<' => {
                let mut v = String::new();
                loop {
                    match it.next() {
                        Some((_, '>')) => break,
                        Some((_, ch)) => v.push(ch),
                        None => return Err(format_err("unterminated <flag>")),
                    }
                }
                out.push(Tok::Flag(v));
            }
            '[' => {
                while it.next_if(|p| p.1 != ']').is_some() {}
                it.next();
            }
            c if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' => {
                let mut end = i + c.len_utf8();
                while let Some(&(j, ch)) = it.peek() {
                    if !(ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '-' | '+')) {
                        break;
                    }
                    it.next();
                    end = j + ch.len_utf8();
                }
                let text = &s[i..end];
                let secs = text
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format_err(format!("bad number {text:?}")))?;
                out.push(Tok::Num(Time { secs, text: text.to_string() }));
            }
            c if c.is_alphabetic() || c == '_' => {
                while it.next_if(|p| p.1.is_alphanumeric() || p.1 == '_').is_some() {}
            }
            _ => {}
        }
    }
    Ok(out)
}

struct Cursor<'a>(std::slice::Iter<'a, Tok>);

impl Cursor<'_> {
    fn num(&mut self, what: &str) -> Result<Time, ImportError> {
        match self.0.next() {
            Some(Tok::Num(t)) => Ok(t.clone()),
            other => Err(format_err(format!("expected {what}, found {other:?}"))),
        }
    }
    fn count(&mut self, what: &str) -> Result<usize, ImportError> {
        let t = self.num(what)?;
        if t.secs < 0.0 || t.secs.fract() != 0.0 {
            return Err(format_err(format!("{what} {} is not a count", t.text)));
        }
        Ok(t.secs as usize)
    }
    fn str(&mut self, what: &str) -> Result<String, ImportError> {
        match self.0.next() {
            Some(Tok::Str(s)) => Ok(s.clone()),
            other => Err(format_err(format!("expected {what}, found {other:?}"))),
        }
    }
}

fn range(c: &mut Cursor, what: &str) -> Result<(Time, Time), ImportError> {
    let (a, b) = (c.num("xmin")?, c.num("xmax")?);
    if b.secs < a.secs {
        return Err(format_err(format!("{what}: xmax {} before xmin {}", b.text, a.text)));
    }
    Ok((a, b))
}

pub(crate) fn parse(input: &str) -> Result<Grid, ImportError> {
    let toks = tokenize(strip_bom(input))?;
    let mut c = Cursor(toks.iter());
    let ft = c.str("File type")?;
    if ft != "ooTextFile" && ft != "ooTextFile short" {
        return Err(format_err(format!("file type {ft:?} is not a Praat text file")));
    }
    let class = c.str("Object class")?;
    if class != "TextGrid" {
        return Err(format_err(format!("object class {class:?} is not TextGrid")));
    }
    let (xmin, xmax) = range(&mut c, "TextGrid")?;
    let mut tiers = Vec::new();
    match c.0.next() {
        Some(Tok::Flag(f)) if f == "exists" => {
            for _ in 0..c.count("tier count")? {
                let class = match c.str("tier class")?.as_str() {
                    "IntervalTier" => Class::Interval,
                    "TextTier" => Class::Text,
                    other => return Err(format_err(format!("unknown tier class {other:?}"))),
                };
                let name = c.str("tier name")?;
                let (txmin, txmax) = range(&mut c, &name)?;
                let mut items = Vec::new();
                for _ in 0..c.count("item count")? {
                    let item = match class {
                        Class::Interval => {
                            let (start, end) = range(&mut c, &name)?;
                            Item { start, end, label: c.str("interval text")? }
                        }
                        Class::Text => {
                            let t = c.num("point time")?;
                            Item { start: t.clone(), end: t, label: c.str("point mark")? }
                        }
                    };
                    items.push(item);
                }
                tiers.push(TierIn { class, name, xmin: txmin, xmax: txmax, items });
            }
        }
        Some(Tok::Flag(f)) if f == "absent" => {}
        other => return Err(format_err(format!("expected <exists> or <absent>, found {other:?}"))),
    }
    if let Some(t) = c.0.next() {
        return Err(format_err(format!("trailing data after the last tier: {t:?}")));
    }
    Ok(Grid { xmin, xmax, tiers })
}

/// Whether an item becomes a segment. Empty intervals do not: export's gap
/// filling regenerates them. Points have no gap filling, so every point is
/// kept, empty mark or not.
fn carried(class: Class, item: &Item) -> bool {
    class == Class::Text || !item.label.is_empty()
}

/// Content key: the same annotations give the same text, so the original
/// file and our own unedited export resolve to one text. None for a grid
/// with no label at all: two blank grids of the same shape are different
/// recordings, and keying them by shape would merge them into one text.
fn content_key(g: &Grid) -> Option<String> {
    if g.tiers.iter().flat_map(|t| &t.items).all(|i| i.label.is_empty()) {
        return None;
    }
    let mut s = format!("{}|{}", g.xmin.ms(), g.xmax.ms());
    for t in &g.tiers {
        s.push_str(&format!("\u{1e}{}\u{1f}{}", t.class.name(), t.name));
        for i in t.items.iter().filter(|i| carried(t.class, i)) {
            s.push_str(&format!("\u{1d}{}\u{1c}{}\u{1c}{}", i.start.ms(), i.end.ms(), i.label));
        }
    }
    Some(format!("praat-fnv:{:016x}", fnv1a(&s)))
}

// ------------------------------------------------------------------ import

#[derive(Debug, Default)]
pub struct TextGridReport {
    pub text: String,
    /// False when the file's annotations matched a text already in the
    /// project: its tiers and segments were added to that text, not a new
    /// one. Blank grids (no label anywhere) always get a new text.
    pub text_created: bool,
    pub tiers_created: usize,
    pub tiers_skipped: usize,
    pub segments_created: usize,
    pub segments_skipped: usize,
    /// Intervals with empty text: not stored, regenerated by export's gap
    /// filling.
    pub empty_intervals_skipped: usize,
    /// Intervals that do not start exactly where the one before them on
    /// their tier ended (an overlap or a gap), which Praat never writes.
    /// They are imported as written; export re-tiles the tier, filling gaps
    /// and snapping overlaps shorter than a millisecond.
    pub non_tiling_intervals: usize,
    pub commits: usize,
    pub findings: usize,
}

/// Import one TextGrid as a text. Txns: the text (its own scope), the
/// missing tiers (project scope), then one per tier's segments (text scope).
/// Re-importing the same annotations resolves to the same text, and tiers
/// and segments already there are skipped by origin, so a re-run after a
/// crash fills in only what is missing.
pub fn import_textgrid(project: &mut Project, input: &str, title_fallback: &str) -> Result<TextGridReport, ImportError> {
    let grid = parse(input)?;
    let key = content_key(&grid);
    let mut report = TextGridReport::default();

    let existing = match &key {
        Some(k) => origin_index(project, "text")?.remove(k),
        None => None,
    };
    let text = match existing {
        Some(id) => id,
        None => {
            let mut t = project.intent("import-praat");
            let mut fields = vec![
                ("title", Value::str(title_fallback)),
                ("kind", Value::str("unspecified")),
                (F_XMIN, Value::str(&grid.xmin.text)),
                (F_XMAX, Value::str(&grid.xmax.text)),
            ];
            if let Some(k) = &key {
                fields.push((F_ORIGIN, Value::str(k)));
            }
            let id = t.create_scope_root("text", "txt", fields);
            t.commit()?;
            report.text_created = true;
            report.commits += 1;
            id
        }
    };
    report.text = text.clone();

    let existing_tiers = origins_by_ref(project, F_TIER_TEXT, &text, "tier")?;
    let mut occ = Occurrences::default();
    let origins: Vec<String> = grid.tiers.iter().map(|t| occ.key(format!("praat:{}", t.name))).collect();
    let mut tier_ids = Vec::new();
    let mut t = project.intent("import-praat");
    for (order, (tier, origin)) in grid.tiers.iter().zip(&origins).enumerate() {
        tier_ids.push(match existing_tiers.get(origin) {
            Some(id) => {
                report.tiers_skipped += 1;
                id.clone()
            }
            None => {
                report.tiers_created += 1;
                t.create(
                    "tier",
                    "tir",
                    SCOPE_CONFIG,
                    vec![
                        ("name", Value::str(&tier.name)),
                        ("kind", Value::str("time-aligned")),
                        ("participant", Value::Bool(false)),
                        (F_ORIGIN, Value::str(origin)),
                        (F_TIER_TEXT, Value::str(&text)),
                        (F_CLASS, Value::str(tier.class.name())),
                        (F_ORDER, Value::Num(order as f64)),
                        (F_XMIN, Value::str(&tier.xmin.text)),
                        (F_XMAX, Value::str(&tier.xmax.text)),
                    ],
                )
            }
        });
    }
    if report.tiers_created > 0 {
        let res = t.commit()?;
        report.commits += 1;
        report.findings += res.findings.len();
    } else {
        drop(t);
    }

    let existing_segs = origins_by_ref(project, "text", &text, "segment")?;
    for ((tier, origin), tier_id) in grid.tiers.iter().zip(&origins).zip(&tier_ids) {
        let mut t = project.intent("import-praat");
        let mut pending = 0;
        let mut pos = 0;
        if tier.class == Class::Interval {
            report.non_tiling_intervals += tier.items.windows(2).filter(|w| w[1].start.secs != w[0].end.secs).count();
        }
        for (n, item) in tier.items.iter().enumerate() {
            if !carried(tier.class, item) {
                report.empty_intervals_skipped += 1;
                continue;
            }
            let seg_origin = format!("{origin}/{}", n + 1);
            let p = pos_seq(pos);
            pos += 1;
            if existing_segs.contains_key(&seg_origin) {
                report.segments_skipped += 1;
                continue;
            }
            t.create(
                "segment",
                "seg",
                &text,
                vec![
                    ("text", Value::str(&text)),
                    ("tier", Value::str(tier_id)),
                    ("pos", Value::str(&p)),
                    ("start", Value::Num(item.start.ms())),
                    ("end", Value::Num(item.end.ms())),
                    ("content", Value::str(&item.label)),
                    (F_ORIGIN, Value::str(&seg_origin)),
                    (F_XMIN, Value::str(&item.start.text)),
                    (F_XMAX, Value::str(&item.end.text)),
                ],
            );
            report.segments_created += 1;
            pending += 1;
        }
        if pending > 0 {
            let res = t.commit()?;
            report.commits += 1;
            report.findings += res.findings.len();
        }
    }
    Ok(report)
}

fn origins_by_ref(project: &Project, field: &str, parent: &str, kind: &str) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for id in project.proj.children_by_ref(field, parent, kind)? {
        if let Some(o) = opt_str(project, &id, F_ORIGIN)? {
            out.insert(o, id);
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ export

pub struct TextGridExport {
    pub textgrid: String,
    /// Segments Praat cannot hold, by reason. Never dropped silently.
    pub not_exported: Counts,
}

struct SegOut {
    id: String,
    start: Time,
    end: Time,
    label: String,
}

/// Render a text as a long-form TextGrid. Reads the projection only.
pub fn export_textgrid(project: &Project, text: &str) -> Result<TextGridExport, String> {
    export_mapped(project, text).map(|(e, _)| e)
}

/// Which segment each written interval or point is, tier by tier in file
/// order. `None` is a gap interval export filled in.
#[derive(Clone, Debug, PartialEq)]
pub struct TierMap {
    pub tier: String,
    pub name: String,
    pub items: Vec<Option<String>>,
}

/// [`export_textgrid`] plus the segment behind every item it wrote, so an
/// edited copy of the file can be traced back to segments.
pub fn export_mapped(project: &Project, text: &str) -> Result<(TextGridExport, Vec<TierMap>), String> {
    let p = &project.proj;
    if p.entity_kind(text)?.as_deref() != Some("text") {
        return Err(format!("{text} is not a text"));
    }
    let mut not_exported = Counts::default();

    let mut by_tier: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for seg in p.children_by_ref("text", text, "segment")? {
        let tier = opt_str(project, &seg, "tier")?.unwrap_or_default();
        let pos = opt_str(project, &seg, "pos")?.unwrap_or_default();
        by_tier.entry(tier).or_default().push((pos, seg));
    }
    let mut tier_ids: Vec<String> = p.children_by_ref(F_TIER_TEXT, text, "tier")?;
    let mut referenced: Vec<&String> = by_tier.keys().filter(|t| !tier_ids.contains(t)).collect();
    referenced.sort();
    tier_ids.extend(referenced.into_iter().cloned());

    struct TierOut {
        id: String,
        order: f64,
        name: String,
        class: Class,
        stored: (Option<String>, Option<String>),
        segs: Vec<SegOut>,
    }
    let mut tiers = Vec::new();
    for tier in tier_ids {
        let mut segs = Vec::new();
        let mut members = by_tier.remove(&tier).unwrap_or_default();
        members.sort();
        for (_, seg) in members {
            let (Some(s), Some(e)) = (p.field_num(&seg, "start")?, p.field_num(&seg, "end")?) else {
                not_exported.add("segment without start and end");
                continue;
            };
            segs.push(SegOut {
                id: seg.clone(),
                start: Time::from_ms(s, opt_str(project, &seg, F_XMIN)?),
                end: Time::from_ms(e, opt_str(project, &seg, F_XMAX)?),
                label: opt_str(project, &seg, "content")?.unwrap_or_default(),
            });
        }
        let from_praat = opt_str(project, &tier, F_TIER_TEXT)?.as_deref() == Some(text);
        if segs.is_empty() && !from_praat {
            continue;
        }
        segs.sort_by(|a, b| a.start.secs.total_cmp(&b.start.secs));
        let class = match opt_str(project, &tier, F_CLASS)?.as_deref() {
            Some("TextTier") => Class::Text,
            Some(_) => Class::Interval,
            None if !segs.is_empty() && segs.iter().all(|s| s.start.secs == s.end.secs) => Class::Text,
            None => Class::Interval,
        };
        tiers.push(TierOut {
            id: tier.clone(),
            order: p.field_num(&tier, F_ORDER)?.unwrap_or(f64::MAX),
            name: opt_str(project, &tier, "name")?.unwrap_or_else(|| tier.clone()),
            class,
            stored: (opt_str(project, &tier, F_XMIN)?, opt_str(project, &tier, F_XMAX)?),
            segs,
        });
    }
    tiers.sort_by(|a, b| a.order.total_cmp(&b.order).then_with(|| a.name.cmp(&b.name)));

    let first = tiers.iter().filter_map(|t| t.segs.first()).map(|s| s.start.secs).fold(f64::INFINITY, f64::min);
    let last = tiers.iter().flat_map(|t| &t.segs).map(|s| s.end.secs).fold(f64::NEG_INFINITY, f64::max);
    let (gmin, gmax) = covering(
        (opt_str(project, text, F_XMIN)?, opt_str(project, text, F_XMAX)?),
        first,
        last,
        (Time { secs: 0.0, text: "0".into() }, None),
    );

    let mut out = String::new();
    out.push_str("File type = \"ooTextFile\"\nObject class = \"TextGrid\"\n\n");
    out.push_str(&format!("xmin = {} \nxmax = {} \n", gmin.text, gmax.text));
    if tiers.is_empty() {
        out.push_str("tiers? <absent> \n");
        return Ok((TextGridExport { textgrid: out, not_exported }, Vec::new()));
    }
    let mut maps = Vec::new();
    out.push_str(&format!("tiers? <exists> \nsize = {} \nitem []: \n", tiers.len()));
    for (n, t) in tiers.iter().enumerate() {
        let tfirst = t.segs.first().map_or(f64::INFINITY, |s| s.start.secs);
        let tlast = t.segs.iter().map(|s| s.end.secs).fold(f64::NEG_INFINITY, f64::max);
        let (xmin, xmax) = covering(t.stored.clone(), tfirst, tlast, (gmin.clone(), Some(gmax.clone())));
        out.push_str(&format!(
            "    item [{}]:\n        class = \"{}\" \n        name = {} \n        xmin = {} \n        xmax = {} \n",
            n + 1,
            t.class.name(),
            quote(&t.name),
            xmin.text,
            xmax.text
        ));
        match t.class {
            Class::Interval => {
                let ivs = intervals(&t.segs, &xmin, &xmax, &mut not_exported);
                maps.push(TierMap { tier: t.id.clone(), name: t.name.clone(), items: ivs.iter().map(|iv| iv.3.clone()).collect() });
                out.push_str(&format!("        intervals: size = {} \n", ivs.len()));
                for (i, (a, b, label, _)) in ivs.iter().enumerate() {
                    out.push_str(&format!(
                        "        intervals [{}]:\n            xmin = {} \n            xmax = {} \n            text = {} \n",
                        i + 1,
                        a.text,
                        b.text,
                        quote(label)
                    ));
                }
            }
            Class::Text => {
                let mut pts = Vec::new();
                for s in &t.segs {
                    if s.start.secs != s.end.secs {
                        not_exported.add("segment with a duration on a point tier");
                    } else {
                        pts.push(s);
                    }
                }
                maps.push(TierMap { tier: t.id.clone(), name: t.name.clone(), items: pts.iter().map(|s| Some(s.id.clone())).collect() });
                out.push_str(&format!("        points: size = {} \n", pts.len()));
                for (i, s) in pts.iter().enumerate() {
                    out.push_str(&format!(
                        "        points [{}]:\n            number = {} \n            mark = {} \n",
                        i + 1,
                        s.start.text,
                        quote(&s.label)
                    ));
                }
            }
        }
    }
    Ok((TextGridExport { textgrid: out, not_exported }, maps))
}

/// The stored range when it still covers the content, else the fallback
/// widened to the content.
fn covering(stored: (Option<String>, Option<String>), first: f64, last: f64, fallback: (Time, Option<Time>)) -> (Time, Time) {
    let parse = |s: Option<String>| s.and_then(|t| Some(Time { secs: t.parse().ok()?, text: t }));
    let xmin = match parse(stored.0) {
        Some(t) if t.secs <= first => t,
        _ if fallback.0.secs <= first => fallback.0,
        _ => Time::from_ms((first * 1000.0).round(), None),
    };
    let xmax = match (parse(stored.1), fallback.1) {
        (Some(t), _) if t.secs >= last && t.secs >= xmin.secs => t,
        (_, Some(f)) if f.secs >= last && f.secs >= xmin.secs => f,
        _ if last.is_finite() => Time::from_ms((last * 1000.0).round(), None),
        _ => xmin.clone(),
    };
    (xmin, xmax)
}

/// Contiguous intervals from xmin to xmax: segments in time order, empty
/// intervals in the gaps. Overlap is judged in ms, the unit wugs stores: a
/// segment starting in the same ms as the previous end (a sub-ms overlap in
/// the source, or a boundary snapped to the ms in wugs) starts at that end's
/// exact time instead. Real overlaps and zero-length segments cannot be
/// represented and are counted instead.
fn intervals(segs: &[SegOut], xmin: &Time, xmax: &Time, skipped: &mut Counts) -> Vec<(Time, Time, String, Option<String>)> {
    let mut out = Vec::new();
    let mut cursor = xmin.clone();
    for s in segs {
        if s.end.secs <= s.start.secs {
            skipped.add("zero-length segment on an interval tier");
            continue;
        }
        let start = if s.start.secs >= cursor.secs {
            s.start.clone()
        } else if s.start.ms() == cursor.ms() && s.end.secs > cursor.secs {
            cursor.clone()
        } else {
            skipped.add("segment overlapping the one before on its tier");
            continue;
        };
        if start.secs > cursor.secs {
            out.push((cursor.clone(), start.clone(), String::new(), None));
        }
        out.push((start, s.end.clone(), s.label.clone(), Some(s.id.clone())));
        cursor = s.end.clone();
    }
    if cursor.secs < xmax.secs || out.is_empty() {
        out.push((cursor, xmax.clone(), String::new(), None));
    }
    out
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
