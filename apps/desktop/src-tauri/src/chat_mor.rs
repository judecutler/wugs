//! CHAT's `%mor` and `%gra` tiers read as each sentence's tags and
//! dependency tree, so CHILDES transcripts that come with CLAN's MOR and
//! POST analyses show in the Sentence panel, CQP queries, lemma and tag
//! lists and word sketches.
//!
//! A `%mor` item is `pos|stem-SUFFIX&FUSION=gloss` (`n|doggie-PL`,
//! `cop|be&3S`, `n|perro=dog`), clitics joined by `~` or `$`
//! (`pro|it~cop|be&3S`), compounds `n|+n|bath+n|tub`, prefixes `un#adj|happy`.
//! Each clitic part is a word, as in `%gra`, whose `i|head|REL` triples
//! number them with the punctuation. The parts of one written word become
//! a CoNLL-U multi-word token. UPOS comes from the MOR part of speech, which
//! is kept as XPOS; features from the common suffix codes; relations are
//! GRASP's (`subj`, `obj`, `jct`…, lower-cased), with GRASP's heads, not
//! UD's. A sentence whose words do not line up with its items (fillers
//! MOR left out, a `%mor` typed by hand) is left alone and counted.
//! Written as a tagger's analysis (`wugs_conllu::annotate`), so it stays
//! marked until someone checks it.

use std::collections::BTreeMap;

use serde::Serialize;
use wugs_conllu::format::{Cols, Line, Sentence, DEPREL, FEATS, FORM, HEAD, LEMMA, MISC, UPOS, XPOS};

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// The provenance written with each analysis.
pub const BY: &str = "CHAT %mor and %gra";

/// One `%mor` part (between `~` or `$`): part of speech, lemma, features, an English gloss.
#[derive(Debug, PartialEq)]
struct Unit {
    pos: String,
    lemma: String,
    feats: BTreeMap<&'static str, &'static str>,
    gloss: String,
}

/// Features the common MOR suffix and fusion codes stand for.
fn feats_of(code: &str) -> &'static [(&'static str, &'static str)] {
    match code {
        "PL" => &[("Number", "Plur")],
        "SG" => &[("Number", "Sing")],
        "PAST" => &[("Tense", "Past")],
        "PRES" => &[("Tense", "Pres")],
        "1S" => &[("Number", "Sing"), ("Person", "1")],
        "2S" => &[("Number", "Sing"), ("Person", "2")],
        "3S" => &[("Number", "Sing"), ("Person", "3")],
        "1P" => &[("Number", "Plur"), ("Person", "1")],
        "3P" => &[("Number", "Plur"), ("Person", "3")],
        "PRESP" => &[("Tense", "Pres"), ("VerbForm", "Part")],
        "PASTP" | "PERF" => &[("Tense", "Past"), ("VerbForm", "Part")],
        "INF" => &[("VerbForm", "Inf")],
        "CP" => &[("Degree", "Cmp")],
        "SP" => &[("Degree", "Sup")],
        "POSS" => &[("Poss", "Yes")],
        _ => &[],
    }
}

/// A MOR part of speech as a UD tag.
fn upos_of(pos: &str) -> &'static str {
    match pos {
        "n:prop" => return "PROPN",
        "conj:subor" | "comp" => return "SCONJ",
        "det:num" => return "NUM",
        _ => {}
    }
    match pos.split(':').next().unwrap_or("") {
        "n" => "NOUN",
        "v" | "part" => "VERB",
        "cop" | "aux" | "mod" => "AUX",
        "pro" => "PRON",
        "det" | "qn" | "art" | "quant" => "DET",
        "adj" => "ADJ",
        "adv" => "ADV",
        "prep" | "post" => "ADP",
        "conj" | "coord" => "CCONJ",
        "neg" | "inf" | "poss" => "PART",
        "co" | "on" => "INTJ",
        "num" => "NUM",
        "cm" | "bq" | "eq" | "beg" | "end" => "PUNCT",
        _ => "X",
    }
}

/// The stem of `stem-SUFFIX&FUSION`, and its codes.
fn stem_and_codes(main: &str) -> (&str, Vec<&str>) {
    let cut = main.find(['-', '&']).unwrap_or(main.len());
    (&main[..cut], main[cut..].split(['-', '&']).filter(|c| !c.is_empty()).collect())
}

fn unit(part: &str) -> Option<Unit> {
    let (pre, rest) = part.split_once('|')?;
    let pos = pre.rsplit('#').next().unwrap_or(pre).to_string();
    let (main, gloss) = rest.split_once('=').map_or((rest, ""), |(m, g)| (m, g));
    let mut feats = BTreeMap::new();
    let lemma = if let Some(compound) = main.strip_prefix('+') {
        // `+n|bath+n|tub`: the stems joined; the codes of the last part count.
        let parts: Vec<&str> = compound.split('+').filter(|c| !c.is_empty()).map(|c| c.split_once('|').map_or(c, |(_, s)| s)).collect();
        let mut out = String::new();
        for (i, p) in parts.iter().enumerate() {
            let (stem, codes) = stem_and_codes(p);
            out.push_str(stem);
            if i + 1 == parts.len() {
                feats.extend(codes.iter().flat_map(|c| feats_of(c).iter().copied()));
            }
        }
        out
    } else {
        let (stem, codes) = stem_and_codes(main);
        feats.extend(codes.iter().flat_map(|c| feats_of(c).iter().copied()));
        stem.to_string()
    };
    if pos.is_empty() || lemma.is_empty() {
        return None;
    }
    Some(Unit { pos, lemma, feats, gloss: gloss.to_string() })
}

/// An item with no `|` (`.`, `?`, `+...`) or the comma (`cm|cm`) is punctuation.
fn punct_form(item: &str) -> Option<&str> {
    if item == "cm|cm" {
        return Some(",");
    }
    (!item.contains('|')).then_some(item)
}

/// CHAT's placeholders for what could not be made out: no `%mor` item.
fn is_placeholder(w: &str) -> bool {
    matches!(w.to_lowercase().as_str(), "xxx" | "yyy" | "www")
}

/// The parts of a written word: the first lemma's spelling at its start when
/// it starts so (`it's` → `it` + `'s`), the rest after it; a later part with
/// nothing left takes its lemma.
fn split_surface(word: &str, units: &[Unit]) -> Vec<String> {
    if units.len() == 1 {
        return vec![word.to_string()];
    }
    let first = &units[0].lemma;
    let lower = word.to_lowercase();
    let mut out = Vec::new();
    if lower.starts_with(&first.to_lowercase()) && word.len() > first.len() && word.is_char_boundary(first.len()) {
        out.push(word[..first.len()].to_string());
        out.push(word[first.len()..].to_string());
    } else {
        out.push(word.to_string());
    }
    while out.len() < units.len() {
        out.push(units[out.len()].lemma.clone());
    }
    out
}

fn dash(s: &str) -> String {
    if s.is_empty() {
        "_".to_string()
    } else {
        s.to_string()
    }
}

/// One sentence from its written words and its `%mor` (and `%gra`, when it
/// numbers the same parts); None when the words and items do not line up.
pub fn sentence(words: &[String], mor: &str, gra: Option<&str>) -> Option<Sentence> {
    // Each `%mor` part, numbered as `%gra` numbers them, becomes a word; `at` maps that number to its CoNLL-U id.
    let mut lines: Vec<Line> = Vec::new();
    let mut at: Vec<u32> = Vec::new();
    let mut next = 1u32;
    let mut surface = words.iter().peekable();
    let word = |id: u32, form: &str, u: Option<&Unit>| -> Line {
        let mut cols: Cols = Default::default();
        cols[FORM] = dash(form);
        match u {
            Some(u) => {
                cols[LEMMA] = dash(&u.lemma);
                cols[UPOS] = upos_of(&u.pos).to_string();
                cols[XPOS] = dash(&u.pos);
                cols[FEATS] = dash(&u.feats.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("|"));
                cols[MISC] = if u.gloss.is_empty() { "_".to_string() } else { format!("Gloss={}", u.gloss.replace('|', "/")) };
            }
            None => {
                cols[LEMMA] = "_".into();
                cols[UPOS] = if form.chars().all(|c| !c.is_alphanumeric()) { "PUNCT".into() } else { "X".into() };
                cols[XPOS] = "_".into();
                cols[FEATS] = "_".into();
                cols[MISC] = "_".into();
            }
        }
        cols[HEAD] = "_".into();
        cols[DEPREL] = "_".into();
        cols[7] = "_".into();
        Line::Word { id, cols }
    };
    for item in mor.split_whitespace() {
        if let Some(form) = punct_form(item) {
            lines.push(word(next, form, None));
            at.push(next);
            next += 1;
            continue;
        }
        while surface.peek().is_some_and(|w| is_placeholder(w)) {
            lines.push(word(next, surface.next().unwrap(), None));
            next += 1;
        }
        let units: Vec<Unit> = item.split(['~', '$']).filter(|p| !p.is_empty()).map(unit).collect::<Option<_>>()?;
        if units.is_empty() {
            return None;
        }
        let w = surface.next()?;
        let forms = split_surface(w, &units);
        if units.len() > 1 {
            let mut cols: Cols = Default::default();
            cols[FORM] = w.clone();
            for c in cols.iter_mut().skip(1) {
                *c = "_".into();
            }
            lines.push(Line::Range { first: next, last: next + units.len() as u32 - 1, cols });
        }
        for (u, f) in units.iter().zip(&forms) {
            lines.push(word(next, f, Some(u)));
            at.push(next);
            next += 1;
        }
    }
    while surface.peek().is_some_and(|w| is_placeholder(w)) {
        lines.push(word(next, surface.next().unwrap(), None));
        next += 1;
    }
    if surface.next().is_some() {
        return None;
    }
    // `%gra`: `i|head|REL`, numbered over the `%mor` parts with their punctuation.
    if let Some(gra) = gra {
        let triples: Vec<(usize, usize, String)> = gra
            .split_whitespace()
            .filter_map(|t| {
                let mut it = t.splitn(3, '|');
                Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.to_lowercase()))
            })
            .collect();
        let fits = triples.len() == at.len() && triples.iter().enumerate().all(|(i, (n, h, _))| *n == i + 1 && *h <= at.len() && *h != *n);
        if fits {
            for (i, (_, h, rel)) in triples.into_iter().enumerate() {
                let id = at[i];
                let head = if h == 0 { "0".to_string() } else { at[h - 1].to_string() };
                for line in &mut lines {
                    if let Line::Word { id: wid, cols } = line {
                        if *wid == id {
                            cols[HEAD] = head.clone();
                            cols[DEPREL] = rel.clone();
                        }
                    }
                }
            }
        }
    }
    Some(Sentence { comments: Vec::new(), lines })
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MorRead {
    /// Sentences with a `%mor` tier.
    pub with_mor: usize,
    /// Of those, ones with a `%gra` tier too.
    pub with_gra: usize,
    /// Sentences given tags (and a tree when `%gra` lined up).
    pub read: usize,
    /// Sentences that already had an analysis, left alone.
    pub kept: usize,
    /// Sentences whose words did not line up with their `%mor` items.
    pub unaligned: usize,
    pub words: usize,
}

/// A text's sentences with `%mor`: their segments, written words and tiers.
fn gather(session: &mut Session, text: &str) -> Result<(Vec<(String, Vec<String>, String, Option<String>)>, MorRead), String> {
    let p = project(session)?;
    let mut found = Vec::new();
    let mut r = MorRead::default();
    each_segment(p, Read::Words, &Scope::of(Some(text.to_string()), None), |seg| {
        let Some(mor) = p.proj.field_str(&seg.id, "x.chat.mor")?.filter(|m| !m.trim().is_empty()) else {
            return Ok(());
        };
        let gra = p.proj.field_str(&seg.id, "x.chat.gra")?.filter(|g| !g.trim().is_empty());
        r.with_mor += 1;
        r.with_gra += usize::from(gra.is_some());
        found.push((seg.id.clone(), seg.words.clone(), mor, gra));
        Ok(())
    })?;
    Ok((found, r))
}

/// How many of a text's sentences carry `%mor` and `%gra`, writing nothing.
pub fn summary(session: &mut Session, text: &str) -> Result<MorRead, String> {
    Ok(gather(session, text)?.1)
}

/// Read a text's `%mor` and `%gra` tiers into its sentences' analyses.
pub fn read(session: &mut Session, text: &str) -> Result<MorRead, String> {
    let (found, mut r) = gather(session, text)?;
    let mut items = Vec::new();
    for (seg, words, mor, gra) in &found {
        match sentence(words, mor, gra.as_deref()) {
            Some(s) => items.push((seg.clone(), s)),
            None => r.unaligned += 1,
        }
    }
    let p = project(session)?;
    let a = wugs_conllu::annotate::annotate(p, text, &items, BY)?;
    r.read = a.annotated;
    r.kept = a.kept;
    r.unaligned += a.differ;
    r.words = a.words;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;
    use std::fs;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    fn rows(s: &Sentence) -> Vec<String> {
        s.lines
            .iter()
            .map(|l| match l {
                Line::Word { id, cols } => format!("{id} {}", cols.join(" ")),
                Line::Range { first, last, cols } => format!("{first}-{last} {}", cols[FORM]),
                Line::Empty { .. } => String::new(),
            })
            .collect()
    }

    #[test]
    fn items_become_words_with_tags_and_a_tree() {
        let s = sentence(&words("what is that"), "pro:int|what cop|be&3S pro:dem|that ?", Some("1|2|SUBJ 2|0|ROOT 3|2|PRED 4|2|PUNCT")).unwrap();
        assert_eq!(
            rows(&s),
            vec![
                "1 what what PRON pro:int _ 2 subj _ _",
                "2 is be AUX cop Number=Sing|Person=3 0 root _ _",
                "3 that that PRON pro:dem _ 2 pred _ _",
                "4 ? _ PUNCT _ _ 2 punct _ _",
            ]
        );
    }

    #[test]
    fn clitics_compounds_glosses_and_placeholders() {
        let s = sentence(&words("it's a big xxx bathtub"), "pro:per|it~cop|be&3S det:art|a adj|big n|+n|bath+n|tub-PL .", Some("1|2|SUBJ 2|0|ROOT 3|5|DET 4|5|MOD 5|2|PRED 6|2|PUNCT")).unwrap();
        let r = rows(&s);
        assert_eq!(r[0], "1-2 it's");
        assert_eq!(r[1], "1 it it PRON pro:per _ 2 subj _ _");
        assert_eq!(r[2], "2 's be AUX cop Number=Sing|Person=3 0 root _ _");
        assert_eq!(r[5], "5 xxx _ X _ _ _ _ _ _", "a placeholder has no item, so no tags and no head");
        assert_eq!(r[6], "6 bathtub bathtub NOUN n Number=Plur 2 pred _ _", "%gra's fifth part is the sixth word");
        assert_eq!(r[4], "4 big big ADJ adj _ 6 mod _ _");
        let es = sentence(&words("perros"), "n|perro-PL=dog .", None).unwrap();
        assert_eq!(rows(&es)[0], "1 perros perro NOUN n Number=Plur _ _ _ Gloss=dog");
        // Words and items that do not line up are left alone.
        assert!(sentence(&words("uh what"), "pro:int|what ?", None).is_none());
        assert!(sentence(&words("what"), "pro:int|what cop|be&3S ?", None).is_none());
        // A %gra that numbers other parts gives no tree but keeps the tags.
        let s = sentence(&words("what"), "pro:int|what ?", Some("1|0|ROOT")).unwrap();
        assert_eq!(rows(&s)[0], "1 what what PRON pro:int _ _ _ _ _");
    }

    #[test]
    fn a_chat_texts_mor_and_gra_read_into_its_sentences() {
        let dir = std::env::temp_dir().join(format!("wugs-chat-mor-{}", wugs_model::ulid()));
        let mut s: session::Session = None;
        session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let file = dir.join("lunch.cha");
        fs::write(
            &file,
            "@Begin\n*MOT:\twhat is that ?\n%mor:\tpro:int|what cop|be&3S pro:dem|that ?\n%gra:\t1|2|SUBJ 2|0|ROOT 3|2|PRED 4|2|PUNCT\n*CHI:\tdoggies .\n%mor:\tn|doggie-PL .\n*CHI:\tuh doggie .\n%mor:\tn|doggie .\n*MOT:\tyes .\n@End\n",
        )
        .unwrap();
        let text = crate::interchange::import_file(&mut s, file.to_str().unwrap()).unwrap().text.unwrap();
        assert_eq!(summary(&mut s, &text).unwrap(), MorRead { with_mor: 3, with_gra: 1, ..Default::default() });
        let r = read(&mut s, &text).unwrap();
        assert_eq!((r.read, r.unaligned, r.words), (2, 1, 6), "the filler has no item, so that sentence is left alone");
        // The corpus tools now see lemmas and tags.
        let lemma = crate::cqp::run(&mut s, "[lemma=\"be\" & upos=\"AUX\"]", None).unwrap();
        assert_eq!(lemma.total, 1);
        // Reading again keeps what is there.
        let again = read(&mut s, &text).unwrap();
        assert_eq!((again.read, again.kept), (0, 2));
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
