//! IMDI session files (the metadata of The Language Archive, ELAR's older
//! deposits, DoBeS and many older corpora): each `<Session>` names its
//! title, date, place, genre, people and the files that belong to it (an
//! ELAN transcription, a recording). Read into the same shape as a SayMore
//! session, so archived material comes back as texts with their recordings
//! and details through one import.

use std::path::{Path, PathBuf};

use wugs_xml::XmlNode as Element;

use crate::saymore::SayMoreSession;

/// Values IMDI writes for "not filled in".
fn filled(v: &str) -> bool {
    let v = v.trim();
    !v.is_empty() && !["unspecified", "unknown", "unspecified.", "n/a"].contains(&v.to_lowercase().as_str())
}

fn text_of(e: &Element) -> String {
    e.text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn child<'a>(e: &'a Element, name: &str) -> Option<&'a Element> {
    e.children.iter().find(|c| c.name.rsplit(':').next() == Some(name))
}

fn children<'a>(e: &'a Element, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
    e.children.iter().filter(move |c| c.name.rsplit(':').next() == Some(name))
}

fn value(e: &Element, path: &[&str]) -> Option<String> {
    let mut at = e;
    for p in path {
        at = child(at, p)?;
    }
    Some(text_of(at)).filter(|v| filled(v))
}

/// Every `<Session>` in an IMDI file, each with its links as written.
pub fn read(xml: &str) -> Result<Vec<(String, String, Vec<(String, String)>, Vec<(String, String)>)>, String> {
    let root = wugs_xml::parse(xml.trim_start_matches('\u{feff}')).map_err(|e| format!("not an IMDI file: {e:?}"))?;
    if root.name.rsplit(':').next() != Some("METATRANSCRIPT") {
        return Err(format!("not an IMDI file: its root is <{}>", root.name));
    }
    let mut out = Vec::new();
    for ses in children(&root, "Session") {
        let name = value(ses, &["Name"]).unwrap_or_default();
        let title = value(ses, &["Title"]).unwrap_or_else(|| name.clone());
        let mut details: Vec<(String, String)> = Vec::new();
        let mut put = |k: &str, v: Option<String>| {
            if let Some(v) = v {
                details.push((k.to_string(), v));
            }
        };
        put("date", value(ses, &["Date"]));
        put("description", children(ses, "Description").map(text_of).find(|v| filled(v)));
        if let Some(md) = child(ses, "MDGroup") {
            if let Some(loc) = child(md, "Location") {
                // Most specific first: an address, the region, the country.
                let parts: Vec<String> = ["Address", "Region", "Country"].iter().filter_map(|k| value(loc, &[k])).collect();
                put("place", Some(parts.join(", ")).filter(|p| !p.is_empty()));
            }
            put("project", value(md, &["Project", "Title"]).or_else(|| value(md, &["Project", "Name"])));
            if let Some(content) = child(md, "Content") {
                put("genre", value(content, &["Genre"]));
                put("subgenre", value(content, &["SubGenre"]));
                put("task", value(content, &["Task"]));
                put("setting", value(content, &["CommunicationContext", "EventStructure"]));
                if let Some(keys) = child(content, "Keys") {
                    for k in children(keys, "Key") {
                        if let Some(n) = k.attr("Name") {
                            if let Some(key) = wugs_core::metadata::key_of(&n) {
                                put(&key, Some(text_of(k)).filter(|v| filled(v)));
                            }
                        }
                    }
                }
            }
            if let Some(actors) = child(md, "Actors") {
                let mut speakers = Vec::new();
                let mut others = Vec::new();
                for a in children(actors, "Actor") {
                    let Some(who) = value(a, &["FullName"]).or_else(|| value(a, &["Name"])) else { continue };
                    let role = value(a, &["Role"]).unwrap_or_default().to_lowercase();
                    if role.contains("speaker") || role.contains("consultant") || role.contains("informant") || role.contains("singer") || role.is_empty() {
                        speakers.push(who);
                    } else {
                        others.push(format!("{who} ({role})"));
                    }
                }
                put("speakers", Some(speakers.join("; ")).filter(|v| !v.is_empty()));
                put("participants", Some(others.join("; ")).filter(|v| !v.is_empty()));
            }
        }
        let mut links: Vec<(String, String)> = Vec::new();
        if let Some(res) = child(ses, "Resources") {
            for r in res.children.iter() {
                let kind = r.name.rsplit(':').next().unwrap_or_default().to_string();
                if let Some(link) = value(r, &["ResourceLink"]) {
                    let format = value(r, &["Format"]).unwrap_or_default();
                    links.push((format!("{kind} {format}"), link));
                }
            }
        }
        out.push((name, title, details, links));
    }
    if out.is_empty() {
        return Err("this IMDI file describes no session (it may be a corpus node)".into());
    }
    Ok(out)
}

/// Where a ResourceLink points: a path below the IMDI file's folder (never
/// `..` or absolute), else a file of that name beside it, which is where an
/// archive download or a copied corpus puts it.
fn resolve(base: &Path, link: &str) -> Option<PathBuf> {
    let path = crate::interchange::url_path(link).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| link.to_string());
    let rel = Path::new(&path);
    if rel.is_relative() && !rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        let p = base.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    let name = path.rsplit(['/', '\\']).next()?;
    let p = base.join(name);
    p.is_file().then_some(p)
}

const AUDIO: &[&str] = &["wav", "mp3", "m4a", "flac", "ogg", "aif", "aiff", "wave"];

/// Every session of the IMDI files in `folder` and the folders below it (to three levels).
pub fn sessions(folder: &Path) -> Result<Vec<SayMoreSession>, String> {
    let mut files = Vec::new();
    collect(folder, 0, &mut files);
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let xml = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let Ok(found) = read(&xml) else { continue };
        let base = f.parent().unwrap_or(Path::new("."));
        for (name, title, details, links) in found {
            let id = if name.is_empty() { f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() } else { name };
            let found_of = |want: &dyn Fn(&Path) -> bool| links.iter().filter_map(|(_, l)| resolve(base, l)).find(|p| want(p));
            let ext = |p: &Path| p.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
            let eaf = found_of(&|p| ext(p) == "eaf");
            let audio = found_of(&|p| AUDIO.contains(&ext(p).as_str()));
            out.push(SayMoreSession { title: if title.is_empty() { id.clone() } else { title }, id, details, eaf, audio });
        }
    }
    Ok(out)
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && depth < 3 {
            collect(&p, depth + 1, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("imdi")) {
            out.push(p);
        }
    }
}

/// Whether `folder` holds IMDI files (to three levels down).
pub fn has_imdi(folder: &Path) -> bool {
    let mut files = Vec::new();
    collect(folder, 0, &mut files);
    !files.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<METATRANSCRIPT xmlns="http://www.mpi.nl/IMDI/Schema/IMDI" Type="SESSION" Version="0">
  <Session>
    <Name>kanu_fish_01</Name>
    <Title>How we catch fish</Title>
    <Date>2004-05-02</Date>
    <Description LanguageId="ISO639-3:eng">A procedure told on the porch.</Description>
    <MDGroup>
      <Location><Continent>Africa</Continent><Country>Ghana</Country><Region>Volta</Region><Address>Unspecified</Address></Location>
      <Project><Name>KANU</Name><Title>Kanu documentation</Title></Project>
      <Keys/>
      <Content>
        <Genre>Procedural</Genre><SubGenre>Unspecified</SubGenre><Task>Unknown</Task>
        <Keys><Key Name="Topic">fishing</Key></Keys>
      </Content>
      <Actors>
        <Actor><Role>Speaker</Role><Name>AH</Name><FullName>Awi Heole</FullName></Actor>
        <Actor><Role>Recorder</Role><Name>JD</Name><FullName>Jo Doe</FullName></Actor>
      </Actors>
    </MDGroup>
    <Resources>
      <MediaFile><ResourceLink>media/kanu_fish_01.wav</ResourceLink><Type>audio</Type><Format>audio/x-wav</Format></MediaFile>
      <WrittenResource><ResourceLink>https://archive.example/files/kanu_fish_01.eaf</ResourceLink><Type>Annotation</Type><Format>text/x-eaf+xml</Format></WrittenResource>
    </Resources>
  </Session>
</METATRANSCRIPT>"#;

    #[test]
    fn a_session_is_read_into_details_and_its_files_found() {
        let dir = std::env::temp_dir().join(format!("wugs-imdi-{}", wugs_model::ulid()));
        std::fs::create_dir_all(dir.join("corpus").join("media")).unwrap();
        std::fs::write(dir.join("corpus").join("kanu_fish_01.imdi"), SESSION).unwrap();
        std::fs::write(dir.join("corpus").join("media").join("kanu_fish_01.wav"), b"RIFF").unwrap();
        std::fs::write(dir.join("corpus").join("kanu_fish_01.eaf"), "<ANNOTATION_DOCUMENT/>").unwrap();
        std::fs::write(dir.join("corpus.imdi"), r#"<METATRANSCRIPT Type="CORPUS"><Corpus><Name>c</Name></Corpus></METATRANSCRIPT>"#).unwrap();
        assert!(has_imdi(&dir));
        let found = sessions(&dir).unwrap();
        assert_eq!(found.len(), 1, "the corpus node is no session");
        let s = &found[0];
        assert_eq!((s.id.as_str(), s.title.as_str()), ("kanu_fish_01", "How we catch fish"));
        assert_eq!(s.audio, Some(dir.join("corpus").join("media").join("kanu_fish_01.wav")));
        assert_eq!(s.eaf, Some(dir.join("corpus").join("kanu_fish_01.eaf")), "a URL is found by its name beside the file");
        let get = |k: &str| s.details.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str());
        assert_eq!(get("place"), Some("Volta, Ghana"));
        assert_eq!(get("speakers"), Some("Awi Heole"));
        assert_eq!(get("participants"), Some("Jo Doe (recorder)"));
        assert_eq!((get("genre"), get("subgenre"), get("topic")), (Some("Procedural"), None, Some("fishing")));
        assert_eq!((get("date"), get("project")), (Some("2004-05-02"), Some("Kanu documentation")));
        assert!(read("<Session/>").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
