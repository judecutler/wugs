//! Machine transcription (roadmap item 27). A phone model's suggestion is
//! written like any other edit, under its own intent, and its record goes
//! in `x.asr:<content op id>`, a field named after the content op the
//! machine wrote. The segment is *unverified* exactly while the content's
//! winner is an op with such a record, so a person's edit, made on any
//! device and by any path (the Transcribe panel, the timeline, the conflict
//! queue), verifies it without a second write. One field per op means two
//! suggestions never conflict over their records and a record can never be
//! paired with the other suggestion's text. The log keeps both: what the
//! machine heard and what the person wrote.

use wugs_model::Value;

use crate::{CommitResult, Project};

/// The intent every machine transcription is written under.
pub const MACHINE_INTENT: &str = "machine-transcribe";

/// The field holding the record of the machine write `op`.
pub fn record_field(op: &str) -> String {
    format!("x.asr:{op}")
}

/// What a phone model heard in one segment.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineTranscript {
    /// The transcription to write: IPA, spaces at pauses.
    pub text: String,
    /// The model that heard it, as the catalog names it.
    pub model: String,
    /// Mean phone confidence, 0 to 1.
    pub confidence: f64,
    /// Each phone with its own confidence, in order.
    pub phones: Vec<(String, f64)>,
    /// The source whose recording the model heard, kept in the record so a
    /// corrections export knows which audio the segment's times point into.
    pub source: Option<String>,
}

/// A segment whose transcription is still the machine's.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineMark {
    pub model: String,
    pub confidence: f64,
}

impl Project {
    /// Write a machine transcription into an empty segment. A segment that
    /// already holds text is refused: a suggestion never overwrites a
    /// person's transcription. A plain write, so it closes no conflict.
    pub fn machine_transcribe(&mut self, text: &str, segment: &str, m: &MachineTranscript) -> Result<CommitResult, String> {
        self.require_in_text(segment, "segment", text)?;
        if !self.proj.field_str(segment, "content")?.unwrap_or_default().trim().is_empty() {
            return Err("This segment already has a transcription; suggestions only fill empty segments.".into());
        }
        // Empty only as one side of an open conflict: the other side is someone's text.
        if self.proj.has_conflict(segment, "content")? {
            return Err("This segment's transcription has a conflict to settle first (Conflicts panel).".into());
        }
        if m.text.trim().is_empty() {
            return Err("The model heard no speech in this segment.".into());
        }
        if !m.confidence.is_finite() || m.model.trim().is_empty() {
            return Err("A machine transcription needs a model and a confidence.".into());
        }
        let clamp = |c: f64| if c.is_finite() { c.clamp(0.0, 1.0) } else { 0.0 };
        // A field that was typed in and cleared already has collaborative
        // text history; a plain set would sit outside it, and the next merged
        // edit would add the whole text again. Write into the history there.
        let has_history = !self.proj.text_updates(segment, "content")?.is_empty();
        // The span the model heard, so an export can tell when it moved since.
        let span = [("start", self.proj.field_num(segment, "start")?), ("end", self.proj.field_num(segment, "end")?)];
        let mut t = self.intent(MACHINE_INTENT);
        if has_history {
            t.text_edit("segment", segment, text, "content", &m.text)?;
        } else {
            t.set("segment", segment, text, "content", Value::str(&m.text))?;
        }
        let content_op = t.ops.last().expect("the content op was just written").id.clone();
        let phones = m.phones.iter().map(|(p, c)| Value::Arr(vec![Value::str(p), Value::Num(clamp(*c))])).collect();
        let mut record = vec![
            ("model", Value::str(&m.model)),
            ("confidence", Value::Num(clamp(m.confidence))),
            ("op", Value::str(&content_op)),
            ("phones", Value::Arr(phones)),
            ("text", Value::str(&m.text)),
        ];
        if let Some(source) = &m.source {
            record.push(("source", Value::str(source)));
        }
        for (k, ms) in span {
            if let Some(ms) = ms {
                record.push((k, Value::Num(ms)));
            }
        }
        let asr = Value::obj(record);
        t.set("segment", segment, text, &record_field(&content_op), asr)?;
        t.commit()
    }

    /// The segment's machine mark: set while any open version of its
    /// transcription is a machine write nobody has answered. A person's
    /// concurrent edit can merge with a machine one, and then the newest op
    /// says nothing about whose text shows; checking every open version
    /// means unseen machine text never passes as checked (a person's own
    /// concurrent typing may stay marked until their next edit, the safe
    /// side). Several machine versions report the least sure one.
    pub fn machine_mark(&self, segment: &str) -> Result<Option<MachineMark>, String> {
        let mut mark: Option<MachineMark> = None;
        for op in self.proj.open_version_ops(segment, "content")? {
            let Some((asr, _)) = self.proj.field(segment, &record_field(&op))? else { continue };
            let m = MachineMark {
                model: asr.get("model").and_then(Value::as_str).unwrap_or_default().to_string(),
                confidence: asr.get("confidence").and_then(Value::as_f64).unwrap_or(0.0),
            };
            if mark.as_ref().is_none_or(|have| m.confidence < have.confidence) {
                mark = Some(m);
            }
        }
        Ok(mark)
    }
}

/// One machine suggestion a person has since checked: what the model heard
/// and what the transcription says now. Pairs like this are the
/// human-in-the-loop data (`docs/design/PACKAGES.md` §3): a benchmark set
/// for the languages people document, or fine-tuning data.
#[derive(Clone, Debug, PartialEq)]
pub struct Correction {
    pub segment: String,
    pub text: String,
    /// The source whose recording the segment's times point into.
    pub source: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub model: String,
    pub confidence: f64,
    /// What the model heard.
    pub machine: String,
    /// What the person settled on (possibly the machine's text, kept).
    pub person: String,
    /// The segment's times moved after the suggestion, so `machine` was
    /// heard on a different span than the one `person` describes.
    pub retimed: bool,
    /// What the project's consent records say about AI use of this clip.
    /// Information for the person exporting, never a gate: wugs records
    /// what the linguist entered and leaves the ethics to them and their
    /// review board.
    pub consent: Consent,
    /// The text (its own level, else its linked recording's) or the
    /// recording heard has a restricted or community access level.
    pub restricted: bool,
}

/// What consent records covering a clip's text, recording or project say
/// about AI use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consent {
    /// Some record says `"AI-ok": true` and none says otherwise.
    Yes,
    /// No record speaks about AI use.
    Unrecorded,
    /// A record says anything but `"AI-ok": true`.
    No,
}

impl Consent {
    pub fn as_str(self) -> &'static str {
        match self {
            Consent::Yes => "recorded-yes",
            Consent::Unrecorded => "not-recorded",
            Consent::No => "recorded-no",
        }
    }
}

/// Every checked suggestion in the project, each with what its records say,
/// and how many suggestions could not be paired.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Corrections {
    pub pairs: Vec<Correction>,
    /// Still unchecked: the machine's text is what the segment shows.
    pub unchecked: usize,
    /// No recording or times to cut the audio from, or the segment was emptied.
    pub unusable: usize,
}

impl Project {
    /// The checked machine suggestions in this project, for a corrections
    /// export. A segment counts once, with its newest suggestion, once a
    /// person's edit has answered it (`machine_mark` is clear) and it still
    /// holds text. Each pair carries what consent records and access levels
    /// say about it; which ones leave is the person's choice. Derived from
    /// the projection only; nothing is written.
    pub fn corrections(&self) -> Result<Corrections, String> {
        let mut out = Corrections::default();
        let consent = self.ai_consent()?;
        // Each suggested segment's records, from one scan of the field index.
        let mut suggested: Vec<(String, Vec<String>)> = Vec::new();
        for (seg, field) in self.proj.fields_with_prefix("x.asr:")? {
            match suggested.last_mut() {
                Some((s, fields)) if *s == seg => fields.push(field),
                _ => suggested.push((seg, vec![field])),
            }
        }
        for (seg, fields) in suggested {
            if self.proj.entity_kind(&seg)?.as_deref() != Some("segment") || self.proj.is_tombstoned(&seg)? {
                continue;
            }
            // The newest suggestion this segment ever got.
            let mut newest: Option<(String, Value)> = None;
            for field in fields {
                if let Some((record, hlc)) = self.proj.field(&seg, &field)? {
                    if newest.as_ref().is_none_or(|(h, _)| hlc > *h) {
                        newest = Some((hlc, record));
                    }
                }
            }
            let Some((_, record)) = newest else { continue };
            let Some(text) = self.proj.field_str(&seg, "text")? else { continue };
            if self.proj.is_tombstoned(&text)? {
                continue;
            }
            if let Some(tier) = self.proj.field_str(&seg, "tier")? {
                if self.proj.is_tombstoned(&tier)? {
                    continue;
                }
            }
            if self.machine_mark(&seg)?.is_some() {
                out.unchecked += 1;
                continue;
            }
            let person = self.proj.field_str(&seg, "content")?.unwrap_or_default();
            let source = record.get("source").and_then(Value::as_str).map(str::to_string).or(self.proj.field_str(&text, "source")?);
            let times = (self.proj.field_num(&seg, "start")?, self.proj.field_num(&seg, "end")?);
            let (Some(source), (Some(start_ms), Some(end_ms))) = (source, times) else {
                out.unusable += 1;
                continue;
            };
            if self.proj.entity_kind(&source)?.as_deref() != Some("source") || self.proj.is_tombstoned(&source)? {
                out.unusable += 1;
                continue;
            }
            if person.trim().is_empty() || end_ms <= start_ms {
                out.unusable += 1;
                continue;
            }
            // The text's own linked recording too, if it differs from the one heard.
            let linked = self.proj.field_str(&text, "source")?;
            let mut covers = vec![text.as_str(), source.as_str(), self.project_id.as_str()];
            covers.extend(linked.as_deref());
            let said = if covers.iter().any(|c| consent.get(*c) == Some(&false)) {
                Consent::No
            } else if covers.iter().any(|c| consent.get(*c) == Some(&true)) {
                Consent::Yes
            } else {
                Consent::Unrecorded
            };
            // What the transcription belongs to, and the audio it describes.
            let restricted = self.text_access(&text)? != "open" || self.proj.field_str(&source, "access")?.is_some_and(|a| a != "open");
            // Records from before the text was kept: the phones without pauses.
            let machine = match record.get("text").and_then(Value::as_str) {
                Some(t) => t.to_string(),
                None => match record.get("phones") {
                    Some(Value::Arr(phones)) => phones.iter().filter_map(|p| match p {
                        Value::Arr(pair) => pair.first().and_then(Value::as_str),
                        _ => None,
                    }).collect(),
                    _ => String::new(),
                },
            };
            let heard_on = (record.get("start").and_then(Value::as_f64), record.get("end").and_then(Value::as_f64));
            out.pairs.push(Correction {
                retimed: matches!(heard_on, (Some(s), Some(e)) if s != start_ms || e != end_ms),
                consent: said,
                restricted,
                segment: seg,
                text,
                source,
                start_ms,
                end_ms,
                model: record.get("model").and_then(Value::as_str).unwrap_or_default().to_string(),
                confidence: record.get("confidence").and_then(Value::as_f64).unwrap_or(0.0),
                machine,
                person,
            });
        }
        out.pairs.sort_by(|a, b| a.text.cmp(&b.text).then(a.start_ms.total_cmp(&b.start_ms)).then(a.segment.cmp(&b.segment)));
        Ok(out)
    }

    /// What live consent records say about AI use, per thing they cover:
    /// false when any refuses, true when some allow and none refuses. Any
    /// `AI-ok` other than `true` (`false`, `"no"`, anything) is a refusal.
    fn ai_consent(&self) -> Result<std::collections::HashMap<String, bool>, String> {
        let mut said = std::collections::HashMap::new();
        for c in self.proj.entities_of_kind("consent")? {
            if self.proj.is_tombstoned(&c)? {
                continue;
            }
            let Some(covers) = self.proj.field_str(&c, "covers")? else { continue };
            let Some((terms, _)) = self.proj.field(&c, "terms")? else { continue };
            let Some(term) = terms.get("AI-ok") else { continue };
            let ok = matches!(term, Value::Bool(true));
            let entry = said.entry(covers).or_insert(ok);
            *entry = *entry && ok;
        }
        Ok(said)
    }

    /// A text's effective access: its own level, else its linked source's,
    /// else open (DATA-MODEL §4, the rule sync's `policy_from_access` uses).
    fn text_access(&self, text: &str) -> Result<String, String> {
        if let Some(a) = self.proj.field_str(text, "access")? {
            return Ok(a);
        }
        if let Some(src) = self.proj.field_str(text, "source")? {
            if let Some(a) = self.proj.field_str(&src, "access")? {
                return Ok(a);
            }
        }
        Ok("open".into())
    }
}
