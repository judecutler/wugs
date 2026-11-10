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
        let mut t = self.intent(MACHINE_INTENT);
        if has_history {
            t.text_edit("segment", segment, text, "content", &m.text)?;
        } else {
            t.set("segment", segment, text, "content", Value::str(&m.text))?;
        }
        let content_op = t.ops.last().expect("the content op was just written").id.clone();
        let phones = m.phones.iter().map(|(p, c)| Value::Arr(vec![Value::str(p), Value::Num(clamp(*c))])).collect();
        let asr = Value::obj(vec![
            ("model", Value::str(&m.model)),
            ("confidence", Value::Num(clamp(m.confidence))),
            ("op", Value::str(&content_op)),
            ("phones", Value::Arr(phones)),
        ]);
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
