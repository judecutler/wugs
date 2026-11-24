//! A project's answers to a typological questionnaire as a CLDF
//! StructureDataset, the shape Grambank and WALS publish and what their
//! tools read: the language, the features answered, each feature's codes,
//! and one value per answer with its note. The caller gives the features
//! and the answers (the desktop holds the Grambank and WALS tables), so this
//! crate holds no dataset.

use wugs_core::Project;
use wugs_model::Value;

use crate::{cldf_id, col, plural, pretty, Column, Table, TERMS};

/// One feature of the questionnaire, with its codes as `(value, label)`.
pub struct Feature<'a> {
    pub id: &'a str,
    pub question: &'a str,
    pub codes: Vec<(String, &'a str)>,
}

/// One answer: a code of its feature, or `?` for asked but not known.
pub struct Answer<'a> {
    pub feature: &'a str,
    pub value: &'a str,
    pub note: &'a str,
}

pub struct StructureOptions<'a> {
    pub title: &'a str,
    pub generator: &'a str,
    /// The questionnaire, in words, with its edition and licence.
    pub source: &'a str,
    pub features: &'a [Feature<'a>],
    pub answers: &'a [Answer<'a>],
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Structure {
    /// (file name, contents), metadata first.
    pub files: Vec<(String, String)>,
    /// Answers with a code, and answers of `?`.
    pub coded: usize,
    pub unknown: usize,
    pub notes: Vec<String>,
}

/// Build the dataset.
pub fn export_structure(p: &Project, o: &StructureOptions) -> Result<Structure, String> {
    let mut out = Structure::default();
    let lang = p.project_language()?;
    let lang_id = if lang.glottocode.is_empty() { "language".to_string() } else { lang.glottocode.clone() };
    let name = if lang.name.is_empty() { "Unnamed language".to_string() } else { lang.name.clone() };
    if lang.glottocode.is_empty() {
        out.notes.push("The project's language has no Glottocode yet; fill it in under Import and export so the answers can join Grambank's or WALS's.".into());
    }
    let languages = Table {
        url: "languages.csv",
        component: "LanguageTable",
        columns: vec![col("ID", Some("id")), col("Name", Some("name")), col("Glottocode", Some("glottocode")), col("ISO639P3code", Some("iso639P3code"))],
        rows: vec![vec![lang_id.clone(), name.clone(), lang.glottocode.clone(), lang.iso.clone()]],
        foreign: Vec::new(),
    };
    let mut parameters = Table { url: "parameters.csv", component: "ParameterTable", columns: vec![col("ID", Some("id")), col("Name", Some("name")), col("Description", Some("description"))], rows: Vec::new(), foreign: Vec::new() };
    let mut codes = Table {
        url: "codes.csv",
        component: "CodeTable",
        columns: vec![col("ID", Some("id")), Column { required: true, ..col("Parameter_ID", Some("parameterReference")) }, col("Name", Some("name")), col("Description", Some("description"))],
        rows: Vec::new(),
        foreign: vec![("Parameter_ID", "parameters.csv")],
    };
    let mut values = Table {
        url: "values.csv",
        component: "ValueTable",
        columns: vec![
            col("ID", Some("id")),
            Column { required: true, ..col("Language_ID", Some("languageReference")) },
            Column { required: true, ..col("Parameter_ID", Some("parameterReference")) },
            col("Value", Some("value")),
            col("Code_ID", Some("codeReference")),
            col("Comment", Some("comment")),
        ],
        rows: Vec::new(),
        foreign: vec![("Language_ID", "languages.csv"), ("Parameter_ID", "parameters.csv"), ("Code_ID", "codes.csv")],
    };
    for f in o.features {
        let Some(a) = o.answers.iter().find(|a| a.feature == f.id) else { continue };
        let value = a.value.trim();
        if value.is_empty() {
            continue;
        }
        let pid = cldf_id(f.id);
        parameters.rows.push(vec![pid.clone(), f.id.to_string(), f.question.to_string()]);
        for (v, label) in &f.codes {
            codes.rows.push(vec![format!("{pid}-{}", cldf_id(v)), pid.clone(), v.clone(), label.to_string()]);
        }
        let code = if value == "?" {
            out.unknown += 1;
            String::new()
        } else if f.codes.iter().any(|(v, _)| v == value) {
            out.coded += 1;
            format!("{pid}-{}", cldf_id(value))
        } else {
            return Err(format!("«{value}» is not an answer to {}.", f.id));
        };
        values.rows.push(vec![format!("{lang_id}-{pid}"), lang_id.clone(), pid.clone(), value.to_string(), code, a.note.trim().to_string()]);
    }
    if values.rows.is_empty() {
        out.notes.push("No feature is answered yet: answer features in the Typology panel first.".into());
    }
    let tables = [&languages, &parameters, &codes, &values];
    let metadata = Value::obj(vec![
        ("@context", Value::Arr(vec![Value::str("http://www.w3.org/ns/csvw"), Value::obj(vec![("@language", Value::str("en"))])])),
        ("dc:conformsTo", Value::str(&format!("{TERMS}StructureDataset"))),
        ("dc:title", Value::str(o.title)),
        ("dc:source", Value::str(o.source)),
        ("prov:wasGeneratedBy", Value::Arr(vec![Value::obj(vec![("dc:title", Value::str(o.generator)), ("dc:description", Value::str("https://github.com/judecutler/wugs"))])])),
        ("tables", Value::Arr(tables.iter().map(|t| t.schema()).collect())),
    ]);
    out.files.push(("StructureDataset-metadata.json".into(), pretty(&metadata)));
    for t in tables {
        out.files.push((t.url.to_string(), t.csv()));
    }
    out.files.push((
        "README.md".into(),
        format!(
            "# {title}\n\nA CLDF StructureDataset of {name}, exported from wugs ({generator}): {answers} to the questions of {source} ({coded} coded, {unknown} asked but not known, `?`).\n\n\
- `values.csv`: one row per answered feature: the value, its code and the note written with it\n\
- `parameters.csv`: the features answered, with their questions\n\
- `codes.csv`: each answered feature's possible answers\n\
- `languages.csv`: the language\n\n\
The questions and codes are {source}'s; cite it beside this dataset. Validate with `pip install pycldf` and `cldf validate StructureDataset-metadata.json`. Before publishing, add a licence and a citation.\n",
            title = o.title,
            generator = o.generator,
            answers = plural(out.coded + out.unknown, "answer", "answers"),
            source = o.source,
            coded = out.coded,
            unknown = out.unknown,
        ),
    ));
    Ok(out)
}
