//! Archive metadata written into the bag: OLAC, a minimal IMDI, and the
//! access/consent dump. Reads the projection only.

use std::collections::{BTreeMap, BTreeSet};

use wugs_core::{Project, SCOPE_CONFIG, SCOPE_LEXICON, SCOPE_PEOPLE};
use wugs_oplog::sanitize;
use wugs_model::Value;
use wugs_xml::XmlNode;

use crate::{all_of_kind, fields_of, live_of_kind};

pub struct TextEntry {
    pub id: String,
    pub title: String,
    /// bag payload paths (relative to data/) exported for this text
    pub files: Vec<String>,
    /// The session's particulars for IMDI, from the text's details, else its recording's.
    pub session: SessionInfo,
}

/// What IMDI's Session asks for, as the project knows it.
#[derive(Default)]
pub struct SessionInfo {
    pub date: Option<String>,
    pub place: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub actors: Vec<Actor>,
}

/// A participant: a person with a consent record covering the text or its recording.
pub struct Actor {
    pub role: String,
    pub name: String,
    pub details: Vec<(String, String)>,
}

/// The session's particulars of one text: its own details first, then its
/// recording's; the people whose consent records cover the text or its
/// recording, with their details.
pub fn session_info(project: &Project, text: &str) -> Result<SessionInfo, String> {
    let own = project.text_metadata(text)?;
    let source = project.text_recording(text)?.map(|(s, _)| s);
    let recorded = match &source {
        Some(s) => project.source_details(s)?,
        None => Vec::new(),
    };
    let pick = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| own.iter().find(|(m, _)| m == k).map(|(_, v)| v.clone()))
            .or_else(|| keys.iter().find_map(|k| recorded.iter().find(|(m, _)| m == k).map(|(_, v)| v.clone())))
    };
    let mut actors = Vec::new();
    for person in project.people()? {
        if person.deleted {
            continue;
        }
        let covers = person.consents.iter().any(|c| c.covers == text || source.as_deref() == Some(c.covers.as_str()));
        if covers {
            actors.push(Actor { role: person.roles.first().cloned().unwrap_or_else(|| "Unspecified".into()), name: person.name.clone(), details: project.person_details(&person.id)? });
        }
    }
    Ok(SessionInfo {
        date: pick(&["date", "recorded", "recording_date"]),
        place: pick(&["place", "location"]),
        description: pick(&["description", "summary"]),
        genre: pick(&["genre"]),
        actors,
    })
}

pub fn olac(project: &Project, title: &str, date: &str, has_lexicon: bool) -> Result<String, String> {
    let dc = |name: &str, text: &str| XmlNode::new(&format!("dc:{name}")).with_text(text);
    let mut rec = XmlNode::new("olac:olac")
        .with_attr("xmlns:olac", "http://www.language-archives.org/OLAC/1.1/")
        .with_attr("xmlns:dc", "http://purl.org/dc/elements/1.1/")
        .with_attr("xmlns:dcterms", "http://purl.org/dc/terms/")
        .with_attr("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
        .with_attr(
            "xsi:schemaLocation",
            "http://www.language-archives.org/OLAC/1.1/ http://www.language-archives.org/OLAC/1.1/olac.xsd",
        )
        .with_child(dc("title", title))
        .with_child(dc("identifier", &project.project_id))
        .with_child(dc("type", "Collection").with_attr("xsi:type", "dcterms:DCMIType"))
        .with_child(
            XmlNode::new("dc:type")
                .with_attr("xsi:type", "olac:linguistic-type")
                .with_attr("olac:code", "primary_text"),
        );
    if has_lexicon {
        rec.children.push(
            XmlNode::new("dc:type")
                .with_attr("xsi:type", "olac:linguistic-type")
                .with_attr("olac:code", "lexicon"),
        );
    }
    rec.children.push(dc("date", date));
    for lng in live_of_kind(project, "language")? {
        let name = project.proj.field_str(&lng, "name")?.unwrap_or_default();
        let mut node = dc("language", &name);
        if let Some(code) = project.proj.field_str(&lng, "iso639-3")? {
            node = node.with_attr("xsi:type", "olac:language").with_attr("olac:code", &code);
        }
        rec.children.push(node);
    }
    Ok(rec.to_xml())
}

/// IMDI: one Session per text with its name, date, description, an MDGroup
/// (location, genre and the actors whose consent records cover it, with
/// their details) and the exported files. IMDI 3.0 expects one Session per
/// METATRANSCRIPT; this keeps them together in one file, not yet schema-valid.
pub fn imdi(texts: &[TextEntry], date: &str) -> String {
    let mut root = XmlNode::new("METATRANSCRIPT")
        .with_attr("xmlns", "http://www.mpi.nl/IMDI/Schema/IMDI")
        .with_attr("Type", "SESSION")
        .with_attr("Version", "0")
        .with_attr("Date", date)
        .with_attr("Originator", "wugs-seal");
    for t in texts {
        let mut resources = XmlNode::new("Resources");
        for f in &t.files {
            resources.children.push(
                XmlNode::new("WrittenResource")
                    .with_child(XmlNode::new("ResourceLink").with_text(&format!("../{f}")))
                    .with_child(XmlNode::new("Format").with_text(format_of(f))),
            );
        }
        let info = &t.session;
        let mut session = XmlNode::new("Session")
            .with_child(XmlNode::new("Name").with_text(&t.id))
            .with_child(XmlNode::new("Title").with_text(&t.title))
            .with_child(XmlNode::new("Date").with_text(info.date.as_deref().unwrap_or(date)));
        if let Some(d) = &info.description {
            session.children.push(XmlNode::new("Description").with_text(d));
        }
        let mut group = XmlNode::new("MDGroup");
        if let Some(place) = &info.place {
            group.children.push(XmlNode::new("Location").with_child(XmlNode::new("Address").with_text(place)));
        }
        if let Some(genre) = &info.genre {
            group.children.push(XmlNode::new("Content").with_child(XmlNode::new("Genre").with_text(genre)));
        }
        let mut actors = XmlNode::new("Actors");
        for a in &info.actors {
            let detail = |k: &str| a.details.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
            let mut actor = XmlNode::new("Actor").with_child(XmlNode::new("Role").with_text(&a.role)).with_child(XmlNode::new("Name").with_text(&a.name)).with_child(XmlNode::new("FullName").with_text(&a.name));
            for (key, element) in [("ethnic_group", "EthnicGroup"), ("born", "BirthDate"), ("sex", "Sex"), ("education", "Education")] {
                if let Some(v) = detail(key) {
                    actor.children.push(XmlNode::new(element).with_text(&v));
                }
            }
            if let Some(v) = detail("languages") {
                actor.children.push(XmlNode::new("Languages").with_child(XmlNode::new("Description").with_text(&v)));
            }
            actors.children.push(actor);
        }
        group.children.push(actors);
        session.children.push(group);
        session.children.push(resources);
        root.children.push(session);
    }
    root.to_xml()
}

fn format_of(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("eaf") => "text/x-eaf+xml",
        Some("flextext") => "text/xml",
        _ => "Unspecified",
    }
}

/// What the project treats an unset access level as (DATA-MODEL §4; an
/// open owner decision, so reported here rather than chosen).
const DEFAULT_ACCESS: &str = "open";

/// Access level of every source and text, deleted ones included (their ops
/// are still in the copied log), of every copied `log/<scope>/`, and every
/// person and consent record verbatim, so the depositing archive can apply
/// the conditions. Returns the JSON and whether any listed level is not open.
pub fn access(project: &Project, log_scopes: &BTreeSet<String>) -> Result<(String, bool), String> {
    let p = &project.proj;
    // scope directory name -> access of the source or text rooted there
    let mut scope_access = BTreeMap::new();
    let mut source_access = BTreeMap::new();
    let mut sources = Vec::new();
    for (src, deleted) in all_of_kind(project, "source")? {
        let access = p.field_str(&src, "access")?.unwrap_or_else(|| DEFAULT_ACCESS.into());
        source_access.insert(src.clone(), access.clone());
        scope_access.insert(sanitize(&src), access.clone());
        sources.push(entry(vec![("id", Value::str(&src)), ("access", Value::Str(access))], deleted));
    }
    let mut texts = Vec::new();
    for (txt, deleted) in all_of_kind(project, "text")? {
        // A cleared link is JSON null, which is no source.
        let source = p.field(&txt, "source")?.and_then(|(v, _)| v.as_str().map(str::to_string));
        // DATA-MODEL §3: a text inherits its source's access unless it sets its own.
        let access = match p.field_str(&txt, "access")? {
            Some(a) => a,
            None => source
                .as_ref()
                .and_then(|s| source_access.get(s).cloned())
                .unwrap_or_else(|| DEFAULT_ACCESS.into()),
        };
        scope_access.insert(sanitize(&txt), access.clone());
        let mut fields = vec![
            ("id", Value::str(&txt)),
            ("title", p.field_str(&txt, "title")?.map(Value::Str).unwrap_or(Value::Null)),
            ("access", Value::Str(access)),
        ];
        if let Some(s) = source {
            fields.push(("source", Value::Str(s)));
        }
        texts.push(entry(fields, deleted));
    }
    let scopes: Vec<(String, String)> = log_scopes
        .iter()
        .map(|dir| {
            let access = match dir.as_str() {
                SCOPE_CONFIG | SCOPE_LEXICON => DEFAULT_ACCESS.to_string(),
                // DATA-MODEL §4: people/consent is the most restricted scope;
                // sync never shares it by default.
                SCOPE_PEOPLE => "restricted".to_string(),
                // Held or unknown scopes fail closed, as in sync.
                d => scope_access.get(d).cloned().unwrap_or_else(|| "unknown".into()),
            };
            (dir.clone(), access)
        })
        .collect();
    let contains_restricted = sources
        .iter()
        .chain(&texts)
        .filter_map(|v| v.get_str("access"))
        .chain(scopes.iter().map(|(_, a)| a.as_str()))
        .any(|a| a != "open");

    // Signed consent forms are blobs, so every seal copies them into
    // `media/` beside the recordings. They are personal data: list each one
    // by its media path, flagged restricted, so an archivist scanning
    // `media/` can tell a consent form from a recording. Deleted records
    // are listed too (their blob may still be in the store, and the copied
    // log holds their ops either way).
    let mut consent_forms = Vec::new();
    for (c, deleted) in all_of_kind(project, "consent")? {
        let Some(evidence) = p.field_str(&c, "evidence")? else { continue };
        let Some(hex) = wugs_model::parse_blob_ref(&evidence) else { continue };
        let mut fields = vec![("media", Value::Str(format!("media/{hex}"))), ("consent", Value::str(&c))];
        if let Some(name) = p.field_str(&c, "x.evidence-name")? {
            fields.push(("name", Value::Str(name)));
        }
        if let Some(person) = p.field_str(&c, "person")? {
            fields.push(("person", Value::Str(person)));
        }
        fields.push(("access", Value::str("restricted")));
        consent_forms.push(entry(fields, deleted));
    }
    let contains_restricted = contains_restricted || !consent_forms.is_empty();

    let records = |kind: &str| -> Result<Value, String> {
        let mut out = Vec::new();
        for id in live_of_kind(project, kind)? {
            out.push(Value::obj(vec![("id", Value::str(&id)), ("fields", Value::Obj(fields_of(project, &id)?))]));
        }
        Ok(Value::Arr(out))
    };
    let scopes = scopes
        .into_iter()
        .map(|(dir, a)| Value::obj(vec![("log", Value::Str(format!("log/{dir}/"))), ("access", Value::Str(a))]))
        .collect();
    let json = Value::obj(vec![
        ("project", Value::str(&project.project_id)),
        ("default_access", Value::str(DEFAULT_ACCESS)),
        ("contains_restricted", Value::Bool(contains_restricted)),
        ("sources", Value::Arr(sources)),
        ("texts", Value::Arr(texts)),
        ("scopes", Value::Arr(scopes)),
        (
            "media_access",
            Value::str(
                "not derivable in this version: media blobs are not yet linked to sources, \
                 so apply the most restrictive level listed here to media/; \
                 the files under consent_forms are signed consent forms (personal data)",
            ),
        ),
        ("people", records("person")?),
        ("consents", records("consent")?),
        ("consent_forms", Value::Arr(consent_forms)),
    ])
    .to_json();
    Ok((json, contains_restricted))
}

fn entry(mut fields: Vec<(&str, Value)>, deleted: bool) -> Value {
    if deleted {
        fields.push(("deleted", Value::Bool(true)));
    }
    Value::obj(fields)
}
