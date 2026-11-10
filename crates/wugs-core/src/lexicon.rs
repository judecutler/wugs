//! Lexicon intents (DATA-MODEL §3 "Lexicon", roadmap item 22): entries,
//! senses, glosses in several languages, definitions, and semantic domains.
//!
//! Field edits here are a person's direct edits in the Lexicon panel, so
//! their txns are `resolving()`: they name every open version of the field
//! they write and settle its conflicts (DATA-MODEL §2.2, §5.3). Creates,
//! deletes, links and the domain import write no existing field's value and
//! stay plain. Every intent
//! returns its commit's findings, so a duplicate lemma (validator #8) or a
//! character outside the writing system (#5) reaches the panel.

use wugs_model::Value;

use crate::{gloss_field, CommitResult, Project, SCOPE_CONFIG, SCOPE_LEXICON};

/// A lexeme field the panel may write. Others (`ws`, `language`, `x.*`) are
/// set by importers or later tools.
const LEXEME_FIELDS: &[&str] = &["lemma", "citation", "pos-of-speech"];

/// One semantic domain to load: SIL Semantic Domains or any list with the
/// same shape. `code` is the outline number (`1.1.1`); its parent is the code
/// minus its last part, which must come earlier in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainRow {
    pub code: String,
    /// Permanent identifier of the domain in its source list (SIL GUIDs).
    pub guid: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Default)]
pub struct ImportedDomains {
    pub created: usize,
    /// Rows whose domain the project already holds (an earlier load, or one
    /// from another device that arrived by sync). Left untouched.
    pub already: usize,
    pub findings: Vec<crate::Finding>,
}

/// The id a list's domain gets in every project: derived from the list's own
/// permanent GUID, so two devices that load the same list create the same
/// entities instead of two parallel trees. A ULID-shaped string (26
/// Crockford base-32 characters for the 128 bits), so it passes as an id.
pub fn domain_id(guid: &str) -> Result<String, String> {
    let hex: String = guid.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return Err(format!("not a GUID: {guid}"));
    }
    let n = u128::from_str_radix(&hex, 16).map_err(|_| format!("not a GUID: {guid}"))?;
    Ok(id_from_bits("dom", n))
}

/// `<prefix>_` + 128 bits as 26 Crockford base-32 characters: the shape of
/// a ULID, so `looks_like_id` accepts it.
pub(crate) fn id_from_bits(prefix: &str, n: u128) -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut out = [0u8; 26];
    for (i, slot) in out.iter_mut().enumerate() {
        let shift = 5 * (25 - i);
        *slot = ALPHABET[((n >> shift) & 31) as usize];
    }
    format!("{prefix}_{}", String::from_utf8(out.to_vec()).expect("ascii"))
}

/// A stable id for an entity named by a permanent key in a published list
/// (FNV-1a, 128 bit, over `key`): every device derives the same id.
pub(crate) fn id_from_key(prefix: &str, key: &str) -> String {
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    for b in key.bytes() {
        h ^= u128::from(b);
        h = h.wrapping_mul(0x0000000001000000000000000000013B);
    }
    id_from_bits(prefix, h)
}

impl Project {
    /// A new entry with one sense: `(lexeme, sense)` and the commit's findings.
    pub fn add_lexeme(&mut self, lemma: &str, gloss: &str) -> Result<((String, String), CommitResult), String> {
        let lemma = lemma.trim();
        if lemma.is_empty() {
            return Err("an entry needs a lemma".into());
        }
        let mut t = self.intent("create-lexeme");
        // No `ws` here: a lexeme's writing system is optional (set later with
        // Txn::set), and the charset audit only sees lexemes that carry one.
        let lex = t.create("lexeme", "lex", SCOPE_LEXICON, vec![("lemma", Value::str(lemma))]);
        let sen = t.create(
            "sense",
            "sen",
            SCOPE_LEXICON,
            vec![
                ("lexeme", Value::str(&lex)),
                ("gloss", Value::str(gloss.trim())),
                ("pos", Value::str(&wugs_model::pos_seq(0))),
            ],
        );
        let res = t.commit()?;
        Ok(((lex, sen), res))
    }

    /// Set `lemma`, `citation` or `pos-of-speech` of an entry. An empty
    /// lemma is refused; the other two may be cleared with "".
    pub fn set_lexeme_field(&mut self, lexeme: &str, field: &str, value: &str) -> Result<CommitResult, String> {
        self.require(lexeme, "lexeme")?;
        if !LEXEME_FIELDS.contains(&field) {
            return Err(format!("the lexicon does not edit a lexeme's {field}"));
        }
        let value = value.trim();
        if field == "lemma" && value.is_empty() {
            return Err("an entry needs a lemma".into());
        }
        let mut t = self.intent("edit-lexeme").resolving();
        t.set("lexeme", lexeme, SCOPE_LEXICON, field, Value::str(value))?;
        t.commit()
    }

    /// Number an entry as a homograph (1, 2, …), or clear the number with None.
    pub fn set_homograph(&mut self, lexeme: &str, number: Option<u32>) -> Result<CommitResult, String> {
        self.require(lexeme, "lexeme")?;
        let value = match number {
            Some(0) => return Err("homograph numbers start at 1".into()),
            Some(n) => Value::Num(f64::from(n)),
            None => Value::Null,
        };
        let mut t = self.intent("edit-lexeme").resolving();
        t.set("lexeme", lexeme, SCOPE_LEXICON, "homograph", value)?;
        t.commit()
    }

    /// Number every entry spelled like this one as homographs, in one txn:
    /// entries that already carry a number keep it, the others get the
    /// lowest free numbers in the order they were created (ids sort by time).
    pub fn number_homographs(&mut self, lexeme: &str) -> Result<CommitResult, String> {
        self.require(lexeme, "lexeme")?;
        let lemma = self.proj.nfc(&self.proj.field_str(lexeme, "lemma")?.unwrap_or_default());
        let key = wugs_model::lexicon_key(&lemma);
        let mut twins = Vec::new();
        for other in self.proj.by_lookup_key("lexeme", &key)? {
            if self.proj.entity_kind(&other)?.as_deref() == Some("lexeme")
                && !self.proj.is_tombstoned(&other)?
                && self.proj.nfc(&self.proj.field_str(&other, "lemma")?.unwrap_or_default()) == lemma
            {
                twins.push(other);
            }
        }
        twins.sort();
        let mut taken: Vec<u32> = Vec::new();
        for t in &twins {
            if let Some(n) = self.proj.field_num(t, "homograph")?.filter(|n| *n >= 1.0) {
                taken.push(n as u32);
            }
        }
        let mut t = self.intent("number-homographs").resolving();
        if twins.len() > 1 {
            for twin in &twins {
                if t.project.proj.field_num(twin, "homograph")?.filter(|n| *n >= 1.0).is_some() {
                    continue;
                }
                let mut n = 1;
                while taken.contains(&n) {
                    n += 1;
                }
                taken.push(n);
                t.set("lexeme", twin, SCOPE_LEXICON, "homograph", Value::Num(f64::from(n)))?;
            }
        }
        t.commit()
    }

    /// Delete an entry and its senses (tombstones, so both stay in the log
    /// and can be restored; morphs that linked a sense keep their glosses,
    /// and search and lookups stop offering the senses).
    pub fn delete_lexeme(&mut self, lexeme: &str) -> Result<CommitResult, String> {
        self.require(lexeme, "lexeme")?;
        let senses = self.proj.ordered_children("lexeme", lexeme, "sense")?;
        let mut t = self.intent("delete-lexeme");
        for sense in &senses {
            t.delete("sense", sense, SCOPE_LEXICON);
        }
        t.delete("lexeme", lexeme, SCOPE_LEXICON);
        t.commit()
    }

    /// A new sense after the entry's last one.
    pub fn add_sense(&mut self, lexeme: &str, gloss: &str) -> Result<(String, CommitResult), String> {
        self.require(lexeme, "lexeme")?;
        let mut last = String::new();
        for sense in self.proj.ordered_children("lexeme", lexeme, "sense")? {
            last = last.max(self.proj.field_str(&sense, "pos")?.unwrap_or_default());
        }
        let pos = after(&last);
        let mut t = self.intent("add-sense");
        let sen = t.create(
            "sense",
            "sen",
            SCOPE_LEXICON,
            vec![("lexeme", Value::str(lexeme)), ("gloss", Value::str(gloss.trim())), ("pos", Value::str(&pos))],
        );
        let res = t.commit()?;
        Ok((sen, res))
    }

    /// A sense's gloss in the primary analysis language (`lang` None) or in
    /// another language (`gloss:<tag>`). "" clears it.
    pub fn set_sense_gloss(&mut self, sense: &str, gloss: &str, lang: Option<&str>) -> Result<CommitResult, String> {
        self.require(sense, "sense")?;
        let field = gloss_field(lang)?;
        let mut t = self.intent("edit-sense").resolving();
        t.set("sense", sense, SCOPE_LEXICON, &field, Value::str(gloss.trim()))?;
        t.commit()
    }

    /// A sense's `definition` or `usage` note: collaborative text
    /// (DATA-MODEL §3), so two devices editing one definition merge.
    pub fn set_sense_text(&mut self, sense: &str, field: &str, text: &str) -> Result<CommitResult, String> {
        self.require(sense, "sense")?;
        if !matches!(field, "definition" | "usage") {
            return Err(format!("{field} is not a sense text field"));
        }
        let mut t = self.intent("edit-sense").resolving();
        t.text_edit("sense", sense, SCOPE_LEXICON, field, text)?;
        t.commit()
    }

    pub fn delete_sense(&mut self, sense: &str) -> Result<CommitResult, String> {
        self.require(sense, "sense")?;
        let mut t = self.intent("delete-sense");
        t.delete("sense", sense, SCOPE_LEXICON);
        t.commit()
    }

    /// The live semantic domains a sense is assigned to, as (link, domain).
    pub fn sense_domains(&self, sense: &str) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for link in self.proj.children_by_ref("sense", sense, "sense-domain")? {
            if let Some(dom) = self.proj.field_str(&link, "domain")? {
                if self.proj.entity_kind(&dom)?.as_deref() == Some("domain") && !self.proj.is_tombstoned(&dom)? {
                    out.push((link, dom));
                }
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        Ok(out)
    }

    /// Assign a sense to a semantic domain (a SenseDomain link entity,
    /// DATA-MODEL §3). Nothing is written when the link already exists.
    pub fn assign_domain(&mut self, sense: &str, domain: &str) -> Result<CommitResult, String> {
        self.require(sense, "sense")?;
        self.require(domain, "domain")?;
        let mut t = self.intent("assign-domain");
        if !t.project.sense_domains(sense)?.iter().any(|(_, d)| d == domain) {
            t.create("sense-domain", "sdm", SCOPE_LEXICON, vec![("sense", Value::str(sense)), ("domain", Value::str(domain))]);
        }
        t.commit()
    }

    /// Remove a sense from a domain: every live link between the two is
    /// tombstoned (two devices may each have made one).
    pub fn unassign_domain(&mut self, sense: &str, domain: &str) -> Result<CommitResult, String> {
        self.require(sense, "sense")?;
        let links: Vec<String> =
            self.sense_domains(sense)?.into_iter().filter(|(_, d)| d == domain).map(|(l, _)| l).collect();
        let mut t = self.intent("unassign-domain");
        for link in links {
            t.delete("sense-domain", &link, SCOPE_LEXICON);
        }
        t.commit()
    }

    /// Load a list of semantic domains into the project config, one txn, as
    /// `domain` entities (label, code, description, parent, pos, origin).
    /// Ids derive from each row's GUID (`domain_id`), so loading twice, or on
    /// two devices, never makes a second tree: a domain the project already
    /// holds, live or deleted, is left as it is. Rows must list parents
    /// before children.
    pub fn import_semantic_domains(&mut self, origin: &str, rows: &[DomainRow]) -> Result<ImportedDomains, String> {
        let mut out = ImportedDomains::default();
        let mut by_code: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
        let mut sibling: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut plan = Vec::with_capacity(rows.len());
        for row in rows {
            let id = domain_id(&row.guid)?;
            if by_code.insert(row.code.as_str(), id.clone()).is_some() {
                return Err(format!("domain code {} appears twice", row.code));
            }
            let parent = match row.code.rsplit_once('.') {
                Some((p, _)) => Some(
                    by_code.get(p).cloned().ok_or_else(|| format!("domain {} comes before its parent {p}", row.code))?,
                ),
                None => None,
            };
            let n = sibling.entry(parent.clone().unwrap_or_default()).or_insert(0);
            let pos = wugs_model::pos_seq(*n);
            *n += 1;
            if self.proj.entity_kind(&id)?.is_some() {
                out.already += 1;
                continue;
            }
            plan.push((id, row, parent, pos));
        }
        let mut t = self.intent("load-semantic-domains");
        for (id, row, parent, pos) in plan {
            let mut fields = vec![
                ("label", Value::str(&row.label)),
                ("code", Value::str(&row.code)),
                ("pos", Value::str(&pos)),
                ("origin", Value::str(&format!("{origin}:{}", row.guid.to_ascii_uppercase()))),
            ];
            if !row.description.is_empty() {
                fields.push(("description", Value::str(&row.description)));
            }
            if let Some(p) = &parent {
                fields.push(("parent", Value::str(p)));
            }
            t.push_create("domain", &id, SCOPE_CONFIG, fields);
            out.created += 1;
        }
        out.findings = t.commit()?.findings;
        Ok(out)
    }
}

/// A position after `last` in `pos_seq` order (eight hex digits), or the
/// first position when there is none.
fn after(last: &str) -> String {
    match u64::from_str_radix(last, 16) {
        Ok(n) if last.len() == 8 => wugs_model::pos_seq(n as usize + 1),
        _ if last.is_empty() => wugs_model::pos_seq(0),
        // A foreign pos (an import's own scheme): sort after it by extension.
        _ => format!("{last}~"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (std::path::PathBuf, Project) {
        let root = std::env::temp_dir().join(format!("wugs-lexicon-test-{}", wugs_model::ulid()));
        let p = Project::open(&root, "prs_Ana").unwrap();
        (root, p)
    }

    fn rows() -> Vec<DomainRow> {
        let row = |code: &str, guid: &str, label: &str| DomainRow {
            code: code.into(),
            guid: guid.into(),
            label: label.into(),
            description: format!("Use this domain for {label}."),
        };
        vec![
            row("1", "63403699-07C1-43F3-A47C-069D6E4316E5", "Universe, creation"),
            row("1.1", "999581C4-1611-4ACB-AE1B-5E6C1DFE6F0C", "Sky"),
            row("1.1.1", "DC1A2C6F-1B32-4631-8823-36DACC8CB7BB", "Sun"),
            row("1.2", "B47D2604-8B23-41E9-9158-01526DD83894", "World"),
        ]
    }

    #[test]
    fn domain_ids_are_ulid_shaped_and_stable() {
        let a = domain_id("dc1a2c6f-1b32-4631-8823-36dacc8cb7bb").unwrap();
        assert_eq!(a, domain_id("DC1A2C6F-1B32-4631-8823-36DACC8CB7BB").unwrap());
        assert!(wugs_model::looks_like_id(&a), "{a}");
        assert!(domain_id("not-a-guid").is_err());
    }

    #[test]
    fn domains_load_once_as_a_tree_and_a_second_load_adds_nothing() {
        let (root, mut p) = project();
        let r = p.import_semantic_domains("semdom:v4", &rows()).unwrap();
        assert_eq!((r.created, r.already), (4, 0));
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        let sun = domain_id("DC1A2C6F-1B32-4631-8823-36DACC8CB7BB").unwrap();
        let sky = domain_id("999581C4-1611-4ACB-AE1B-5E6C1DFE6F0C").unwrap();
        assert_eq!(p.proj.field_str(&sun, "parent").unwrap(), Some(sky.clone()));
        assert_eq!(p.proj.field_str(&sun, "code").unwrap().as_deref(), Some("1.1.1"));
        assert_eq!(p.proj.field_str(&sun, "origin").unwrap().as_deref(), Some("semdom:v4:DC1A2C6F-1B32-4631-8823-36DACC8CB7BB"));
        let world = domain_id("B47D2604-8B23-41E9-9158-01526DD83894").unwrap();
        assert!(p.proj.field_str(&sky, "pos").unwrap() < p.proj.field_str(&world, "pos").unwrap(), "siblings keep list order");
        let again = p.import_semantic_domains("semdom:v4", &rows()).unwrap();
        assert_eq!((again.created, again.already), (0, 4));
        assert_eq!(p.proj.entities_of_kind("domain").unwrap().len(), 4);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_child_before_its_parent_is_refused() {
        let (root, mut p) = project();
        let mut r = rows();
        r.swap(1, 2);
        assert!(p.import_semantic_domains("semdom:v4", &r).unwrap_err().contains("before its parent"));
        assert!(p.proj.entities_of_kind("domain").unwrap().is_empty(), "nothing written");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn entries_senses_glosses_and_domains() {
        let (root, mut p) = project();
        p.import_semantic_domains("semdom:v4", &rows()).unwrap();
        let sun = domain_id("DC1A2C6F-1B32-4631-8823-36DACC8CB7BB").unwrap();
        let ((lex, s1), _) = p.add_lexeme("güneş", "sun").unwrap();
        let (s2, _) = p.add_sense(&lex, "day").unwrap();
        assert_eq!(p.proj.ordered_children("lexeme", &lex, "sense").unwrap(), [s1.clone(), s2.clone()]);
        p.set_sense_gloss(&s1, "sol", Some("es")).unwrap();
        assert_eq!(p.proj.field_str(&s1, "gloss:es").unwrap().as_deref(), Some("sol"));
        p.set_sense_text(&s1, "definition", "the star the earth goes round").unwrap();
        assert_eq!(p.proj.field_str(&s1, "definition").unwrap().as_deref(), Some("the star the earth goes round"));
        p.set_lexeme_field(&lex, "pos-of-speech", "n").unwrap();
        assert!(p.set_lexeme_field(&lex, "ws", "ws_x").is_err());
        assert!(p.set_lexeme_field(&lex, "lemma", "  ").is_err());

        p.assign_domain(&s1, &sun).unwrap();
        let again = p.assign_domain(&s1, &sun).unwrap();
        assert_eq!(again.ops, 0, "an existing link is not written twice");
        assert_eq!(p.sense_domains(&s1).unwrap().len(), 1);
        p.unassign_domain(&s1, &sun).unwrap();
        assert!(p.sense_domains(&s1).unwrap().is_empty());
        assert!(p.assign_domain(&s1, &lex).is_err(), "only a domain can be assigned");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn loading_on_two_devices_before_a_sync_gives_one_tree() {
        let (ra, mut a) = project();
        let (rb, mut b) = project();
        a.import_semantic_domains("semdom:v4", &rows()).unwrap();
        b.import_semantic_domains("semdom:v4", &rows()).unwrap();
        let theirs: Vec<_> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect();
        a.ingest(theirs).unwrap();
        assert_eq!(a.proj.entities_of_kind("domain").unwrap().len(), 4, "the same ids, not a second tree");
        // The twin creates hold equal values: nothing to choose, no conflict.
        assert_eq!(a.proj.count("conflicts").unwrap(), 0);
        // A real difference after that still conflicts.
        let sun = domain_id("DC1A2C6F-1B32-4631-8823-36DACC8CB7BB").unwrap();
        a.set_lexeme_field_any(&sun, "label", "Sun (star)");
        b.set_lexeme_field_any(&sun, "label", "The sun");
        let theirs: Vec<_> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device() && o.verb == "set").collect();
        a.ingest(theirs).unwrap();
        // Two rows: B's label against A's, and against the twin create that
        // A's plain write did not name (it now differs from the winner).
        assert_eq!(a.proj.count("conflicts").unwrap(), 2);
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    impl Project {
        /// A plain set of any config field, for tests.
        fn set_lexeme_field_any(&mut self, target: &str, field: &str, value: &str) {
            let mut t = self.intent("test");
            t.set("domain", target, SCOPE_CONFIG, field, Value::str(value)).unwrap();
            t.commit().unwrap();
        }
    }

    #[test]
    fn deleting_an_entry_hides_its_senses_from_search_too() {
        let (root, mut p) = project();
        let ((lex, sen), _) = p.add_lexeme("ev", "house").unwrap();
        p.delete_lexeme(&lex).unwrap();
        assert!(p.proj.is_tombstoned(&sen).unwrap());
        assert!(p.proj.search("house", &["lexeme", "sense"], 10).unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_duplicate_lemma_warns_until_numbered() {
        let (root, mut p) = project();
        p.add_lexeme("dil", "tongue").unwrap();
        let ((second, _), res) = p.add_lexeme("dil", "language").unwrap();
        assert!(res.findings.iter().any(|f| f.code == "duplicate-lemma"), "{:?}", res.findings);
        let first = p.proj.entities_of_kind("lexeme").unwrap().into_iter().find(|l| *l != second).unwrap();
        p.set_homograph(&first, Some(1)).unwrap();
        let res = p.set_homograph(&second, Some(2)).unwrap();
        assert!(res.findings.iter().all(|f| f.code != "duplicate-lemma"), "{:?}", res.findings);
        assert!(p.set_homograph(&second, Some(0)).is_err());
        let ((third, _), _) = p.add_lexeme("dil", "strip").unwrap();
        let ((fourth, _), _) = p.add_lexeme("dil", "slice").unwrap();
        let res = p.number_homographs(&fourth).unwrap();
        assert!(res.findings.iter().all(|f| f.code != "duplicate-lemma"), "{:?}", res.findings);
        assert_eq!(p.proj.field_num(&third, "homograph").unwrap(), Some(3.0));
        assert_eq!(p.proj.field_num(&fourth, "homograph").unwrap(), Some(4.0));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_direct_lexicon_edit_settles_a_gloss_conflict() {
        let (ra, mut a) = project();
        let rb = std::env::temp_dir().join(format!("wugs-lexicon-test-{}", wugs_model::ulid()));
        let mut b = Project::open(&rb, "prs_Bo").unwrap();
        let ((_, sen), _) = a.add_lexeme("ev", "house").unwrap();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        a.set_sense_gloss(&sen, "home", None).unwrap();
        b.set_sense_gloss(&sen, "dwelling", None).unwrap();
        let theirs: Vec<_> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect();
        a.ingest(theirs).unwrap();
        assert_eq!(a.proj.count("conflicts").unwrap(), 1);
        a.set_sense_gloss(&sen, "house", None).unwrap();
        assert_eq!(a.proj.count("conflicts").unwrap(), 0);
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }
}
