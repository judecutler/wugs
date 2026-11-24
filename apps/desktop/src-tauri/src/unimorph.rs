//! Inflected forms as UniMorph data (https://unimorph.github.io): the
//! format computational morphology shares (the SIGMORPHON tasks, morphological
//! inflection models), a line per form: lemma, form and its features joined
//! by `;`. Two sources: the forms typed into the Lexicon's paradigm grids,
//! and every glossed word of the texts whose root is linked to an entry,
//! its affixes' Leipzig glosses read as UniMorph features.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::concordance::{each_segment, morphs_of, Read, Scope};
use crate::interchange::outside_project;
use crate::session::{self, Session};

/// A Leipzig gloss abbreviation as UniMorph writes it, where they differ.
/// Abbreviations not listed are written upper-cased, as UniMorph mostly
/// takes Leipzig's (SG, PL, PST, NOM, …), and named in the README.
const RENAMED: &[(&str, &str)] = &[
    ("PERF", "PRF"),
    ("INF", "NFIN"),
    ("SBJ", "NOM"),
    ("PRES", "PRS"),
    ("POT", "POT"),
    ("IMPF", "IPFV"),
    ("IMPERF", "IPFV"),
    ("PERFV", "PFV"),
    ("POSS", "PSS"),
    ("PTCP", "V.PTCP"),
    ("CVB", "V.CVB"),
    ("NMLZ", "V.MSDR"),
    ("Q", "INT"),
];

/// The features UniMorph knows (its schema's labels), so the README can name the rest.
const KNOWN: &[&str] = &[
    "1", "2", "3", "SG", "PL", "DU", "TRI", "PAUC", "INCL", "EXCL", "NOM", "ACC", "GEN", "DAT", "ERG", "ABS", "INS", "LOC", "ABL", "ALL", "COM", "VOC", "ESS", "TRANS", "PRIV", "BEN", "PRP", "INTER", "PRS", "PST", "FUT", "NFUT", "IPFV", "PFV", "PRF", "PROG", "HAB", "ITER", "IND", "SBJV", "IMP", "COND", "OPT", "POT", "PURP", "INT", "NEG", "POS", "ACT", "PASS", "MID", "ANTIP", "APPL", "CAUS", "RECP", "REFL", "DEF", "INDF", "NFIN", "V.PTCP", "V.CVB", "V.MSDR", "MASC", "FEM", "NEUT", "AN", "INAN", "PSS", "PROX", "DIST", "EVID", "NEC", "OBLIG", "DECL", "AUX", "DIR", "INV",
];

/// A category as UniMorph's part of speech, when it names one.
fn part_of_speech(category: &str) -> Option<&'static str> {
    let c = category.trim().trim_end_matches('.').to_lowercase();
    Some(match c.as_str() {
        "n" | "noun" | "nom" | "sustantivo" | "nomen" => "N",
        "v" | "verb" | "vb" | "vi" | "vt" | "verbe" | "verbo" => "V",
        "adj" | "adjective" | "a" => "ADJ",
        "adv" | "adverb" => "ADV",
        "pro" | "pron" | "pronoun" => "PRO",
        "det" | "determiner" => "DET",
        "num" | "numeral" => "NUM",
        "adp" | "prep" | "postp" | "preposition" | "postposition" => "ADP",
        _ => return None,
    })
}

/// A Leipzig gloss's grammatical parts as UniMorph features: `3SG.PST` gives
/// 3, SG, PST; a lower-case gloss (a meaning) gives none.
pub fn features_of(gloss: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in gloss.split(['.', ':', ';', '>']).map(str::trim).filter(|p| !p.is_empty()) {
        if part.chars().any(char::is_lowercase) {
            continue;
        }
        // A person digit joined to a number: 3SG, 1PL.
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        let rest = &part[digits.len()..];
        if !digits.is_empty() && digits.len() <= 1 {
            out.push(digits);
        }
        if !rest.is_empty() && rest.chars().all(|c| c.is_alphanumeric()) {
            let up = rest.to_uppercase();
            out.push(RENAMED.iter().find(|(a, _)| *a == up).map_or(up, |(_, b)| (*b).to_string()));
        }
    }
    out
}

/// The paradigm grids' features (`person` 1, `number` sg, `tense` pst) as UniMorph features.
fn grid_features(features: &BTreeMap<String, String>) -> Vec<String> {
    features.values().flat_map(|v| features_of(&v.to_uppercase())).collect()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UniMorph {
    pub path: String,
    pub code: String,
    /// Lines written: from the grids, from the texts.
    pub from_grids: usize,
    pub from_texts: usize,
    /// Glossed words with a linked root but no grammatical gloss, left out.
    pub bare: usize,
    /// Features written that UniMorph's schema does not list.
    pub unknown: Vec<String>,
}

pub fn export(s: &mut Session, folder: &Path, date: &str) -> Result<UniMorph, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let date = crate::corrections::date_or_today(date);
    let lang = p.project_language()?;
    let code = if !lang.iso.trim().is_empty() { lang.iso.trim().to_string() } else if !lang.glottocode.trim().is_empty() { lang.glottocode.trim().to_string() } else { "und".to_string() };

    // (lemma, form, features) once each, in the order met.
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut lines: Vec<(String, String, String)> = Vec::new();
    let mut add = |lemma: &str, form: &str, feats: Vec<String>, pos: Option<&str>, lines: &mut Vec<(String, String, String)>| -> bool {
        let mut all: Vec<String> = pos.map(str::to_string).into_iter().collect();
        for f in feats {
            if !all.contains(&f) {
                all.push(f);
            }
        }
        let row = (lemma.trim().to_string(), form.trim().to_string(), all.join(";"));
        if row.0.is_empty() || row.1.is_empty() || !seen.insert(row.clone()) {
            return false;
        }
        lines.push(row);
        true
    };

    let mut from_grids = 0;
    let mut entry_of: HashMap<String, (String, Option<&'static str>)> = HashMap::new();
    for lex in p.proj.entities_of_kind("lexeme")? {
        if p.proj.is_tombstoned(&lex)? {
            continue;
        }
        let lemma = p.proj.nfc(p.proj.field_str(&lex, "lemma")?.unwrap_or_default().trim());
        let pos = part_of_speech(&p.proj.field_str(&lex, "pos-of-speech")?.unwrap_or_default());
        for f in p.inflected_forms(&lex)? {
            let feats = grid_features(&f.features);
            if !feats.is_empty() && add(&lemma, &f.form, feats, pos, &mut lines) {
                from_grids += 1;
            }
        }
        entry_of.insert(lex, (lemma, pos));
    }

    let (mut from_texts, mut bare) = (0, 0);
    each_segment(p, Read::Analyses, &Scope::all(), |seg| {
        for tok in &seg.tokens {
            let morphs = morphs_of(p, tok)?;
            let Some(root) = morphs.iter().find(|m| m.kind == "root") else { continue };
            let Some(sense) = &root.sense else { continue };
            let Some(lex) = p.proj.field_str(sense, "lexeme")? else { continue };
            let Some((lemma, pos)) = entry_of.get(&lex) else { continue };
            let feats: Vec<String> = morphs.iter().filter(|m| m.kind != "root").flat_map(|m| features_of(&m.gloss)).collect();
            if feats.is_empty() {
                bare += 1;
                continue;
            }
            let form = p.proj.field_str(tok, "form")?.unwrap_or_default();
            let form = p.proj.nfc(form.trim_matches(|c: char| !c.is_alphanumeric()));
            if add(lemma, &form, feats, *pos, &mut lines) {
                from_texts += 1;
            }
        }
        Ok(())
    })?;
    if lines.is_empty() {
        return Err("No inflected forms yet: fill a paradigm grid in the Lexicon, or gloss words whose roots are linked to entries and whose affixes have grammatical glosses (PL, PST, 3SG…).".into());
    }
    let unknown: Vec<String> = lines
        .iter()
        .flat_map(|(_, _, f)| f.split(';').map(str::to_string).collect::<Vec<_>>())
        .filter(|f| !f.is_empty() && !KNOWN.contains(&f.as_str()) && !["N", "V", "ADJ", "ADV", "PRO", "DET", "NUM", "ADP"].contains(&f.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut target = folder.join(format!("wugs-unimorph-{date}"));
    let mut k = 2;
    while target.symlink_metadata().is_ok() {
        target = folder.join(format!("wugs-unimorph-{date}-{k}"));
        k += 1;
    }
    let scratch = folder.join(format!(".{}.partial", target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let name = if lang.name.trim().is_empty() { "the language".to_string() } else { lang.name.trim().to_string() };
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let body: String = lines.iter().map(|(l, f, x)| format!("{l}\t{f}\t{x}\n")).collect();
        fs::write(scratch.join(&code), body).map_err(|e| e.to_string())?;
        let unknown_line = if unknown.is_empty() {
            "Every feature written is one UniMorph's schema lists.".to_string()
        } else {
            format!("These features are not in UniMorph's schema and were written as glossed; rename them before sharing: {}.", unknown.join(", "))
        };
        fs::write(
            scratch.join("README.txt"),
            format!(
                "Inflected forms of {name} in UniMorph's format, made by wugs on {date}.\n\n\
                 The file {code} has a line per form: lemma, form and features joined by `;`\n\
                 (https://unimorph.github.io/schema/). {from_grids} come from the Lexicon's paradigm grids and\n\
                 {from_texts} from glossed words in the texts whose root is linked to a lexicon entry, their\n\
                 affixes' Leipzig glosses read as features (PERF as PRF, INF as NFIN and so on). The part of speech\n\
                 comes first when the entry's category names one. Glossed words with no grammatical gloss ({bare})\n\
                 are left out.\n\n{unknown_line}\n\n\
                 Check that the texts' glossing conventions match UniMorph's before training a model on this.\n"
            ),
        )
        .map_err(|e| e.to_string())?;
        fs::rename(&scratch, &target).map_err(|e| format!("{}: {e}", target.display()))
    })();
    if let Err(e) = written {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    Ok(UniMorph { path: target.display().to_string(), code, from_grids, from_texts, bare, unknown })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leipzig_glosses_become_unimorph_features() {
        assert_eq!(features_of("3SG.PST"), vec!["3", "SG", "PST"]);
        assert_eq!(features_of("PERF"), vec!["PRF"]);
        assert_eq!(features_of("INF"), vec!["NFIN"]);
        assert_eq!(features_of("dog"), Vec::<String>::new(), "a meaning is no feature");
        assert_eq!(features_of("1PL.INCL"), vec!["1", "PL", "INCL"]);
        assert_eq!(part_of_speech("n."), Some("N"));
        assert_eq!(part_of_speech("clitic"), None);
        let mut grid = BTreeMap::new();
        grid.insert("number".to_string(), "pl".to_string());
        grid.insert("person".to_string(), "1".to_string());
        assert_eq!(grid_features(&grid), vec!["PL", "1"]);
    }

    #[test]
    fn grids_and_glossed_words_are_written() {
        let dir = std::env::temp_dir().join(format!("wugs-unimorph-{}", wugs_model::ulid()));
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("p"), "tester").unwrap();
        let p = session::project(&mut s).unwrap();
        let lex = p.proj.entities_of_kind("lexeme").unwrap().into_iter().next().unwrap();
        p.add_inflected_form(&lex, "wʌnz", &[("number", "pl")]).unwrap();
        crate::sample::add_morphology(&mut s).unwrap();
        let r = export(&mut s, &dir.join("out"), "2026-11-24").unwrap();
        assert_eq!(r.from_grids, 1);
        let body = std::fs::read_to_string(std::path::Path::new(&r.path).join(&r.code)).unwrap();
        assert!(body.lines().any(|l| l.ends_with("\twʌnz\tPL") || l.contains("\twʌnz\t") && l.ends_with(";PL")), "{body}");
        assert_eq!(body.lines().count(), r.from_grids + r.from_texts);
        assert!(body.lines().any(|l| l == "kani\tnikani\t1;SG"), "the prefix's 1SG read from the texts: {body}");
        assert!(r.unknown.is_empty() || r.unknown == vec!["FOC".to_string()], "{:?}", r.unknown);
        std::fs::remove_dir_all(&dir).ok();
    }
}
