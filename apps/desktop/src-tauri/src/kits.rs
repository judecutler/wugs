//! Installed kits: the bundled Tongues kit, compiled in and read-only, and
//! the user's own kits, one `kits/<id>.toml` each in app data. The active
//! kit is a per-machine choice in `kits/active`. Manifests are read and
//! rewritten by `wugs-kit`; this module only stores them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use wugs_kit::{Kit, Place, Placement, Problem, Wug};
use wugs_model::ulid;

pub const BUNDLED: &str = include_str!("../../../../kits/tongues/kit.toml");
pub const BUNDLED_ID: &str = "org.wugs.tongues";

/// Ids no user kit may take: the built-in kit's, and the name its saved
/// arrangement had before kits had ids (`layouts/tongues.json`).
fn reserved(id: &str) -> bool {
    id == BUNDLED_ID || id == crate::layout::LEGACY_TONGUES
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KitDto {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub version: String,
    pub description: String,
    pub tools: Vec<String>,
    pub layout: Vec<PlacementDto>,
    pub active: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wug: Option<WugDto>,
}

/// The shell's `WugLook`. Colours are checked by `wugs_kit` on every save,
/// so only `#rrggbb` ever reaches the drawing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WugDto {
    pub body: String,
    pub accessories: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colours: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementDto {
    pub panel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KitEntry {
    pub kit: KitDto,
    pub bundled: bool,
}

/// A file in the kits folder that is not a usable kit, shown so it can be
/// fixed or removed rather than vanishing.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokenKit {
    pub file: String,
    pub problems: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KitsView {
    pub active: String,
    pub kits: Vec<KitEntry>,
    pub broken: Vec<BrokenKit>,
}

impl From<&Kit> for KitDto {
    fn from(k: &Kit) -> Self {
        KitDto {
            id: k.id.clone(),
            name: k.name.clone(),
            subtitle: k.subtitle.clone(),
            version: k.version.clone(),
            description: k.description.clone(),
            tools: k.tools.clone(),
            layout: k
                .layout
                .iter()
                .map(|p| PlacementDto {
                    panel: p.panel.clone(),
                    place: p.at.as_ref().map(|(place, _)| place.as_str().to_string()),
                    of: p.at.as_ref().map(|(_, of)| of.clone()),
                    width: p.width,
                    height: p.height,
                })
                .collect(),
            active: k.active.clone(),
            wug: k.wug.as_ref().map(|w| WugDto {
                body: w.body.clone(),
                accessories: w.accessories.clone(),
                colours: w.colours.clone(),
            }),
        }
    }
}

impl KitDto {
    fn to_kit(&self) -> Result<Kit, String> {
        let layout = self
            .layout
            .iter()
            .map(|p| {
                let at = match (&p.place, &p.of) {
                    (None, None) => None,
                    (Some(place), Some(of)) => Some((
                        Place::parse(place).ok_or_else(|| format!("{place:?} is not a place"))?,
                        of.clone(),
                    )),
                    _ => return Err(format!("{:?} needs both place and of", p.panel)),
                };
                Ok(Placement {
                    panel: p.panel.clone(),
                    at,
                    width: p.width,
                    height: p.height,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Kit {
            id: self.id.clone(),
            name: self.name.clone(),
            subtitle: self.subtitle.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            tools: self.tools.clone(),
            layout,
            active: self.active.clone(),
            wug: self.wug.as_ref().map(|w| Wug {
                body: w.body.clone(),
                accessories: w.accessories.clone(),
                colours: w.colours.clone(),
            }),
        })
    }
}

fn problems_text(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn dir(app_data: &Path) -> PathBuf {
    app_data.join("kits")
}

fn user_path(app_data: &Path, id: &str) -> Result<PathBuf, String> {
    if !wugs_kit::valid_id(id) {
        return Err(format!("{id:?} is not a kit id"));
    }
    Ok(dir(app_data).join(format!("{id}.toml")))
}

fn bundled() -> Kit {
    wugs_kit::parse(BUNDLED).expect("the bundled kit parses (tested)")
}

/// Write by temp file and rename, so a crash never leaves half a kit.
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let parent = path.parent().expect("kits dir");
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    let tmp = parent.join(format!(".{}.tmp", ulid()));
    fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("save {}: {e}", path.display()))
}

/// Every installed kit, the bundled one first, then the user's by name.
pub fn list(app_data: &Path) -> Result<KitsView, String> {
    let mut kits = vec![KitEntry {
        kit: KitDto::from(&bundled()),
        bundled: true,
    }];
    let mut broken = Vec::new();
    let entries = match fs::read_dir(dir(app_data)) {
        Ok(entries) => entries.filter_map(Result::ok).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        // The bundled kit still works, so the app still opens; say why the rest is missing.
        Err(e) => {
            broken.push(BrokenKit {
                file: "kits".into(),
                problems: vec![format!("could not read {}: {e}", dir(app_data).display())],
            });
            Vec::new()
        }
    };
    let mut users = Vec::new();
    for entry in entries {
        let path = entry.path();
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = file.strip_suffix(".toml") else {
            continue;
        };
        if file.starts_with('.') {
            continue;
        }
        let mut fail = |problems: Vec<String>| {
            broken.push(BrokenKit {
                file: file.clone(),
                problems,
            })
        };
        let text = match read_capped(&path) {
            Ok(text) => text,
            Err(e) => {
                fail(vec![e]);
                continue;
            }
        };
        match wugs_kit::parse(&text) {
            Err(problems) => fail(problems.iter().map(ToString::to_string).collect()),
            Ok(kit) if kit.id != stem => fail(vec![format!(
                "the file is named {stem}.toml but its kit.id is {:?}",
                kit.id
            )]),
            Ok(kit) if reserved(&kit.id) => fail(vec![format!(
                "{:?} is reserved for the built-in kit; give this kit its own id",
                kit.id
            )]),
            Ok(kit) => users.push(KitEntry {
                kit: KitDto::from(&kit),
                bundled: false,
            }),
        }
    }
    users.sort_by(|a, b| {
        a.kit
            .name
            .to_lowercase()
            .cmp(&b.kit.name.to_lowercase())
            .then(a.kit.id.cmp(&b.kit.id))
    });
    kits.extend(users);
    broken.sort_by(|a, b| a.file.cmp(&b.file));
    let chosen = fs::read_to_string(dir(app_data).join("active")).unwrap_or_default();
    let chosen = chosen.trim();
    let active = if kits.iter().any(|k| k.kit.id == chosen) {
        chosen.to_string()
    } else {
        BUNDLED_ID.to_string()
    };
    Ok(KitsView {
        active,
        kits,
        broken,
    })
}

/// Reads at most one byte past the cap, so a pipe or device that reports
/// no length is still bounded.
fn read_capped(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let err = |e: std::io::Error| format!("read {}: {e}", path.display());
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(err)?
        .take(wugs_kit::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > wugs_kit::MAX_BYTES {
        return Err(format!("a kit file is at most {} KiB", wugs_kit::MAX_BYTES / 1024));
    }
    String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8 text", path.display()))
}

/// The kit's file as written, comments and all, for the code view.
pub fn code(app_data: &Path, id: &str) -> Result<String, String> {
    if id == BUNDLED_ID {
        return Ok(BUNDLED.to_string());
    }
    read_capped(&user_path(app_data, id)?)
}

fn refuse_bundled(id: &str) -> Result<(), String> {
    if id == BUNDLED_ID {
        Err("The bundled kit cannot be changed. Duplicate it to make your own.".into())
    } else {
        Ok(())
    }
}

/// Write the form's edits into the user kit's file in place.
pub fn save(app_data: &Path, id: &str, kit: &KitDto) -> Result<KitEntry, String> {
    refuse_bundled(id)?;
    if kit.id != id {
        return Err("A kit's id cannot change; duplicate it under the new name instead.".into());
    }
    let path = user_path(app_data, id)?;
    let text = read_capped(&path)?;
    let out = wugs_kit::update(&text, &kit.to_kit()?).map_err(|p| problems_text(&p))?;
    write_kit(app_data, &path, &text, &out)
}

/// Replace a user kit's file. When the new text changes how the kit starts,
/// the saved rearrangement of the old start goes, so the kit next opens as
/// its manifest now says.
fn write_kit(app_data: &Path, path: &Path, old: &str, new: &str) -> Result<KitEntry, String> {
    let kit = wugs_kit::parse(new).map_err(|p| problems_text(&p))?;
    write_atomic(path, new)?;
    let start = |k: &Kit| (k.layout.clone(), k.active.clone());
    if wugs_kit::parse(old).map(|o| start(&o)).ok() != Some(start(&kit)) {
        if let Ok(layout) = crate::layout::layout_path(app_data, &kit.id) {
            let _ = fs::remove_file(layout);
        }
    }
    Ok(KitEntry { kit: KitDto::from(&kit), bundled: false })
}

/// Replace the user kit's file with code typed in the code view. The text
/// is kept exactly as typed once it parses; its id must stay the same.
pub fn save_code(app_data: &Path, id: &str, code: &str) -> Result<KitEntry, String> {
    refuse_bundled(id)?;
    let kit = wugs_kit::parse(code).map_err(|p| problems_text(&p))?;
    if kit.id != id {
        return Err(format!(
            "kit.id must stay {id:?}; to make a kit under another id, duplicate this one"
        ));
    }
    let path = user_path(app_data, id)?;
    let old = read_capped(&path)?;
    write_kit(app_data, &path, &old, code)
}

/// An id for a kit named `name` that no installed kit uses yet.
pub fn fresh_id(app_data: &Path, name: &str) -> Result<String, String> {
    let mut base: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(48);
    let base = base.trim_end_matches('-');
    let base = if base.is_empty() { "kit" } else { base };
    let taken =
        |id: &str| reserved(id) || user_path(app_data, id).map(|p| p.exists()).unwrap_or(true);
    let id = (1..)
        .map(|n| {
            if n == 1 {
                base.to_string()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|id| !taken(id))
        .expect("some suffix is free");
    Ok(id)
}

/// A copy of an installed kit under a new name, as a user kit. The copy
/// keeps the source's comments, templates and validators.
pub fn duplicate(app_data: &Path, source: &str, name: &str) -> Result<KitEntry, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Name the new kit.".into());
    }
    let text = code(app_data, source)?;
    let mut kit = wugs_kit::parse(&text).map_err(|p| problems_text(&p))?;
    kit.id = fresh_id(app_data, name)?;
    kit.name = name.to_string();
    let out = wugs_kit::update(&text, &kit).map_err(|p| problems_text(&p))?;
    write_atomic(&user_path(app_data, &kit.id)?, &out)?;
    Ok(KitEntry {
        kit: KitDto::from(&kit),
        bundled: false,
    })
}

/// Remove a user kit and its saved arrangement. The bundled kit stays. If
/// it was the active kit, Tongues becomes active again.
pub fn delete(app_data: &Path, id: &str) -> Result<KitsView, String> {
    refuse_bundled(id)?;
    let path = user_path(app_data, id)?;
    fs::remove_file(&path).map_err(|e| format!("delete {}: {e}", path.display()))?;
    if let Ok(layout) = crate::layout::layout_path(app_data, id) {
        let _ = fs::remove_file(layout);
    }
    // A deleted active kit falls back to Tongues in list(); make that stick.
    set_active(app_data, &list(app_data)?.active)?;
    list(app_data)
}

/// Install a kit file someone shared. It is checked before it is copied,
/// and never replaces an installed kit.
pub fn install(app_data: &Path, file: &Path) -> Result<KitEntry, String> {
    let text = read_capped(file)?;
    let kit = wugs_kit::parse(&text).map_err(|p| problems_text(&p))?;
    let path = user_path(app_data, &kit.id)?;
    if reserved(&kit.id) || path.exists() {
        return Err(format!("A kit with the id {:?} is already installed. Delete it first, or change kit.id in the file.", kit.id));
    }
    write_atomic(&path, &text)?;
    Ok(KitEntry {
        kit: KitDto::from(&kit),
        bundled: false,
    })
}

pub fn set_active(app_data: &Path, id: &str) -> Result<(), String> {
    if id != BUNDLED_ID && !user_path(app_data, id)?.exists() {
        return Err(format!("no kit {id:?} is installed"));
    }
    write_atomic(&dir(app_data).join("active"), id)
}
