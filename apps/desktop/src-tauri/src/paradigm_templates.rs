//! A project's own paradigm templates (the Lexicon panel's Paradigm grids
//! beyond the three built in, `src/lexicon/paradigms.ts`): case × number,
//! noun class × number, whatever the language needs. Each is a project
//! setting `paradigm-template:<id>` holding the template as JSON, so the
//! team shares them and two people making different templates never
//! conflict. The engine still knows no templates: a grid is only the
//! features and values the panel asks for.

use serde::{Deserialize, Serialize};

use crate::session::{self, Session};

const PREFIX: &str = "paradigm-template:";
/// Values one axis may hold; a grid is for looking at.
const MAX_VALUES: usize = 16;
const MAX_LABEL: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureValue {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Axis {
    pub feature: String,
    pub label: String,
    pub values: Vec<FeatureValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub label: String,
    pub rows: Axis,
    pub cols: Option<Axis>,
}

fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn clean(s: &str, nfc: &dyn Fn(&str) -> String) -> String {
    nfc(&s.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// A template as it is kept: names trimmed, every rule checked, so a bad
/// one is refused in words before it is shared.
fn checked(mut t: Template, nfc: &dyn Fn(&str) -> String) -> Result<Template, String> {
    if !valid_id(&t.id) {
        return Err(format!("«{}» is not a template id (small letters, digits and hyphens)", t.id));
    }
    t.label = clean(&t.label, nfc);
    if t.label.is_empty() || t.label.chars().count() > MAX_LABEL {
        return Err(format!("A template needs a name of at most {MAX_LABEL} characters."));
    }
    let axis = |a: &mut Axis| -> Result<(), String> {
        a.feature = a.feature.trim().to_ascii_lowercase();
        wugs_core::paradigms::feature_field(&a.feature)?;
        a.label = clean(&a.label, nfc);
        if a.label.is_empty() {
            a.label = a.feature.clone();
        }
        for v in &mut a.values {
            v.value = clean(&v.value, nfc);
            v.label = clean(&v.label, nfc);
            if v.label.is_empty() {
                v.label = v.value.clone();
            }
        }
        a.values.retain(|v| !v.value.is_empty());
        if a.values.is_empty() || a.values.len() > MAX_VALUES {
            return Err(format!("«{}» needs between 1 and {MAX_VALUES} values.", a.label));
        }
        let mut seen = std::collections::HashSet::new();
        if let Some(v) = a.values.iter().find(|v| !seen.insert(v.value.to_lowercase())) {
            return Err(format!("«{}» lists the value «{}» twice.", a.label, v.value));
        }
        Ok(())
    };
    axis(&mut t.rows)?;
    if let Some(c) = t.cols.as_mut() {
        axis(c)?;
        if c.feature == t.rows.feature {
            return Err("The rows and the columns need two different features.".into());
        }
    }
    Ok(t)
}

/// The project's own templates, by name. One that does not read (written
/// by a later version, say) is left out rather than failing the list.
pub fn templates(session: &mut Session) -> Result<Vec<Template>, String> {
    let p = session::project(session)?;
    let mut out: Vec<Template> = p
        .settings_with_prefix(PREFIX)?
        .into_iter()
        .filter_map(|(_, json)| serde_json::from_str::<Template>(&json).ok())
        .filter_map(|t| checked(t, &|s: &str| s.to_string()).ok())
        .collect();
    out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// Keep a template (a new id makes one, a known id replaces it); the whole list back.
pub fn save(session: &mut Session, template: Template) -> Result<Vec<Template>, String> {
    let p = session::project(session)?;
    let t = checked(template, &|s: &str| p.proj.nfc(s))?;
    let json = serde_json::to_string(&t).map_err(|e| e.to_string())?;
    let key = format!("{PREFIX}{}", t.id);
    if p.setting(&key)?.as_deref() != Some(json.as_str()) {
        p.set_setting(&key, &json).map(|_| ())?;
    }
    templates(session)
}

/// Remove a template (its forms stay in the lexicon); the whole list back.
pub fn delete(session: &mut Session, id: &str) -> Result<Vec<Template>, String> {
    if !valid_id(id) {
        return Err(format!("«{id}» is not a template id"));
    }
    let p = session::project(session)?;
    let key = format!("{PREFIX}{id}");
    if p.setting(&key)?.is_some_and(|v| !v.is_empty()) {
        p.set_setting(&key, "").map(|_| ())?;
    }
    templates(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(feature: &str, values: &[&str]) -> Axis {
        Axis { feature: feature.into(), label: String::new(), values: values.iter().map(|v| FeatureValue { value: v.to_string(), label: String::new() }).collect() }
    }

    #[test]
    fn a_projects_templates_are_kept_checked_and_removed() {
        let dir = std::env::temp_dir().join(format!("wugs-pdg-template-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut session: Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        assert!(templates(&mut session).unwrap().is_empty());
        let case = Template { id: "case-number".into(), label: "  Case ×  number ".into(), rows: axis("Case", &["nom", "acc", "gen", " "]), cols: Some(axis("number", &["sg", "pl"])) };
        let list = save(&mut session, case).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "Case × number");
        assert_eq!(list[0].rows.feature, "case");
        assert_eq!(list[0].rows.values.len(), 3, "an empty value is dropped");
        assert_eq!(list[0].rows.values[0].label, "nom", "a value with no label is labelled by itself");
        let class = Template { id: "class".into(), label: "Noun class".into(), rows: axis("class", &["1", "2", "3"]), cols: None };
        assert_eq!(save(&mut session, class).unwrap().iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["case-number", "class"]);
        let mut bad = |t: Template| save(&mut session, t).unwrap_err();
        assert!(bad(Template { id: "Bad Id".into(), label: "x".into(), rows: axis("a", &["1"]), cols: None }).contains("not a template id"));
        assert!(bad(Template { id: "x".into(), label: "x".into(), rows: axis("noun class", &["1"]), cols: None }).contains("not a feature name"));
        assert!(bad(Template { id: "x".into(), label: "x".into(), rows: axis("a", &["1", "1"]), cols: None }).contains("twice"));
        assert!(bad(Template { id: "x".into(), label: "x".into(), rows: axis("a", &["1"]), cols: Some(axis("A", &["2"])) }).contains("two different features"));
        assert_eq!(delete(&mut session, "class").unwrap().len(), 1);
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
