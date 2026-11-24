//! Flashcards from the lexicon, for people learning the language: each entry
//! a card with its headword (and its sound, when a pronunciation is heard in
//! a recording here) on one side and its meanings, picture, part of speech
//! and an example on the other. Written as an Anki import file (both directions,
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
    /// Cards with a picture.
    pub pictures: usize,
    /// Entries left out because no sense has a gloss yet.
    pub no_gloss: usize,
    /// Commonest first: cards whose word the texts use (the rest come after); None A to Z.
    pub used: Option<usize>,
    /// Commonest first with a limit: glossed entries past it, left out.
    pub past_limit: usize,
    /// Sentence cards (`Order::Sentences`): `no_gloss` then counts sentences with no translation.
    pub sentences: bool,
}

/// Which cards, in what order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "order")]
pub enum Order {
    /// Every glossed entry by its headword.
    Alphabet,
    /// The words the texts use most first, then the rest by headword; at
    /// most `limit` cards (0 = all), so a class can start with the commonest.
    Commonest { limit: usize },
    /// A card per translated sentence of the texts (with its stretch of the
    /// recording when timed), for learning to hear and read whole sentences.
    Sentences,
}

/// At most this many sentence cards are written.
pub const MAX_SENTENCE_CARDS: usize = 5000;

/// How many words of the texts each entry's senses are linked to (a word
/// counted once however many of its morphs link to the entry).
pub fn uses_by_entry(p: &wugs_core::Project) -> Result<std::collections::HashMap<String, usize>, String> {
    let mut entry_of: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    let mut uses: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    crate::concordance::each_segment(p, crate::concordance::Read::Analyses, &crate::concordance::Scope::all(), |seg| {
        for tok in &seg.tokens {
            let mut seen: Vec<String> = Vec::new();
            for m in p.proj.members(tok)? {
                let Some(sense) = p.proj.field_str(&m, "sense")?.filter(|s| !s.is_empty()) else { continue };
                if !entry_of.contains_key(&sense) {
                    let lexeme = p.proj.field_str(&sense, "lexeme")?;
                    entry_of.insert(sense.clone(), lexeme);
                }
                if let Some(Some(entry)) = entry_of.get(&sense) {
                    if !seen.contains(entry) {
                        seen.push(entry.clone());
                        *uses.entry(entry.clone()).or_default() += 1;
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(uses)
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
fn back(e: &EntryView, picture: Option<&str>) -> (String, String) {
    let glosses: Vec<&str> = e.senses.iter().map(|s| s.gloss.trim()).filter(|g| !g.is_empty()).collect();
    let meaning = if glosses.len() == 1 {
        glosses[0].to_string()
    } else {
        glosses.iter().enumerate().map(|(i, g)| format!("{}. {g}", i + 1)).collect::<Vec<_>>().join("; ")
    };
    let mut html = format!("<b>{}</b>", esc(&flat(&meaning)));
    if let Some(file) = picture {
        html.push_str(&format!("<br><img src=\"{}\">", esc(file)));
    }
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

anki.txt     every entry as a card, both ways: the word (with its sound) and its meaning (with its picture)
quizlet.txt  the same as plain term<TAB>meaning lines
media/       the sounds and pictures the cards show

Anki (desktop)
1. Copy every file in media/ into Anki's media folder. To find it, open
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
and set \"between term and definition\" to Tab. Sounds and pictures are not
carried there.
";

const SENTENCE_README: &str = "Sentence flashcards made with wugs
==================================

anki.txt     every translated sentence of the texts as a card: the sentence
             (with its sound, when it is timed in a recording) and its meaning
quizlet.txt  the same as plain sentence<TAB>meaning lines
media/       the sounds the cards play

Anki (desktop)
1. Copy every file in media/ into Anki's media folder (Tools > Check Media >
   View Files, or collection.media inside your Anki profile folder).
2. Choose File > Import and pick anki.txt. The cards use the note type
   \"Basic\": the sentence is asked, its meaning is the answer.
Importing a newer anki.txt later updates the cards you have (each carries its
sentence's id), so your progress is kept.

Quizlet and other flashcard sites: import quizlet.txt with Tab between term and
definition. Sounds are not carried there.
";

/// A card per translated sentence (open texts unless restricted ones are included).
fn sentence_cards(p: &wugs_core::Project, include_restricted: bool) -> Result<(String, String, Vec<(String, String, f64, f64)>, Flashcards), String> {
    use crate::concordance::{each_segment, Read, Scope};
    let lang = p.project_language()?;
    let deck = flat(&if lang.name.trim().is_empty() { "wugs sentences".to_string() } else { format!("{} sentences", lang.name) });
    let mut anki = String::from("#separator:tab\n#html:true\n#guid column:1\n#notetype column:2\n#deck column:3\n#columns:GUID\tNotetype\tDeck\tFront\tBack\n");
    let mut quizlet = String::new();
    let mut sounds = Vec::new();
    let mut out = Flashcards { path: String::new(), cards: 0, sounds: 0, pictures: 0, no_gloss: 0, used: None, past_limit: 0, sentences: true };
    let mut open: std::collections::HashMap<String, (bool, Option<String>)> = std::collections::HashMap::new();
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        if !open.contains_key(&seg.text) {
            let ok = include_restricted || p.text_access(&seg.text)? == "open";
            let blob = p.text_recording(&seg.text)?.map(|(_, b)| b).filter(|b| wugs_blob::parse_ref(b).is_some_and(|hex| p.root.join("blobs").join(hex).is_file()));
            open.insert(seg.text.clone(), (ok, blob));
        }
        let (ok, blob) = open.get(&seg.text).cloned().unwrap_or((false, None));
        if !ok {
            return Ok(());
        }
        let sentence = flat(&p.proj.field_str(&seg.id, "content")?.unwrap_or_default());
        if sentence.trim().is_empty() {
            return Ok(());
        }
        let Some((_, meaning)) = crate::interchange::translations_of(p, &seg.id)?.into_iter().next() else {
            out.no_gloss += 1;
            return Ok(());
        };
        if out.cards >= MAX_SENTENCE_CARDS {
            out.past_limit += 1;
            return Ok(());
        }
        let mut front = esc(&sentence);
        if let (Some(blob), Some(start), Some(end)) = (&blob, p.proj.field_num(&seg.id, "start")?, p.proj.field_num(&seg.id, "end")?) {
            if end > start {
                let file = format!("wugs-s-{}.wav", seg.id.to_ascii_lowercase().replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "-"));
                front.push_str(&format!(" [sound:{file}]"));
                sounds.push((file, blob.clone(), start, end));
            }
        }
        let back = format!("{}<br><small>{}</small>", esc(&flat(&meaning)), esc(&flat(&seg.title)));
        anki.push_str(&format!("wugs-s-{}\tBasic\t{deck}\t{front}\t{back}\n", seg.id));
        quizlet.push_str(&format!("{sentence}\t{}\n", flat(&meaning)));
        out.cards += 1;
        Ok(())
    })?;
    Ok((anki, quizlet, sounds, out))
}

pub fn export_flashcards(s: &mut Session, folder: &Path, date: &str, include_restricted: bool, order: Order) -> Result<Flashcards, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    if order == Order::Sentences {
        let (anki, quizlet, sounds, mut out) = sentence_cards(p, include_restricted)?;
        if out.cards == 0 {
            return Err("No sentence of the texts has a translation yet, or only texts that are not open do (tick Include texts that are not open to use them).".into());
        }
        out.path = write_cards(p, folder, &crate::corrections::date_or_today(date), &anki, &quizlet, SENTENCE_README, &sounds, &[])?;
        out.sounds = sounds.len();
        return Ok(out);
    }
    let date = crate::corrections::date_or_today(date);
    let lang = p.project_language()?;
    let deck = if lang.name.trim().is_empty() { "wugs words".to_string() } else { format!("{} words", flat(&lang.name)) };
    let mut entries: Vec<EntryView> = p.proj.entities_of_kind("lexeme")?.iter().map(|id| entry_view(p, id)).collect::<Result<_, _>>()?;
    entries.retain(|e| !e.lemma.trim().is_empty());
    entries.sort_by(|a, b| wugs_model::fold_key(&p.proj.nfc(&a.lemma)).cmp(&wugs_model::fold_key(&p.proj.nfc(&b.lemma))).then(a.homograph.cmp(&b.homograph)).then(a.id.cmp(&b.id)));
    let uses = match order {
        Order::Alphabet => None,
        Order::Sentences => unreachable!("written above"),
        Order::Commonest { .. } => {
            let uses = uses_by_entry(p)?;
            // Stable: equal counts keep the headword order.
            entries.sort_by_key(|e| std::cmp::Reverse(uses.get(&e.id).copied().unwrap_or(0)));
            Some(uses)
        }
    };
    let limit = match order {
        Order::Commonest { limit } if limit > 0 => limit,
        _ => usize::MAX,
    };

    // The note type and deck as columns, not `#notetype`/`#deck` headers, which some Anki versions ignore.
    let mut anki = String::from("#separator:tab\n#html:true\n#guid column:1\n#notetype column:2\n#deck column:3\n#columns:GUID\tNotetype\tDeck\tFront\tBack\n");
    let deck = flat(&deck);
    let mut quizlet = String::new();
    let mut sounds: Vec<(String, String, f64, f64)> = Vec::new();
    let mut pictures: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut out = Flashcards { path: String::new(), cards: 0, sounds: 0, pictures: 0, no_gloss: 0, used: uses.as_ref().map(|_| 0), past_limit: 0, sentences: false };
    for e in &entries {
        if e.senses.iter().all(|s| s.gloss.trim().is_empty()) {
            out.no_gloss += 1;
            continue;
        }
        if out.cards >= limit {
            out.past_limit += 1;
            continue;
        }
        if let (Some(u), Some(n)) = (&uses, out.used.as_mut()) {
            if u.get(&e.id).is_some_and(|c| *c > 0) {
                *n += 1;
            }
        }
        let word = headword(e);
        let mut front = esc(&word);
        if let Some((blob, start, end)) = headword_clip(p, e, include_restricted)? {
            let file = format!("wugs-{}.wav", e.id.to_ascii_lowercase().replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "-"));
            front.push_str(&format!(" [sound:{file}]"));
            sounds.push((file, blob, start, end));
        }
        let picture = e.senses.iter().find_map(|s| s.picture.as_ref().and_then(|x| x.path.as_ref().map(|path| (s, x, path)))).map(|(s, x, path)| {
            let file = format!("wugs-{}.{}", s.id.to_ascii_lowercase().replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "-"), crate::lexicon::picture_ext(&x.name));
            pictures.push((file.clone(), std::path::PathBuf::from(path)));
            file
        });
        let (html, plain) = back(e, picture.as_deref());
        anki.push_str(&format!("wugs-{}\t{NOTETYPE}\t{deck}\t{front}\t{html}\n", e.id));
        quizlet.push_str(&format!("{word}\t{plain}\n"));
        out.cards += 1;
    }
    if out.cards == 0 {
        return Err(if entries.is_empty() { "The lexicon has no entries yet.".into() } else { "No entry has a gloss yet, so no card would have a back.".into() });
    }

    out.path = write_cards(p, folder, &date, &anki, &quizlet, README, &sounds, &pictures)?;
    out.sounds = sounds.len();
    out.pictures = pictures.len();
    Ok(out)
}

/// The cards, their README and media into `wugs-flashcards-<date>[-n]`, whole through a scratch folder.
#[allow(clippy::too_many_arguments)]
fn write_cards(p: &wugs_core::Project, folder: &Path, date: &str, anki: &str, quizlet: &str, readme: &str, sounds: &[(String, String, f64, f64)], pictures: &[(String, std::path::PathBuf)]) -> Result<String, String> {
    let mut target = folder.join(format!("wugs-flashcards-{date}"));
    let mut k = 2;
    while target.symlink_metadata().is_ok() {
        target = folder.join(format!("wugs-flashcards-{date}-{k}"));
        k += 1;
    }
    let scratch = folder.join(format!(".{}.partial", target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        fs::write(scratch.join("anki.txt"), anki).map_err(|e| format!("anki.txt: {e}"))?;
        fs::write(scratch.join("quizlet.txt"), quizlet).map_err(|e| format!("quizlet.txt: {e}"))?;
        fs::write(scratch.join("README.txt"), readme).map_err(|e| format!("README.txt: {e}"))?;
        if !sounds.is_empty() || !pictures.is_empty() {
            fs::create_dir(scratch.join("media")).map_err(|e| format!("media: {e}"))?;
        }
        for (file, from) in pictures {
            fs::copy(from, scratch.join("media").join(file)).map_err(|e| format!("{file}: {e}"))?;
        }
        for (file, blob, start, end) in sounds {
            let hex = wugs_blob::parse_ref(blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?;
            cut_clip(&p.root.join("blobs").join(hex), *start, *end, &scratch.join("media").join(file)).map_err(|e| format!("{file}: {e}"))?;
        }
        fs::rename(&scratch, &target).map_err(|e| format!("{}: {e}", target.display()))
    })();
    if let Err(e) = written {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    Ok(target.display().to_string())
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

        let r = export_flashcards(&mut s, &out, "2026-11-23", false, Order::Alphabet).unwrap();
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
        assert!(root.join("media").join(&file).is_file());
        assert!(fs::read_to_string(root.join("quizlet.txt")).unwrap().lines().any(|l| l == "wʌn\tone"));
        assert!(!anki.contains("zzz"));
        // The recording made restricted: the card stays, its sound does not.
        session::project(&mut s).unwrap().set_access(&text, "restricted").unwrap();
        let src = session::project(&mut s).unwrap().text_recording(&text).unwrap().unwrap().0;
        session::project(&mut s).unwrap().set_access(&src, "restricted").unwrap();
        let again = export_flashcards(&mut s, &out, "2026-11-23", false, Order::Alphabet).unwrap();
        assert!(again.path.ends_with("-2") && again.sounds == 0 && again.cards == r.cards, "{again:?}");
        // Commonest first: the words the texts use, then the rest; with a limit, only the first.
        let uses = uses_by_entry(session::project(&mut s).unwrap()).unwrap();
        assert_eq!(uses.get(&entry), Some(&1), "wʌn is glossed in one word of the tutorial");
        let top = export_flashcards(&mut s, &out, "2026-11-23", false, Order::Commonest { limit: 2 }).unwrap();
        assert_eq!((top.cards, top.used, top.past_limit), (2, Some(2), r.cards - 2), "{top:?}");
        assert_eq!(r.used, None, "A to Z says nothing of uses");
        let all = export_flashcards(&mut s, &out, "2026-11-23", false, Order::Commonest { limit: 0 }).unwrap();
        assert_eq!(all.cards, r.cards);
        let quizlet = fs::read_to_string(Path::new(&all.path).join("quizlet.txt")).unwrap();
        let words: Vec<&str> = quizlet.lines().map(|l| l.split('\t').next().unwrap()).collect();
        let n = all.used.unwrap();
        assert!(words[..n].contains(&"wʌn") && !words[n..].contains(&"wʌn"), "{words:?}");
        // A card per translated sentence, with its stretch of the recording.
        // The tutorial's text shares as restricted, so it is left out unless restricted texts are included.
        assert!(export_flashcards(&mut s, &out, "2026-11-23", false, Order::Sentences).is_err());
        let sentences = export_flashcards(&mut s, &out, "2026-11-23", true, Order::Sentences).unwrap();
        assert!(sentences.sentences && sentences.cards >= 1 && sentences.sounds == sentences.cards, "{sentences:?}");
        let anki = fs::read_to_string(Path::new(&sentences.path).join("anki.txt")).unwrap();
        assert!(anki.lines().any(|l| l.starts_with("wugs-s-") && l.contains("\tBasic\tTutorial English sentences\twʌn tuː θɹiː [sound:")), "{anki}");
        assert!(Path::new(&sentences.path).join("README.txt").is_file());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_picture_goes_with_its_sense_into_every_dictionary() {
        let dir = std::env::temp_dir().join(format!("wugs-pics-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let png = dir.join("one finger.PNG");
        fs::write(&png, b"\x89PNG\r\n\x1a\nnot a real picture").unwrap();
        let txt = dir.join("notes.txt");
        fs::write(&txt, b"x").unwrap();
        let entry = crate::lexicon::lexicon_list(&mut s, "").unwrap().into_iter().find(|r| r.lemma == "wʌn").unwrap().id;
        let sense = crate::lexicon::entry_view(session::project(&mut s).unwrap(), &entry).unwrap().senses[0].id.clone();
        assert!(crate::lexicon::lexicon_set_picture(&mut s, &sense, txt.to_str().unwrap()).unwrap_err().contains("not a picture"));
        let e = crate::lexicon::lexicon_set_picture(&mut s, &sense, png.to_str().unwrap()).unwrap().entry.unwrap();
        let pic = e.senses[0].picture.clone().unwrap();
        assert_eq!(pic.name, "one finger.PNG");
        assert!(pic.path.as_deref().is_some_and(|p| Path::new(p).is_file()), "{pic:?}");

        let printed = crate::dictionary::export_dictionary(&mut s, &out, "2026-11-23", false).unwrap();
        assert_eq!(printed.pictures, 1);
        let page = fs::read_to_string(&printed.path).unwrap();
        assert!(page.contains("<img class=\"pic\" src=\"data:image/png;base64,iVBORw0KGgpub3QgYSByZWFsIHBpY3R1cmU=\" alt=\"one\">"), "{page}");

        let site = crate::website::export_website(&mut s, &out, "2026-11-23", true, false).unwrap();
        assert_eq!(site.pictures, 1);
        assert!(Path::new(&site.path).join("pictures/p-1.png").is_file());
        assert!(fs::read_to_string(Path::new(&site.path).join("dictionary.html")).unwrap().contains("src=\"pictures/p-1.png\""));

        let cards = export_flashcards(&mut s, &out, "2026-11-23", false, Order::Alphabet).unwrap();
        assert_eq!(cards.pictures, 1);
        let file = format!("wugs-{}.png", sense.to_ascii_lowercase());
        assert!(Path::new(&cards.path).join("media").join(&file).is_file());
        assert!(fs::read_to_string(Path::new(&cards.path).join("anki.txt")).unwrap().contains(&format!("<br><img src=\"{file}\">")));

        let e = crate::lexicon::lexicon_clear_picture(&mut s, &sense).unwrap().entry.unwrap();
        assert!(e.senses[0].picture.is_none());
        fs::remove_dir_all(&dir).ok();
    }
}
