//! Flashcards from the lexicon, for people learning the language: each entry
//! a card with its headword (and its sound, when a pronunciation is heard in
//! a recording here) on one side and its meanings, part of speech and an
//! example on the other. Written as an Anki import file (both directions,
//! GUIDs from the entry ids, so importing again updates rather than doubles),
//! a plain `term<TAB>definition` file for Quizlet and the like, the sounds,
//! and a README saying how to bring them in. Whole into a scratch folder,
//! then renamed, never inside the project.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::dictionary::headword_clip;
use crate::interchange::{cut_clip, outside_project};
use crate::lexicon::{entry_view, EntryView};
use crate::session::{self, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flashcards {
    pub path: String,
    pub cards: usize,
    /// Cards with a sound.
    pub sounds: usize,
    /// Entries left out because no sense has a gloss yet.
    pub no_gloss: usize,
}

/// Anki's built-in note type that makes a card each way.
const NOTETYPE: &str = "Basic (and reversed card)";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// One line of a field: tabs and line breaks would break the file's rows.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The card's back: numbered meanings when there are several, the part of
/// speech, and the first example with its translation.
fn back(e: &EntryView) -> (String, String) {
    let glosses: Vec<&str> = e.senses.iter().map(|s| s.gloss.trim()).filter(|g| !g.is_empty()).collect();
    let meaning = if glosses.len() == 1 {
        glosses[0].to_string()
    } else {
        glosses.iter().enumerate().map(|(i, g)| format!("{}. {g}", i + 1)).collect::<Vec<_>>().join("; ")
    };
    let mut html = format!("<b>{}</b>", esc(&flat(&meaning)));
    if !e.pos_of_speech.trim().is_empty() {
        html.push_str(&format!("<br><i>{}</i>", esc(&flat(&e.pos_of_speech))));
    }
    if let Some(x) = e.senses.iter().flat_map(|s| &s.examples).find(|x| !x.sentence.trim().is_empty()) {
        html.push_str(&format!("<br><br>{}", esc(&flat(&x.sentence))));
        if !x.translation.trim().is_empty() {
            html.push_str(&format!("<br><i>‘{}’</i>", esc(&flat(&x.translation))));
        }
    }
    (html, flat(&meaning))
}

/// The headword with its homograph number, as the front of a card shows it.
fn headword(e: &EntryView) -> String {
    let w = flat(&e.lemma);
    match e.homograph {
        Some(n) => format!("{w}{}", n.to_string().chars().map(|d| char::from_u32(0x2080 + d.to_digit(10).unwrap_or(0)).unwrap_or(d)).collect::<String>()),
        None => w,
    }
}

const README: &str = "Flashcards made with wugs
=========================

anki.txt     every entry as a card, both ways: the word (with its sound) and its meaning
quizlet.txt  the same as plain term<TAB>meaning lines
sounds/      the sounds the cards play

Anki (desktop)
1. Copy every file in sounds/ into Anki's media folder. To find it, open
   Anki, choose Tools > Check Media, and click View Files; or look for
   collection.media inside your Anki profile folder.
2. Choose File > Import and pick anki.txt. The file names its deck and the
   note type \"Basic (and reversed card)\", so each word is asked both ways;
   click Import. (In an Anki set to another language that note type has a
   translated name: choose it under Note Type in the import window.)
Importing a newer anki.txt later updates the cards you already have
(each card carries its entry's id), so your progress is kept.

AnkiDroid and AnkiMobile: import on the desktop and sync through AnkiWeb.

Quizlet and other flashcard sites: choose Import, paste or pick quizlet.txt,
and set \"between term and definition\" to Tab. Sounds are not carried there.
";

pub fn export_flashcards(s: &mut Session, folder: &Path, date: &str, include_restricted: bool) -> Result<Flashcards, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let date = crate::corrections::date_or_today(date);
    let lang = p.project_language()?;
    let deck = if lang.name.trim().is_empty() { "wugs words".to_string() } else { format!("{} words", flat(&lang.name)) };
    let mut entries: Vec<EntryView> = p.proj.entities_of_kind("lexeme")?.iter().map(|id| entry_view(p, id)).collect::<Result<_, _>>()?;
    entries.retain(|e| !e.lemma.trim().is_empty());
    entries.sort_by(|a, b| wugs_model::fold_key(&p.proj.nfc(&a.lemma)).cmp(&wugs_model::fold_key(&p.proj.nfc(&b.lemma))).then(a.homograph.cmp(&b.homograph)).then(a.id.cmp(&b.id)));

    // The note type and deck as columns, not `#notetype`/`#deck` headers, which some Anki versions ignore.
    let mut anki = String::from("#separator:tab\n#html:true\n#guid column:1\n#notetype column:2\n#deck column:3\n#columns:GUID\tNotetype\tDeck\tFront\tBack\n");
    let deck = flat(&deck);
    let mut quizlet = String::new();
    let mut sounds: Vec<(String, String, f64, f64)> = Vec::new();
    let mut out = Flashcards { path: String::new(), cards: 0, sounds: 0, no_gloss: 0 };
    for e in &entries {
        if e.senses.iter().all(|s| s.gloss.trim().is_empty()) {
            out.no_gloss += 1;
            continue;
        }
        let word = headword(e);
        let mut front = esc(&word);
        if let Some((blob, start, end)) = headword_clip(p, e, include_restricted)? {
            let file = format!("wugs-{}.wav", e.id.to_ascii_lowercase().replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "-"));
            front.push_str(&format!(" [sound:{file}]"));
            sounds.push((file, blob, start, end));
        }
        let (html, plain) = back(e);
        anki.push_str(&format!("wugs-{}\t{NOTETYPE}\t{deck}\t{front}\t{html}\n", e.id));
        quizlet.push_str(&format!("{word}\t{plain}\n"));
        out.cards += 1;
    }
    if out.cards == 0 {
        return Err(if entries.is_empty() { "The lexicon has no entries yet.".into() } else { "No entry has a gloss yet, so no card would have a back.".into() });
    }

    let mut target = folder.join(format!("wugs-flashcards-{date}"));
    let mut k = 2;
    while target.symlink_metadata().is_ok() {
        target = folder.join(format!("wugs-flashcards-{date}-{k}"));
        k += 1;
    }
    let scratch = folder.join(format!(".{}.partial", target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        fs::write(scratch.join("anki.txt"), &anki).map_err(|e| format!("anki.txt: {e}"))?;
        fs::write(scratch.join("quizlet.txt"), &quizlet).map_err(|e| format!("quizlet.txt: {e}"))?;
        fs::write(scratch.join("README.txt"), README).map_err(|e| format!("README.txt: {e}"))?;
        if !sounds.is_empty() {
            fs::create_dir(scratch.join("sounds")).map_err(|e| format!("sounds: {e}"))?;
        }
        for (file, blob, start, end) in &sounds {
            let hex = wugs_blob::parse_ref(blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?;
            cut_clip(&p.root.join("blobs").join(hex), *start, *end, &scratch.join("sounds").join(file)).map_err(|e| format!("{file}: {e}"))?;
        }
        fs::rename(&scratch, &target).map_err(|e| format!("{}: {e}", target.display()))
    })();
    if let Err(e) = written {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    out.sounds = sounds.len();
    out.path = target.display().to_string();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutorial_lexicon_becomes_cards_with_a_sound() {
        let dir = std::env::temp_dir().join(format!("wugs-cards-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        session::project(&mut s).unwrap().set_project_language("Tutorial English", "", "eng").unwrap();
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        let seg = session::get_text(&mut s, &text).unwrap().segments.into_iter().find(|g| g.content.starts_with("wʌn")).unwrap();
        let entry = crate::lexicon::lexicon_list(&mut s, "").unwrap().into_iter().find(|r| r.lemma == "wʌn").unwrap().id;
        crate::lexicon::lexicon_add_pronunciation(&mut s, &entry, "ˈwʌn", Some(&seg.id)).unwrap();
        // An entry with no gloss yet makes no card.
        session::project(&mut s).unwrap().add_lexeme("zzz", "").unwrap();

        let r = export_flashcards(&mut s, &out, "2026-11-23", false).unwrap();
        assert!(r.cards >= 5 && r.sounds == 1 && r.no_gloss == 1, "{r:?}");
        let root = Path::new(&r.path);
        assert!(r.path.ends_with("wugs-flashcards-2026-11-23"));
        let anki = fs::read_to_string(root.join("anki.txt")).unwrap();
        assert!(anki.starts_with("#separator:tab\n#html:true\n#guid column:1\n#notetype column:2\n#deck column:3\n"), "{anki}");
        let line = anki.lines().find(|l| l.contains("\twʌn ")).unwrap();
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 5, "{line}");
        assert_eq!(cols[..3], [format!("wugs-{entry}").as_str(), "Basic (and reversed card)", "Tutorial English words"]);
        let file = format!("wugs-{}.wav", entry.to_ascii_lowercase());
        assert_eq!(cols[3], format!("wʌn [sound:{file}]"));
        assert!(cols[4].starts_with("<b>one</b>"), "{line}");
        assert!(root.join("sounds").join(&file).is_file());
        assert!(fs::read_to_string(root.join("quizlet.txt")).unwrap().lines().any(|l| l == "wʌn\tone"));
        assert!(!anki.contains("zzz"));
        // The recording made restricted: the card stays, its sound does not.
        session::project(&mut s).unwrap().set_access(&text, "restricted").unwrap();
        let src = session::project(&mut s).unwrap().text_recording(&text).unwrap().unwrap().0;
        session::project(&mut s).unwrap().set_access(&src, "restricted").unwrap();
        let again = export_flashcards(&mut s, &out, "2026-11-23", false).unwrap();
        assert!(again.path.ends_with("-2") && again.sounds == 0 && again.cards == r.cards, "{again:?}");
        fs::remove_dir_all(&dir).ok();
    }
}
