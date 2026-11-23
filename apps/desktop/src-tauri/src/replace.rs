//! Find and replace in transcriptions or translations across texts: a
//! preview of every sentence that would change, then the chosen ones
//! written, each as a person's edit of that sentence (a resolving
//! collaborative text edit, like typing it). A sentence that changed since
//! the preview is skipped, never overwritten, and the same call run with
//! before and after swapped is the undo. Headwords take part too, so an
//! orthography change reaches the dictionary: each entry's lemma, citation
//! form and other forms, each written as a person's edit of that field.
//! Glosses take part as well, so an abbreviation can be renamed across the
//! project (`PST` to `PAST`): each morph's gloss written as `set_gloss`.

use regex::{NoExpand, Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{self, Session};

/// At most this many sentences in one preview.
const MAX_CHANGES: usize = 2000;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Track {
    Transcription,
    Translation,
    /// Lexicon headwords: lemmas, citation forms and other forms.
    Headwords,
    /// Morph glosses (the primary language) of the transcriptions' words.
    Glosses,
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
    /// For a headword: the field changed (`lemma`, `citation`, or `form` of
    /// an other form), and `segment_id` holds the entry or form; for a gloss
    /// `gloss`, with the morph in `segment_id`; `None` for a sentence.
    #[serde(default)]
    pub field: Option<String>,
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
    let mut changes = Vec::new();
    let mut total = 0;
    if f.track == Track::Headwords {
        let mut lexemes: Vec<(String, String)> = Vec::new();
        for id in p.proj.entities_of_kind("lexeme")? {
            if !p.proj.is_tombstoned(&id)? {
                lexemes.push((wugs_model::fold_key(&p.proj.field_str(&id, "lemma")?.unwrap_or_default()), id));
            }
        }
        lexemes.sort();
        for (_, lex) in lexemes {
            let lemma = p.proj.field_str(&lex, "lemma")?.unwrap_or_default();
            let mut fields: Vec<(String, &str, String)> = ["lemma", "citation"].iter().map(|f| Ok((lex.clone(), *f, p.proj.field_str(&lex, f)?.unwrap_or_default()))).collect::<Result<_, String>>()?;
            fields.extend(p.allomorphs(&lex)?.into_iter().map(|(form, surface)| (form, "form", surface)));
            for (id, field, value) in fields {
                let count = re.find_iter(&value).count();
                if count == 0 {
                    continue;
                }
                let after = re.replace_all(&value, NoExpand(&with)).into_owned();
                if after == value || after.trim().is_empty() {
                    continue;
                }
                total += 1;
                if changes.len() < MAX_CHANGES {
                    changes.push(Change {
                        text_id: String::new(),
                        text_title: format!("Lexicon «{lemma}»"),
                        tier_id: String::new(),
                        segment_id: id,
                        before: value.clone(),
                        after,
                        count,
                        glossed: false,
                        parts: parts(&re, &value, &with),
                        field: Some(field.to_string()),
                    });
                }
            }
        }
        return Ok(Preview { changes, total });
    }
    if f.track == Track::Glosses {
        let scope = match f.text.as_deref() {
            Some(t) => Scope::text(t),
            None => Scope::all(),
        };
        each_segment(p, Read::Analyses, &scope, |seg| {
            for tok in &seg.tokens {
                let word = p.proj.field_str(tok, "form")?.unwrap_or_default();
                for m in p.proj.members(tok)? {
                    let value = p.proj.field_str(&m, "gloss")?.unwrap_or_default();
                    let count = re.find_iter(&value).count();
                    if count == 0 {
                        continue;
                    }
                    let after = re.replace_all(&value, NoExpand(&with)).into_owned();
                    if after == value || after.trim().is_empty() {
                        continue;
                    }
                    total += 1;
                    if changes.len() < MAX_CHANGES {
                        changes.push(Change {
                            text_id: seg.text.clone(),
                            text_title: format!("{} · {word}", seg.title),
                            tier_id: seg.tier.clone(),
                            segment_id: m.clone(),
                            before: value.clone(),
                            after,
                            count,
                            glossed: false,
                            parts: parts(&re, &value, &with),
                            field: Some("gloss".into()),
                        });
                    }
                }
            }
            Ok(())
        })?;
        return Ok(Preview { changes, total });
    }
    let (translation, transcription) = wugs_cldf::corpus_tiers(p)?;
    let tiers = if f.track == Track::Transcription { transcription } else { translation };
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
                    field: None,
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
        if c.field.as_deref() == Some("gloss") {
            let now = p.proj.field_str(&c.segment_id, "gloss")?.unwrap_or_default();
            if p.proj.entity_kind(&c.segment_id)?.as_deref() != Some("morph") || p.proj.is_tombstoned(&c.segment_id)? || now != c.before {
                skipped += 1;
                continue;
            }
            p.set_gloss(&c.text_id, &c.segment_id, &c.after, None)?;
            done.push(c);
            continue;
        }
        if let Some(field) = c.field.as_deref() {
            let kind = if field == "form" { "form" } else { "lexeme" };
            let now = p.proj.field_str(&c.segment_id, field)?.unwrap_or_default();
            if !matches!(field, "lemma" | "citation" | "form") || p.proj.entity_kind(&c.segment_id)?.as_deref() != Some(kind) || p.proj.is_tombstoned(&c.segment_id)? || now != c.before {
                skipped += 1;
                continue;
            }
            if field == "form" {
                p.set_form_surface(&c.segment_id, &c.after)?;
            } else {
                p.set_lexeme_field(&c.segment_id, field, &c.after)?;
            }
            done.push(c);
            continue;
        }
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
    fn a_gloss_is_renamed_everywhere_and_comes_back() {
        let dir = std::env::temp_dir().join(format!("wugs-replace-gloss-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        crate::sample::add_morphology(&mut s).unwrap();
        let rename = Find { track: Track::Glosses, whole_word: true, match_case: true, ..find("SG", "SING") };
        // `1SG` is one word: SG inside it is not a whole word, so nothing changes.
        assert_eq!(preview(&mut s, &rename).unwrap().total, 0);
        let all = preview(&mut s, &Find { track: Track::Glosses, whole_word: true, match_case: true, ..find("1SG", "1S") }).unwrap();
        assert_eq!(all.total, 1);
        let c = &all.changes[0];
        assert_eq!((c.before.as_str(), c.after.as_str(), c.field.as_deref()), ("1SG", "1S", Some("gloss")));
        assert!(c.text_title.ends_with("· nikani"), "{}", c.text_title);
        let done = apply(&mut s, all.changes.clone()).unwrap();
        assert_eq!((done.done.len(), done.skipped), (1, 0));
        let p = session::project(&mut s).unwrap();
        assert_eq!(p.proj.field_str(&c.segment_id, "gloss").unwrap().as_deref(), Some("1S"));
        // Applied twice, the second finds it changed and leaves it.
        assert_eq!(apply(&mut s, all.changes.clone()).unwrap().skipped, 1);
        // The undo: before and after swapped.
        let undo: Vec<Change> = done.done.into_iter().map(|c| Change { before: c.after.clone(), after: c.before.clone(), ..c }).collect();
        assert_eq!(apply(&mut s, undo).unwrap().done.len(), 1);
        let p = session::project(&mut s).unwrap();
        assert_eq!(p.proj.field_str(&c.segment_id, "gloss").unwrap().as_deref(), Some("1SG"));
        std::fs::remove_dir_all(&dir).ok();
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
        // Headwords: lemmas, citation forms and other forms, never a sentence.
        let wan = crate::lexicon::lexicon_list(&mut s, "wʌn").unwrap()[0].id.clone();
        crate::lexicon::lexicon_add_allomorph(&mut s, &wan, "wʌnə").unwrap();
        let heads = preview(&mut s, &Find { track: Track::Headwords, ..find("ʌ", "a") }).unwrap();
        assert!(heads.changes.iter().any(|c| c.field.as_deref() == Some("lemma") && c.before == "wʌn" && c.after == "wan" && c.text_title == "Lexicon «wʌn»"), "{:?}", heads.changes);
        assert!(heads.changes.iter().any(|c| c.field.as_deref() == Some("form") && c.after == "wanə"));
        assert!(heads.changes.iter().all(|c| c.field.is_some()));
        let done = apply(&mut s, heads.changes.clone()).unwrap();
        assert_eq!(done.skipped, 0);
        assert_eq!(crate::lexicon::lexicon_list(&mut s, "wan").unwrap()[0].lemma, "wan");
        let undo: Vec<Change> = done.done.iter().map(|c| Change { before: c.after.clone(), after: c.before.clone(), ..c.clone() }).collect();
        assert_eq!(apply(&mut s, undo).unwrap().skipped, 0);
        assert_eq!(crate::lexicon::lexicon_list(&mut s, "wʌn").unwrap()[0].lemma, "wʌn", "the undo puts it back");
        std::fs::remove_dir_all(dir).ok();
    }
}
