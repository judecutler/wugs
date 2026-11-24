//! "Publish as a website": the project's texts and dictionary as a folder of
//! plain HTML pages that open from disk, a USB stick or any web host, with
//! no server and nothing to install. Built for the people whose language it
//! is (roadmap 43, community apps): each text sentence by sentence, glossed
//! where it is glossed, with its translation and, when asked, a button that
//! plays the sentence from its recording; the dictionary as the printable
//! page. Texts that are not open are left out unless chosen, and notes
//! (the linguist's own remarks) are never published.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use wugs_core::Project;

use crate::interchange::{cut_clip, one_track, outside_project, translations_of};
use crate::session::{self, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub path: String,
    pub texts: usize,
    pub sentences: usize,
    pub entries: usize,
    pub clips: usize,
    pub restricted_left_out: usize,
    /// Timed sentences without a clip: no recording here, or it is not open.
    pub clips_missing: usize,
    /// Of `clips`, the dictionary's headwords heard in a recording.
    pub headword_clips: usize,
    /// Dictionary senses shown with their picture.
    pub pictures: usize,
    /// Page images copied for the texts' marked lines.
    pub pages: usize,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A gloss with its grammatical parts (`1SG`, `PL`) in small capitals, as
/// the Leipzig Glossing Rules write them.
fn gloss_html(g: &str) -> String {
    let mut out = String::new();
    let mut piece = String::new();
    let flush = |piece: &mut String, out: &mut String| {
        let gram = piece.chars().any(|c| c.is_uppercase()) && !piece.chars().any(|c| c.is_lowercase());
        if gram {
            out.push_str(&format!("<span class=\"sc\">{}</span>", esc(&piece.to_lowercase())));
        } else {
            out.push_str(&esc(piece));
        }
        piece.clear();
    };
    for c in g.chars() {
        if matches!(c, '-' | '=' | '.' | '~' | ':' | ' ') {
            flush(&mut piece, &mut out);
            out.push_str(&esc(&c.to_string()));
        } else {
            piece.push(c);
        }
    }
    flush(&mut piece, &mut out);
    out
}

/// A word's morphs and glosses joined at Leipzig's markers.
pub(crate) fn word_lines(p: &Project, tok: &str) -> Result<Option<(String, String)>, String> {
    let morphs = p.proj.members(tok)?;
    if morphs.is_empty() {
        return Ok(None);
    }
    let (mut forms, mut glosses) = (String::new(), String::new());
    for (i, m) in morphs.iter().enumerate() {
        let kind = p.proj.field_str(m, "type")?.unwrap_or_default();
        let surface = p.proj.field_str(m, "surface")?.unwrap_or_default();
        let gloss = p.proj.field_str(m, "gloss")?.filter(|g| !g.trim().is_empty()).unwrap_or_else(|| "?".into());
        let join = match (i, kind.as_str()) {
            (0, _) => "",
            (_, "clitic") => "=",
            (_, "reduplication") => "~",
            _ => "-",
        };
        forms.push_str(join);
        glosses.push_str(join);
        forms.push_str(surface.trim_matches(|c| matches!(c, '-' | '=')));
        glosses.push_str(&gloss);
    }
    Ok(Some((forms, glosses)))
}

const STYLE: &str = "
:root { color-scheme: light; }
body { font-family: 'Charis SIL', 'Gentium Plus', 'Doulos SIL', 'Noto Serif', Georgia, serif; font-size: 17px; line-height: 1.45; color: #1b1a17; background: #fffdf5; max-width: 52em; margin: 0 auto; padding: 1.5em 1em 4em; }
a { color: #00718a; } h1 { font-size: 1.9em; margin: .2em 0; } .front, .details { color: #555; }
figure.where { margin: 1em 0; } figure.where img { width: 100%; max-width: 32em; display: block; border: 1px solid #e0d6b6; } figure.where figcaption { color: #555; font-size: .9em; }
nav { margin-bottom: 1em; } ul.texts { list-style: none; padding: 0; } ul.texts li { margin: .6em 0; } ul.texts .count { color: #666; font-size: .9em; }
ol.sentences { list-style: none; padding: 0; } ol.sentences li { display: grid; grid-template-columns: 2.4em 2em 1fr; gap: 0 .4em; padding: .6em 0; border-bottom: 1px solid #eee6cc; }
.n { color: #888; text-align: right; } .body { grid-column: 3; } .tx { font-size: 1.1em; margin: 0; } .crop { max-width: 40em; background-repeat: no-repeat; border: 1px solid #ccc; border-radius: 3px; margin: .2em 0 .4em; }
.igt { display: flex; flex-wrap: wrap; gap: .2em 1.1em; margin: .3em 0; } .w { display: inline-flex; flex-direction: column; } .m { font-style: italic; } .g { font-size: .92em; }
.sc { font-variant: small-caps; } .ft { margin: .2em 0 0; color: #333; }
button.play { grid-column: 2; align-self: start; border: 1px solid #00718a; color: #00718a; background: none; border-radius: 50%; width: 1.8em; height: 1.8em; cursor: pointer; font-size: .9em; padding: 0; }
button.play:hover, button.play:focus-visible { background: #d6f3f9; }
footer { margin-top: 3em; color: #777; font-size: .85em; }
ol.sentences li:target { background: #fff3c4; }
.search input { font: inherit; width: 100%; box-sizing: border-box; padding: .4em .6em; border: 1px solid #b9b19a; border-radius: .4em; background: #fff; }
.search label { display: block; margin-bottom: .3em; color: #555; }
ul.hits { list-style: none; padding: 0; } ul.hits li { margin: .45em 0; } .hits .hw { font-weight: bold; } .hits .gl, .hits .ft { color: #333; }
.count { color: #666; font-size: .9em; } mark { background: #fde68a; color: inherit; }
#results:not(:empty) { margin-bottom: 1.5em; padding-bottom: .5em; border-bottom: 1px solid #eee6cc; }
";

/// The ▶ beside a headword on the site's dictionary page.
const DICTIONARY_PLAY: &str = "button.play { border: 1px solid #00718a; color: #00718a; background: none; border-radius: 50%; width: 1.6em; height: 1.6em; cursor: pointer; font-size: .75em; padding: 0; vertical-align: .1em; } button.play:hover, button.play:focus-visible { background: #d6f3f9; } @media print { button.play { display: none; } }";

const SCRIPT: &str = "<script>\nlet playing = null;\nfor (const b of document.querySelectorAll('button.play')) b.addEventListener('click', () => { if (playing) playing.pause(); playing = new Audio(b.dataset.src); playing.play(); });\n</script>\n";

/// The site's search, run in the reader's browser over `search-index.js`.
const SEARCH: &str = include_str!("site-search.js");

/// The tags that make every page installable as an app on a phone or
/// computer (a web app manifest and its icon).
const HEAD_APP: &str = "<link rel=\"manifest\" href=\"manifest.webmanifest\">\n<link rel=\"icon\" href=\"icon-192.png\">\n<meta name=\"theme-color\" content=\"#00718a\">\n";
/// Registers the service worker that keeps the site for reading offline.
/// Browsers allow one only over https or on localhost, never from a file,
/// so a site opened from disk or a USB stick simply goes without.
const REGISTER: &str = "<script>\nif ('serviceWorker' in navigator && location.protocol !== 'file:') navigator.serviceWorker.register('sw.js').catch(() => {});\n</script>\n";
const ICON_192: &[u8] = include_bytes!("../site-icons/icon-192.png");
const ICON_512: &[u8] = include_bytes!("../icons/icon.png");

/// The web app manifest: the language's name, the site's colours, its icons.
/// A map of recording sites: at least 8° by 5° around them (Natural Earth
/// 1:110m is too coarse for less), a dot per site titled with its texts.
fn sites_svg(sites: &[(f64, f64, String)]) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let r1 = |v: f64| (v * 100.0).round() / 100.0;
    let xs: Vec<f64> = sites.iter().map(|s| s.1 + 180.0).collect();
    let ys: Vec<f64> = sites.iter().map(|s| 90.0 - s.0).collect();
    let span = |v: &[f64], least: f64| {
        let (lo, hi) = v.iter().fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        let half = least.max((hi - lo) * 1.15) / 2.0 + 1.0;
        ((lo + hi) / 2.0 - half, 2.0 * half)
    };
    let (x, w) = span(&xs, 8.0);
    let (y, h) = span(&ys, 5.0);
    let (y, h) = (y.max(0.0), h.min(180.0));
    let r = w.max(h) / 70.0;
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" role=\"img\" aria-label=\"Where the texts were recorded\"><rect x=\"{0}\" y=\"{1}\" width=\"{2}\" height=\"{3}\" fill=\"#dbeef4\"/>", r1(x), r1(y), r1(w), r1(h));
    for dx in [-360, 0, 360] {
        out.push_str(&format!("<path d=\"{}\" transform=\"translate({dx} 0)\" fill=\"#f2ede0\" stroke=\"#b9ad8f\" stroke-width=\"{}\"/>", crate::typology::LAND.trim(), r1(r / 4.0).max(0.01)));
    }
    // One dot per site (coordinates to about 10 m), titled with its texts.
    let mut by_site: Vec<((i64, i64), f64, f64, Vec<&str>)> = Vec::new();
    for (lat, lon, title) in sites {
        let key = ((lat * 1e4).round() as i64, (lon * 1e4).round() as i64);
        match by_site.iter_mut().find(|s| s.0 == key) {
            Some(s) => s.3.push(title),
            None => by_site.push((key, *lat, *lon, vec![title])),
        }
    }
    for (_, lat, lon, titles) in &by_site {
        out.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#00718a\" stroke=\"#fff\" stroke-width=\"{}\"><title>{}</title></circle>", r1(lon + 180.0), r1(90.0 - lat), r1(r), r1(r / 4.0).max(0.01), esc(&titles.join(", "))));
    }
    out.push_str("</svg>\n");
    out
}

fn manifest(name: &str, short: &str) -> String {
    serde_json::json!({
        "name": name,
        "short_name": short,
        "start_url": "index.html",
        "scope": "./",
        "display": "standalone",
        "background_color": "#fff8de",
        "theme_color": "#00718a",
        "icons": [
            { "src": "icon-192.png", "sizes": "192x192", "type": "image/png" },
            { "src": "icon-512.png", "sizes": "512x512", "type": "image/png" }
        ]
    })
    .to_string()
}

/// A service worker that stores the pages, styles, scripts and pictures when
/// the site is first opened, and each sound clip the first time it plays, so
/// the site keeps working with no connection. Its cache is named for this
/// version of the site, and older versions' caches are dropped.
fn service_worker(version: &str, files: &[String]) -> String {
    format!(
        "// Made by wugs: keeps this site for reading offline.\nconst CACHE = {cache};\nconst FILES = {files};\n\
self.addEventListener('install', (e) => {{ e.waitUntil(caches.open(CACHE).then((c) => c.addAll(FILES)).then(() => self.skipWaiting())); }});\n\
self.addEventListener('activate', (e) => {{ e.waitUntil(caches.keys().then((keys) => Promise.all(keys.filter((k) => k.startsWith('wugs-site-') && k !== CACHE).map((k) => caches.delete(k)))).then(() => self.clients.claim())); }});\n\
self.addEventListener('fetch', (e) => {{\n  const req = e.request;\n  if (req.method !== 'GET' || new URL(req.url).origin !== location.origin) return;\n  e.respondWith(caches.open(CACHE).then((c) => c.match(req, {{ ignoreSearch: true }}).then((hit) => hit || fetch(req).then((res) => {{ if (res.ok && res.status === 200) c.put(req, res.clone()); return res; }}))));\n}});\n",
        cache = serde_json::json!(format!("wugs-site-{version}")),
        files = serde_json::json!(files),
    )
}

/// FNV-1a over what the site holds, so a new export gets a new cache.
fn fingerprint(parts: &[&[u8]]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in parts {
        for b in part.iter().chain(&[0u8]) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    format!("{h:016x}")
}

fn page(title: &str, lang_attr: &str, body: &str, footer: &str, scripts: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{t}</title>\n<link rel=\"stylesheet\" href=\"style.css\">\n{HEAD_APP}</head>\n<body>\n{body}<footer>{footer}</footer>\n{SCRIPT}{scripts}{REGISTER}</body>\n</html>\n",
        t = esc(title),
        body = body.replace("<h1>", &format!("<h1{lang_attr}>")),
    )
}

/// What the search looks through, as a script that sets `WUGS_INDEX`
/// (a script, not JSON, because browsers refuse to fetch files from a page opened from disk).
fn search_index(entries: &[crate::dictionary::Headword], sentences: &[(String, usize, String, String, String)]) -> String {
    use serde_json::{json, Value};
    let entries: Vec<Value> = entries.iter().map(|h| json!([h.id, h.lemma, h.homograph, h.glosses])).collect();
    let sentences: Vec<Value> = sentences.iter().map(|(f, n, t, tr, title)| json!([f, n, t, tr, title])).collect();
    format!("var WUGS_INDEX = {};\n", json!({ "entries": entries, "sentences": sentences }))
}

struct Clip {
    file: String,
    blob: String,
    start: f64,
    end: f64,
}

/// Write the site as `wugs-site-<date>` (then `-2` …) inside `folder`.
pub fn export_website(s: &mut Session, folder: &Path, date: &str, include_restricted: bool, audio: bool) -> Result<Site, String> {
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    outside_project(s, folder)?;
    let p = session::project(s)?;
    let date = crate::corrections::date_or_today(date);
    let lang = p.project_language()?;
    let name = if lang.name.is_empty() { "Texts and dictionary".to_string() } else { format!("{}: texts and dictionary", lang.name) };
    let lang_attr = if lang.iso.is_empty() { String::new() } else { format!(" lang=\"{}\"", esc(&lang.iso)) };
    let footer = format!("Made with wugs on {}.", esc(&date));
    let mut site = Site { path: String::new(), texts: 0, sentences: 0, entries: 0, clips: 0, restricted_left_out: 0, clips_missing: 0, headword_clips: 0, pictures: 0, pages: 0 };
    let mut files: Vec<(String, String)> = vec![("style.css".into(), STYLE.trim_start().to_string())];
    let mut clips: Vec<Clip> = Vec::new();
    let mut pictures: Vec<(String, String)> = Vec::new();

    let mut texts: Vec<(String, String)> = Vec::new();
    for t in p.proj.entities_of_kind("text")? {
        if !p.proj.is_tombstoned(&t)? {
            texts.push((p.proj.field_str(&t, "title")?.unwrap_or_default(), t));
        }
    }
    texts.sort_by(|a, b| wugs_model::fold_key(&a.0).cmp(&wugs_model::fold_key(&b.0)).then(a.1.cmp(&b.1)));
    let mut listed = Vec::new();
    let mut searched: Vec<(String, usize, String, String, String)> = Vec::new();
    for (title, text) in &texts {
        if !include_restricted && p.text_access(text)? != "open" {
            site.restricted_left_out += 1;
            continue;
        }
        let mut ignored = Vec::new();
        let segments = one_track(p, text, &mut ignored, "A website")?;
        let segments: Vec<String> = segments.into_iter().filter(|g| p.proj.field_str(g, "content").ok().flatten().is_some_and(|c| !c.trim().is_empty())).collect();
        if segments.is_empty() {
            continue;
        }
        let n = listed.len() + 1;
        let file = format!("text-{n}.html");
        // The recording, when it is here and may be shared.
        let recording = match p.text_recording(text)? {
            Some((source, blob)) if audio => {
                let open = include_restricted || p.proj.field_str(&source, "access")?.is_none_or(|a| a == "open");
                let here = wugs_blob::parse_ref(&blob).is_some_and(|hex| p.root.join("blobs").join(hex).is_file());
                (open && here).then_some(blob)
            }
            _ => None,
        };
        let mut body = format!("<nav><a href=\"index.html\">← All texts and search</a></nav>\n<h1>{}</h1>\n", esc(title));
        let details = p.text_metadata(text)?;
        if !details.is_empty() {
            let d: Vec<String> = details.iter().map(|(k, v)| format!("{}: {}", esc(&k.replace('_', " ")), esc(v))).collect();
            body.push_str(&format!("<p class=\"details\">{}</p>\n", d.join(" · ")));
        }
        body.push_str("<ol class=\"sentences\">\n");
        // The text's page images that are here, copied as files, with their sizes for the crops.
        let mut page_files: HashMap<String, (String, u32, u32)> = HashMap::new();
        for (k, page) in p.text_pages(text)?.iter().enumerate() {
            let Some(blob) = p.proj.field_str(page, "blob")? else { continue };
            let Some(path) = wugs_blob::parse_ref(&blob).map(|hex| p.root.join("blobs").join(hex)).filter(|f| f.is_file()) else { continue };
            let head = {
                use std::io::Read;
                let mut buf = vec![0u8; 256 * 1024];
                let n = fs::File::open(&path).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
                buf.truncate(n);
                buf
            };
            if let Some((w, h)) = crate::facsimile::image_size(&head) {
                let label = p.proj.field_str(page, "label")?.unwrap_or_default();
                let file = format!("pages/{n}-{}.{}", k + 1, crate::lexicon::picture_ext(&label));
                pictures.push((file.clone(), blob));
                page_files.insert(page.clone(), (file, w, h));
            }
        }
        for (i, seg) in segments.iter().enumerate() {
            let content = p.proj.field_str(seg, "content")?.unwrap_or_default();
            body.push_str(&format!("<li id=\"s{}\"><span class=\"n\">{}</span>", i + 1, i + 1));
            match (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?) {
                (Some(a), Some(b)) if b > a && audio => match &recording {
                    Some(blob) => {
                        let clip = format!("audio/{n}-{}.wav", i + 1);
                        body.push_str(&format!("<button class=\"play\" data-src=\"{clip}\" aria-label=\"Play sentence {}\">▶</button>", i + 1));
                        clips.push(Clip { file: clip, blob: blob.clone(), start: a, end: b });
                    }
                    None => site.clips_missing += 1,
                },
                _ => {}
            }
            body.push_str("<div class=\"body\">");
            // The line's piece of its page, when it is marked on one.
            if let Some(r) = p.segment_region(seg)? {
                if let Some((file, w, h)) = page_files.get(&r.source) {
                    let pos = |start: f64, side: f64| if side >= 1.0 { 0.0 } else { start / (1.0 - side) * 100.0 };
                    body.push_str(&format!(
                        "<div class=\"crop\" role=\"img\" aria-label=\"Sentence {} on its page\" style=\"background-image:url('{file}');background-size:{:.3}% auto;background-position:{:.3}% {:.3}%;aspect-ratio:{} / {}\"></div>",
                        i + 1,
                        100.0 / r.w,
                        pos(r.x, r.w),
                        pos(r.y, r.h),
                        ((r.w * *w as f64).round() as u64).max(1),
                        ((r.h * *h as f64).round() as u64).max(1)
                    ));
                }
            }
            body.push_str(&format!("<p class=\"tx\">{}</p>", esc(&content)));
            let mut words = Vec::new();
            for tok in p.proj.members(seg)? {
                if p.proj.is_punct_token(&tok) {
                    continue;
                }
                // The sentence line above gives the words; under it each word's morphs and glosses.
                if let Some((m, g)) = word_lines(p, &tok)? {
                    words.push(format!("<span class=\"w\"><span class=\"m\">{}</span><span class=\"g\">{}</span></span>", esc(&m), gloss_html(&g)));
                }
            }
            if !words.is_empty() {
                body.push_str(&format!("<div class=\"igt\">{}</div>", words.join("")));
            }
            let translation = translations_of(p, seg)?.into_iter().next().map(|(_, t)| t.trim().to_string()).unwrap_or_default();
            if !translation.is_empty() {
                body.push_str(&format!("<p class=\"ft\">‘{}’</p>", esc(&translation)));
            }
            searched.push((file.clone(), i + 1, content.trim().to_string(), translation, title.clone()));
            body.push_str("</div></li>\n");
        }
        body.push_str("</ol>\n");
        site.sentences += segments.len();
        files.push((file.clone(), page(title, &lang_attr, &body, &footer, "")));
        listed.push((file, title.clone(), segments.len(), details));
    }
    site.texts = listed.len();

    let mut headwords = Vec::new();
    let dictionary = match crate::dictionary::document(p, &date, include_restricted, audio, crate::dictionary::Pictures::Files) {
        Ok((_, html, d)) => {
            site.entries = d.entries;
            headwords = d.headwords;
            pictures.extend(d.picture_files);
            site.headword_clips = d.clips.len();
            for c in d.clips {
                clips.push(Clip { file: c.file, blob: c.blob, start: c.start, end: c.end });
            }
            // The printable page, with a way back and its ▶ working.
            let html = html
                .replacen("</head>", &format!("{HEAD_APP}</head>"), 1)
                .replacen("<body>\n", "<body>\n<nav><a href=\"index.html\">← Home</a></nav>\n", 1)
                .replacen("</body>", &format!("<style>{DICTIONARY_PLAY}</style>\n{SCRIPT}{REGISTER}</body>"), 1);
            files.push(("dictionary.html".into(), html));
            true
        }
        Err(_) => false,
    };
    if listed.is_empty() && !dictionary {
        return Err("There is nothing to publish yet: no text with transcribed sentences, and no lexicon.".into());
    }

    let mut body = format!("<h1>{}</h1>\n", esc(&name));
    let codes: Vec<String> = [(!lang.glottocode.is_empty()).then(|| format!("Glottocode {}", lang.glottocode)), (!lang.iso.is_empty()).then(|| format!("ISO 639-3 {}", lang.iso))].into_iter().flatten().collect();
    if !codes.is_empty() {
        body.push_str(&format!("<p class=\"front\">{}</p>\n", esc(&codes.join(" · "))));
    }
    // Where the language is spoken, as Glottolog places it, with its nearest languages.
    if let Some(place) = (!lang.glottocode.is_empty()).then(|| crate::glottolog::place(&lang.glottocode, 8)).flatten() {
        let label = if lang.name.trim().is_empty() { "the language".to_string() } else { lang.name.trim().to_string() };
        files.push(("map.svg".into(), crate::glottolog::locator_svg(&place, &format!("Where {label} is spoken"))));
        let near: Vec<&str> = place.neighbours.iter().take(4).map(|n| n.name).collect();
        body.push_str(&format!(
            "<figure class=\"where\"><img src=\"map.svg\" alt=\"{}\"><figcaption>Where Glottolog places {}{}. Land from Natural Earth.</figcaption></figure>\n",
            esc(&format!("A map showing where {label} is spoken")),
            esc(&label),
            if near.is_empty() { String::new() } else { esc(&format!(", near {}", near.join(", "))) }
        ));
    }
    body.push_str(&format!(
        "<div class=\"search\" role=\"search\"><label for=\"q\">Search {}</label><input id=\"q\" type=\"search\" autocomplete=\"off\" spellcheck=\"false\"></div>\n<div id=\"results\" aria-live=\"polite\"></div>\n",
        match (dictionary, listed.is_empty()) {
            (true, false) => "the dictionary and the texts",
            (true, true) => "the dictionary",
            _ => "the texts",
        }
    ));
    if dictionary {
        body.push_str(&format!("<p><a href=\"dictionary.html\">Dictionary</a> <span class=\"count\">({} entries)</span></p>\n", site.entries));
    }
    // Where the published texts were recorded, from their coordinates details.
    let sites: Vec<(f64, f64, String)> = listed.iter().filter_map(|(_, title, _, details)| wugs_core::metadata::place_of(details).map(|(lat, lon)| (lat, lon, title.clone()))).collect();
    if !sites.is_empty() {
        files.push(("sites.svg".into(), sites_svg(&sites)));
        let n = sites.len();
        body.push_str(&format!(
            "<figure class=\"where\"><img src=\"sites.svg\" alt=\"A map of where the texts were recorded\"><figcaption>Where {} recorded. Land from Natural Earth.</figcaption></figure>\n",
            if n == 1 { "one text was".to_string() } else { format!("{n} texts were") }
        ));
    }
    if !listed.is_empty() {
        body.push_str("<h2>Texts</h2>\n<ul class=\"texts\">\n");
        for (file, title, count, details) in &listed {
            let extra: Vec<&str> = details.iter().filter(|(k, _)| matches!(k.as_str(), "genre" | "speakers" | "speaker" | "date" | "place")).map(|(_, v)| v.as_str()).collect();
            let extra = if extra.is_empty() { String::new() } else { format!(" · {}", esc(&extra.join(" · "))) };
            body.push_str(&format!(
                "<li><a href=\"{file}\">{}</a> <span class=\"count\">{count} {}{extra}</span></li>\n",
                esc(if title.trim().is_empty() { "(untitled)" } else { title }),
                if *count == 1 { "sentence" } else { "sentences" }
            ));
        }
        body.push_str("</ul>\n");
    }
    files.push(("index.html".into(), page(&name, &lang_attr, &body, &footer, "<script src=\"search-index.js\"></script>\n<script src=\"search.js\"></script>\n")));
    files.push(("search-index.js".into(), search_index(&headwords, &searched)));
    files.push(("search.js".into(), SEARCH.to_string()));
    let short = if lang.name.trim().is_empty() { "Texts".to_string() } else { lang.name.trim().to_string() };
    files.push(("manifest.webmanifest".into(), manifest(&name, &short)));
    // Everything but the sound clips is stored at install; clips as they play.
    let mut stored: Vec<String> = vec!["./".into()];
    stored.extend(files.iter().map(|(n, _)| n.clone()));
    stored.extend(["icon-192.png".to_string(), "icon-512.png".to_string()]);
    stored.extend(pictures.iter().map(|(f, _)| f.clone()));
    let mut parts: Vec<&[u8]> = files.iter().map(|(_, c)| c.as_bytes()).collect();
    parts.extend(pictures.iter().map(|(_, b)| b.as_bytes()));
    let clip_names: Vec<String> = clips.iter().map(|c| format!("{}{}{}{}", c.file, c.blob, c.start, c.end)).collect();
    parts.extend(clip_names.iter().map(|c| c.as_bytes()));
    let version = format!("{date}-{}", fingerprint(&parts));
    files.push(("sw.js".into(), service_worker(&version, &stored)));

    let mut out = folder.join(format!("wugs-site-{date}"));
    let mut k = 2;
    while out.symlink_metadata().is_ok() {
        out = folder.join(format!("wugs-site-{date}-{k}"));
        k += 1;
    }
    // Written whole into a scratch folder beside it, then renamed, so a failure leaves nothing half-written.
    let scratch = folder.join(format!(".{}.partial", out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let written = (|| {
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        for (name, contents) in &files {
            fs::write(scratch.join(name), contents).map_err(|e| format!("{name}: {e}"))?;
        }
        fs::write(scratch.join("icon-192.png"), ICON_192).map_err(|e| format!("icon-192.png: {e}"))?;
        fs::write(scratch.join("icon-512.png"), ICON_512).map_err(|e| format!("icon-512.png: {e}"))?;
        if !clips.is_empty() {
            fs::create_dir(scratch.join("audio")).map_err(|e| format!("audio: {e}"))?;
        }
        for c in &clips {
            let hex = wugs_blob::parse_ref(&c.blob).ok_or_else(|| format!("malformed blob ref {:?}", c.blob))?;
            cut_clip(&p.root.join("blobs").join(hex), c.start, c.end, &scratch.join(&c.file)).map_err(|e| format!("{}: {e}", c.file))?;
        }
        for dir in ["pictures", "pages"] {
            if pictures.iter().any(|(f, _)| f.starts_with(&format!("{dir}/"))) {
                fs::create_dir(scratch.join(dir)).map_err(|e| format!("{dir}: {e}"))?;
            }
        }
        for (file, blob) in &pictures {
            let hex = wugs_blob::parse_ref(blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?;
            fs::copy(p.root.join("blobs").join(hex), scratch.join(file)).map_err(|e| format!("{file}: {e}"))?;
        }
        fs::rename(&scratch, &out).map_err(|e| format!("{}: {e}", out.display()))
    })();
    if let Err(e) = written {
        fs::remove_dir_all(&scratch).ok();
        return Err(e);
    }
    site.clips = clips.len();
    site.pictures = pictures.iter().filter(|(f, _)| f.starts_with("pictures/")).count();
    site.pages = pictures.len() - site.pictures;
    site.path = out.display().to_string();
    Ok(site)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammatical_glosses_are_small_capitals() {
        assert_eq!(gloss_html("1SG-count"), "<span class=\"sc\">1sg</span>-count");
        assert_eq!(gloss_html("dog=PL"), "dog=<span class=\"sc\">pl</span>");
        assert_eq!(gloss_html("<b>"), "&lt;b&gt;");
    }

    #[test]
    fn the_tutorial_becomes_a_site_with_sound() {
        let dir = std::env::temp_dir().join(format!("wugs-site-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        session::project(&mut s).unwrap().set_project_language("Tutorial English", "stan1293", "eng").unwrap();
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        // A restricted text: nothing of it goes out unless chosen.
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        session::project(&mut s).unwrap().set_access(&text, "restricted").unwrap();
        session::project(&mut s).unwrap().set_text_metadata(&text, "coordinates", "51.75 N, 1.26 W").unwrap();
        // A headword heard in the recording: its first word, «wʌn».
        let seg = session::get_text(&mut s, &text).unwrap().segments.into_iter().find(|g| g.content.starts_with("wʌn")).unwrap();
        let entry = crate::lexicon::lexicon_list(&mut s, "").unwrap().into_iter().find(|r| r.lemma == "wʌn").unwrap().id;
        let e = crate::lexicon::lexicon_add_pronunciation(&mut s, &entry, "ˈwʌn", Some(&seg.id)).unwrap().entry.unwrap();
        assert_eq!(e.pronunciations[0].audio.as_ref().map(|a| a.start_ms), seg.start_ms, "heard in the segment's span");
        // The first line marked on a page image: its crop goes out with the page.
        let png = dir.join("page.png");
        fs::write(&png, [&[0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0, 0, 0x07, 0xd0, 0, 0, 0x0b, 0xb8][..], &[0u8; 40]].concat()).unwrap();
        let v = crate::facsimile::add_page(&mut s, &text, png.to_str().unwrap()).unwrap();
        let area = crate::facsimile::Area { source: v.pages[0].source.clone(), x: 0.1, y: 0.5, w: 0.5, h: 0.1 };
        crate::facsimile::set_area(&mut s, &text, &seg.id, Some(area)).unwrap();
        let closed = export_website(&mut s, &out, "2026-11-23", false, true).unwrap();
        assert_eq!((closed.texts, closed.restricted_left_out), (0, 1), "{closed:?}");
        assert!(fs::read_to_string(Path::new(&closed.path).join("index.html")).unwrap().contains("dictionary.html"));
        let site = export_website(&mut s, &out, "2026-11-23", true, true).unwrap();
        assert!(site.path.ends_with("wugs-site-2026-11-23-2"), "never replaces a site: {}", site.path);
        assert_eq!(site.texts, 1);
        assert!(site.sentences >= 2 && site.clips >= 2, "{site:?}");
        let root = Path::new(&site.path);
        let index = fs::read_to_string(root.join("index.html")).unwrap();
        assert!(index.contains("<h1 lang=\"eng\">Tutorial English: texts and dictionary</h1>") && index.contains("href=\"text-1.html\""), "{index}");
        // With a Glottocode, a locator map beside the front page, kept for offline use.
        assert!(index.contains("<img src=\"map.svg\"") && index.contains("Where Glottolog places Tutorial English, near "), "{index}");
        assert!(root.join("map.svg").is_file());
        assert!(index.contains("<img src=\"sites.svg\"") && index.contains("Where one text was recorded."), "{index}");
        let sites = fs::read_to_string(root.join("sites.svg")).unwrap();
        assert!(sites.contains("<circle cx=\"178.74\" cy=\"38.25\"") && sites.contains("<title>"), "{sites}");
        assert!(std::fs::read_to_string(root.join("sw.js")).unwrap().contains("map.svg"));
        let text = fs::read_to_string(root.join("text-1.html")).unwrap();
        assert!(text.contains("<p class=\"tx\">wʌn tuː θɹiː</p>"), "{text}");
        assert!(text.contains("<div class=\"crop\" role=\"img\" aria-label=\"Sentence 1 on its page\" style=\"background-image:url('pages/1-1.png');background-size:200.000% auto;background-position:20.000% 55.556%;aspect-ratio:1000 / 300\"></div>"), "{text}");
        assert!(root.join("pages/1-1.png").is_file() && site.pages == 1, "{site:?}");
        assert!(text.contains("<button class=\"play\" data-src=\"audio/1-1.wav\""), "{text}");
        assert!(text.contains("class=\"ft\">‘"), "the translation is shown: {text}");
        assert!(root.join("audio/1-1.wav").is_file() && root.join("style.css").is_file() && root.join("dictionary.html").is_file());
        // The search: a box on the front page over an index of what the site shows.
        assert!(index.contains("<input id=\"q\"") && index.contains("<script src=\"search-index.js\"></script>"), "{index}");
        let found = fs::read_to_string(root.join("search-index.js")).unwrap();
        assert!(found.starts_with("var WUGS_INDEX = {") && found.contains("[\"text-1.html\",1,\"wʌn tuː θɹiː\""), "{found}");
        assert!(text.contains("<li id=\"s1\">"), "each sentence is a search target");
        assert!(root.join("search.js").is_file());
        // Installable and readable offline once hosted.
        assert!(index.contains(r#"<link rel="manifest" href="manifest.webmanifest">"#) && index.contains("serviceWorker.register('sw.js')"), "{index}");
        assert!(text.contains("manifest.webmanifest") && text.contains("sw.js"), "every page registers it");
        let manifest = fs::read_to_string(root.join("manifest.webmanifest")).unwrap();
        assert!(manifest.contains("\"short_name\":\"Tutorial English\"") && manifest.contains("icon-512.png"), "{manifest}");
        assert!(root.join("icon-192.png").is_file() && root.join("icon-512.png").is_file());
        let sw = fs::read_to_string(root.join("sw.js")).unwrap();
        assert!(sw.contains("const CACHE = \"wugs-site-2026-11-23-") && sw.contains("\"text-1.html\"") && sw.contains("\"dictionary.html\""), "{sw}");
        assert!(!sw.contains("audio/"), "clips are stored as they play, not all at once: {sw}");
        // The dictionary page says the headword and plays it.
        let dict = fs::read_to_string(root.join("dictionary.html")).unwrap();
        assert!(dict.contains("<span class=\"pr\">ˈwʌn</span>") && dict.contains("data-src=\"audio/w-1.wav\" aria-label=\"Hear wʌn\""), "{dict}");
        assert!(dict.contains("playing = new Audio") && root.join("audio/w-1.wav").is_file() && site.headword_clips == 1, "{site:?}");
        assert!(!dir.read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".partial")));
        assert!(export_website(&mut s, &dir.join("sample"), "2026-11-23", true, true).is_err(), "never inside the project");
        // WUGS_KEEP_SITE=1 keeps the site to look at by hand.
        if std::env::var_os("WUGS_KEEP_SITE").is_some() {
            eprintln!("kept {}", site.path);
            return;
        }
        drop(s);
        fs::remove_dir_all(dir).ok();
    }
}
