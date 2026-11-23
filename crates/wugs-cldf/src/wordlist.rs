//! A project's elicited word lists as a CLDF Wordlist dataset, the shape
//! comparative and historical work reads (Lexibank, CLICS, EDICTOR,
//! LingPy): one form per answered concept of a Concepticon list, each with
//! the concept set it answers and its form cut into sound segments.
//!
//! - `languages.csv`: the project's language.
//! - `parameters.csv`: the concepts of the lists, with their Concepticon
//!   concept sets.
//! - `forms.csv`: one row per sense answering a concept: the headword as
//!   written (`Value`, `Form`), its segments, the sense's own gloss.
//! - `edictor.tsv`: the same forms as the tab-separated wordlist EDICTOR and
//!   LingPy open (`ID`, `DOCULECT`, `CONCEPT`, `CONCEPTICON_ID`, `VALUE`,
//!   `FORM`, `TOKENS`, `NOTE`), for cognate judgements.
//!
//! The caller says how a form is cut into segments (the desktop uses the
//! project's orthography profile, else letters with their marks), so this
//! crate holds no Unicode tables.

use wugs_core::Project;
use wugs_model::Value;

use crate::{cldf_id, col, live, plural, pretty, text_of, Column, Table, TERMS};

pub struct WordlistOptions<'a> {
    pub title: &'a str,
    pub generator: &'a str,
    /// One concept list by id, or every list the project holds.
    pub list: Option<&'a str>,
    /// A form as sound segments (empty when it cannot be cut).
    pub segment: &'a dyn Fn(&str) -> Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Wordlist {
    /// (file name, contents), metadata first.
    pub files: Vec<(String, String)>,
    pub lists: usize,
    pub concepts: usize,
    /// Concepts with at least one form.
    pub answered: usize,
    pub forms: usize,
    /// Concepts marked not applicable (N/A) in the Elicit panel.
    pub skipped: usize,
    pub notes: Vec<String>,
}

struct Concept {
    id: String,
    gloss: String,
    concepticon: String,
    concepticon_gloss: String,
    skipped: bool,
}

fn concepts(p: &Project, list: &str) -> Result<Vec<Concept>, String> {
    let mut rows = Vec::new();
    for id in p.proj.children_by_ref("list", list, "concept")? {
        let number = p.proj.field_num(&id, "number")?.unwrap_or(f64::MAX);
        rows.push((number, id));
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    rows.into_iter()
        .map(|(_, id)| {
            Ok(Concept {
                gloss: text_of(p, &id, "gloss")?,
                concepticon: text_of(p, &id, "concepticon")?,
                concepticon_gloss: text_of(p, &id, "concepticon-gloss")?,
                skipped: matches!(p.proj.field(&id, "skipped")?, Some((Value::Bool(true), _))),
                id,
            })
        })
        .collect()
}

/// Build the dataset.
pub fn export_wordlist(p: &Project, o: &WordlistOptions) -> Result<Wordlist, String> {
    let mut out = Wordlist::default();
    let lang = p.project_language()?;
    let lang_id = if lang.glottocode.is_empty() { "language".to_string() } else { lang.glottocode.clone() };
    let name = if lang.name.is_empty() { "Unnamed language".to_string() } else { lang.name.clone() };
    let languages = Table {
        url: "languages.csv",
        component: "LanguageTable",
        columns: vec![col("ID", Some("id")), col("Name", Some("name")), col("Glottocode", Some("glottocode")), col("ISO639P3code", Some("iso639P3code"))],
        rows: vec![vec![lang_id.clone(), name.clone(), lang.glottocode.clone(), lang.iso.clone()]],
        foreign: Vec::new(),
    };
    if lang.glottocode.is_empty() {
        out.notes.push("The project's language has no Glottocode yet; fill it in under Import and export so the word list can join others.".into());
    }
    let mut lists = Vec::new();
    for l in p.proj.entities_of_kind("concept-list")? {
        if o.list.is_none_or(|want| want == l) && live(p, &l, "concept-list")? {
            lists.push(l);
        }
    }
    lists.sort_by_key(|l| text_of(p, l, "name").unwrap_or_default());
    if o.list.is_some() && lists.is_empty() {
        return Err("That word list is not in the project.".into());
    }
    out.lists = lists.len();

    let mut parameters = Table {
        url: "parameters.csv",
        component: "ParameterTable",
        columns: vec![col("ID", Some("id")), col("Name", Some("name")), col("Concepticon_ID", Some("concepticonReference")), col("Concepticon_Gloss", None), col("List", None)],
        rows: Vec::new(),
        foreign: Vec::new(),
    };
    let mut forms = Table {
        url: "forms.csv",
        component: "FormTable",
        columns: vec![
            col("ID", Some("id")),
            Column { required: true, ..col("Language_ID", Some("languageReference")) },
            Column { required: true, ..col("Parameter_ID", Some("parameterReference")) },
            Column { required: true, ..col("Value", Some("value")) },
            Column { required: true, ..col("Form", Some("form")) },
            Column { separator: Some(" "), ..col("Segments", Some("segments")) },
            col("Gloss", None),
            col("Comment", Some("comment")),
            col("Entry_ID", None),
        ],
        rows: Vec::new(),
        foreign: vec![("Language_ID", "languages.csv"), ("Parameter_ID", "parameters.csv")],
    };
    let mut edictor = String::from("ID\tDOCULECT\tCONCEPT\tCONCEPTICON_ID\tVALUE\tFORM\tTOKENS\tNOTE\n");
    let tsv = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    let mut seen_params = std::collections::HashSet::new();
    let mut uncut = 0usize;
    for list in &lists {
        let list_name = text_of(p, list, "name")?;
        for c in concepts(p, list)? {
            out.concepts += 1;
            if c.skipped {
                out.skipped += 1;
            }
            let pid = cldf_id(&c.id);
            if seen_params.insert(pid.clone()) {
                parameters.rows.push(vec![pid.clone(), c.gloss.clone(), c.concepticon.clone(), c.concepticon_gloss.clone(), list_name.clone()]);
            }
            let senses = p.concept_answers(&c.id)?;
            if !senses.is_empty() {
                out.answered += 1;
            }
            for sense in senses {
                let lexeme = text_of(p, &sense, "lexeme")?;
                let value = text_of(p, &lexeme, "lemma")?;
                if value.trim().is_empty() {
                    continue;
                }
                let segments = (o.segment)(&value);
                if segments.is_empty() {
                    uncut += 1;
                }
                let gloss = text_of(p, &sense, "gloss")?;
                let comment = if gloss.trim().eq_ignore_ascii_case(c.gloss.trim()) { String::new() } else { gloss.clone() };
                out.forms += 1;
                let fid = format!("{lang_id}-{}-{}", out.forms, cldf_id(&sense));
                forms.rows.push(vec![fid, lang_id.clone(), pid.clone(), value.clone(), value.clone(), segments.join(" "), gloss.clone(), comment.clone(), cldf_id(&lexeme)]);
                edictor.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", out.forms, tsv(&name), tsv(&c.gloss), tsv(&c.concepticon), tsv(&value), tsv(&value), tsv(&segments.join(" ")), tsv(&comment)));
            }
        }
    }
    if out.forms == 0 {
        out.notes.push("No concept has an answer yet: answer concepts in the Elicit panel's word lists first.".into());
    }
    if uncut > 0 {
        out.notes.push(format!("{} could not be cut into segments and {} an empty Segments cell.", plural(uncut, "form", "forms"), if uncut == 1 { "has" } else { "have" }));
    }
    let tables = [&languages, &parameters, &forms];
    let metadata = Value::obj(vec![
        ("@context", Value::Arr(vec![Value::str("http://www.w3.org/ns/csvw"), Value::obj(vec![("@language", Value::str("en"))])])),
        ("dc:conformsTo", Value::str(&format!("{TERMS}Wordlist"))),
        ("dc:title", Value::str(o.title)),
        ("prov:wasGeneratedBy", Value::Arr(vec![Value::obj(vec![("dc:title", Value::str(o.generator)), ("dc:description", Value::str("https://github.com/judecutler/wugs"))])])),
        ("tables", Value::Arr(tables.iter().map(|t| t.schema()).collect())),
    ]);
    out.files.push(("Wordlist-metadata.json".into(), pretty(&metadata)));
    for t in tables {
        out.files.push((t.url.to_string(), t.csv()));
    }
    out.files.push(("edictor.tsv".into(), edictor));
    out.files.push((
        "README.md".into(),
        format!(
            "# {title}\n\nA CLDF Wordlist of {name}, exported from wugs ({generator}): {forms} answering {answered} of {concepts} in {lists}.\n\n\
- `forms.csv`: each form as written (`Value`, `Form`), cut into sound segments (`Segments`), with the gloss its lexicon sense gives\n\
- `parameters.csv`: the concepts, with their Concepticon concept sets\n\
- `languages.csv`: the language\n\
- `edictor.tsv`: the same forms as a wordlist EDICTOR (https://digling.org/edictor/) and LingPy open, for cognate judgements\n\n\
Validate with `pip install pycldf` and `cldf validate Wordlist-metadata.json`. Before publishing, add a licence and a citation, and check that the speakers agreed to publication.\n",
            title = o.title,
            generator = o.generator,
            forms = plural(out.forms, "form", "forms"),
            answered = out.answered,
            concepts = plural(out.concepts, "concept", "concepts"),
            lists = plural(out.lists, "list", "lists"),
        ),
    ));
    Ok(out)
}
