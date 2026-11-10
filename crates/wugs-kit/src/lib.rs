//! Kit manifests. A kit is a recipe, not a container: a `kit.toml` names the
//! tools a kit turns on, how the workspace is laid out, and template
//! defaults. This crate reads one into a [`Kit`], checks it, and writes
//! form edits back into the file in place, so the comments and every table
//! it does not model (`[templates]`, `[validators]`) survive.
//!
//! Kits come from other people, so everything here is input validation:
//! a size cap, path-safe ids, bounded strings, and no `[scripts]`.

use std::fmt;
use std::ops::Range;

use toml_edit::{Array, Document, DocumentMut, InlineTable, Item, Table, Value};

/// A kit file larger than this is refused unread.
pub const MAX_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kit {
    /// Path-safe: it names the kit's files on disk.
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub version: String,
    pub description: String,
    /// Tool ids. The shell decides which ids it knows; unknown ones are
    /// kept, so a kit written for a newer version still loads.
    pub tools: Vec<String>,
    /// How the workspace starts: the first placement fills it, every later
    /// one is placed beside or as a tab of a panel placed before it.
    pub layout: Vec<Placement>,
    /// Panels brought to the front once laid out, in order.
    pub active: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub panel: String,
    /// `None` only for the first placement.
    pub at: Option<(Place, String)>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Left,
    Right,
    Above,
    Below,
    Tab,
}

impl Place {
    pub const ALL: [Place; 5] = [
        Place::Left,
        Place::Right,
        Place::Above,
        Place::Below,
        Place::Tab,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Place::Left => "left",
            Place::Right => "right",
            Place::Above => "above",
            Place::Below => "below",
            Place::Tab => "tab",
        }
    }

    pub fn parse(s: &str) -> Option<Place> {
        Place::ALL.into_iter().find(|p| p.as_str() == s)
    }
}

/// One thing wrong with a kit file: where (1-based line and column, when
/// the text has a position for it) and what, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "line {l}, column {c}: {}", self.message),
            _ => f.write_str(&self.message),
        }
    }
}

const NAME_MAX: usize = 40;
const TEXT_MAX: usize = 500;
const ID_MAX: usize = 64;
const LIST_MAX: usize = 64;
const SIZE_RANGE: Range<i64> = 50..4001;

/// Whether `id` can name a kit: lowercase letters, digits, `.` and `-`,
/// starting with a letter or digit, no `..`, at most 64 bytes, and not a
/// name Windows reserves for a device (`con`, `nul`, `com1`, also as
/// `con.anything`), since the id names files.
pub fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    let stem = id.split('.').next().unwrap_or("");
    !b.is_empty()
        && b.len() <= ID_MAX
        && b[0].is_ascii_alphanumeric()
        && !id.contains("..")
        && !id.ends_with('.')
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'.' || *c == b'-')
        && !windows_device(stem)
}

fn windows_device(stem: &str) -> bool {
    matches!(stem, "con" | "prn" | "aux" | "nul")
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

/// Read and check a kit file. Every problem found is returned, not just the
/// first, so the code view can list them all.
pub fn parse(text: &str) -> Result<Kit, Vec<Problem>> {
    if text.len() > MAX_BYTES {
        return Err(vec![Problem {
            line: None,
            column: None,
            message: format!("a kit file is at most {} KiB", MAX_BYTES / 1024),
        }]);
    }
    let doc = match Document::parse(text) {
        Ok(doc) => doc,
        Err(e) => {
            return Err(vec![problem(
                text,
                e.span(),
                e.message().trim().to_string(),
            )])
        }
    };
    let mut r = Reader {
        text,
        problems: Vec::new(),
    };
    let root = doc.as_table();
    if let Some(item) = root.get("scripts") {
        r.fail(
            item.span(),
            "this kit has [scripts]; kits with scripts cannot be installed yet",
        );
    }
    let kit = r.table(root, "kit");
    let id = r.string(kit, "kit", "id", ID_MAX, true);
    if !id.is_empty() && !valid_id(&id) {
        r.fail(
            kit.and_then(|t| t.get("id")).and_then(Item::span),
            "kit.id is lowercase letters, digits, '.' and '-', starting with a letter or digit",
        );
    }
    let name = r.string(kit, "kit", "name", NAME_MAX, true);
    let subtitle = r.string(kit, "kit", "subtitle", NAME_MAX, false);
    let version = r.string(kit, "kit", "version", NAME_MAX, false);
    let description = r.string(kit, "kit", "description", TEXT_MAX, false);
    let tools_table = r.table(root, "tools");
    let tools = r.names(tools_table, "tools", "enabled");
    let layout_table = r.table(root, "layout");
    let layout = r.placements(layout_table);
    let active = r.names(layout_table, "layout", "active");
    for name in &active {
        if !layout.iter().any(|p| &p.panel == name) {
            r.fail(
                layout_table
                    .and_then(|t| t.get("active"))
                    .and_then(Item::span),
                &format!("layout.active names {name:?}, which the layout does not place"),
            );
        }
    }
    if r.problems.is_empty() {
        Ok(Kit {
            id,
            name,
            subtitle,
            version,
            description,
            tools,
            layout,
            active,
        })
    } else {
        Err(r.problems)
    }
}

/// Write `kit`'s modelled fields into `text`, keeping everything else
/// (comments, `[templates]`, `[validators]`, unknown keys) as it was. The
/// result is parsed again, so what is returned is a valid kit file.
pub fn update(text: &str, kit: &Kit) -> Result<String, Vec<Problem>> {
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        vec![problem(text, e.span(), e.message().trim().to_string())]
    })?;
    // Only what changed is rewritten, so an untouched list keeps its comments.
    let before = parse(text).ok();
    let changed = |f: &dyn Fn(&Kit) -> bool| before.as_ref().is_none_or(|b| !f(b));
    {
        let t = table_mut(&mut doc, "kit");
        set_str(t, "id", &kit.id, true);
        set_str(t, "name", &kit.name, true);
        set_str(t, "subtitle", &kit.subtitle, false);
        set_str(t, "version", &kit.version, false);
        set_str(t, "description", &kit.description, false);
    }
    if changed(&|b| b.tools == kit.tools) {
        set_value(
            table_mut(&mut doc, "tools"),
            "enabled",
            Value::Array(multiline(kit.tools.iter().map(|s| Value::from(s.as_str())))),
        );
    }
    if changed(&|b| b.layout == kit.layout && b.active == kit.active) {
        let t = table_mut(&mut doc, "layout");
        // The two keys the first manifest sketch used; placements replace them.
        t.remove("primary");
        t.remove("secondary");
        let panels = multiline(
            kit.layout
                .iter()
                .map(|p| Value::InlineTable(placement_table(p))),
        );
        set_value(t, "panels", Value::Array(panels));
        let mut active = Array::new();
        active.extend(kit.active.iter().map(String::as_str));
        set_value(t, "active", Value::Array(active));
    }
    let out = doc.to_string();
    parse(&out)?;
    Ok(out)
}

fn placement_table(p: &Placement) -> InlineTable {
    let mut t = InlineTable::new();
    t.insert("panel", p.panel.as_str().into());
    if let Some((place, of)) = &p.at {
        t.insert("place", place.as_str().into());
        t.insert("of", of.as_str().into());
    }
    if let Some(w) = p.width {
        t.insert("width", i64::from(w).into());
    }
    if let Some(h) = p.height {
        t.insert("height", i64::from(h).into());
    }
    t
}

/// One array element per line, so the code view reads like a list.
fn multiline(values: impl Iterator<Item = Value>) -> Array {
    let mut a = Array::new();
    for mut v in values {
        v.decor_mut().set_prefix("\n  ");
        a.push_formatted(v);
    }
    a.set_trailing("\n");
    a.set_trailing_comma(true);
    a
}

fn table_mut<'a>(doc: &'a mut DocumentMut, key: &str) -> &'a mut Table {
    if !doc.contains_table(key) {
        doc.insert(key, Item::Table(Table::new()));
    }
    doc[key]
        .as_table_mut()
        .expect("a table: parse() would have refused anything else")
}

/// Replace a value in place, so the comment lines above its key and the
/// comment after it on its line stay.
fn set_value(t: &mut Table, key: &str, mut value: Value) {
    match t.get_mut(key) {
        Some(Item::Value(old)) => {
            *value.decor_mut() = old.decor().clone();
            *old = value;
        }
        _ => {
            t.insert(key, Item::Value(value));
        }
    }
}

fn set_str(t: &mut Table, key: &str, value: &str, required: bool) {
    if value.is_empty() && !required {
        t.remove(key);
    } else if t.get(key).and_then(Item::as_str) != Some(value) {
        set_value(t, key, value.into());
    }
}

fn problem(text: &str, span: Option<Range<usize>>, message: String) -> Problem {
    match span {
        Some(s) => {
            let before = &text[..s.start.min(text.len())];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
            Problem {
                line: Some(line),
                column: Some(column),
                message,
            }
        }
        None => Problem {
            line: None,
            column: None,
            message,
        },
    }
}

struct Reader<'t> {
    text: &'t str,
    problems: Vec<Problem>,
}

impl Reader<'_> {
    fn fail(&mut self, span: Option<Range<usize>>, message: &str) {
        self.problems
            .push(problem(self.text, span, message.to_string()));
    }

    /// A top-level table; missing is fine (an empty kit section), anything
    /// else in its place is a problem.
    fn table<'d>(&mut self, root: &'d Table, key: &str) -> Option<&'d Table> {
        match root.get(key) {
            None => None,
            Some(Item::Table(t)) => Some(t),
            Some(other) => {
                self.fail(other.span(), &format!("[{key}] must be a table"));
                None
            }
        }
    }

    fn string(
        &mut self,
        t: Option<&Table>,
        section: &str,
        key: &str,
        max: usize,
        required: bool,
    ) -> String {
        match t.and_then(|t| t.get(key)) {
            None if required => {
                self.fail(
                    t.and_then(Table::span),
                    &format!("{section}.{key} is required"),
                );
                String::new()
            }
            None => String::new(),
            Some(item) => match item.as_str() {
                Some(s) if s.chars().count() > max => {
                    self.fail(
                        item.span(),
                        &format!("{section}.{key} is at most {max} characters"),
                    );
                    String::new()
                }
                Some(s) if required && s.trim().is_empty() => {
                    self.fail(item.span(), &format!("{section}.{key} cannot be empty"));
                    String::new()
                }
                Some(s) => s.to_string(),
                None => {
                    self.fail(item.span(), &format!("{section}.{key} must be a string"));
                    String::new()
                }
            },
        }
    }

    /// A list of distinct short names (tool or panel ids).
    fn names(&mut self, t: Option<&Table>, section: &str, key: &str) -> Vec<String> {
        let Some(item) = t.and_then(|t| t.get(key)) else {
            return Vec::new();
        };
        let Some(array) = item.as_array() else {
            self.fail(
                item.span(),
                &format!("{section}.{key} must be a list of names"),
            );
            return Vec::new();
        };
        if array.len() > LIST_MAX {
            self.fail(
                item.span(),
                &format!("{section}.{key} has more than {LIST_MAX} names"),
            );
            return Vec::new();
        }
        let mut out: Vec<String> = Vec::new();
        for v in array.iter() {
            match v.as_str() {
                Some(s) if valid_name(s) => {
                    if out.iter().any(|o| o == s) {
                        self.fail(v.span(), &format!("{section}.{key} names {s:?} twice"));
                    } else {
                        out.push(s.to_string());
                    }
                }
                _ => self.fail(
                    v.span(),
                    &format!("{section}.{key} holds names: lowercase letters, digits and '-'"),
                ),
            }
        }
        out
    }

    fn placements(&mut self, layout: Option<&Table>) -> Vec<Placement> {
        let Some(item) = layout.and_then(|t| t.get("panels")) else {
            return Vec::new();
        };
        let entries: Vec<(Option<Range<usize>>, &dyn TableLike)> = match item {
            Item::Value(Value::Array(a)) if a.len() <= LIST_MAX => {
                let mut out = Vec::new();
                for v in a.iter() {
                    match v.as_inline_table() {
                        Some(t) => out.push((v.span(), t as &dyn TableLike)),
                        None => self.fail(v.span(), "each entry of layout.panels is a table like { panel = \"lexicon\", place = \"right\", of = \"timeline\" }"),
                    }
                }
                out
            }
            Item::ArrayOfTables(a) if a.len() <= LIST_MAX => {
                a.iter().map(|t| (t.span(), t as &dyn TableLike)).collect()
            }
            Item::Value(Value::Array(_)) | Item::ArrayOfTables(_) => {
                self.fail(
                    item.span(),
                    &format!("layout.panels places at most {LIST_MAX} panels"),
                );
                return Vec::new();
            }
            _ => {
                self.fail(item.span(), "layout.panels must be a list of placements");
                return Vec::new();
            }
        };
        let mut out: Vec<Placement> = Vec::new();
        for (i, (span, t)) in entries.into_iter().enumerate() {
            let text = |key: &str| t.value(key).and_then(Value::as_str);
            let Some(panel) = text("panel").filter(|p| valid_name(p)) else {
                self.fail(span, "a placement needs panel = \"<panel id>\"");
                continue;
            };
            if out.iter().any(|p| p.panel == panel) {
                self.fail(span, &format!("layout.panels places {panel:?} twice"));
                continue;
            }
            for key in t.keys() {
                if !matches!(key, "panel" | "place" | "of" | "width" | "height") {
                    self.fail(span.clone(), &format!("a placement has panel, place, of, width and height; {key:?} is not one of them"));
                }
            }
            let at = match (text("place"), text("of")) {
                (None, None) if i == 0 => None,
                (_, _) if i == 0 => {
                    self.fail(
                        span,
                        "the first placement fills the workspace, so it has no place or of",
                    );
                    continue;
                }
                (Some(place), Some(of)) => {
                    let Some(place) = Place::parse(place) else {
                        self.fail(
                            span,
                            &format!(
                                "place is one of left, right, above, below or tab, not {place:?}"
                            ),
                        );
                        continue;
                    };
                    if !out.iter().any(|p| p.panel == of) {
                        self.fail(
                            span,
                            &format!(
                                "{panel:?} is placed {} of {of:?}, which is not placed before it",
                                place.as_str()
                            ),
                        );
                        continue;
                    }
                    Some((place, of.to_string()))
                }
                _ => {
                    self.fail(span, &format!("{panel:?} needs both place and of"));
                    continue;
                }
            };
            let mut size = |key: &str| match t.value(key) {
                None => None,
                Some(v) => match v.as_integer() {
                    Some(n) if SIZE_RANGE.contains(&n) => Some(n as u32),
                    _ => {
                        self.fail(
                            v.span(),
                            &format!(
                                "{key} is a number of pixels from {} to {}",
                                SIZE_RANGE.start,
                                SIZE_RANGE.end - 1
                            ),
                        );
                        None
                    }
                },
            };
            let width = size("width");
            let height = size("height");
            out.push(Placement {
                panel: panel.to_string(),
                at,
                width,
                height,
            });
        }
        out
    }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= ID_MAX
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// Inline tables (`{ panel = … }`) and `[[layout.panels]]` tables read alike.
trait TableLike {
    fn value(&self, key: &str) -> Option<&Value>;
    fn keys(&self) -> Vec<&str>;
}

impl TableLike for InlineTable {
    fn value(&self, key: &str) -> Option<&Value> {
        self.get(key)
    }
    fn keys(&self) -> Vec<&str> {
        self.iter().map(|(k, _)| k).collect()
    }
}

impl TableLike for Table {
    fn value(&self, key: &str) -> Option<&Value> {
        self.get(key).and_then(Item::as_value)
    }
    fn keys(&self) -> Vec<&str> {
        self.iter().map(|(k, _)| k).collect()
    }
}
