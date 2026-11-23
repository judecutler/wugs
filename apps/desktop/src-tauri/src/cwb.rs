//! The whole project as a corpus for the Corpus Workbench (SCROLLS.md step
//! 5: "a project can also be exported to CWB and searched with the real
//! `cqp`"): one vertical file with every text, the commands that encode it
//! (`cwb-encode`, `cwb-make`), and a README saying how CQPweb, NoSketch
//! Engine and Sketch Engine take it. wugs does not run CWB; it hands over
//! a corpus those tools load as their own.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::interchange::{one_track, outside_project, translations_of};
use crate::session::{self, Session};

/// What the export wrote.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusExported {
    pub path: String,
    /// The corpus id CWB knows it by.
    pub corpus: String,
    pub texts: usize,
    pub sentences: usize,
    /// Texts that are not open, left out (unless included).
    pub restricted_left_out: usize,
    pub notes: Vec<String>,
}

/// A CWB attribute name: ASCII lower-case letters, digits and `_`, a letter first.
fn cwb_name(key: &str) -> bool {
    key.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The corpus id: `WUGS`, or the project language's ISO code before it (`TUR_WUGS`).
fn corpus_id(iso: &str) -> String {
    if !iso.is_empty() && iso.chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{}_WUGS", iso.to_ascii_uppercase())
    } else {
        "WUGS".to_string()
    }
}

pub fn export_corpus(s: &mut Session, folder: &Path, date: &str, include_restricted: bool) -> Result<CorpusExported, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let mut texts: Vec<(String, String)> = p.proj.entities_of_kind("text")?.into_iter().map(|t| Ok((p.proj.field_str(&t, "title")?.unwrap_or_default(), t))).collect::<Result<_, String>>()?;
    texts.sort();
    let mut vert = String::new();
    let mut notes: Vec<String> = Vec::new();
    let (mut written, mut sentences, mut left_out) = (0usize, 0usize, 0usize);
    let mut keys: std::collections::BTreeSet<String> = Default::default();
    for (title, text) in &texts {
        if !include_restricted && p.text_access(text)? != "open" {
            left_out += 1;
            continue;
        }
        let mut text_notes = Vec::new();
        let segments = one_track(p, text, &mut text_notes, "CWB")?;
        if segments.is_empty() {
            continue;
        }
        let es = segments.into_iter().map(|segment| Ok(wugs_conllu::ExportSentence { translations: translations_of(p, &segment)?, segment })).collect::<Result<Vec<_>, String>>()?;
        let out = wugs_conllu::vertical(p, title, text, &es)?;
        sentences += es.len();
        vert.push_str(&out.conllu);
        for n in text_notes.into_iter().chain(out.notes) {
            let n = format!("«{title}»: {n}");
            if !notes.contains(&n) {
                notes.push(n);
            }
        }
        keys.extend(p.text_metadata(text)?.into_iter().map(|(k, _)| k));
        written += 1;
    }
    if written == 0 {
        return Err(if left_out > 0 { "Every text with sentences is restricted; tick the box to include them.".into() } else { "No text has sentences to export yet.".into() });
    }
    let unnamed: Vec<&String> = keys.iter().filter(|k| !cwb_name(k) || *k == "id" || *k == "title").collect();
    if !unnamed.is_empty() {
        notes.push(format!("CWB attribute names are ASCII, so these details are in the file but not declared in encode.sh: {}.", unnamed.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")));
    }
    let text_attrs: Vec<&str> = std::iter::once("id").chain(std::iter::once("title")).chain(keys.iter().map(String::as_str).filter(|k| cwb_name(k) && *k != "id" && *k != "title")).collect();
    let lang = p.project_language()?;
    let id = corpus_id(&lang.iso);
    let lower = id.to_lowercase();
    let name = format!("{lower}.vert");
    let encode = format!(
        "#!/bin/sh\n\
         # Encode {name} as the CWB corpus {id} (written by wugs).\n\
         # Needs the IMS Open Corpus Workbench (cwb.sourceforge.io). Set REGISTRY\n\
         # to your CWB registry folder (CQPweb's, or /usr/local/share/cwb/registry).\n\
         set -e\n\
         REGISTRY=\"${{REGISTRY:-/usr/local/share/cwb/registry}}\"\n\
         DATA=\"${{DATA:-$(pwd)/data}}\"\n\
         mkdir -p \"$DATA\"\n\
         cwb-encode -c utf8 -x -s -B -d \"$DATA\" -f {name} -R \"$REGISTRY/{lower}\" \\\n\
         \x20 -P lemma -P upos -P xpos -P feats \\\n\
         \x20 -S text:0+{text} \\\n\
         \x20 -S s:0+id+translation \\\n\
         \x20 -S g:0\n\
         cwb-make -r \"$REGISTRY\" -V {id}\n\
         echo 'Done: cqp -r \"'\"$REGISTRY\"'\" then {id};'\n",
        text = text_attrs.join("+"),
    );
    let readme = format!(
        "# {id}: a corpus from wugs\n\n\
         {written} {texts_word}, {sentences} {sentences_word}, exported {date}.{left}\n\n\
         `{name}` is a CWB vertical file: one token a line with five columns (word, lemma, UPOS, XPOS, features; `_` when empty), \
         `<s id translation>` around each sentence, `<text id title …>` around each text with its details as attributes, \
         and `<g/>` where no space follows.\n\n\
         ## Load it\n\n\
         - **CQPweb or the command-line `cqp`:** run `sh encode.sh` (set `REGISTRY` to your registry folder first), then install the corpus in CQPweb's admin pages, or open it with `cqp`.\n\
         - **NoSketch Engine:** the same vertical file; write a corpus configuration naming the attributes in `encode.sh`.\n\
         - **Sketch Engine:** upload `{name}` as a vertical file; its \"glue\" structure is `<g/>`.\n\n\
         Lemmas and tags are what the project holds: imported from CoNLL-U or TEI, written by a tagger in wugs, corrected by hand, or, \
         for words glossed in wugs, the root's lexicon entry. Words with no tag show `_`.\n",
        texts_word = if written == 1 { "text" } else { "texts" },
        sentences_word = if sentences == 1 { "sentence" } else { "sentences" },
        date = crate::corrections::date_or_today(date),
        left = if left_out > 0 { format!(" {left_out} {} that {} not open {} left out.", if left_out == 1 { "text" } else { "texts" }, if left_out == 1 { "is" } else { "are" }, if left_out == 1 { "was" } else { "were" }) } else if include_restricted { " Texts that are not open are included: treat this folder as restricted.".to_string() } else { String::new() },
    );
    let date = crate::corrections::date_or_today(date);
    let mut out = folder.join(format!("wugs-cwb-{date}"));
    let mut n = 2;
    while out.symlink_metadata().is_ok() {
        out = folder.join(format!("wugs-cwb-{date}-{n}"));
        n += 1;
    }
    // Written whole into a scratch folder beside it, then renamed, so a failure leaves nothing half-written.
    let scratch = folder.join(format!(".{}.partial", out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let files = [(name.as_str(), vert.as_str()), ("encode.sh", encode.as_str()), ("README.md", readme.as_str())];
    let result = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        for (file, contents) in files {
            fs::write(scratch.join(file), contents).map_err(|e| format!("{file}: {e}"))?;
        }
        fs::rename(&scratch, &out).map_err(|e| format!("{}: {e}", out.display()))
    })();
    if let Err(e) = result {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    Ok(CorpusExported { path: out.display().to_string(), corpus: id, texts: written, sentences, restricted_left_out: left_out, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_becomes_one_cwb_corpus() {
        let dir = std::env::temp_dir().join(format!("wugs-cwb-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let p = session::project(&mut s).unwrap();
        let tier = session::utterance_tier(p).unwrap();
        let other = wugs_conllu::import_conllu(p, "# text = Vamos.\n1\tVamos\tir\tVERB\t_\tMood=Imp\t0\troot\t_\tSpaceAfter=No\n2\t.\t.\tPUNCT\t_\t_\t1\tpunct\t_\t_\n\n", "playa", &tier).unwrap().texts[0].clone();
        p.set_text_metadata(&other, "Genre", "talk").unwrap();
        p.set_text_metadata(&other, "Ort", "Köln").unwrap();
        p.set_text_metadata(&other, "Région", "x").unwrap();
        p.set_project_language("Kanu", "", "kan").unwrap();
        let hidden = p.create_text("Secret").unwrap();
        p.add_segment(&hidden, &tier, 0, None, None, "not for the web").unwrap();
        p.set_access(&hidden, "restricted").unwrap();
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let r = export_corpus(&mut s, &out, "2026-11-23", false).unwrap();
        assert_eq!((r.corpus.as_str(), r.texts, r.restricted_left_out), ("KAN_WUGS", 2, 1), "{r:?}");
        let folder = Path::new(&r.path);
        let vert = fs::read_to_string(folder.join("kan_wugs.vert")).unwrap();
        assert!(vert.contains("<text id=\"") && vert.contains("genre=\"talk\"") && vert.contains("Vamos\tir\tVERB\t_\tMood=Imp\n<g/>\n.\t.\tPUNCT"), "{vert}");
        assert!(!vert.contains("not for the web"), "restricted texts stay out");
        let encode = fs::read_to_string(folder.join("encode.sh")).unwrap();
        assert!(encode.contains("-S text:0+id+title+genre+ort \\") && encode.contains("cwb-make -r \"$REGISTRY\" -V KAN_WUGS"), "{encode}");
        assert!(r.notes.iter().any(|n| n.contains("région")), "a non-ASCII key is named: {:?}", r.notes);
        #[cfg(unix)]
        assert!(std::process::Command::new("sh").arg("-n").arg(folder.join("encode.sh")).status().unwrap().success(), "encode.sh is valid shell");
        let all = export_corpus(&mut s, &out, "2026-11-23", true).unwrap();
        assert!(all.path.ends_with("wugs-cwb-2026-11-23-2") && all.texts == 3);
        assert!(export_corpus(&mut s, &dir.join("sample"), "2026-11-23", false).is_err(), "never inside the project");
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
