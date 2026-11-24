//! EXMARaLDA basic transcriptions (`.exb`), the Partitur Editor's format and
//! the one the Hamburg corpora (HZSK) and much conversation analysis keep
//! their data in: a common timeline of points, and tiers of events between
//! two points, one or more per speaker.
//!
//! In: each transcription tier (`type="t"`) becomes a track named for its
//! speaker; events chained end to start make one utterance until one ends in
//! sentence-final punctuation (HIAT writes a word an event), and each
//! utterance is a timed segment. Annotation tiers (`type="a"`) of the same
//! speaker are kept on the utterance they fall in as `x.exb.<category>`;
//! description tiers (`type="d"`) are counted and left out. The head's
//! project and transcription names and its user-defined information become
//! the text's details.
//!
//! Out: every transcription track's timed segments as a `t` tier of a
//! speaker, the kept annotations as `a` tiers again, and the sentences'
//! translations as an `a` tier `tr` when none came in that way.

use std::collections::BTreeMap;

use wugs_xml::XmlNode;

use crate::interchange::plural;

/// One event: text between two timeline points.
#[derive(Debug, Clone)]
struct Event {
    start: usize,
    end: usize,
    text: String,
}

#[derive(Debug, Clone)]
struct Tier {
    speaker: String,
    category: String,
    kind: String,
    display: String,
    events: Vec<Event>,
}

/// A file read: its timeline (each point's time in ms, when given, in
/// timeline order), tiers, speakers by id and head details.
#[derive(Debug)]
pub struct Exb {
    times: Vec<Option<f64>>,
    tiers: Vec<Tier>,
    speakers: BTreeMap<String, String>,
    pub title: String,
    pub details: Vec<(String, String)>,
}

/// One utterance of a transcription tier, with what annotates it.
#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    pub track: String,
    pub text: String,
    pub span: Option<(f64, f64)>,
    pub annotations: Vec<(String, String)>,
}

/// Read a basic transcription.
pub fn read(input: &str) -> Result<Exb, String> {
    let root = wugs_xml::parse(input.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    if root.name != "basic-transcription" {
        return Err("this is not an EXMARaLDA basic transcription (no <basic-transcription>)".into());
    }
    let head = root.child("head");
    let meta = head.and_then(|h| h.child("meta-information"));
    let text_of = |n: Option<&XmlNode>, name: &str| n.and_then(|n| n.child(name)).map(|c| c.deep_text().trim().to_string()).unwrap_or_default();
    let mut details = Vec::new();
    let project = text_of(meta, "project-name");
    if !project.is_empty() {
        details.push(("project".to_string(), project));
    }
    if let Some(ud) = meta.and_then(|m| m.child("ud-meta-information")) {
        for info in ud.children_named("ud-information") {
            let (Some(name), value) = (info.attr("attribute-name"), info.deep_text()) else { continue };
            if !value.trim().is_empty() {
                details.push((name.to_string(), value.trim().to_string()));
            }
        }
    }
    let comment = text_of(meta, "comment");
    if !comment.is_empty() {
        details.push(("comment".to_string(), comment));
    }
    let mut speakers = BTreeMap::new();
    if let Some(table) = head.and_then(|h| h.child("speakertable")) {
        for s in table.children_named("speaker") {
            let id = s.attr("id").unwrap_or_default().to_string();
            let abbr = text_of(Some(s), "abbreviation");
            speakers.insert(id.clone(), if abbr.is_empty() { id } else { abbr });
        }
    }
    let body = root.child("basic-body").ok_or("the file has no <basic-body>")?;
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut times = Vec::new();
    for (i, tli) in body.child("common-timeline").into_iter().flat_map(|t| t.children_named("tli")).enumerate() {
        index.insert(tli.attr("id").unwrap_or_default().to_string(), i);
        times.push(tli.attr("time").and_then(|t| t.trim().parse::<f64>().ok()).filter(|t| t.is_finite() && *t >= 0.0).map(|t| (t * 1000.0).round()));
    }
    let mut tiers = Vec::new();
    for t in body.children_named("tier") {
        let mut events = Vec::new();
        for e in t.children_named("event") {
            let (Some(&start), Some(&end)) = (e.attr("start").and_then(|s| index.get(s)), e.attr("end").and_then(|s| index.get(s))) else {
                return Err(format!("an event of tier «{}» names a timeline point that is not there", t.attr("id").unwrap_or_default()));
            };
            events.push(Event { start, end, text: e.deep_text() });
        }
        events.sort_by_key(|e| (e.start, e.end));
        tiers.push(Tier {
            speaker: t.attr("speaker").unwrap_or_default().to_string(),
            category: t.attr("category").unwrap_or_default().to_string(),
            kind: t.attr("type").unwrap_or("t").to_string(),
            display: t.attr("display-name").unwrap_or_default().to_string(),
            events,
        });
    }
    Ok(Exb { times, tiers, speakers, title: text_of(meta, "transcription-name"), details })
}

/// Sentence-final punctuation, after which a chain of events ends an utterance.
fn ends_utterance(text: &str) -> bool {
    text.trim_end().ends_with(['.', '?', '!', '…', '。', '？', '！'])
}

impl Exb {
    /// A track name per transcription tier: its speaker's abbreviation, or
    /// with the category when the speaker has several such tiers.
    fn track_names(&self) -> Vec<Option<String>> {
        let transcription = |t: &Tier| t.kind == "t";
        self.tiers
            .iter()
            .map(|t| {
                if !transcription(t) {
                    return None;
                }
                let who = self.speakers.get(&t.speaker).cloned().filter(|s| !s.trim().is_empty()).or_else(|| Some(t.display.clone()).filter(|d| !d.trim().is_empty())).unwrap_or_else(|| "speaker".into());
                let many = self.tiers.iter().filter(|o| transcription(o) && o.speaker == t.speaker).count() > 1;
                Some(if many && !t.category.is_empty() { format!("{who} {}", t.category) } else { who })
            })
            .collect()
    }

    /// The utterances of every transcription tier, in timeline order, and
    /// how many annotation and description events stayed out.
    pub fn utterances(&self) -> (Vec<Utterance>, usize, usize) {
        let names = self.track_names();
        let mut out: Vec<(usize, usize, usize, Utterance)> = Vec::new();
        for (ti, t) in self.tiers.iter().enumerate() {
            let Some(track) = &names[ti] else { continue };
            let mut current: Option<(usize, usize, String)> = None;
            let flush = |cur: &mut Option<(usize, usize, String)>, out: &mut Vec<(usize, usize, usize, Utterance)>| {
                if let Some((s, e, text)) = cur.take() {
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !text.is_empty() {
                        let span = match (self.times[s], self.times[e]) {
                            (Some(a), Some(b)) if b > a => Some((a, b)),
                            _ => None,
                        };
                        out.push((ti, s, e, Utterance { track: track.clone(), text, span, annotations: Vec::new() }));
                    }
                }
            };
            for ev in &t.events {
                let joins = current.as_ref().is_some_and(|(_, end, text)| *end == ev.start && !ends_utterance(text));
                if !joins {
                    flush(&mut current, &mut out);
                }
                match &mut current {
                    Some((_, end, text)) => {
                        *end = ev.end;
                        text.push_str(&ev.text);
                    }
                    None => current = Some((ev.start, ev.end, ev.text.clone())),
                }
            }
            flush(&mut current, &mut out);
        }
        // Annotations go on the utterance of their speaker they start inside.
        let mut left = 0;
        let mut described = 0;
        for t in &self.tiers {
            match t.kind.as_str() {
                "a" => {
                    for ev in &t.events {
                        let home = out.iter_mut().find(|(ti, s, e, _)| self.tiers[*ti].speaker == t.speaker && *s <= ev.start && ev.start < *e);
                        let text = ev.text.split_whitespace().collect::<Vec<_>>().join(" ");
                        match home {
                            Some((_, _, _, u)) if !text.is_empty() => {
                                let category = if t.category.is_empty() { "annotation".to_string() } else { t.category.clone() };
                                match u.annotations.iter_mut().find(|(c, _)| *c == category) {
                                    Some((_, v)) => {
                                        v.push(' ');
                                        v.push_str(&text);
                                    }
                                    None => u.annotations.push((category, text)),
                                }
                            }
                            Some(_) => {}
                            None => left += 1,
                        }
                    }
                }
                "d" => described += t.events.len(),
                _ => {}
            }
        }
        out.sort_by_key(|(ti, s, _, _)| (*s, *ti));
        (out.into_iter().map(|(_, _, _, u)| u).collect(), left, described)
    }
}

/// Bring a basic transcription in as one text: a track per speaker's
/// transcription tier (found by name or made), a timed segment per utterance.
pub fn import(p: &mut wugs_core::Project, input: &str, stem: &str) -> Result<(Option<String>, Vec<String>), String> {
    let exb = read(input)?;
    let (utterances, left, described) = exb.utterances();
    if utterances.is_empty() {
        return Err("no transcription tier in it has text, so nothing was imported".into());
    }
    let title = p.proj.nfc(if exb.title.trim().is_empty() { stem } else { exb.title.trim() });
    let units: Vec<String> = utterances.iter().map(|u| p.proj.nfc(&u.text)).collect();
    let mut lines = Vec::new();
    if let Some(t) = crate::interchange::same_text(p, &title, &units)? {
        lines.push("It matched a text already here; nothing was added.".into());
        return Ok((Some(t), lines));
    }
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    let mut tracks: Vec<(String, String)> = Vec::new();
    let mut made = Vec::new();
    for u in &utterances {
        if tracks.iter().any(|(name, _)| *name == u.track) {
            continue;
        }
        let mut found = None;
        for t in &transcription {
            if !p.proj.is_tombstoned(t)? && p.proj.field_str(t, "name")?.unwrap_or_default().trim().eq_ignore_ascii_case(u.track.trim()) {
                found = Some(t.clone());
                break;
            }
        }
        let id = match found {
            Some(t) => t,
            None => {
                made.push(u.track.clone());
                p.add_tier(&u.track, "time-aligned", Some("transcription"), None)?.0
            }
        };
        tracks.push((u.track.clone(), id));
    }
    let extras: Vec<Vec<(String, String)>> = utterances.iter().map(|u| u.annotations.iter().map(|(c, v)| (format!("x.exb.{c}"), p.proj.nfc(v))).collect()).collect();
    let rows: Vec<wugs_core::WrittenRow> = utterances
        .iter()
        .enumerate()
        .map(|(i, u)| wugs_core::WrittenRow { text: &units[i], translation: "", extras: extras[i].clone(), span: u.span, tier: tracks.iter().find(|(n, _)| *n == u.track).map(|(_, t)| t.as_str()) })
        .collect();
    let first = tracks[0].1.clone();
    let (text, _) = p.import_written_rows(&title, &first, None, &rows)?;
    let meta: Vec<(String, String, String)> = exb.details.iter().map(|(k, v)| (text.clone(), k.clone(), p.proj.nfc(&v.chars().take(2000).collect::<String>()))).collect();
    p.import_text_metadata(&meta)?;
    let names: Vec<&str> = tracks.iter().map(|(n, _)| n.as_str()).collect();
    lines.push(format!("A new text with {}, a track per speaker ({}).", plural(units.len(), "utterance", "utterances"), names.join(", ")));
    if !made.is_empty() {
        lines.push(format!("New {}: {}.", if made.len() == 1 { "track" } else { "tracks" }, made.join(", ")));
    }
    let kept: Vec<String> = {
        let mut c: Vec<String> = utterances.iter().flat_map(|u| u.annotations.iter().map(|(c, _)| c.clone())).collect();
        c.sort();
        c.dedup();
        c
    };
    if !kept.is_empty() {
        lines.push(format!("Annotation tiers ({}) are kept with each utterance and go back out in an EXMARaLDA export.", kept.join(", ")));
    }
    let untimed = utterances.iter().filter(|u| u.span.is_none()).count();
    if untimed > 0 {
        lines.push(format!("{} had no times on the timeline, so {} untimed.", plural(untimed, "utterance", "utterances"), if untimed == 1 { "it came in" } else { "they came in" }));
    }
    if left > 0 {
        lines.push(format!("{} outside any utterance of {} speaker {} left out.", plural(left, "annotation", "annotations"), if left == 1 { "its" } else { "their" }, if left == 1 { "was" } else { "were" }));
    }
    if described > 0 {
        lines.push(format!("{} on description tiers {} left out.", plural(described, "event", "events"), if described == 1 { "was" } else { "were" }));
    }
    lines.push("Pick the recording in the Timeline (or the Media panel) to hear it.".into());
    Ok((Some(text), lines))
}

/// A basic transcription of one text: each transcription track with timed
/// segments as a speaker's `t` tier, its kept annotations and translations
/// as `a` tiers.
pub fn export(p: &wugs_core::Project, text_id: &str) -> Result<(String, Vec<String>), String> {
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    let mut notes = Vec::new();
    // Per track (in first-met order): its segments as (start, end, text, annotations).
    type Seg = (f64, f64, String, Vec<(String, String)>);
    let mut tracks: Vec<(String, Vec<Seg>)> = Vec::new();
    let mut untimed = 0;
    for seg in p.proj.ordered_children("text", text_id, "segment")? {
        let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
        if !transcription.contains(&tier) {
            continue;
        }
        let content = p.proj.field_str(&seg, "content")?.unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }
        let (Some(start), Some(end)) = (p.proj.field_num(&seg, "start")?, p.proj.field_num(&seg, "end")?) else {
            untimed += 1;
            continue;
        };
        if end <= start {
            untimed += 1;
            continue;
        }
        let mut annotations: Vec<(String, String)> = Vec::new();
        for (field, value) in p.proj.fields_of(&seg)? {
            if let (Some(cat), wugs_model::Value::Str(v)) = (field.strip_prefix("x.exb."), value) {
                annotations.push((cat.to_string(), v));
            }
        }
        if !annotations.iter().any(|(c, _)| c == "tr") {
            if let Some((_, t)) = crate::interchange::translations_of(p, &seg)?.into_iter().next() {
                annotations.push(("tr".into(), t));
            }
        }
        annotations.sort();
        let name = p.proj.field_str(&tier, "name")?.unwrap_or_default();
        match tracks.iter_mut().find(|(n, _)| *n == name) {
            Some((_, segs)) => segs.push((start, end, content, annotations)),
            None => tracks.push((name, vec![(start, end, content, annotations)])),
        }
    }
    if tracks.is_empty() {
        return Err("This text has no timed sentences, so there is nothing to write on a timeline.".into());
    }
    // A tier's events may not overlap: a segment that starts before the last one ends is left out.
    let mut overlapping = 0;
    for (_, segs) in &mut tracks {
        segs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut kept: Vec<Seg> = Vec::new();
        for s in segs.drain(..) {
            if kept.last().is_some_and(|k| s.0 < k.1) {
                overlapping += 1;
            } else {
                kept.push(s);
            }
        }
        *segs = kept;
    }
    let mut times: Vec<f64> = tracks.iter().flat_map(|(_, s)| s.iter().flat_map(|x| [x.0, x.1])).collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    let tli = |t: f64| format!("T{}", times.partition_point(|x| *x < t));
    let title = p.proj.field_str(text_id, "title")?.unwrap_or_default();
    let meta = p.text_metadata(text_id)?;
    let mut info = XmlNode::new("ud-meta-information");
    let mut project = String::new();
    for (k, v) in &meta {
        if k == "project" {
            project = v.clone();
        } else {
            info = info.with_child(XmlNode::new("ud-information").with_attr("attribute-name", k).with_text(v));
        }
    }
    let mut speakertable = XmlNode::new("speakertable");
    let mut body = XmlNode::new("basic-body");
    let mut timeline = XmlNode::new("common-timeline");
    for (i, t) in times.iter().enumerate() {
        timeline = timeline.with_child(XmlNode::new("tli").with_attr("id", &format!("T{i}")).with_attr("time", &format!("{:.3}", t / 1000.0)));
    }
    body = body.with_child(timeline);
    let mut tier_n = 0;
    for (si, (name, segs)) in tracks.iter().enumerate() {
        let spk = format!("SPK{si}");
        speakertable = speakertable.with_child(
            XmlNode::new("speaker")
                .with_attr("id", &spk)
                .with_child(XmlNode::new("abbreviation").with_text(name))
                .with_child(XmlNode::new("sex").with_attr("value", "u"))
                .with_child(XmlNode::new("languages-used"))
                .with_child(XmlNode::new("l1"))
                .with_child(XmlNode::new("l2"))
                .with_child(XmlNode::new("ud-speaker-information"))
                .with_child(XmlNode::new("comment")),
        );
        let mut tier = XmlNode::new("tier").with_attr("id", &format!("TIE{tier_n}")).with_attr("speaker", &spk).with_attr("category", "v").with_attr("type", "t").with_attr("display-name", &format!("{name} [v]"));
        tier_n += 1;
        for (s, e, text, _) in segs {
            tier = tier.with_child(XmlNode::new("event").with_attr("start", &tli(*s)).with_attr("end", &tli(*e)).with_text(&format!("{text} ")));
        }
        body = body.with_child(tier);
        let mut categories: Vec<&str> = segs.iter().flat_map(|s| s.3.iter().map(|(c, _)| c.as_str())).collect();
        categories.sort();
        categories.dedup();
        for cat in categories {
            let mut tier = XmlNode::new("tier").with_attr("id", &format!("TIE{tier_n}")).with_attr("speaker", &spk).with_attr("category", cat).with_attr("type", "a").with_attr("display-name", &format!("{name} [{cat}]"));
            tier_n += 1;
            for (s, e, _, ann) in segs {
                if let Some((_, v)) = ann.iter().find(|(c, _)| c == cat) {
                    tier = tier.with_child(XmlNode::new("event").with_attr("start", &tli(*s)).with_attr("end", &tli(*e)).with_text(v));
                }
            }
            body = body.with_child(tier);
        }
    }
    let head = XmlNode::new("head")
        .with_child(
            XmlNode::new("meta-information")
                .with_child(XmlNode::new("project-name").with_text(&project))
                .with_child(XmlNode::new("transcription-name").with_text(&title))
                .with_child(XmlNode::new("referenced-file").with_attr("url", ""))
                .with_child(info)
                .with_child(XmlNode::new("comment"))
                .with_child(XmlNode::new("transcription-convention")),
        )
        .with_child(speakertable);
    let doc = XmlNode::new("basic-transcription").with_child(head).with_child(body);
    if untimed > 0 {
        notes.push(format!("{} with no times {} left out: EXMARaLDA places every event on its timeline.", plural(untimed, "sentence", "sentences"), if untimed == 1 { "was" } else { "were" }));
    }
    if overlapping > 0 {
        notes.push(format!("{} overlapping another on the same track {} left out.", plural(overlapping, "sentence", "sentences"), if overlapping == 1 { "was" } else { "were" }));
    }
    Ok((doc.to_xml(), notes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{self, Session};

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<basic-transcription>
 <head>
  <meta-information>
   <project-name>Market talk</project-name>
   <transcription-name>At the stall</transcription-name>
   <referenced-file url="stall.wav"/>
   <ud-meta-information><ud-information attribute-name="place">Harbour market</ud-information></ud-meta-information>
   <comment></comment>
   <transcription-convention>HIAT</transcription-convention>
  </meta-information>
  <speakertable>
   <speaker id="SPK0"><abbreviation>AN</abbreviation><sex value="f"/></speaker>
   <speaker id="SPK1"><abbreviation>BO</abbreviation><sex value="m"/></speaker>
  </speakertable>
 </head>
 <basic-body>
  <common-timeline>
   <tli id="T0" time="0.0"/><tli id="T1" time="0.4"/><tli id="T2" time="0.9"/><tli id="T3" time="1.5"/><tli id="T4"/><tli id="T5" time="2.6"/><tli id="T6" time="3.0"/>
  </common-timeline>
  <tier id="TIE0" speaker="SPK0" category="v" type="t" display-name="AN [v]">
   <event start="T0" end="T1">how </event><event start="T1" end="T2">much? </event><event start="T2" end="T3">too </event><event start="T3" end="T4">much. </event>
  </tier>
  <tier id="TIE1" speaker="SPK0" category="en" type="a" display-name="AN [en]">
   <event start="T0" end="T2">How much?</event><event start="T2" end="T4">Too much.</event>
  </tier>
  <tier id="TIE2" speaker="SPK1" category="v" type="t" display-name="BO [v]">
   <event start="T5" end="T6">five &amp; six </event>
  </tier>
  <tier id="TIE3" speaker="SPK1" category="nn" type="d" display-name="BO [nn]">
   <event start="T5" end="T6">((laughs))</event>
  </tier>
 </basic-body>
</basic-transcription>"#;

    #[test]
    fn events_chain_into_utterances_per_speaker() {
        let exb = read(SAMPLE).unwrap();
        assert_eq!(exb.title, "At the stall");
        assert_eq!(exb.details, vec![("project".to_string(), "Market talk".to_string()), ("place".to_string(), "Harbour market".to_string())]);
        let (u, left, described) = exb.utterances();
        let got: Vec<(&str, &str, Option<(f64, f64)>)> = u.iter().map(|u| (u.track.as_str(), u.text.as_str(), u.span)).collect();
        assert_eq!(got, [("AN", "how much?", Some((0.0, 900.0))), ("AN", "too much.", None), ("BO", "five & six", Some((2600.0, 3000.0)))], "a ? ends an utterance; a point with no time leaves it untimed");
        assert_eq!(u[0].annotations, vec![("en".to_string(), "How much?".to_string())]);
        assert_eq!(u[1].annotations, vec![("en".to_string(), "Too much.".to_string())]);
        assert_eq!((left, described), (0, 1));
        assert!(read("<annotation-document/>").is_err());
    }

    #[test]
    fn a_transcription_comes_in_and_goes_back_out() {
        let dir = std::env::temp_dir().join(format!("wugs-exb-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.to_str().unwrap(), "tester").unwrap();
        let p = session::project(&mut s).unwrap();
        let (text, lines) = import(p, SAMPLE, "stall").unwrap();
        let text = text.unwrap();
        assert!(lines[0].contains("3 utterances") && lines[0].contains("AN, BO"), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("description tiers")), "{lines:?}");
        assert_eq!(p.proj.field_str(&text, "title").unwrap().as_deref(), Some("At the stall"));
        assert!(p.text_metadata(&text).unwrap().contains(&("place".to_string(), "Harbour market".to_string())));
        let (again, _) = import(p, SAMPLE, "stall").unwrap();
        assert_eq!(again.as_deref(), Some(text.as_str()), "the same file twice adds nothing");
        let (out, notes) = export(p, &text).unwrap();
        assert_eq!(notes.len(), 1, "the untimed utterance is left out: {notes:?}");
        let back = read(&out).unwrap();
        assert_eq!(back.title, "At the stall");
        assert!(back.details.contains(&("place".to_string(), "Harbour market".to_string())));
        let (u, _, _) = back.utterances();
        let got: Vec<(&str, &str)> = u.iter().map(|u| (u.track.as_str(), u.text.as_str())).collect();
        assert_eq!(got, [("AN", "how much?"), ("BO", "five & six")]);
        assert_eq!(u[0].annotations, vec![("en".to_string(), "How much?".to_string())], "the annotation tier goes back out");
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
