//! Kit manifests. A kit is a recipe, not a container: a `kit.toml` names the
//! tools a kit turns on, how the workspace is laid out, and template
//! defaults. This crate reads one into a [`Kit`], checks it, and writes
//! form edits back into the file in place, so the comments and every table
//! it does not model (`[templates]`, `[validators]`) survive.
//!
//! Kits come from other people, so everything here is input validation:
//! a size cap, path-safe ids, bounded strings, `#rrggbb` colours, and no
//! `[scripts]`.

use std::collections::BTreeMap;
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
    /// The kit's mascot; `None` shows the brand wug.
    pub wug: Option<Wug>,
    /// What the kit is as a shareable package (`docs/design/BUNDLES.md`):
    /// `[package]`, `[bundle]`, `[[requires]]` and `[examples]`. `None` for
    /// a plain kit file. Read only: `update` never rewrites these tables.
    pub bundle: Option<Bundle>,
}

/// A kit as a bundle: who made it, under what licence, and every package
/// it relies on, each pinned by version and file-list hash.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Bundle {
    /// `[package]` licence (SPDX), of the kit's own files.
    pub license: String,
    /// `[package]` source: where the kit's files are published.
    pub source: String,
    pub authors: Vec<String>,
    pub summary: String,
    pub requires: Vec<Requirement>,
    /// `[examples]` project: a folder inside the bundle (relative, `/`-separated).
    pub examples: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    /// `model`, `dataset`, `integration` or `plugin`.
    pub kind: String,
    pub id: String,
    pub version: String,
    /// SHA-256 of the package's canonical file list (`wugs_package::file_list_sha256`), 64 lowercase hex.
    pub files_sha256: String,
    /// For a model: the capability it serves here (features, not models).
    pub capability: Option<String>,
    /// Offered and ticked at install; the person may choose another provider.
    pub recommended: bool,
    pub why: String,
}

/// The kinds of package a bundle may require (`plugin` waits for item 36's sandbox).
pub const REQUIREMENT_KINDS: [&str; 4] = ["model", "dataset", "integration", "plugin"];

/// A wug look: the shell draws it (`apps/desktop/src/theme/wug.ts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wug {
    /// `#rrggbb`.
    pub body: String,
    /// Accessory ids. Unknown ones are kept, like tool ids, so a kit made
    /// with a newer version still loads; the shell leaves them off.
    pub accessories: Vec<String>,
    /// Fill overrides by drawing part id (`hat-shape`), each `#rrggbb`.
    pub colours: BTreeMap<String, String>,
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

/// Whether `s` is a colour a kit may carry: `#` and six hex digits.
pub fn valid_colour(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

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
    let wug = r.table(root, "wug").map(|t| r.wug(t));
    let bundle = r.bundle(root, &id, &version);
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
            wug,
            bundle,
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
    // A bundle's `[package]` that repeats the id or version follows `[kit]`.
    if let Some(p) = doc.get_mut("package").and_then(Item::as_table_mut) {
        for (key, value) in [("id", &kit.id), ("version", &kit.version)] {
            if p.contains_key(key) {
                set_str(p, key, value, true);
            }
        }
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
    if changed(&|b| b.wug == kit.wug) {
        match &kit.wug {
            None => {
                doc.remove("wug");
            }
            Some(w) => {
                let was = before.as_ref().and_then(|b| b.wug.as_ref());
                let t = table_mut(&mut doc, "wug");
                set_str(t, "body", &w.body, true);
                if was.is_none_or(|b| b.accessories != w.accessories) {
                    let mut a = Array::new();
                    a.extend(w.accessories.iter().map(String::as_str));
                    set_value(t, "accessories", Value::Array(a));
                }
                if was.is_none_or(|b| b.colours != w.colours) {
                    if w.colours.is_empty() {
                        t.remove("colours");
                    } else {
                        let mut c = InlineTable::new();
                        for (part, colour) in &w.colours {
                            c.insert(part, colour.as_str().into());
                        }
                        set_value(t, "colours", Value::InlineTable(c));
                    }
                }
            }
        }
    }
    let out = doc.to_string();
    parse(&out)?;
    Ok(out)
}

/// A kit file made ready to be someone else's copy: the original's
/// `package.source` and `bundle.authors` are taken out, so a duplicate is
/// never shared under the original's name and address. Its licence stays.
pub fn as_copy(text: &str) -> Result<String, Vec<Problem>> {
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        vec![problem(text, e.span(), e.message().trim().to_string())]
    })?;
    if let Some(p) = doc.get_mut("package").and_then(Item::as_table_mut) {
        p.remove("source");
    }
    if let Some(b) = doc.get_mut("bundle").and_then(Item::as_table_mut) {
        b.remove("authors");
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

    /// `[wug]`: a body colour, accessory ids and part colours. Keys it does
    /// not know are left alone, as in `[kit]`, so a newer kit still loads.
    fn wug(&mut self, t: &Table) -> Wug {
        let body = self.string(Some(t), "wug", "body", NAME_MAX, true);
        if !body.is_empty() && !valid_colour(&body) {
            self.fail(t.get("body").and_then(Item::span), COLOUR);
        }
        let accessories = self.names(Some(t), "wug", "accessories");
        let colours = t
            .get("colours")
            .map(|c| self.colours(c))
            .unwrap_or_default();
        Wug {
            body,
            accessories,
            colours,
        }
    }

    fn colours(&mut self, item: &Item) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let table: &dyn TableLike = match item {
            Item::Table(t) => t,
            Item::Value(Value::InlineTable(t)) => t,
            _ => {
                self.fail(
                    item.span(),
                    "wug.colours must be a table like { \"hat-shape\" = \"#a1691c\" }",
                );
                return out;
            }
        };
        if table.keys().len() > LIST_MAX {
            self.fail(
                item.span(),
                &format!("wug.colours has more than {LIST_MAX} parts"),
            );
            return out;
        }
        for key in table.keys() {
            let value = table.value(key);
            let span = value.and_then(Value::span).or_else(|| item.span());
            match value.and_then(Value::as_str) {
                _ if !valid_name(key) => self.fail(
                    span,
                    "wug.colours names drawing parts: lowercase letters, digits and '-'",
                ),
                Some(c) if valid_colour(c) => {
                    out.insert(key.to_string(), c.to_string());
                }
                _ => self.fail(span, COLOUR),
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

const AUTHORS_MAX: usize = 16;
const REQUIREMENT_KEYS: [&str; 7] = ["type", "id", "version", "files_sha256", "capability", "recommended", "why"];
const REQUIRES_MAX: usize = 32;

/// Whether `p` is a safe relative folder path inside a bundle: `/`-separated
/// parts of letters, digits, `.`, `-` and `_`, none empty, `.` or `..`,
/// none ending in `.` (Windows drops it), no Windows device names, at most
/// 200 bytes.
pub fn valid_bundle_path(p: &str) -> bool {
    let p = p.strip_suffix('/').unwrap_or(p);
    !p.is_empty()
        && p.len() <= 200
        && p.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with('.')
                && part.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
                && !windows_device(&part.split('.').next().unwrap_or("").to_ascii_lowercase())
        })
}

impl Reader<'_> {
    /// `[package]`, `[bundle]`, `[[requires]]`, `[examples]`: present only in
    /// a kit shared as a bundle. `[package]` may repeat `[kit]`'s id and
    /// version; when it does they must agree.
    fn bundle(&mut self, root: &Table, kit_id: &str, kit_version: &str) -> Option<Bundle> {
        let package = self.table(root, "package");
        let bundle = self.table(root, "bundle");
        let examples = self.table(root, "examples");
        let requires = root.get("requires");
        if package.is_none() && bundle.is_none() && examples.is_none() && requires.is_none() {
            return None;
        }
        let mut out = Bundle::default();
        if let Some(p) = package {
            let kind = self.string(Some(p), "package", "type", NAME_MAX, true);
            if !kind.is_empty() && kind != "kit" {
                self.fail(p.get("type").and_then(Item::span), "package.type of a kit file is \"kit\"");
            }
            for (key, kit_value) in [("id", kit_id), ("version", kit_version)] {
                let v = self.string(Some(p), "package", key, if key == "id" { ID_MAX } else { NAME_MAX }, false);
                if !v.is_empty() && kit_value.is_empty() {
                    self.fail(p.get(key).and_then(Item::span), &format!("package.{key} is {v:?}: set kit.{key} to the same"));
                } else if !v.is_empty() && v != kit_value {
                    self.fail(p.get(key).and_then(Item::span), &format!("package.{key} {v:?} differs from kit.{key} {kit_value:?}"));
                } else if key == "version" && !v.is_empty() && !valid_version(&v) {
                    self.fail(p.get(key).and_then(Item::span), "package.version is letters, digits, '.', '-', '+' and '_', like \"1.2.0\"");
                }
            }
            out.license = self.string(Some(p), "package", "license", NAME_MAX, false);
            out.source = self.string(Some(p), "package", "source", TEXT_MAX, false);
            if !out.source.is_empty() && out.source.strip_prefix("https://").is_none_or(|rest| rest.split('/').next().is_none_or(|host| !host.contains('.'))) {
                self.fail(p.get("source").and_then(Item::span), "package.source is an https:// address, like \"https://github.com/you/your-kit\"");
            }
        }
        if let Some(b) = bundle {
            out.summary = self.string(Some(b), "bundle", "summary", TEXT_MAX, false);
            if let Some(item) = b.get("authors") {
                match item.as_array() {
                    Some(a) if a.len() <= AUTHORS_MAX => {
                        for v in a.iter() {
                            match v.as_str() {
                                Some(s) if !s.trim().is_empty() && s.chars().count() <= NAME_MAX * 2 => out.authors.push(s.to_string()),
                                _ => self.fail(v.span(), "bundle.authors holds names, each at most 80 characters"),
                            }
                        }
                    }
                    _ => self.fail(item.span(), &format!("bundle.authors is a list of at most {AUTHORS_MAX} names")),
                }
            }
        }
        if let Some(e) = examples {
            let project = self.string(Some(e), "examples", "project", 200, true);
            if !project.is_empty() {
                if valid_bundle_path(&project) {
                    out.examples = Some(project);
                } else {
                    self.fail(e.get("project").and_then(Item::span), "examples.project is a folder inside the bundle: relative, '/'-separated, letters, digits, '.', '-' and '_'");
                }
            }
        }
        if let Some(item) = requires {
            match item.as_array_of_tables() {
                Some(list) if list.len() <= REQUIRES_MAX => {
                    for t in list.iter() {
                        if let Some(req) = self.requirement(t) {
                            if out.requires.iter().any(|r| r.kind == req.kind && r.id == req.id) {
                                self.fail(t.span(), &format!("requires names {} {:?} twice", req.kind, req.id));
                            } else {
                                out.requires.push(req);
                            }
                        }
                    }
                }
                _ => self.fail(item.span(), &format!("requires is written as [[requires]] tables, at most {REQUIRES_MAX}")),
            }
        }
        Some(out)
    }

    fn requirement(&mut self, t: &Table) -> Option<Requirement> {
        let before = self.problems.len();
        // A misspelt key here would change what gets installed, so it is named.
        for (key, item) in t.iter() {
            if !REQUIREMENT_KEYS.contains(&key) {
                self.fail(item.span(), &format!("requires has no key {key:?}; the keys are {}", REQUIREMENT_KEYS.join(", ")));
            }
        }
        let kind = self.string(Some(t), "requires", "type", NAME_MAX, true);
        if !kind.is_empty() && !REQUIREMENT_KINDS.contains(&kind.as_str()) {
            self.fail(t.get("type").and_then(Item::span), "requires.type is model, dataset, integration or plugin");
        }
        let id = self.string(Some(t), "requires", "id", ID_MAX, true);
        if !id.is_empty() && !valid_package_id(&id) {
            self.fail(t.get("id").and_then(Item::span), "requires.id is lowercase letters, digits, '.', '-' and '_', starting with a letter or digit");
        }
        let version = self.string(Some(t), "requires", "version", NAME_MAX, true);
        if !version.is_empty() && !valid_version(&version) {
            self.fail(t.get("version").and_then(Item::span), "requires.version is letters, digits, '.', '-', '+' and '_', like \"1.2.0\"");
        }
        let files_sha256 = self.string(Some(t), "requires", "files_sha256", 64, true);
        if !files_sha256.is_empty() && !(files_sha256.len() == 64 && files_sha256.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))) {
            self.fail(t.get("files_sha256").and_then(Item::span), "requires.files_sha256 is 64 lowercase hex digits");
        }
        let capability = self.string(Some(t), "requires", "capability", ID_MAX, false);
        if kind == "model" && capability.is_empty() {
            self.fail(t.span(), "a model requirement names its capability (features, not models)");
        }
        if !capability.is_empty() && !valid_name(&capability) {
            self.fail(t.get("capability").and_then(Item::span), "requires.capability is lowercase letters, digits and '-'");
        }
        let recommended = match t.get("recommended") {
            None => false,
            Some(item) => match item.as_bool() {
                Some(b) => b,
                None => {
                    self.fail(item.span(), "requires.recommended is true or false");
                    false
                }
            },
        };
        let why = self.string(Some(t), "requires", "why", TEXT_MAX, false);
        (self.problems.len() == before).then(|| Requirement {
            kind,
            id,
            version,
            files_sha256,
            capability: (!capability.is_empty()).then_some(capability),
            recommended,
            why,
        })
    }
}

/// A package id as the index lists it: lowercase letters, digits, `.`, `-`,
/// `_`, starting with a letter or digit, no `..`, not ending in `.`, at most
/// 64 bytes, no device name. Lowercase only, because ids name folders and
/// Windows and macOS fold case.
pub fn valid_package_id(id: &str) -> bool {
    let b = id.as_bytes();
    !b.is_empty()
        && b.len() <= ID_MAX
        && b[0].is_ascii_alphanumeric()
        && !id.contains("..")
        && !id.ends_with('.')
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_'))
        && !windows_device(&id.split('.').next().unwrap_or("").to_ascii_lowercase())
}

/// A package version, which names a folder (`packages/<type>/<id>/<version>/`):
/// ASCII letters, digits, `.`, `-`, `+`, `_`, starting with a letter or
/// digit, no `..`, not ending in `.`, at most 40 bytes.
pub fn valid_version(v: &str) -> bool {
    let b = v.as_bytes();
    !b.is_empty()
        && b.len() <= 40
        && b[0].is_ascii_alphanumeric()
        && !v.contains("..")
        && !v.ends_with('.')
        && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'+' | b'_'))
}

const COLOUR: &str = "a wug colour is '#' and six hex digits, like \"#f08060\"";

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
