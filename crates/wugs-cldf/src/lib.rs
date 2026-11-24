//! Export a project as a CLDF Dictionary dataset (Cross-Linguistic Data
//! Formats, <https://cldf.clld.org>): what linguists publish dictionaries
//! and glossed examples in, what CLLD sites (Dictionaria, IMTVault) and
//! pycldf read, and what Zenodo archives beside a paper.
//!
//! A dataset is a folder of CSV files and one JSON metadata file (CSVW):
//!
//! - `languages.csv`: the project's language (name, Glottocode, ISO 639-3)
//!   and the languages of its translation tracks.
//! - `entries.csv`: one row per live lexicon entry (headword, part of
//!   speech, citation form, homograph number).
//! - `senses.csv`: one row per live sense (gloss, other-language glosses,
//!   definition, usage, semantic domains, the Concepticon concept set the
//!   Elicit panel linked it to, and the examples that use it).
//! - `examples.csv`: one row per glossed sentence (Leipzig IGT: the
//!   sentence, its words broken into morphs, their glosses, the free
//!   translation), with its text and time span.
//!
//! The crate only builds file contents; the shell decides where they go.

use std::collections::{BTreeMap, BTreeSet};

use wugs_core::Project;
use wugs_model::Value;

pub mod structure;
pub mod wordlist;
pub use structure::{export_structure, Structure, StructureOptions};
pub use wordlist::{export_wordlist, Wordlist, WordlistOptions};

const TERMS: &str = "http://cldf.clld.org/v1.0/terms.rdf#";

/// What the caller decides: which tiers hold sentences and translations
/// (the shell knows the roles), and whether examples from texts that are
/// not open go in.
pub struct Options<'a> {
    pub title: &'a str,
    /// "wugs 0.2.1", for the metadata's `prov:wasGeneratedBy`.
    pub generator: &'a str,
    pub transcription_tiers: &'a [String],
    pub translation_tiers: &'a [String],
    pub include_restricted: bool,
    /// Cut a sound clip of each timed example from its text's recording:
    /// `Some(has)` where `has(blob)` says the recording's audio is on this
    /// computer. The caller cuts each `Dataset::clips` entry into the folder.
    pub audio: Option<&'a dyn Fn(&str) -> bool>,
}

/// One example's sound clip for the caller to cut: the span of `blob`'s
/// recording, written as `file` inside the dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub file: String,
    pub blob: String,
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Dataset {
    /// (file name, contents), metadata first.
    pub files: Vec<(String, String)>,
    pub entries: usize,
    pub senses: usize,
    /// Glossed sentences from the texts.
    pub examples: usize,
    /// Example sentences the lexicon holds for its senses (also in `examples.csv`).
    pub lexicon_examples: usize,
    /// Sentences left out because their text is not open (`include_restricted` off).
    pub restricted_left_out: usize,
    /// The examples' sound clips, when asked for (`Options::audio`).
    pub clips: Vec<Clip>,
    /// Timed examples left without a clip: no recording linked, or its
    /// audio is not on this computer, or the recording is not open.
    pub clips_missing: usize,
    /// What the person should know, one sentence each.
    pub notes: Vec<String>,
}

/// A CLDF identifier: letters, digits, `_` and `-` (wugs ids already are).
fn cldf_id(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

/// One CSV cell, quoted when it must be (RFC 4180).
fn cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

struct Table {
    url: &'static str,
    component: &'static str,
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
    foreign: Vec<(&'static str, &'static str)>,
}

struct Column {
    name: String,
    term: Option<&'static str>,
    separator: Option<&'static str>,
    required: bool,
}

fn col(name: &str, term: Option<&'static str>) -> Column {
    Column { name: name.to_string(), term, separator: None, required: false }
}

impl Table {
    fn csv(&self) -> String {
        let mut out = self.columns.iter().map(|c| cell(&c.name)).collect::<Vec<_>>().join(",");
        out.push('\n');
        for r in &self.rows {
            out.push_str(&r.iter().map(|c| cell(c)).collect::<Vec<_>>().join(","));
            out.push('\n');
        }
        out
    }

    fn schema(&self) -> Value {
        let columns = self
            .columns
            .iter()
            .map(|c| {
                let mut m = vec![("name", Value::str(&c.name)), ("datatype", Value::str("string"))];
                if let Some(t) = c.term {
                    m.push(("propertyUrl", Value::str(&format!("{TERMS}{t}"))));
                }
                if let Some(s) = c.separator {
                    m.push(("separator", Value::str(s)));
                }
                if c.required {
                    m.push(("required", Value::Bool(true)));
                }
                if c.name == "ID" {
                    m.push(("datatype", Value::obj(vec![("base", Value::str("string")), ("format", Value::str("[a-zA-Z0-9_\\-]+"))])));
                }
                Value::obj(m)
            })
            .collect();
        let foreign = self
            .foreign
            .iter()
            .map(|(column, resource)| {
                Value::obj(vec![
                    ("columnReference", Value::Arr(vec![Value::str(column)])),
                    ("reference", Value::obj(vec![("resource", Value::str(resource)), ("columnReference", Value::Arr(vec![Value::str("ID")]))])),
                ])
            })
            .collect();
        Value::obj(vec![
            ("url", Value::str(self.url)),
            ("dc:conformsTo", Value::str(&format!("{TERMS}{}", self.component))),
            ("tableSchema", Value::obj(vec![("columns", Value::Arr(columns)), ("primaryKey", Value::Arr(vec![Value::str("ID")])), ("foreignKeys", Value::Arr(foreign))])),
        ])
    }
}

/// Which tiers hold translations and which hold transcribed sentences, as
/// every corpus tool reads them (the desktop's Concordance delegates here):
/// role `translation`, FLEx's free-translation tier or a role-less
/// symbolic association are translations; a time-aligned tier (or one
/// with no kind) whose role is `transcription` or unset holds sentences.
pub fn corpus_tiers(p: &Project) -> Result<(Vec<String>, Vec<String>), String> {
    let mut translation_tiers = Vec::new();
    let mut transcription_tiers = Vec::new();
    for t in p.proj.entities_of_kind("tier")? {
        let role = p.proj.field_str(&t, "role")?;
        let origin = p.proj.field_str(&t, "origin")?;
        let tier_kind = p.proj.field_str(&t, "kind")?;
        let translation = role.as_deref() == Some("translation")
            || origin.as_deref() == Some(wugs_flex::flextext::TIER_FREE)
            || (role.is_none() && tier_kind.as_deref() == Some("symbolic-association"));
        if translation {
            translation_tiers.push(t);
        } else if matches!(role.as_deref(), None | Some("transcription")) && matches!(tier_kind.as_deref(), None | Some("time-aligned")) {
            transcription_tiers.push(t);
        }
    }
    Ok((translation_tiers, transcription_tiers))
}

fn live(p: &Project, id: &str, kind: &str) -> Result<bool, String> {
    Ok(p.proj.entity_kind(id)?.as_deref() == Some(kind) && !p.proj.is_tombstoned(id)?)
}

fn text_of(p: &Project, id: &str, field: &str) -> Result<String, String> {
    Ok(p.proj.field_str(id, field)?.unwrap_or_default())
}

/// Build the dataset.
pub fn export(p: &Project, o: &Options) -> Result<Dataset, String> {
    let mut ds = Dataset::default();
    let lang = p.project_language()?;
    let lang_id = if lang.glottocode.is_empty() { "language".to_string() } else { lang.glottocode.clone() };
    let mut languages = Table {
        url: "languages.csv",
        component: "LanguageTable",
        columns: vec![col("ID", Some("id")), col("Name", Some("name")), col("Glottocode", Some("glottocode")), col("ISO639P3code", Some("iso639P3code"))],
        rows: vec![vec![lang_id.clone(), if lang.name.is_empty() { "Unnamed language".into() } else { lang.name.clone() }, lang.glottocode.clone(), lang.iso.clone()]],
        foreign: Vec::new(),
    };
    if lang.name.is_empty() && lang.glottocode.is_empty() {
        ds.notes.push("The project's language has no name or Glottocode yet; fill them in under Import and export so the dataset can join others.".into());
    }

    // Examples first: senses list the examples that use them.
    let mut meta_langs: BTreeMap<String, String> = BTreeMap::new();
    let mut examples = Table {
        url: "examples.csv",
        component: "ExampleTable",
        columns: vec![
            col("ID", Some("id")),
            col("Language_ID", Some("languageReference")),
            col("Primary_Text", Some("primaryText")),
            Column { name: "Analyzed_Word".into(), term: Some("analyzedWord"), separator: Some("\t"), required: false },
            Column { name: "Gloss".into(), term: Some("gloss"), separator: Some("\t"), required: false },
            col("Translated_Text", Some("translatedText")),
            col("Meta_Language_ID", Some("metaLanguageReference")),
            col("Text_Title", None),
            col("Start_Ms", None),
            col("End_Ms", None),
        ],
        rows: Vec::new(),
        foreign: vec![("Language_ID", "languages.csv"), ("Meta_Language_ID", "languages.csv")],
    };
    let mut media = Table {
        url: "media.csv",
        component: "MediaTable",
        columns: vec![col("ID", Some("id")), col("Name", Some("name")), col("Description", Some("description")), col("Media_Type", Some("mediaType")), col("Download_URL", Some("downloadUrl"))],
        rows: Vec::new(),
        foreign: Vec::new(),
    };
    media.columns[3].required = true;
    if o.audio.is_some() {
        examples.columns.push(col("Media_ID", Some("mediaReference")));
        examples.foreign.push(("Media_ID", "media.csv"));
    }
    let mut uses: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut texts = p.proj.entities_of_kind("text")?;
    texts.sort();
    for text in texts {
        if p.proj.is_tombstoned(&text)? {
            continue;
        }
        let title = text_of(p, &text, "title")?;
        let open = p.text_access(&text)? == "open";
        // The recording the clips come from: linked, on this computer, and
        // open unless restricted material was asked for.
        let recording = match o.audio {
            Some(has) => match p.text_recording(&text)? {
                Some((src, blob)) if has(&blob) && (o.include_restricted || p.proj.field_str(&src, "access")?.map_or(true, |a| a == "open")) => Some(blob),
                _ => None,
            },
            None => None,
        };
        let segs = p.proj.ordered_children("text", &text, "segment")?;
        let mut translations: Vec<(String, Option<f64>, Option<f64>, String, String)> = Vec::new();
        for s in &segs {
            let tier = text_of(p, s, "tier")?;
            if o.translation_tiers.contains(&tier) && !p.proj.is_tombstoned(s)? {
                translations.push((s.clone(), p.proj.field_num(s, "start")?, p.proj.field_num(s, "end")?, tier, text_of(p, s, "content")?));
            }
        }
        for seg in &segs {
            if p.proj.is_tombstoned(seg)? || !o.transcription_tiers.contains(&text_of(p, seg, "tier")?) {
                continue;
            }
            let Some(igt) = interlinear(p, seg)? else { continue };
            if !open && !o.include_restricted {
                ds.restricted_left_out += 1;
                continue;
            }
            let (start, end) = (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?);
            // The translation that names this sentence, else the timed one overlapping it most.
            let paired = translations.iter().find(|t| p.proj.field_str(&t.0, "parent").ok().flatten().as_deref() == Some(seg.as_str()));
            let timed = || {
                let (Some(a), Some(b)) = (start, end) else { return None };
                translations
                    .iter()
                    .filter_map(|t| {
                        let (Some(c), Some(d)) = (t.1, t.2) else { return None };
                        let overlap = b.min(d) - a.max(c);
                        (overlap > 0.0).then_some((overlap, t))
                    })
                    .max_by(|x, y| x.0.total_cmp(&y.0))
                    .map(|(_, t)| t)
            };
            let translation = paired.or_else(timed);
            let meta = match translation {
                Some(t) => match p.proj.field_text(&t.3, "language")? {
                    Some(code) if !code.trim().is_empty() => {
                        let id = cldf_id(&format!("meta-{}", code.trim()));
                        meta_langs.entry(id.clone()).or_insert_with(|| code.trim().to_string());
                        id
                    }
                    _ => String::new(),
                },
                None => String::new(),
            };
            let id = cldf_id(seg);
            for sense in &igt.senses {
                uses.entry(sense.clone()).or_default().insert(id.clone());
            }
            let ms = |v: Option<f64>| v.map(|x| format!("{}", x.round() as i64)).unwrap_or_default();
            let mut media_id = String::new();
            if o.audio.is_some() {
                match (&recording, start, end) {
                    (Some(blob), Some(a), Some(b)) if b > a => {
                        media_id = format!("audio-{id}");
                        let file = format!("audio/{media_id}.wav");
                        let seconds = |x: f64| format!("{:.2}", x / 1000.0);
                        media.rows.push(vec![
                            media_id.clone(),
                            format!("{media_id}.wav"),
                            format!("{} {}–{} s", if title.is_empty() { "(untitled)" } else { title.as_str() }, seconds(a), seconds(b)),
                            "audio/wav".into(),
                            file.clone(),
                        ]);
                        ds.clips.push(Clip { file, blob: blob.clone(), start_ms: a, end_ms: b });
                    }
                    (_, Some(_), Some(_)) => ds.clips_missing += 1,
                    _ => {}
                }
            }
            let mut row = vec![
                id,
                lang_id.clone(),
                text_of(p, seg, "content")?,
                igt.words.join("\t"),
                igt.glosses.join("\t"),
                translation.map(|t| t.4.clone()).unwrap_or_default(),
                meta,
                title.clone(),
                ms(start),
                ms(end),
            ];
            if o.audio.is_some() {
                row.push(media_id);
            }
            examples.rows.push(row);
        }
    }
    // The lexicon's own example sentences (`wugs_core::examples`): lexicon
    // data, so shown whatever their source text's access; no word-by-word
    // analysis, so Analyzed_Word and Gloss stay empty.
    for lex in p.proj.entities_of_kind("lexeme")? {
        if !live(p, &lex, "lexeme")? {
            continue;
        }
        for sen in p.proj.ordered_children("lexeme", &lex, "sense")? {
            if !live(p, &sen, "sense")? {
                continue;
            }
            for x in p.examples(&sen)? {
                let id = cldf_id(&x.id);
                uses.entry(sen.clone()).or_default().insert(id.clone());
                let mut row = vec![id, lang_id.clone(), x.sentence, String::new(), String::new(), x.translation, String::new(), String::new(), String::new(), String::new()];
                if o.audio.is_some() {
                    row.push(String::new());
                }
                examples.rows.push(row);
                ds.lexicon_examples += 1;
            }
        }
    }
    for (id, code) in &meta_langs {
        languages.rows.push(vec![id.clone(), code.clone(), String::new(), if code.len() == 3 && code.bytes().all(|b| b.is_ascii_lowercase()) { code.clone() } else { String::new() }]);
    }

    // Entries and senses.
    let mut entries = Table {
        url: "entries.csv",
        component: "EntryTable",
        columns: vec![
            col("ID", Some("id")),
            col("Language_ID", Some("languageReference")),
            col("Headword", Some("headword")),
            col("Part_Of_Speech", Some("partOfSpeech")),
            col("Citation_Form", None),
            col("Homograph", None),
        ],
        rows: Vec::new(),
        foreign: vec![("Language_ID", "languages.csv")],
    };
    let mut lexemes: Vec<(String, String, i64, String)> = Vec::new();
    for lex in p.proj.entities_of_kind("lexeme")? {
        if p.proj.is_tombstoned(&lex)? {
            continue;
        }
        let lemma = text_of(p, &lex, "lemma")?;
        let hom = p.proj.field_num(&lex, "homograph")?.map(|h| h as i64).unwrap_or(0);
        lexemes.push((wugs_model::fold_key(&lemma), lemma, hom, lex));
    }
    lexemes.sort();
    let mut sense_rows: Vec<(String, String, BTreeMap<String, String>)> = Vec::new();
    let mut gloss_langs: BTreeSet<String> = BTreeSet::new();
    // Where a word came from, as columns only when some entry says.
    let mut origins: Vec<[String; 2]> = Vec::new();
    for (_, lemma, hom, lex) in &lexemes {
        entries.rows.push(vec![
            cldf_id(lex),
            lang_id.clone(),
            lemma.clone(),
            text_of(p, lex, "pos-of-speech")?,
            text_of(p, lex, "citation")?,
            if *hom > 0 { hom.to_string() } else { String::new() },
        ]);
        origins.push([text_of(p, lex, "borrowed-from")?.trim().to_string(), text_of(p, lex, "etymology")?.split_whitespace().collect::<Vec<_>>().join(" ")]);
        for sen in p.proj.ordered_children("lexeme", lex, "sense")? {
            if !live(p, &sen, "sense")? {
                continue;
            }
            let mut extra = BTreeMap::new();
            for (k, v) in p.proj.fields_of(&sen)? {
                if let (Some(tag), Some(text)) = (k.strip_prefix("gloss:"), v.as_str()) {
                    if !text.is_empty() {
                        gloss_langs.insert(tag.to_string());
                        extra.insert(tag.to_string(), text.to_string());
                    }
                }
            }
            sense_rows.push((sen, cldf_id(lex), extra));
        }
    }
    for (i, name) in ["Borrowed_From", "Etymology"].into_iter().enumerate() {
        if origins.iter().any(|o| !o[i].is_empty()) {
            entries.columns.push(col(name, None));
            for (row, o) in entries.rows.iter_mut().zip(&origins) {
                row.push(o[i].clone());
            }
        }
    }
    let mut senses = Table {
        url: "senses.csv",
        component: "SenseTable",
        columns: vec![col("ID", Some("id")), col("Entry_ID", Some("entryReference")), col("Description", Some("description"))],
        rows: Vec::new(),
        foreign: vec![("Entry_ID", "entries.csv")],
    };
    senses.columns[1].required = true;
    for tag in &gloss_langs {
        senses.columns.push(col(&format!("Gloss_{}", cldf_id(tag)), None));
    }
    senses.columns.push(col("Definition", None));
    senses.columns.push(col("Usage", None));
    senses.columns.push(Column { name: "Semantic_Domains".into(), term: None, separator: Some("; "), required: false });
    senses.columns.push(col("Concepticon_ID", Some("concepticonReference")));
    senses.columns.push(col("Concepticon_Gloss", None));
    senses.columns.push(Column { name: "Example_IDs".into(), term: Some("exampleReference"), separator: Some(" "), required: false });
    senses.foreign.push(("Example_IDs", "examples.csv"));
    for (sen, entry, extra) in &sense_rows {
        let mut row = vec![cldf_id(sen), entry.clone(), text_of(p, sen, "gloss")?];
        for tag in &gloss_langs {
            row.push(extra.get(tag).cloned().unwrap_or_default());
        }
        row.push(text_of(p, sen, "definition")?);
        row.push(text_of(p, sen, "usage")?);
        let mut domains = Vec::new();
        for (_, dom) in p.sense_domains(sen)? {
            let code = text_of(p, &dom, "code")?;
            let label = text_of(p, &dom, "label")?;
            domains.push(format!("{code} {label}").trim().replace(';', ","));
        }
        row.push(domains.join("; "));
        let (mut cid, mut cgloss) = (String::new(), String::new());
        for link in p.proj.children_by_ref("sense", sen, "sense-concept")? {
            if p.proj.is_tombstoned(&link)? {
                continue;
            }
            if let Some(concept) = p.proj.field_str(&link, "concept")? {
                let id = text_of(p, &concept, "concepticon")?;
                if !id.is_empty() && live(p, &concept, "concept")? {
                    cid = id;
                    cgloss = text_of(p, &concept, "concepticon-gloss")?;
                    break;
                }
            }
        }
        row.push(cid);
        row.push(cgloss);
        row.push(uses.get(sen).map(|s| s.iter().cloned().collect::<Vec<_>>().join(" ")).unwrap_or_default());
        senses.rows.push(row);
    }

    ds.entries = entries.rows.len();
    ds.senses = senses.rows.len();
    ds.examples = examples.rows.len() - ds.lexicon_examples;
    if ds.restricted_left_out > 0 {
        ds.notes.push(format!(
            "{} from texts that are not open {} left out; choose to include them only if the speakers agreed to publication.",
            plural(ds.restricted_left_out, "glossed sentence", "glossed sentences"),
            if ds.restricted_left_out == 1 { "was" } else { "were" }
        ));
    }
    if ds.clips_missing > 0 {
        ds.notes.push(format!(
            "{} {} no sound clip: {} text has no recording linked, its audio is not on this computer, or the recording is not open.",
            plural(ds.clips_missing, "timed example", "timed examples"),
            if ds.clips_missing == 1 { "has" } else { "have" },
            if ds.clips_missing == 1 { "its" } else { "their" }
        ));
    }
    let mut tables = vec![&languages, &entries, &senses, &examples];
    if o.audio.is_some() {
        tables.push(&media);
    }
    let metadata = Value::obj(vec![
        ("@context", Value::Arr(vec![Value::str("http://www.w3.org/ns/csvw"), Value::obj(vec![("@language", Value::str("en"))])])),
        ("dc:conformsTo", Value::str(&format!("{TERMS}Dictionary"))),
        ("dc:title", Value::str(o.title)),
        ("prov:wasGeneratedBy", Value::Arr(vec![Value::obj(vec![("dc:title", Value::str(o.generator)), ("dc:description", Value::str("https://github.com/judecutler/wugs"))])])),
        ("tables", Value::Arr(tables.iter().map(|t| t.schema()).collect())),
    ]);
    ds.files.push(("Dictionary-metadata.json".into(), pretty(&metadata)));
    for t in &tables {
        ds.files.push((t.url.to_string(), t.csv()));
    }
    ds.files.push(("README.md".into(), readme(o, &ds, &lang.name)));
    Ok(ds)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A sentence's interlinear form: each word as its morphs joined at their
/// markers (`ni-kani`), their glosses joined the same way (`1SG-count`),
/// and the senses its roots link to. `None` when no word has an analysis.
struct Igt {
    words: Vec<String>,
    glosses: Vec<String>,
    senses: Vec<String>,
}

fn interlinear(p: &Project, seg: &str) -> Result<Option<Igt>, String> {
    let mut igt = Igt { words: Vec::new(), glosses: Vec::new(), senses: Vec::new() };
    let mut analysed = false;
    for tok in p.proj.members(seg)? {
        if p.proj.is_tombstoned(&tok)? || ["kind", "x.flex.kind"].iter().any(|f| p.proj.field_str(&tok, f).ok().flatten().as_deref() == Some("punct")) {
            continue;
        }
        let morphs: Vec<String> = p.proj.members(&tok)?.into_iter().filter(|m| !p.proj.is_tombstoned(m).unwrap_or(true)).collect();
        if morphs.is_empty() {
            igt.words.push(text_of(p, &tok, "form")?);
            igt.glosses.push("?".into());
            continue;
        }
        analysed = true;
        let (mut w, mut g) = (String::new(), String::new());
        for (i, m) in morphs.iter().enumerate() {
            let surface = text_of(p, m, "surface")?;
            let gloss = text_of(p, m, "gloss")?;
            let joint = match text_of(p, m, "type")?.as_str() {
                "clitic" => "=",
                "reduplication" => "~",
                _ => "-",
            };
            if i > 0 {
                w.push_str(joint);
                g.push_str(joint);
            }
            w.push_str(surface.trim_matches(|c| matches!(c, '-' | '=' | '~')));
            g.push_str(if gloss.is_empty() { "?" } else { gloss.as_str() });
            if let Some(sense) = p.proj.field_str(m, "sense")? {
                if live(p, &sense, "sense")? {
                    igt.senses.push(sense);
                }
            }
        }
        igt.words.push(w);
        igt.glosses.push(g);
    }
    Ok(analysed.then_some(igt))
}

/// JSON with two-space indents, so the metadata reads in a diff.
fn pretty(v: &Value) -> String {
    fn go(v: &Value, depth: usize, out: &mut String) {
        let pad = |d: usize| "  ".repeat(d);
        match v {
            Value::Arr(items) if !items.is_empty() => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    go(item, depth + 1, out);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push(']');
            }
            Value::Obj(map) if !map.is_empty() => {
                out.push_str("{\n");
                for (i, (k, item)) in map.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    out.push_str(&Value::str(k).to_json());
                    out.push_str(": ");
                    go(item, depth + 1, out);
                    out.push_str(if i + 1 < map.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push('}');
            }
            other => out.push_str(&other.to_json()),
        }
    }
    let mut out = String::new();
    go(v, 0, &mut out);
    out.push('\n');
    out
}

fn readme(o: &Options, ds: &Dataset, language: &str) -> String {
    let language = if language.is_empty() { "the project's language" } else { language };
    format!(
        "# {title}\n\nA CLDF Dictionary dataset of {language}, exported from wugs ({generator}).\n\n\
- `entries.csv`: {entries}\n- `senses.csv`: {senses}, with semantic domains and Concepticon concept sets where the lexicon has them\n- `examples.csv`: {examples}, in Leipzig interlinear form (words and glosses separated by tabs), each with its text and time span in milliseconds{lexicon_examples}\n- `languages.csv`: the language and the languages of the translations\n{media}\n\
Validate with `pip install pycldf` and `cldf validate Dictionary-metadata.json`.\n\n\
Before publishing, add a licence (`dc:license` in the metadata) and a citation, and check that the speakers agreed to publication. \
{restricted}\n",
        title = o.title,
        generator = o.generator,
        media = if o.audio.is_some() {
            format!("- `media.csv` and `audio/`: {}, cut from the recordings, each named in its example's `Media_ID`\n", plural(ds.clips.len(), "sound clip", "sound clips"))
        } else {
            String::new()
        },
        entries = plural(ds.entries, "entry", "entries"),
        senses = plural(ds.senses, "sense", "senses"),
        examples = plural(ds.examples, "glossed sentence", "glossed sentences"),
        lexicon_examples = if ds.lexicon_examples == 0 {
            String::new()
        } else {
            format!(", and {} the lexicon gives for its senses (sentence and translation, no glosses)", plural(ds.lexicon_examples, "example sentence", "example sentences"))
        },
        restricted = if o.include_restricted {
            "Sentences from texts marked restricted or community-only are included, as chosen at export."
        } else {
            "Sentences from texts marked restricted or community-only were left out."
        }
    )
}
