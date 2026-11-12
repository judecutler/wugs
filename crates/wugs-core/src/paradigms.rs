//! Paradigms (roadmap item 50, `docs/design/ELICITATION.md` "Paradigms"):
//! a lexeme's inflected forms, each a `Form` of kind `inflected` with a
//! surface (`form`) and a feature set, laid out as a grid of feature-value
//! combinations (person × number, tense × person, …).
//!
//! Each feature is its own sibling field `feature:<name>` (`feature:person`
//! = `1`), as glosses are `gloss:<tag>`, so per-field LWW merges two
//! devices' answers about different features of one form. Coverage is
//! derived from the forms on every read (`Project::paradigm`), never stored.
//!
//! As in `lexicon.rs`, a person's edit of a form's surface or features is
//! resolving (it settles that field's conflicts); creating and deleting a
//! form are plain. Every intent returns its commit's findings.

use std::collections::BTreeMap;

use wugs_model::Value;

use crate::{CommitResult, Project, SCOPE_LEXICON};

/// The prefix of a form's feature fields: `feature:person`, `feature:number`.
pub const FEATURE_PREFIX: &str = "feature:";

/// One axis of a paradigm: a feature and the values it takes, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dimension {
    pub feature: String,
    pub values: Vec<String>,
}

/// A live inflected form of a lexeme with its non-empty features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InflectedForm {
    pub id: String,
    pub form: String,
    pub features: BTreeMap<String, String>,
}

/// One combination of the paradigm's feature values and the forms that
/// carry exactly it. Usually one form; two devices filling the same cell
/// before a sync leave two, both shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub features: BTreeMap<String, String>,
    pub forms: Vec<InflectedForm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paradigm {
    /// Every combination, the first dimension outermost (row by row).
    pub cells: Vec<Cell>,
    /// The lexeme's inflected forms that fit no cell: a feature missing, a
    /// value outside the dimensions, or a feature the paradigm does not have.
    pub others: Vec<InflectedForm>,
}

impl Paradigm {
    /// Cells holding at least one form.
    pub fn filled(&self) -> usize {
        self.cells.iter().filter(|c| !c.forms.is_empty()).count()
    }
}

/// `feature:<name>` for a feature name of lowercase letters, digits and
/// hyphens (1 to 35 characters); upper case is folded, so `Person` and
/// `person` are one field.
pub fn feature_field(name: &str) -> Result<String, String> {
    let name = name.trim();
    let ok = (1..=35).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if !ok {
        return Err(format!("{name:?} is not a feature name (letters, digits and hyphens, 1 to 35 characters)"));
    }
    Ok(format!("{FEATURE_PREFIX}{}", name.to_ascii_lowercase()))
}

impl Project {
    /// A new inflected form of `lexeme`: `(form id, findings)`. Features
    /// with an empty value are left out.
    pub fn add_inflected_form(
        &mut self,
        lexeme: &str,
        surface: &str,
        features: &[(&str, &str)],
    ) -> Result<(String, CommitResult), String> {
        self.require(lexeme, "lexeme")?;
        let surface = surface.trim();
        if surface.is_empty() {
            return Err("a form needs a surface".into());
        }
        let mut fields =
            vec![("lexeme".to_string(), Value::str(lexeme)), ("kind".into(), Value::str("inflected")), ("form".into(), Value::str(surface))];
        for (name, value) in features {
            let field = feature_field(name)?;
            if fields.iter().any(|(f, _)| *f == field) {
                return Err(format!("feature {name} is given twice"));
            }
            if !value.trim().is_empty() {
                fields.push((field, Value::str(value.trim())));
            }
        }
        let mut t = self.intent("add-form");
        let id = t.create("form", "frm", SCOPE_LEXICON, fields.iter().map(|(f, v)| (f.as_str(), v.clone())).collect());
        let res = t.commit()?;
        Ok((id, res))
    }

    /// Retype a form's surface. An empty surface is refused (delete the form instead).
    pub fn set_form_surface(&mut self, form: &str, surface: &str) -> Result<CommitResult, String> {
        self.require(form, "form")?;
        let surface = surface.trim();
        if surface.is_empty() {
            return Err("a form needs a surface; delete the form to remove it".into());
        }
        let mut t = self.intent("edit-form").resolving();
        t.set("form", form, SCOPE_LEXICON, "form", Value::str(surface))?;
        t.commit()
    }

    /// Set some of a form's features in one txn; "" clears one. Features not
    /// named keep their values.
    pub fn set_form_features(&mut self, form: &str, features: &[(&str, &str)]) -> Result<CommitResult, String> {
        self.require(form, "form")?;
        let mut t = self.intent("edit-form").resolving();
        for (name, value) in features {
            t.set("form", form, SCOPE_LEXICON, &feature_field(name)?, Value::str(value.trim()))?;
        }
        t.commit()
    }

    /// Delete a form (a tombstone: it stays in the log and can be restored).
    pub fn delete_form(&mut self, form: &str) -> Result<CommitResult, String> {
        self.require(form, "form")?;
        let mut t = self.intent("delete-form");
        t.delete("form", form, SCOPE_LEXICON);
        t.commit()
    }

    /// The lexeme's live inflected forms, by surface then id.
    pub fn inflected_forms(&self, lexeme: &str) -> Result<Vec<InflectedForm>, String> {
        let mut out = Vec::new();
        for id in self.proj.children_by_ref("lexeme", lexeme, "form")? {
            if self.proj.field_str(&id, "kind")?.as_deref() != Some("inflected") {
                continue;
            }
            let mut form = String::new();
            let mut features = BTreeMap::new();
            for (field, value) in self.proj.fields_of(&id)? {
                match (field.strip_prefix(FEATURE_PREFIX), value.as_str()) {
                    (Some(name), Some(v)) if !v.is_empty() => {
                        features.insert(name.to_string(), v.to_string());
                    }
                    (None, Some(v)) if field == "form" => form = v.to_string(),
                    _ => {}
                }
            }
            out.push(InflectedForm { id, form, features });
        }
        out.sort_by(|a, b| a.form.cmp(&b.form).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// The lexeme's paradigm over `dims`: one cell per combination of their
    /// values. A form sits in a cell when its features are exactly the
    /// cell's, no more and no fewer; every other inflected form is listed in
    /// `others`, so nothing the lexicon holds is hidden by the choice of grid.
    pub fn paradigm(&self, lexeme: &str, dims: &[Dimension]) -> Result<Paradigm, String> {
        self.require(lexeme, "lexeme")?;
        let mut names = Vec::new();
        for d in dims {
            let name = feature_field(&d.feature)?[FEATURE_PREFIX.len()..].to_string();
            if names.contains(&name) {
                return Err(format!("feature {name} is a dimension twice"));
            }
            names.push(name);
        }
        // The grid is the product of the axes, and the axes come from the
        // caller, so an absurd request is refused before it is built.
        let cells: usize = dims.iter().map(|d| d.values.len().max(1)).product();
        if cells > 256 {
            return Err(format!("a grid of {cells} cells is more than the 256 a paradigm draws"));
        }
        let mut combos: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];
        for (d, name) in dims.iter().zip(&names) {
            combos = combos
                .into_iter()
                .flat_map(|c| {
                    d.values.iter().map(move |v| {
                        let mut c = c.clone();
                        c.insert(name.clone(), v.trim().to_string());
                        c
                    })
                })
                .collect();
        }
        let mut cells: Vec<Cell> = combos.into_iter().map(|features| Cell { features, forms: Vec::new() }).collect();
        let mut others = Vec::new();
        for f in self.inflected_forms(lexeme)? {
            match cells.iter_mut().find(|c| c.features == f.features) {
                Some(cell) => cell.forms.push(f),
                None => others.push(f),
            }
        }
        Ok(Paradigm { cells, others })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(actor: &str) -> (std::path::PathBuf, Project) {
        let root = std::env::temp_dir().join(format!("wugs-paradigm-test-{}", wugs_model::ulid()));
        let p = Project::open(&root, actor).unwrap();
        (root, p)
    }

    fn person_number() -> Vec<Dimension> {
        vec![
            Dimension { feature: "person".into(), values: vec!["1".into(), "2".into(), "3".into()] },
            Dimension { feature: "number".into(), values: vec!["sg".into(), "pl".into()] },
        ]
    }

    fn cell<'a>(p: &'a Paradigm, person: &str, number: &str) -> &'a Cell {
        p.cells.iter().find(|c| c.features["person"] == person && c.features["number"] == number).unwrap()
    }

    fn surfaces(c: &Cell) -> Vec<&str> {
        c.forms.iter().map(|f| f.form.as_str()).collect()
    }

    #[test]
    fn forms_fill_the_grid_and_coverage_is_derived() {
        let (root, mut p) = project("prs_Ana");
        let ((lex, _), _) = p.add_lexeme("gitmek", "go").unwrap();
        let (gidiyorum, res) = p.add_inflected_form(&lex, "gidiyorum", &[("person", "1"), ("number", "sg")]).unwrap();
        assert!(res.findings.is_empty(), "{:?}", res.findings);
        p.add_inflected_form(&lex, "gidiyorlar", &[("Person", "3"), ("number", "pl")]).unwrap();
        p.add_inflected_form(&lex, "gitti", &[("tense", "pst"), ("person", "3"), ("number", "sg")]).unwrap();
        p.add_inflected_form(&lex, "git", &[]).unwrap();

        let grid = p.paradigm(&lex, &person_number()).unwrap();
        assert_eq!(grid.cells.len(), 6);
        assert_eq!(
            grid.cells.iter().map(|c| format!("{}{}", c.features["person"], c.features["number"])).collect::<Vec<_>>(),
            ["1sg", "1pl", "2sg", "2pl", "3sg", "3pl"],
            "first dimension outermost"
        );
        assert_eq!(surfaces(cell(&grid, "1", "sg")), ["gidiyorum"]);
        assert_eq!(surfaces(cell(&grid, "3", "pl")), ["gidiyorlar"]);
        assert_eq!(grid.filled(), 2);
        let others: Vec<_> = grid.others.iter().map(|f| f.form.as_str()).collect();
        assert_eq!(others, ["git", "gitti"], "an extra feature or none keeps a form out of the grid");

        p.set_form_surface(&gidiyorum, " gidiyom ").unwrap();
        assert!(p.set_form_surface(&gidiyorum, "  ").is_err());
        assert_eq!(surfaces(cell(&p.paradigm(&lex, &person_number()).unwrap(), "1", "sg")), ["gidiyom"]);

        p.set_form_features(&gidiyorum, &[("number", "pl")]).unwrap();
        let grid = p.paradigm(&lex, &person_number()).unwrap();
        assert!(cell(&grid, "1", "sg").forms.is_empty());
        assert_eq!(surfaces(cell(&grid, "1", "pl")), ["gidiyom"], "editing a feature moves the form");
        p.set_form_features(&gidiyorum, &[("number", "")]).unwrap();
        assert_eq!(p.inflected_forms(&lex).unwrap().iter().find(|f| f.id == gidiyorum).unwrap().features.len(), 1, "\"\" clears");

        p.delete_form(&gidiyorum).unwrap();
        assert!(p.proj.is_tombstoned(&gidiyorum).unwrap());
        assert_eq!(p.inflected_forms(&lex).unwrap().len(), 3);
        assert!(p.set_form_surface(&gidiyorum, "x").is_err(), "a deleted form is not edited");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn bad_input_writes_nothing() {
        let (root, mut p) = project("prs_Ana");
        let ((lex, sen), _) = p.add_lexeme("ev", "house").unwrap();
        let before = p.log.read_all().unwrap().ops.len();
        assert!(p.add_inflected_form(&lex, " ", &[("number", "pl")]).is_err());
        assert!(p.add_inflected_form(&sen, "evler", &[]).is_err(), "only a lexeme has forms");
        assert!(p.add_inflected_form(&lex, "evler", &[("num ber", "pl")]).is_err());
        assert!(p.add_inflected_form(&lex, "evler", &[("number", "pl"), ("Number", "sg")]).is_err());
        let twice = vec![Dimension { feature: "number".into(), values: vec![] }; 2];
        assert!(p.paradigm(&lex, &twice).is_err());
        let huge = Dimension { feature: "person".into(), values: (0..257).map(|i| i.to_string()).collect() };
        assert!(p.paradigm(&lex, &[huge]).unwrap_err().contains("256"), "an absurd grid is refused unbuilt");
        assert_eq!(p.log.read_all().unwrap().ops.len(), before);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn allomorphs_stay_out_and_an_inflected_form_finds_its_entry() {
        let (root, mut p) = project("prs_Ana");
        let ((lex, sen), _) = p.add_lexeme("ev", "house").unwrap();
        let mut t = p.intent("test");
        t.create("form", "frm", SCOPE_LEXICON, vec![("lexeme", Value::str(&lex)), ("kind", Value::str("allomorph")), ("form", Value::str("ew"))]);
        t.commit().unwrap();
        p.add_inflected_form(&lex, "evler", &[("number", "pl")]).unwrap();
        assert_eq!(p.inflected_forms(&lex).unwrap().len(), 1);
        let number = [Dimension { feature: "number".into(), values: vec!["sg".into(), "pl".into()] }];
        assert_eq!(p.paradigm(&lex, &number).unwrap().filled(), 1);
        // Glossing looks an inflected surface up like an allomorph.
        assert!(p.lexicon_lookup("Evler").unwrap().iter().any(|m| m.sense == sen));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn concurrent_edits_of_one_form_conflict_and_a_direct_edit_settles_them() {
        let (ra, mut a) = project("prs_Ana");
        let (rb, mut b) = project("prs_Bo");
        let ((lex, _), _) = a.add_lexeme("gitmek", "go").unwrap();
        let (frm, _) = a.add_inflected_form(&lex, "gidiyorum", &[("person", "1"), ("number", "sg")]).unwrap();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();

        a.set_form_surface(&frm, "gidiyom").unwrap();
        b.set_form_surface(&frm, "gideyrum").unwrap();
        // Different features of one form merge field by field.
        a.set_form_features(&frm, &[("tense", "prs")]).unwrap();
        b.set_form_features(&frm, &[("mood", "ind")]).unwrap();
        let theirs = |p: &Project| -> Vec<_> { p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == p.device()).collect() };
        let from_b = theirs(&b);
        let from_a = theirs(&a);
        a.ingest(from_b).unwrap();
        b.ingest(from_a).unwrap();

        assert_eq!(a.proj.count("conflicts").unwrap(), 1, "one surface, two values");
        assert!(a.proj.has_conflict(&frm, "form").unwrap());
        assert_eq!(a.inflected_forms(&lex).unwrap(), b.inflected_forms(&lex).unwrap(), "both devices show the same winner");
        let f = &a.inflected_forms(&lex).unwrap()[0];
        assert_eq!((f.features["tense"].as_str(), f.features["mood"].as_str()), ("prs", "ind"));

        a.set_form_surface(&frm, "gidiyorum").unwrap();
        assert_eq!(a.proj.count("conflicts").unwrap(), 0);
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn two_devices_filling_one_cell_keep_both_forms_visible() {
        let (ra, mut a) = project("prs_Ana");
        let (rb, mut b) = project("prs_Bo");
        let ((lex, _), _) = a.add_lexeme("gitmek", "go").unwrap();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        a.add_inflected_form(&lex, "gidiyorum", &[("person", "1"), ("number", "sg")]).unwrap();
        b.add_inflected_form(&lex, "gidiyom", &[("person", "1"), ("number", "sg")]).unwrap();
        a.ingest(b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect()).unwrap();
        let grid = a.paradigm(&lex, &person_number()).unwrap();
        assert_eq!(surfaces(cell(&grid, "1", "sg")), ["gidiyom", "gidiyorum"]);
        assert_eq!(grid.filled(), 1);
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }
}
