//! Texts from Wikipedia (roadmap 75's first piece): for a language with a
//! Wikipedia of its own, a written corpus to start from. Articles named by
//! title, or picked at random, are read as plain text through the
//! MediaWiki API (TextExtracts), off the session thread, then each comes in
//! as a text cut like a plain-text file (`plaintext::segments`), with its
//! address, licence (CC BY-SA 4.0), authors ("Wikipedia contributors"),
//! publisher and the day it was read as details, so the corpus says where it
//! came from. An article already here (same title and sentences) adds
//! nothing. Wikimedia asks for a User-Agent naming the program; every
//! request sends one.

use serde::{Deserialize, Serialize};

use crate::interchange::plural;
use crate::session::{self, Session};

/// At most this many articles in one go.
pub const MAX_ARTICLES: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    pub title: String,
    pub url: String,
    pub text: String,
}

/// A Wikipedia's code as its address has it: `sw`, `zh-yue`, `simple`.
pub fn valid_wiki(code: &str) -> bool {
    (2..=16).contains(&code.len()) && code.starts_with(|c: char| c.is_ascii_lowercase()) && code.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The site a code names.
pub fn base_of(wiki: &str) -> Result<String, String> {
    if !valid_wiki(wiki) {
        return Err(format!("«{wiki}» is not a Wikipedia's code; it is the start of its address, such as sw for sw.wikipedia.org"));
    }
    Ok(format!("https://{wiki}.wikipedia.org"))
}

fn agent(base: &str) -> ureq::Agent {
    use std::time::Duration;
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .https_only(!base.starts_with("http://127.0.0.1:"))
        .user_agent(concat!("wugs/", env!("CARGO_PKG_VERSION"), " (https://github.com/judecutler/wugs)"))
        .build()
        .into()
}

fn get_json(base: &str, query: &[(&str, &str)]) -> Result<serde_json::Value, String> {
    let mut req = agent(base).get(format!("{base}/w/api.php"));
    for (k, v) in query {
        req = req.query(*k, *v);
    }
    let mut r = req.call().map_err(|e| format!("Could not reach {base} ({e})."))?;
    let body = r.body_mut().read_to_string().map_err(|e| format!("{base}'s answer could not be read: {e}"))?;
    serde_json::from_str(&body).map_err(|_| format!("{base} answered something wugs cannot read."))
}

/// `n` article titles picked at random from the main namespace.
pub fn random_titles(base: &str, n: usize) -> Result<Vec<String>, String> {
    let limit = n.clamp(1, MAX_ARTICLES).to_string();
    let v = get_json(base, &[("action", "query"), ("list", "random"), ("rnnamespace", "0"), ("rnlimit", &limit), ("format", "json"), ("formatversion", "2")])?;
    Ok(v["query"]["random"].as_array().into_iter().flatten().filter_map(|r| r["title"].as_str().map(str::to_string)).collect())
}

/// One article as plain text, or None when the wiki has no such page.
pub fn article(base: &str, title: &str) -> Result<Option<Article>, String> {
    let v = get_json(
        base,
        &[
            ("action", "query"),
            ("prop", "extracts|info"),
            ("explaintext", "1"),
            ("exsectionformat", "plain"),
            ("inprop", "url"),
            ("redirects", "1"),
            ("titles", title),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )?;
    let Some(page) = v["query"]["pages"].as_array().and_then(|p| p.first()) else { return Ok(None) };
    if page.get("missing").is_some() || page.get("invalid").is_some() {
        return Ok(None);
    }
    let text = page["extract"].as_str().unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(Article { title: page["title"].as_str().unwrap_or(title).to_string(), url: page["fullurl"].as_str().unwrap_or_default().to_string(), text }))
}

/// The articles asked for: each title (trimmed, empty ones skipped), then
/// `random` more. Returns the articles and the titles the wiki does not have.
pub fn fetch(base: &str, titles: &[String], random: usize) -> Result<(Vec<Article>, Vec<String>), String> {
    let mut wanted: Vec<String> = titles.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
    wanted.dedup();
    if random > 0 {
        wanted.extend(random_titles(base, random.min(MAX_ARTICLES))?);
    }
    wanted.truncate(MAX_ARTICLES);
    if wanted.is_empty() {
        return Err("Name an article, or ask for some at random.".into());
    }
    let mut out = Vec::new();
    let mut missing = Vec::new();
    for t in wanted {
        match article(base, &t)? {
            Some(a) if !out.iter().any(|x: &Article| x.title == a.title) => out.push(a),
            Some(_) => {}
            None => missing.push(t),
        }
    }
    Ok((out, missing))
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Gathered {
    /// The first new text, to show.
    pub text: Option<String>,
    pub lines: Vec<String>,
}

/// The articles as texts, each with where it came from.
pub fn import(s: &mut Session, wiki: &str, articles: &[Article], missing: &[String], date: &str) -> Result<Gathered, String> {
    let p = session::project(s)?;
    let tier = session::utterance_tier(p)?;
    let mut out = Gathered::default();
    let (mut added, mut matched, mut sentences) = (0usize, 0usize, 0usize);
    let mut meta = Vec::new();
    for a in articles {
        let (units, _) = crate::plaintext::segments(&a.text);
        let units: Vec<String> = units.iter().map(|u| p.proj.nfc(u)).collect();
        if units.is_empty() {
            continue;
        }
        let title = p.proj.nfc(&a.title);
        if crate::interchange::same_text(p, &title, &units)?.is_some() {
            matched += 1;
            continue;
        }
        let refs: Vec<&str> = units.iter().map(String::as_str).collect();
        let (text, _) = p.import_written_text(&title, &tier, &refs)?;
        out.text.get_or_insert(text.clone());
        added += 1;
        sentences += units.len();
        for (k, v) in [("source", a.url.as_str()), ("licence", "CC BY-SA 4.0"), ("author", "Wikipedia contributors"), ("publisher", &format!("{wiki}.wikipedia.org")), ("genre", "encyclopedia"), ("accessed", date)] {
            if !v.is_empty() {
                meta.push((text.clone(), k.to_string(), v.to_string()));
            }
        }
    }
    p.import_text_metadata(&meta)?;
    if added > 0 {
        out.lines.push(format!("{} from {wiki}.wikipedia.org, {}.", plural(added, "new text", "new texts"), plural(sentences, "sentence", "sentences")));
        out.lines.push("Each keeps its address, licence (CC BY-SA 4.0) and the day it was read as details; credit the Wikipedia contributors when you publish from them.".into());
    }
    if matched > 0 {
        out.lines.push(format!("{} already here, so not added again.", plural(matched, "article was", "articles were")));
    }
    if !missing.is_empty() {
        out.lines.push(format!("Not found on {wiki}.wikipedia.org: {}.", missing.join(", ")));
    }
    if out.lines.is_empty() {
        out.lines.push("Nothing was added.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// A stand-in wiki: answers each GET by what it asks for.
    fn fake_wiki(answers: usize) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let handle = std::thread::spawn(move || {
            let mut asked = Vec::new();
            for _ in 0..answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let mut agent = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.to_ascii_lowercase().starts_with("user-agent:") {
                        agent = line.clone();
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                assert!(agent.contains("wugs/"), "a User-Agent names wugs: {agent}");
                let json = if first.contains("list=random") {
                    serde_json::json!({ "query": { "random": [{ "title": "Mto" }] } })
                } else if first.contains("Haipo") {
                    serde_json::json!({ "query": { "pages": [{ "title": "Haipo", "missing": true }] } })
                } else {
                    let title = if first.contains("Mto") { "Mto" } else { "Mlima Kilimanjaro" };
                    serde_json::json!({ "query": { "pages": [{ "title": title, "fullurl": format!("https://sw.wikipedia.org/wiki/{}", title.replace(' ', "_")), "extract": format!("{title} ni mahali.\n\nHistoria\nWatu wengi walifika. Walikaa huko.") }] } })
                }
                .to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}", json.len()).unwrap();
                asked.push(first);
            }
            asked
        });
        (base, handle)
    }

    #[test]
    fn codes_are_checked() {
        assert!(valid_wiki("sw") && valid_wiki("zh-yue") && valid_wiki("simple"));
        assert!(!valid_wiki("") && !valid_wiki("SW") && !valid_wiki("sw.evil.com") && !valid_wiki("x"));
        assert_eq!(base_of("sw").unwrap(), "https://sw.wikipedia.org");
        assert!(base_of("a/b").is_err());
    }

    #[test]
    fn articles_by_title_and_at_random_come_in_with_their_source() {
        // The random list first, then Kilimanjaro, Haipo (missing) and Mto.
        let (base, server) = fake_wiki(4);
        let (articles, missing) = fetch(&base, &["Mlima Kilimanjaro".into(), " ".into(), "Haipo".into()], 1).unwrap();
        let asked = server.join().unwrap();
        assert_eq!(asked.len(), 4);
        assert!(asked[0].contains("list=random"), "{}", asked[0]);
        assert!(asked[1].contains("explaintext=1") && asked[1].contains("redirects=1"), "{}", asked[1]);
        assert_eq!(articles.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), ["Mlima Kilimanjaro", "Mto"]);
        assert_eq!(missing, ["Haipo"]);
        let dir = std::env::temp_dir().join(format!("wugs-wiki-{}", wugs_model::ulid()));
        let mut s: Session = None;
        session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let g = import(&mut s, "sw", &articles, &missing, "2026-11-24").unwrap();
        assert!(g.lines[0].starts_with("2 new texts from sw.wikipedia.org"), "{:?}", g.lines);
        assert!(g.lines.iter().any(|l| l.contains("Not found") && l.contains("Haipo")));
        let text = g.text.unwrap();
        let meta = session::project(&mut s).unwrap().text_metadata(&text).unwrap();
        assert!(meta.contains(&("licence".into(), "CC BY-SA 4.0".into())) && meta.contains(&("source".into(), "https://sw.wikipedia.org/wiki/Mlima_Kilimanjaro".into())), "{meta:?}");
        assert!(meta.contains(&("accessed".into(), "2026-11-24".into())));
        // Again: matched, nothing added.
        let again = import(&mut s, "sw", &articles, &[], "2026-11-24").unwrap();
        assert!(again.lines.iter().any(|l| l.contains("already here")), "{:?}", again.lines);
        assert!(again.text.is_none());
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
