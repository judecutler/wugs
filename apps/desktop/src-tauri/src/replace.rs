//! Find and replace in transcriptions or translations across texts: a
//! preview of every sentence that would change, then the chosen ones
//! written, each as a person's edit of that sentence (a resolving
//! collaborative text edit, like typing it). A sentence that changed since
//! the preview is skipped, never overwritten, and the same call run with
//! before and after swapped is the undo.

use regex::{NoExpand, Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::session::{self, Session};

/// At most this many sentences in one preview.
const MAX_CHANGES: usize = 2000;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Track {
    Transcription,
    Translation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Find {
    pub find: String,
    pub with: String,
    pub track: Track,
    pub whole_word: bool,
    pub match_case: bool,
    /// One text, or every text.
    pub text: Option<String>,
}

/// One sentence that would change.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub text_id: String,
    pub text_title: String,
    pub tier_id: String,
    pub segment_id: String,
    pub before: String,
    pub after: String,
    /// How many places in the sentence.
    pub count: usize,
    /// Its words are glossed (tokenized), so they will read as out of date.
    pub glossed: bool,
    /// The sentence as kept text and replaced places, for the preview.
    #[serde(default, skip_deserializing)]
    pub parts: Vec<Part>,
}

/// Kept text, then what one place held and what it will hold (both empty
/// after the last place).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub keep: String,
    pub removed: String,
    pub added: String,
}

fn parts(re: &Regex, before: &str, with: &str) -> Vec<Part> {
    let mut out = Vec::new();
    let mut at = 0;
    for m in re.find_iter(before) {
        out.push(Part { keep: before[at..m.start()].into(), removed: m.as_str().into(), added: with.into() });
        at = m.end();
    }
    out.push(Part { keep: before[at..].into(), ..Part::default() });
    out
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub changes: Vec<Change>,
    /// Sentences matched in all, when more than `changes` holds.
    pub total: usize,
}

/// The pattern for `f`: the text exactly as typed (no pattern syntax), NFC,
/// whole words with Unicode word boundaries, case folded unless asked.
fn pattern(f: &Find, nfc: &dyn Fn(&str) -> String) -> Result<Regex, String> {
    let find = nfc(f.find.trim_end_matches(['\n', '\r']));
    if find.is_empty() {
        return Err("Type what to find.".into());
    }
    let escaped = regex::escape(&find);
    // `\b` only at an edge that is a word character: `-ra` as a whole word
    // means `ra` after a hyphen, and a `\b` before the hyphen would need a
    // letter in front of it.
    let word = Regex::new(r"^\w$").expect("valid");
    let edge = |c: Option<char>| f.whole_word && c.is_some_and(|c| word.is_match(c.encode_utf8(&mut [0; 4])));
    let open = if edge(find.chars().next()) { r"\b" } else { "" };
    let close = if edge(find.chars().next_back()) { r"\b" } else { "" };
    let source = format!("{open}{escaped}{close}");
    RegexBuilder::new(&source).case_insensitive(!f.match_case).size_limit(1 << 20).build().map_err(|e| e.to_string())
}

pub fn preview(s: &mut Session, f: &Find) -> Result<Preview, String> {
    let p = session::project(s)?;
    let nfc = |x: &str| p.proj.nfc(x);
    let re = pattern(f, &nfc)?;
    let with = nfc(&f.with);
    if with.contains(['\n', '\r']) {
        return Err("A replacement cannot hold a line break.".into());
    }
    let (translation, transcription) = wugs_cldf::corpus_tiers(p)?;
    let tiers = if f.track == Track::Transcription { transcription } else { translation };
    let mut changes = Vec::new();
    let mut total = 0;
    for text in session::list_texts(s)? {
        if f.text.as_deref().is_some_and(|t| t != text.id) {
            continue;
        }
        let detail = session::get_text(s, &text.id)?;
        let p = session::project(s)?;
        for seg in detail.segments.iter().filter(|x| tiers.contains(&x.tier)) {
            let count = re.find_iter(&seg.content).count();
            if count == 0 {
                continue;
            }
            let after = re.replace_all(&seg.content, NoExpand(&with)).into_owned();
            if after == seg.content {
                continue;
            }
            total += 1;
            if changes.len() < MAX_CHANGES {
                let glossed = !p.proj.members(&seg.id)?.is_empty();
                changes.push(Change {
                    text_id: text.id.clone(),
                    text_title: detail.title.clone(),
                    tier_id: seg.tier.clone(),
                    segment_id: seg.id.clone(),
                    before: seg.content.clone(),
                    after,
                    count,
                    glossed,
                    parts: parts(&re, &seg.content, &with),
                });
            }
        }
    }
    Ok(Preview { changes, total })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    /// The changes written, for the undo (`apply` with before and after swapped).
    pub done: Vec<Change>,
    /// Sentences that changed since the preview, left alone.
    pub skipped: usize,
}

/// Write `changes`, each only if its sentence still reads `before`.
pub fn apply(s: &mut Session, changes: Vec<Change>) -> Result<Applied, String> {
    let mut done = Vec::new();
    let mut skipped = 0;
    for c in changes {
        let p = session::project(s)?;
        let now = p.proj.field_str(&c.segment_id, "content")?.unwrap_or_default();
        if p.proj.is_tombstoned(&c.segment_id)? || now != c.before {
            skipped += 1;
            continue;
        }
        session::edit_segment_content(s, &c.text_id, &c.segment_id, &c.after)?;
        done.push(c);
    }
    Ok(Applied { done, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(f: &str, w: &str) -> Find {
        Find { find: f.into(), with: w.into(), track: Track::Transcription, whole_word: false, match_case: false, text: None }
    }

    #[test]
    fn a_spelling_changes_everywhere_and_comes_back() {
        let dir = std::env::temp_dir().join(format!("wugs-replace-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let text = session::list_texts(&mut s).unwrap().remove(0).id;
        for content in ["θɹiː θɹiːs", "ΘɹIː", "one θɹiː"] {
            session::add_segment(&mut s, &text, None, None, None, content).unwrap();
        }
        // Case folded, not whole words: every θɹiː, θɹiːs's included.
        let all = preview(&mut s, &find("θɹiː", "tʃɹiː")).unwrap();
        assert_eq!(all.total, all.changes.len());
        assert!(all.changes.iter().any(|c| c.before == "θɹiː θɹiːs" && c.after == "tʃɹiː tʃɹiːs" && c.count == 2));
        assert!(all.changes.iter().any(|c| c.before == "ΘɹIː"), "case folded");
        let two = all.changes.iter().find(|c| c.before == "θɹiː θɹiːs").unwrap();
        let shown: Vec<_> = two.parts.iter().map(|p| (p.keep.as_str(), p.removed.as_str(), p.added.as_str())).collect();
        assert_eq!(shown, [("", "θɹiː", "tʃɹiː"), (" ", "θɹiː", "tʃɹiː"), ("s", "", "")]);
        // Whole words and case: only the exact word.
        let exact = preview(&mut s, &Find { whole_word: true, match_case: true, ..find("θɹiː", "tʃɹiː") }).unwrap();
        assert!(exact.changes.iter().any(|c| c.before == "θɹiː θɹiːs" && c.after == "tʃɹiː θɹiːs"));
        assert!(!exact.changes.iter().any(|c| c.before == "ΘɹIː"));
        // A whole word that starts with a hyphen still matches after a space.
        session::add_segment(&mut s, &text, None, None, None, "kɛ -ra kɛra").unwrap();
        let hyphen = preview(&mut s, &Find { whole_word: true, ..find("-ra", "-la") }).unwrap();
        assert!(hyphen.changes.iter().any(|c| c.after == "kɛ -la kɛra"), "{:?}", hyphen.changes);
        let ra = preview(&mut s, &Find { whole_word: true, ..find("ra", "la") }).unwrap();
        assert!(ra.changes.iter().any(|c| c.after == "kɛ -la kɛra"), "{:?}", ra.changes);
        // Pattern syntax is text: a dot is a dot, `$1` is written as is.
        assert_eq!(preview(&mut s, &find(".", "$1")).unwrap().total, 0);
        // The tutorial's first sentence is glossed: it is flagged.
        assert!(all.changes.iter().any(|c| c.before == "wʌn tuː θɹiː" && c.glossed));
        // Apply, then a sentence edited meanwhile is skipped by the undo.
        let applied = apply(&mut s, all.changes.clone()).unwrap();
        assert_eq!((applied.done.len(), applied.skipped), (all.changes.len(), 0));
        let edited = applied.done.iter().find(|c| c.before == "one θɹiː").unwrap().clone();
        session::edit_segment_content(&mut s, &edited.text_id, &edited.segment_id, "one tʃɹiː!").unwrap();
        let undo: Vec<Change> = applied.done.iter().map(|c| Change { before: c.after.clone(), after: c.before.clone(), ..c.clone() }).collect();
        let back = apply(&mut s, undo).unwrap();
        assert_eq!(back.skipped, 1);
        assert_eq!(preview(&mut s, &find("tʃɹiː", "x")).unwrap().total, 1, "only the sentence edited since keeps the new spelling");
        assert!(preview(&mut s, &find("", "x")).is_err());
        // Translations are their own track: the English line, never the IPA.
        let english = preview(&mut s, &Find { track: Track::Translation, ..find("two", "2") }).unwrap();
        assert_eq!(english.total, 1, "{:?}", english.changes);
        assert!(english.changes[0].after.contains('2'));
        assert_eq!(preview(&mut s, &Find { track: Track::Translation, ..find("wʌn", "x") }).unwrap().total, 0);
        std::fs::remove_dir_all(dir).ok();
    }
}
