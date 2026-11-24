//! SayMore projects (SIL's tool for managing fieldwork sessions): a
//! `Sessions/<id>/` folder per recording session, holding `<id>.session`
//! (XML: title, date, location, genre, participants, setting, situation,
//! notes…), the recordings, and ELAN files SayMore wrote for its
//! transcriptions (`<recording>.annotations.eaf`). Each session becomes a
//! text: its ELAN file read as any ELAN file is (bringing its recording), or
//! an empty text with its recording when it has no transcription yet, and the
//! session's metadata as the text's details.

use std::path::{Path, PathBuf};

/// One session as found on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct SayMoreSession {
    pub id: String,
    pub title: String,
    /// The session's metadata as text details (`date`, `place`, `genre`, `speakers`, `description`…).
    pub details: Vec<(String, String)>,
    pub eaf: Option<PathBuf>,
    pub audio: Option<PathBuf>,
}

/// SayMore's field names as wugs's detail keys; others keep their own name.
fn detail_key(field: &str) -> &str {
    match field {
        "location" => "place",
        "participants" => "speakers",
        "situation" | "synopsis" => "description",
        other => other,
    }
}

/// A `.session` file's title and its other fields as details (text values
/// only; `contributions` and other structured fields are left out).
pub fn read_session(xml: &str) -> Result<(String, Vec<(String, String)>), String> {
    let root = wugs_xml::parse(xml.trim_start_matches('\u{feff}')).map_err(|e| format!("not a SayMore session file: {e:?}"))?;
    if root.name != "Session" {
        return Err(format!("not a SayMore session file: its root is <{}>", root.name));
    }
    let mut title = String::new();
    let mut details: Vec<(String, String)> = Vec::new();
    for c in &root.children {
        if !c.children.is_empty() {
            continue;
        }
        let value = c.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if value.is_empty() {
            continue;
        }
        match c.name.as_str() {
            "title" => title = value,
            // SayMore's own workflow fields say nothing of the recording.
            "status" | "access" | "stage_consent" | "stages" => {}
            "participants" => {
                let names: Vec<&str> = value.split(';').map(str::trim).filter(|x| !x.is_empty()).collect();
                details.push(("speakers".into(), names.join("; ")));
            }
            name => {
                let key = detail_key(name).to_string();
                match details.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, v)) => {
                        v.push_str(" ");
                        v.push_str(&value);
                    }
                    None => details.push((key, value)),
                }
            }
        }
    }
    Ok((title, details))
}

const AUDIO: &[&str] = &["wav", "mp3", "m4a", "flac", "ogg", "aif", "aiff", "wave", "aac", "oga", "aifc"];

/// The sessions under a SayMore project folder (or its `Sessions` folder, or one session's folder), by id.
pub fn sessions(folder: &Path) -> Result<Vec<SayMoreSession>, String> {
    let dirs: Vec<PathBuf> = if has_session_file(folder) {
        vec![folder.to_path_buf()]
    } else {
        let base = if folder.join("Sessions").is_dir() { folder.join("Sessions") } else { folder.to_path_buf() };
        let mut d: Vec<PathBuf> = std::fs::read_dir(&base).map_err(|e| format!("{}: {e}", base.display()))?.flatten().map(|e| e.path()).filter(|p| p.is_dir() && has_session_file(p)).collect();
        d.sort();
        d
    };
    let mut out = Vec::new();
    for dir in dirs {
        let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
        files.sort();
        let session_file = files.iter().find(|p| p.extension().is_some_and(|x| x == "session")).cloned().ok_or_else(|| format!("{} has no .session file", dir.display()))?;
        let xml = std::fs::read_to_string(&session_file).map_err(|e| format!("{}: {e}", session_file.display()))?;
        let (title, details) = read_session(&xml)?;
        let eaf = files.iter().find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("eaf"))).cloned();
        // SayMore keeps the original and its own copies; the original (named so, else the first recording) is the one to hear.
        let audios: Vec<&PathBuf> = files.iter().filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| AUDIO.contains(&x.to_ascii_lowercase().as_str()))).collect();
        let audio = audios.iter().find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy().to_lowercase().contains("original"))).or(audios.first()).map(|p| (*p).clone());
        out.push(SayMoreSession { title: if title.is_empty() { id.clone() } else { title }, id, details, eaf, audio });
    }
    Ok(out)
}

fn has_session_file(dir: &Path) -> bool {
    std::fs::read_dir(dir).map(|r| r.flatten().any(|e| e.path().extension().is_some_and(|x| x == "session"))).unwrap_or(false)
}

/** What one session became: its text, a recording its ELAN file named, and how. */
pub struct Placed {
    pub text: String,
    pub recording: Option<PathBuf>,
    pub transcribed: bool,
    pub new_empty: bool,
}

/** A session's text: its ELAN file imported (a new text titled as the session), else the text of its
title, made empty when missing. Importing again finds the same text. */
pub fn place(s: &mut crate::session::Session, ses: &SayMoreSession) -> Result<Placed, String> {
    use crate::session;
    if let Some(eaf) = &ses.eaf {
        let r = crate::interchange::import_file(s, &eaf.display().to_string())?;
        let text = r.text.ok_or("the ELAN file made no text")?;
        if r.lines.first().is_some_and(|l| l.starts_with("A new text")) {
            session::project(s)?.set_text_title(&text, &ses.title)?;
        }
        return Ok(Placed { text, recording: r.recording, transcribed: true, new_empty: false });
    }
    if let Some(t) = session::list_texts(s)?.into_iter().find(|t| t.title == ses.title) {
        return Ok(Placed { text: t.id, recording: None, transcribed: false, new_empty: false });
    }
    let text = session::create_text(s, &ses.title)?;
    Ok(Placed { text, recording: None, transcribed: false, new_empty: true })
}

/** The session's details on its text; how many were written (unchanged ones are not). */
pub fn write_details(s: &mut crate::session::Session, text: &str, ses: &SayMoreSession) -> Result<usize, String> {
    let rows: Vec<(String, String, String)> = ses.details.iter().map(|(k, v)| (text.to_string(), k.clone(), v.clone())).collect();
    crate::session::project(s)?.import_text_metadata(&rows).map(|(n, _)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ETR: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Session>
  <title type="string">How we catch fish with poison bark</title>
  <participants type="string">Awi Heole; Ilawi Amosa</participants>
  <genre type="string">procedural</genre>
  <access type="string">Open</access>
  <location type="string">Huya</location>
  <setting type="string">On the front porch</setting>
  <situation type="string">Walking past the plant, I asked how it is done.</situation>
  <date type="string">2010-06-06</date>
  <status type="string">Incoming</status>
  <contributions type="xml"><contributor><name>X</name></contributor></contributions>
</Session>"#;

    #[test]
    fn a_session_file_becomes_a_title_and_details() {
        let (title, details) = read_session(ETR).unwrap();
        assert_eq!(title, "How we catch fish with poison bark");
        assert_eq!(
            details,
            vec![
                ("speakers".to_string(), "Awi Heole; Ilawi Amosa".to_string()),
                ("genre".into(), "procedural".into()),
                ("place".into(), "Huya".into()),
                ("setting".into(), "On the front porch".into()),
                ("description".into(), "Walking past the plant, I asked how it is done.".into()),
                ("date".into(), "2010-06-06".into()),
            ]
        );
        assert!(read_session("<Person/>").is_err());
    }

    #[test]
    fn sessions_are_found_with_their_files() {
        let dir = std::env::temp_dir().join(format!("wugs-saymore-{}", wugs_model::ulid()));
        let s1 = dir.join("Sessions").join("ETR009");
        let s2 = dir.join("Sessions").join("ETR010");
        std::fs::create_dir_all(&s1).unwrap();
        std::fs::create_dir_all(&s2).unwrap();
        std::fs::write(s1.join("ETR009.session"), ETR).unwrap();
        std::fs::write(s1.join("ETR009_Original.wav"), b"RIFF").unwrap();
        std::fs::write(s1.join("ETR009_Source.wav"), b"RIFF").unwrap();
        std::fs::write(s1.join("ETR009_Original.wav.annotations.eaf"), "<ANNOTATION_DOCUMENT/>").unwrap();
        std::fs::write(s2.join("ETR010.session"), "<Session><date>2010-06-07</date></Session>").unwrap();
        std::fs::write(s2.join("ETR010.mp3"), b"ID3").unwrap();
        std::fs::create_dir_all(dir.join("People")).unwrap();
        let found = sessions(&dir).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].audio, Some(s1.join("ETR009_Original.wav")));
        assert_eq!(found[0].eaf, Some(s1.join("ETR009_Original.wav.annotations.eaf")));
        assert_eq!((found[1].title.as_str(), found[1].eaf.is_none()), ("ETR010", true));
        assert_eq!(sessions(&s2).unwrap().len(), 1, "one session's own folder");
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn a_project_is_read_once_however_often_it_is_imported() {
        let dir = std::env::temp_dir().join(format!("wugs-saymore-in-{}", wugs_model::ulid()));
        let s1 = dir.join("Sessions").join("ETR009");
        let s2 = dir.join("Sessions").join("ETR010");
        std::fs::create_dir_all(&s1).unwrap();
        std::fs::create_dir_all(&s2).unwrap();
        std::fs::write(s1.join("ETR009.session"), ETR).unwrap();
        std::fs::write(s1.join("ETR009_Original.wav.annotations.eaf"), include_str!("../../../../crates/wugs-elan/tests/fixtures/sample.eaf")).unwrap();
        std::fs::write(s2.join("ETR010.session"), "<Session><date>2010-06-07</date></Session>").unwrap();
        let mut session: crate::session::Session = None;
        crate::session::create_project(&mut session, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let found = sessions(&dir).unwrap();
        let mut texts = Vec::new();
        for round in 0..2 {
            let placed: Vec<Placed> = found.iter().map(|ses| place(&mut session, ses).unwrap()).collect();
            let written: usize = found.iter().zip(&placed).map(|(ses, p)| write_details(&mut session, &p.text, ses).unwrap()).sum();
            assert_eq!((placed[0].transcribed, placed[1].new_empty), (true, round == 0));
            assert_eq!(written, if round == 0 { 7 } else { 0 }, "details written once");
            texts.push(placed.iter().map(|p| p.text.clone()).collect::<Vec<_>>());
        }
        assert_eq!(texts[0], texts[1], "the second import finds the same texts");
        let titles: Vec<String> = crate::session::list_texts(&mut session).unwrap().into_iter().map(|t| t.title).collect();
        assert_eq!(titles.len(), 2, "{titles:?}");
        assert!(titles.contains(&"How we catch fish with poison bark".to_string()) && titles.contains(&"ETR010".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }
}
