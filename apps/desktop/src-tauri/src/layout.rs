//! Saved panel layouts, one JSON file per kit in app data. The shape is
//! dockview's `toJSON()`, kept opaque here: a layout is a kit preset, and
//! the Kit Builder turns one into a kit's default arrangement. Files, not webview storage,
//! because WebKitGTK's storage does not reliably survive restarts.

use std::fs;
use std::path::{Path, PathBuf};

use wugs_model::{json, ulid, Value};

pub fn layout_path(app_data: &Path, kit: &str) -> Result<PathBuf, String> {
    if !wugs_kit::valid_id(kit) {
        return Err(format!("{kit:?} is not a kit id"));
    }
    Ok(app_data.join("layouts").join(format!("{kit}.json")))
}

/// Before kits had ids, Tongues saved its arrangement as `tongues.json`.
pub const LEGACY_TONGUES: &str = "tongues";

/// Replace the kit's layout. Each save writes its own temp file and renames
/// it over the old one, so two saves in flight never interleave bytes.
pub fn save_layout(app_data: &Path, kit: &str, layout: &str) -> Result<(), String> {
    if !matches!(json::parse(layout), Ok(Value::Obj(_))) {
        return Err("a layout is a JSON object".into());
    }
    let path = layout_path(app_data, kit)?;
    let dir = path.parent().expect("layouts dir");
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!(".{kit}.{}.tmp", ulid()));
    fs::write(&tmp, layout).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("save {}: {e}", path.display()))
}

/// The kit's saved layout, or `None` when there is none or it no longer
/// parses; the shell then lays out its default.
pub fn load_layout(app_data: &Path, kit: &str) -> Result<Option<String>, String> {
    let path = layout_path(app_data, kit)?;
    if kit == crate::kits::BUNDLED_ID && !path.exists() {
        return load_layout(app_data, LEGACY_TONGUES);
    }
    match fs::read_to_string(&path) {
        Ok(text) if matches!(json::parse(&text), Ok(Value::Obj(_))) => Ok(Some(text)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}
