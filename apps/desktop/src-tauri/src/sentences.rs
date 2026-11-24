//! Sentence elicitation: a list of prompts (sentences in a contact language
//! to translate, as in a questionnaire) worked through one by one. A list
//! is kept as one project setting, `sentence-prompts:<slug>`, holding JSON
//! (its name and prompts, each with an id). Each answer is an ordinary
//! sentence: a segment on the utterance track of the list's own text (made
//! with the first answer, marked `x.sentence-list` = the slug), carrying
//! `x.prompt` = the prompt's id, with the prompt as its translation on the
//! paired translation track, so it can be glossed, searched and exported
//! like any other sentence. Several answers to one prompt are kept.

use serde::{Deserialize, Serialize};
use wugs_core::Project;
use wugs_model::Value;

use crate::session::{project, Session};

pub const PREFIX: &str = "sentence-prompts:";
/// What one setting holds, with room to spare.
const MAX_BYTES: usize = 60 * 1024;
const MAX_PROMPTS: usize = 2000;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Prompt {
    pub id: String,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Stored {
    name: String,
    prompts: Vec<Prompt>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PromptList {
    /// The slug the setting is keyed by.
    pub id: String,
    pub name: String,
    pub total: usize,
    pub answered: usize,
    /// The list's text, once it has an answer.
    pub text_id: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub segment_id: String,
    pub tier_id: String,
    pub content: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PromptRow {
    pub id: String,
    pub number: usize,
    pub prompt: String,
    pub answers: Vec<Answer>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PromptView {
    pub list: PromptList,
    pub prompts: Vec<PromptRow>,
}

/// A list's slug from its name: lower case, letters and digits, other runs one `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let s: String = out.trim_matches('-').chars().take(40).collect();
    if s.is_empty() {
        "prompts".into()
    } else {
        s
    }
}

/// Prompts read from a file's text: a table with a header (a prompt column by
/// name, else the first that is not numbers; an id column by name), or one
/// prompt per line.
pub(crate) fn read_prompts(text: &str, tabs: bool) -> Result<Vec<Prompt>, String> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let table = first.contains('\t') || (!tabs && first.contains(','));
    let mut out: Vec<Prompt> = Vec::new();
    if table {
        let (header, rows, _) = crate::table_text::records(text, tabs || first.contains('\t'))?;
        let named = |names: &[&str]| header.iter().position(|h| names.contains(&h.trim().to_lowercase().as_str()));
        let numeric = |i: usize| rows.iter().all(|r| r.get(i).is_none_or(|c| c.trim().is_empty() || c.trim().trim_end_matches('.').parse::<f64>().is_ok()));
        let id = named(&["id", "number", "no", "no.", "nr", "#", "num"]);
        let prompt = named(&["prompt", "sentence", "stimulus", "english", "translation", "text", "question", "item"])
            .or_else(|| (0..header.len()).find(|i| Some(*i) != id && !numeric(*i)))
            .ok_or("no column holds the prompts: name one prompt or sentence")?;
        for (i, r) in rows.iter().enumerate() {
            let p = r.get(prompt).map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
            if p.is_empty() {
                continue;
            }
            let given = id.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            out.push(Prompt { id: given.unwrap_or_else(|| (i + 1).to_string()), prompt: p });
        }
    } else {
        for line in text.lines() {
            let p = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if !p.is_empty() {
                out.push(Prompt { id: (out.len() + 1).to_string(), prompt: p });
            }
        }
    }
    if out.is_empty() {
        return Err("no prompts in the file".into());
    }
    if out.len() > MAX_PROMPTS {
        return Err(format!("a prompt list holds at most {MAX_PROMPTS} prompts"));
    }
    // Ids must be unique: a repeat gets -2, -3…
    let mut seen = std::collections::HashSet::new();
    for p in &mut out {
        if !seen.insert(p.id.clone()) {
            let mut k = 2;
            while !seen.insert(format!("{}-{k}", p.id)) {
                k += 1;
            }
            p.id = format!("{}-{k}", p.id);
        }
    }
    Ok(out)
}

fn stored(p: &Project, slug: &str) -> Result<Stored, String> {
    let raw = p.setting(&format!("{PREFIX}{slug}"))?.ok_or_else(|| format!("no prompt list «{slug}»"))?;
    serde_json::from_str(&raw).map_err(|e| format!("the prompt list «{slug}» does not read: {e}"))
}

/// The list's text: the live text marked with its slug.
fn text_of(p: &Project, slug: &str) -> Result<Option<String>, String> {
    for t in p.proj.entities_of_kind("text")? {
        if !p.proj.is_tombstoned(&t)? && p.proj.field_str(&t, "x.sentence-list")?.as_deref() == Some(slug) {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

/// Every live answer in the list's text, by prompt id, in text order.
fn answers(p: &Project, text: Option<&str>) -> Result<std::collections::HashMap<String, Vec<Answer>>, String> {
    let mut out: std::collections::HashMap<String, Vec<(String, Answer)>> = std::collections::HashMap::new();
    if let Some(text) = text {
        for seg in p.proj.children_by_ref("text", text, "segment")? {
            if p.proj.is_tombstoned(&seg)? {
                continue;
            }
            let Some(prompt) = p.proj.field_str(&seg, "x.prompt")? else { continue };
            let pos = p.proj.field_str(&seg, "pos")?.unwrap_or_default();
            let a = Answer { segment_id: seg.clone(), tier_id: p.proj.field_str(&seg, "tier")?.unwrap_or_default(), content: p.proj.field_str(&seg, "content")?.unwrap_or_default() };
            out.entry(prompt).or_default().push((pos, a));
        }
    }
    Ok(out
        .into_iter()
        .map(|(k, mut v)| {
            v.sort_by(|a, b| a.0.cmp(&b.0));
            (k, v.into_iter().map(|x| x.1).collect())
        })
        .collect())
}

fn summary(p: &Project, slug: &str, s: &Stored) -> Result<PromptList, String> {
    let text = text_of(p, slug)?;
    let found = answers(p, text.as_deref())?;
    let answered = s.prompts.iter().filter(|x| found.get(&x.id).is_some_and(|a| !a.is_empty())).count();
    Ok(PromptList { id: slug.to_string(), name: s.name.clone(), total: s.prompts.len(), answered, text_id: text })
}

pub fn lists(session: &mut Session) -> Result<Vec<PromptList>, String> {
    let p = project(session)?;
    let mut out = Vec::new();
    for (key, raw) in p.settings_with_prefix(PREFIX)? {
        let Ok(s) = serde_json::from_str::<Stored>(&raw) else { continue };
        out.push(summary(p, &key[PREFIX.len()..], &s)?);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Keeps a prompt list read from a file under its name (the file's stem);
/// loading the same name again replaces its prompts, answers stay.
pub fn import(session: &mut Session, path: &std::path::Path) -> Result<PromptList, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > 5_000_000 {
        return Err("a prompt file is at most 5 MB".into());
    }
    let text = crate::interchange::windows_1252(wugs_legacy::decode(&bytes).map_err(|e| e.to_string())?, &bytes);
    let tabs = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tsv") || e.eq_ignore_ascii_case("tab"));
    let p = project(session)?;
    let prompts: Vec<Prompt> = read_prompts(&p.proj.nfc(&text), tabs)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "Sentences".into());
    let name = p.proj.nfc(&name);
    let slug = slug(&name);
    let json = serde_json::to_string(&Stored { name, prompts }).map_err(|e| e.to_string())?;
    if json.len() > MAX_BYTES {
        return Err("the prompts are too long to keep with the project (60 KB at most); split the file".into());
    }
    if p.setting(&format!("{PREFIX}{slug}"))?.as_deref() != Some(json.as_str()) {
        p.set_setting(&format!("{PREFIX}{slug}"), &json)?;
    }
    let s = stored(p, &slug)?;
    summary(p, &slug, &s)
}

pub fn view(session: &mut Session, list: &str) -> Result<PromptView, String> {
    let p = project(session)?;
    let s = stored(p, list)?;
    let text = text_of(p, list)?;
    let mut found = answers(p, text.as_deref())?;
    let prompts = s.prompts.iter().enumerate().map(|(i, x)| PromptRow { id: x.id.clone(), number: i + 1, prompt: x.prompt.clone(), answers: found.remove(&x.id).unwrap_or_default() }).collect();
    Ok(PromptView { list: summary(p, list, &s)?, prompts })
}

/// Writes an answer to a prompt: the list's text when it has none yet, the
/// sentence on the utterance track with `x.prompt`, and the prompt as its
/// translation, in one txn.
pub fn answer(session: &mut Session, list: &str, prompt: &str, sentence: &str) -> Result<PromptView, String> {
    let p = project(session)?;
    let s = stored(p, list)?;
    let item = s.prompts.iter().find(|x| x.id == prompt).ok_or_else(|| format!("no prompt «{prompt}» in this list"))?.clone();
    let sentence = p.proj.nfc(sentence.trim());
    if sentence.is_empty() {
        return Err("an answer needs some text".into());
    }
    let tier = crate::session::utterance_tier(p)?;
    let translation = crate::session::translation_track(p, &tier, None)?;
    let existing = text_of(p, list)?;
    let index = match &existing {
        Some(t) => crate::session::next_pos_index(p, t)?,
        None => 0,
    };
    let mut t = p.intent("elicit-sentence");
    let text = match existing {
        Some(text) => text,
        None => t.create_scope_root("text", "txt", vec![("title", Value::str(&s.name)), ("x.sentence-list", Value::str(list))]),
    };
    let pos = wugs_model::pos_seq(index);
    let seg = t.create(
        "segment",
        "seg",
        &text,
        vec![("text", Value::str(&text)), ("tier", Value::str(&tier)), ("pos", Value::str(&pos)), ("content", Value::str(&sentence)), ("x.prompt", Value::str(&item.id))],
    );
    t.create(
        "segment",
        "seg",
        &text,
        vec![("text", Value::str(&text)), ("tier", Value::str(&translation)), ("pos", Value::str(&pos)), ("content", Value::str(&item.prompt)), ("parent", Value::str(&seg))],
    );
    t.commit()?;
    view(session, list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_read_from_lines_and_tables() {
        let lines = read_prompts("The dog sleeps.\n\n  The   dog bit the man. \n", false).unwrap();
        assert_eq!(lines, [Prompt { id: "1".into(), prompt: "The dog sleeps.".into() }, Prompt { id: "2".into(), prompt: "The dog bit the man.".into() }]);
        let table = read_prompts("No,Prompt,Notes\n12,The dog sleeps.,intransitive\n12,The dog bit the man.,\n13,,\n", false).unwrap();
        assert_eq!(table[0], Prompt { id: "12".into(), prompt: "The dog sleeps.".into() });
        assert_eq!(table[1].id, "12-2", "a repeated id is made unique");
        assert_eq!(table.len(), 2);
        assert!(read_prompts("\n\n", false).is_err());
        assert_eq!(slug(" Valency  questionnaire (2024)!"), "valency-questionnaire-2024");
    }

    #[test]
    fn answers_become_sentences_with_their_prompt_as_translation() {
        let dir = std::env::temp_dir().join(format!("wugs-sentences-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let file = dir.join("Valency prompts.txt");
        std::fs::write(&file, "The dog sleeps.\nThe dog bit the man.\n").unwrap();
        let l = import(&mut s, &file).unwrap();
        assert_eq!((l.id.as_str(), l.total, l.answered, l.text_id.is_none()), ("valency-prompts", 2, 0, true));
        let v = answer(&mut s, "valency-prompts", "2", " kuli  aba ta ").unwrap();
        assert_eq!(v.list.answered, 1);
        let text = v.list.text_id.clone().unwrap();
        assert_eq!(v.prompts[1].answers[0].content, "kuli  aba ta".trim());
        let v = answer(&mut s, "valency-prompts", "2", "kuli ta aba").unwrap();
        assert_eq!(v.prompts[1].answers.len(), 2, "a second answer is kept beside the first");
        assert_eq!(v.list.text_id.as_deref(), Some(text.as_str()), "one text per list");
        let shown = crate::session::get_text(&mut s, &text).unwrap();
        let first = shown.segments.iter().find(|x| x.content == "kuli  aba ta").unwrap();
        let tr = shown.segments.iter().find(|x| x.parent.as_deref() == Some(first.id.as_str())).unwrap();
        assert_eq!(tr.content, "The dog bit the man.");
        assert!(answer(&mut s, "valency-prompts", "9", "x").is_err());
        assert!(answer(&mut s, "valency-prompts", "1", "  ").is_err());
        // Loading the file again keeps the answers.
        assert_eq!(import(&mut s, &file).unwrap().answered, 1);
        assert_eq!(lists(&mut s).unwrap().len(), 1);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
