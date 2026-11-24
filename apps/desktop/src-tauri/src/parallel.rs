//! The project's sentences and their translations as a parallel corpus, the
//! data machine translation and alignment tools are trained on: for each
//! translation language, a tab-separated table, the two sides as
//! line-aligned plain text (Moses and OPUS's form), and a TMX file that
//! translation memories (OmegaT, Trados, Okapi) open. Written whole through
//! a `.partial` scratch folder, never inside the project.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::interchange::{outside_project, translations_of};
use crate::session::{self, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parallel {
    pub path: String,
    /// Sentence pairs written, in all.
    pub pairs: usize,
    pub texts: usize,
    /// Pairs per translation language code, as the files are named.
    pub languages: Vec<(String, usize)>,
    /// Transcribed sentences with no translation, left out.
    pub untranslated: usize,
    /// Texts left out because they are not open.
    pub restricted_left_out: usize,
}

/// A language code as file names and TMX take it: letters, digits and `-`, else `und`.
fn code(raw: &str) -> String {
    let c: String = raw.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    if c.is_empty() { "und".into() } else { c.to_ascii_lowercase() }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

struct Pair {
    id: String,
    text: String,
    source: String,
    target: String,
}

pub fn export_parallel(s: &mut Session, folder: &Path, date: &str, include_restricted: bool) -> Result<Parallel, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let date = crate::corrections::date_or_today(date);
    let lang = p.project_language()?;
    let src = code(if !lang.iso.trim().is_empty() { &lang.iso } else { &lang.glottocode });

    let mut by_target: BTreeMap<String, Vec<Pair>> = BTreeMap::new();
    let mut texts = std::collections::BTreeSet::new();
    let mut restricted = std::collections::BTreeSet::new();
    let mut untranslated = 0;
    let mut access: BTreeMap<String, bool> = BTreeMap::new();
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        let open = match access.get(&seg.text) {
            Some(o) => *o,
            None => {
                let o = include_restricted || p.text_access(&seg.text)? == "open";
                access.insert(seg.text.clone(), o);
                o
            }
        };
        if !open {
            restricted.insert(seg.text.clone());
            return Ok(());
        }
        let content = one_line(&p.proj.field_str(&seg.id, "content")?.unwrap_or_default());
        if content.is_empty() {
            return Ok(());
        }
        let translations = translations_of(p, &seg.id)?;
        if translations.is_empty() {
            untranslated += 1;
            return Ok(());
        }
        texts.insert(seg.text.clone());
        // One pair per translation language: the first translation in it.
        let mut seen = Vec::new();
        for (tl, t) in translations {
            let tl = code(tl.as_deref().unwrap_or(""));
            if seen.contains(&tl) {
                continue;
            }
            seen.push(tl.clone());
            let target = one_line(&t);
            if target.is_empty() {
                continue;
            }
            by_target.entry(tl).or_default().push(Pair { id: seg.id.clone(), text: seg.title.clone(), source: content.clone(), target });
        }
        Ok(())
    })?;
    let pairs: usize = by_target.values().map(Vec::len).sum();
    if pairs == 0 {
        return Err(if restricted.is_empty() { "No sentence has a translation yet, so there is no parallel corpus to write.".into() } else { "No open text has a translated sentence; tick Include texts that are not open to write the others.".into() });
    }

    let mut target_dir = folder.join(format!("wugs-parallel-{date}"));
    let mut k = 2;
    while target_dir.symlink_metadata().is_ok() {
        target_dir = folder.join(format!("wugs-parallel-{date}-{k}"));
        k += 1;
    }
    let scratch = folder.join(format!(".{}.partial", target_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let name = if lang.name.trim().is_empty() { "the language".to_string() } else { lang.name.trim().to_string() };
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let mut listing = String::new();
        for (tl, list) in &by_target {
            let stem = format!("{src}-{tl}");
            // Moses names each side by its language; two unknown codes would collide.
            let (a, b) = if src == *tl { ("src".to_string(), "tgt".to_string()) } else { (src.clone(), tl.clone()) };
            let mut tsv = String::from("id\tsource\ttarget\ttext\n");
            let mut left = String::new();
            let mut right = String::new();
            let mut tmx = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<tmx version=\"1.4\">\n  <header creationtool=\"wugs\" creationtoolversion=\"1\" segtype=\"sentence\" o-tmf=\"wugs\" adminlang=\"en\" srclang=\"{}\" datatype=\"plaintext\" creationdate=\"{}T000000Z\"/>\n  <body>\n",
                xml(&src),
                date.replace('-', "")
            );
            for pair in list {
                let clean = |s: &str| s.replace(['\t', '\r', '\n'], " ");
                tsv.push_str(&format!("{}\t{}\t{}\t{}\n", pair.id, clean(&pair.source), clean(&pair.target), clean(&pair.text)));
                left.push_str(&pair.source);
                left.push('\n');
                right.push_str(&pair.target);
                right.push('\n');
                tmx.push_str(&format!(
                    "    <tu tuid=\"{}\">\n      <prop type=\"x-text\">{}</prop>\n      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n    </tu>\n",
                    xml(&pair.id),
                    xml(&pair.text),
                    xml(&src),
                    xml(&pair.source),
                    xml(tl),
                    xml(&pair.target)
                ));
            }
            tmx.push_str("  </body>\n</tmx>\n");
            fs::write(scratch.join(format!("{stem}.tsv")), tsv).map_err(|e| e.to_string())?;
            fs::write(scratch.join(format!("{stem}.{a}")), left).map_err(|e| e.to_string())?;
            fs::write(scratch.join(format!("{stem}.{b}")), right).map_err(|e| e.to_string())?;
            fs::write(scratch.join(format!("{stem}.tmx")), tmx).map_err(|e| e.to_string())?;
            listing.push_str(&format!("  {stem}.tsv, {stem}.{a} + {stem}.{b}, {stem}.tmx: {} sentence pairs\n", list.len()));
        }
        fs::write(
            scratch.join("README.txt"),
            format!(
                "A parallel corpus of {name} ({src}), made by wugs on {date}: each transcribed sentence with its\n\
                 translation, {pairs} pairs from {} texts.\n\n{listing}\n\
                 The .tsv files hold the segment id, the sentence, its translation and the text's title.\n\
                 The two plain files of a pair have one sentence a line, line for line, as Moses, OPUS and\n\
                 most machine translation toolkits read them. The .tmx files open in OmegaT, Trados and other\n\
                 translation memory tools.\n\n\
                 Before sharing or training on it, check that the speakers' consent covers this use; texts that\n\
                 are not open were {}.\n",
                texts.len(),
                if include_restricted { "included, as asked" } else { "left out" }
            ),
        )
        .map_err(|e| e.to_string())?;
        fs::rename(&scratch, &target_dir).map_err(|e| e.to_string())
    })();
    if let Err(e) = written {
        let _ = fs::remove_dir_all(&scratch);
        return Err(e);
    }
    Ok(Parallel {
        path: target_dir.display().to_string(),
        pairs,
        texts: texts.len(),
        languages: by_target.iter().map(|(k, v)| (k.clone(), v.len())).collect(),
        untranslated,
        restricted_left_out: restricted.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutorial_s_translated_sentences_make_a_parallel_corpus() {
        let dir = std::env::temp_dir().join(format!("wugs-parallel-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        session::project(&mut s).unwrap().set_project_language("Tutorial English", "", "eng").unwrap();
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let r = export_parallel(&mut s, &out, "2026-11-24", false).unwrap();
        assert!(r.path.ends_with("wugs-parallel-2026-11-24"), "{}", r.path);
        assert_eq!(r.pairs, 2, "the tutorial translates two of its sentences: {r:?}");
        let (tl, n) = r.languages[0].clone();
        assert_eq!(n, 2);
        let root = Path::new(&r.path);
        let tsv = fs::read_to_string(root.join(format!("eng-{tl}.tsv"))).unwrap();
        assert!(tsv.starts_with("id\tsource\ttarget\ttext\n") && tsv.contains("wʌn tuː θɹiː\t"), "{tsv}");
        let side = if tl == "eng" { "src".to_string() } else { "eng".to_string() };
        let left = fs::read_to_string(root.join(format!("eng-{tl}.{side}"))).unwrap();
        assert_eq!(left.lines().count(), 2);
        let tmx = fs::read_to_string(root.join(format!("eng-{tl}.tmx"))).unwrap();
        assert!(tmx.contains("<tmx version=\"1.4\">") && tmx.matches("<tu ").count() == 2 && tmx.contains("srclang=\"eng\""), "{tmx}");
        assert!(fs::read_to_string(root.join("README.txt")).unwrap().contains("2 pairs from 1 texts"));
        // A restricted text is left out unless asked for.
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        session::project(&mut s).unwrap().set_access(&text, "restricted").unwrap();
        assert!(export_parallel(&mut s, &out, "2026-11-24", false).is_err());
        let all = export_parallel(&mut s, &out, "2026-11-24", true).unwrap();
        assert!(all.path.ends_with("-2") && all.pairs == 2);
        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
