//! A spelling dictionary for the language, so the community can spell-check
//! in LibreOffice, Firefox, Thunderbird and every other program built on
//! Hunspell: the lexicon's headwords and other forms, and the words the
//! texts use at least twice, written as a Hunspell `.dic` and `.aff` pair
//! with a plain word list beside them. The folder `wugs-spelling-<date>` is
//! written whole through a `.partial` scratch folder, never inside the project.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::interchange::outside_project;
use crate::session::{self, Session};

/// A word from the texts is kept when it is used at least this often (typos rarely are).
pub const MIN_USES: usize = 2;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spelling {
    pub path: String,
    /// Words written to the dictionary.
    pub words: usize,
    /// Of them, from the lexicon.
    pub from_lexicon: usize,
    /// Words met once in the texts and not in the lexicon, left out.
    pub once: usize,
    /// The file names' language code.
    pub code: String,
}

/// How a word is written in the dictionary: in lower case when the texts
/// ever write it so (a capital at a sentence's start is Hunspell's to allow),
/// else its commonest spelling (a name).
fn spelling_of(spellings: &HashMap<String, usize>) -> String {
    let mut best: Vec<(&String, &usize)> = spellings.iter().collect();
    best.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    if let Some((s, _)) = best.iter().find(|(s, _)| s.chars().all(|c| !c.is_uppercase())) {
        return (*s).clone();
    }
    best.first().map(|(s, _)| (*s).clone()).unwrap_or_default()
}

/// The `.aff` file: UTF-8, letters to try in order of use, and the
/// characters other than letters that belong inside words.
pub fn affix_file(words: &[(String, usize)]) -> String {
    let mut letters: HashMap<char, usize> = HashMap::new();
    let mut inside: Vec<char> = Vec::new();
    for (w, n) in words {
        for c in w.chars() {
            if c.is_alphabetic() {
                *letters.entry(c).or_default() += n;
            } else if !inside.contains(&c) && !c.is_whitespace() {
                inside.push(c);
            }
        }
    }
    let mut order: Vec<(char, usize)> = letters.into_iter().collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let try_line: String = order.iter().map(|(c, _)| *c).collect();
    inside.sort_unstable();
    let mut out = String::from("# Made by wugs from the project's lexicon and texts.\nSET UTF-8\n");
    if !try_line.is_empty() {
        out.push_str(&format!("TRY {try_line}\n"));
    }
    if !inside.is_empty() {
        out.push_str(&format!("WORDCHARS {}\n", inside.iter().collect::<String>()));
    }
    out
}

pub fn export_spelling(s: &mut Session, folder: &Path, date: &str) -> Result<Spelling, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let date = crate::corrections::date_or_today(date);
    let lang = p.project_language()?;
    let code = if !lang.iso.trim().is_empty() { lang.iso.trim().to_string() } else if !lang.glottocode.trim().is_empty() { lang.glottocode.trim().to_string() } else { "und".to_string() };

    // Uses and spellings per folded word, from the texts.
    let mut uses: HashMap<String, (usize, HashMap<String, usize>)> = HashMap::new();
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        for w in &seg.words {
            let spelled = p.proj.nfc(w.trim());
            let key = wugs_model::fold_key(&spelled);
            if key.is_empty() || !spelled.chars().any(char::is_alphabetic) || spelled.contains('/') {
                continue;
            }
            let e = uses.entry(key).or_default();
            e.0 += 1;
            *e.1.entry(spelled).or_default() += 1;
        }
        Ok(())
    })?;
    // The lexicon's headwords and other forms, kept whatever their use.
    let mut lexicon: HashMap<String, String> = HashMap::new();
    for id in p.proj.entities_of_kind("lexeme")? {
        if p.proj.is_tombstoned(&id)? {
            continue;
        }
        let mut forms = vec![p.proj.field_str(&id, "lemma")?.unwrap_or_default()];
        forms.extend(p.allomorphs(&id)?.into_iter().map(|(_, surface)| surface));
        for f in forms {
            // Affixes and clitics (`-ler`, `ni-`, `=ma`) are not words a spell checker meets.
            let f = p.proj.nfc(f.trim());
            if f.is_empty() || f.starts_with(['-', '=', '<']) || f.ends_with(['-', '=', '>']) || f.contains(char::is_whitespace) || f.contains('/') {
                continue;
            }
            lexicon.entry(wugs_model::fold_key(&f)).or_insert(f);
        }
    }
    let mut words: Vec<(String, usize)> = Vec::new();
    let mut once = 0;
    let mut from_lexicon = 0;
    for (key, (n, spellings)) in &uses {
        if lexicon.contains_key(key) {
            continue;
        }
        if *n < MIN_USES {
            once += 1;
            continue;
        }
        words.push((spelling_of(spellings), *n));
    }
    for (key, form) in &lexicon {
        from_lexicon += 1;
        let n = uses.get(key).map_or(1, |u| u.0);
        // A lexicon form in lower case stays so; a capitalised headword takes the texts' lower case when they have it.
        let written = match uses.get(key) {
            Some((_, sp)) if form.chars().any(char::is_uppercase) => spelling_of(sp),
            _ => form.clone(),
        };
        words.push((written, n));
    }
    words.sort_by(|a, b| wugs_model::fold_key(&a.0).cmp(&wugs_model::fold_key(&b.0)).then(a.0.cmp(&b.0)));
    words.dedup_by(|a, b| a.0 == b.0);
    if words.is_empty() {
        return Err("The lexicon and texts have no words to make a spelling dictionary from yet.".into());
    }

    let mut target = folder.join(format!("wugs-spelling-{date}"));
    let mut k = 2;
    while target.symlink_metadata().is_ok() {
        target = folder.join(format!("wugs-spelling-{date}-{k}"));
        k += 1;
    }
    let scratch = folder.join(format!(".{}.partial", target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let name = if lang.name.trim().is_empty() { "the language".to_string() } else { lang.name.trim().to_string() };
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let mut dic = format!("{}\n", words.len());
        for (w, _) in &words {
            dic.push_str(w);
            dic.push('\n');
        }
        fs::write(scratch.join(format!("{code}.dic")), dic).map_err(|e| e.to_string())?;
        fs::write(scratch.join(format!("{code}.aff")), affix_file(&words)).map_err(|e| e.to_string())?;
        let list: String = words.iter().map(|(w, n)| format!("{w}\t{n}\n")).collect();
        fs::write(scratch.join("words.tsv"), list).map_err(|e| e.to_string())?;
        fs::write(
            scratch.join("README.txt"),
            format!(
                "A spelling dictionary for {name}, made by wugs on {date}.\n\n\
                 {code}.dic and {code}.aff are a Hunspell dictionary: the spell checker of LibreOffice, Firefox,\n\
                 Thunderbird, Chrome on Linux and many other programs. It holds the lexicon's headwords and other\n\
                 forms ({from_lexicon}) and every word the texts use at least {MIN_USES} times, {} words in all. Words\n\
                 met only once and not in the lexicon ({once}) are left out, since many are slips.\n\n\
                 To use it in LibreOffice, put both files where LibreOffice keeps dictionaries (or package them\n\
                 as an extension) and choose the language in Tools > Language. Programs that take a plain word\n\
                 list (Word's custom dictionaries, keyboard apps) can read words.tsv: a word and its uses a line.\n\n\
                 Made again when the lexicon and texts have grown, it knows more words.\n",
                words.len()
            ),
        )
        .map_err(|e| e.to_string())?;
        fs::rename(&scratch, &target).map_err(|e| format!("{}: {e}", target.display()))
    })();
    if let Err(e) = written {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    Ok(Spelling { path: target.display().to_string(), words: words.len(), from_lexicon, once, code })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_aff_tries_letters_by_use_and_keeps_inner_marks() {
        let aff = affix_file(&[("taʼa".into(), 3), ("kan-ta".into(), 1)]);
        assert!(aff.starts_with("# Made by wugs"));
        // ʼ is a modifier letter, so a letter to try; the hyphen belongs inside words.
        assert!(aff.contains("SET UTF-8\nTRY atʼkn\n"), "{aff}");
        assert!(aff.contains("WORDCHARS -\n"), "{aff}");
    }

    #[test]
    fn lower_case_wins_unless_a_word_is_always_capitalised() {
        let m = |pairs: &[(&str, usize)]| pairs.iter().map(|(s, n)| (s.to_string(), *n)).collect::<HashMap<_, _>>();
        assert_eq!(spelling_of(&m(&[("Kuri", 5), ("kuri", 2)])), "kuri");
        assert_eq!(spelling_of(&m(&[("Kelani", 4)])), "Kelani");
    }

    #[test]
    fn the_tutorial_makes_a_dictionary() {
        let dir = std::env::temp_dir().join(format!("wugs-spelling-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "tester").unwrap();
        let p = session::project(&mut s).unwrap();
        let tier = session::find_utterance_tier(p).unwrap().unwrap();
        p.import_written_text("More", &tier, &["Kuri mopa", "kuri tela", "slipp"]).unwrap();
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let r = export_spelling(&mut s, &out, "2026-11-24").unwrap();
        assert!(r.path.ends_with("wugs-spelling-2026-11-24"), "{}", r.path);
        assert_eq!(r.code, "und");
        let dic = fs::read_to_string(Path::new(&r.path).join("und.dic")).unwrap();
        let lines: Vec<&str> = dic.lines().collect();
        assert_eq!(lines[0].parse::<usize>().unwrap(), lines.len() - 1, "the count heads the list");
        assert!(lines.contains(&"kuri"), "used twice, in lower case: {dic}");
        assert!(!lines.contains(&"Kuri"));
        assert!(!lines.contains(&"slipp") && !lines.contains(&"mopa"), "met once: {dic}");
        assert!(r.from_lexicon >= 3, "the tutorial's entries: {r:?}");
        assert!(Path::new(&r.path).join("und.aff").is_file() && Path::new(&r.path).join("README.txt").is_file());
        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
