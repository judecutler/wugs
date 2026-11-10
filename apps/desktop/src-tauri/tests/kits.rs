use std::fs;
use std::path::PathBuf;

use wugs_desktop::kits::{self, BUNDLED_ID};
use wugs_desktop::layout;

fn app_data() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wugs-kits-{}", wugs_model::ulid()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_fresh_install_has_tongues_active() {
    let dir = app_data();
    let view = kits::list(&dir).unwrap();
    assert_eq!(view.active, BUNDLED_ID);
    assert_eq!(view.kits.len(), 1);
    assert!(view.kits[0].bundled);
    assert_eq!(view.kits[0].kit.name, "Tongues");
    assert!(view.broken.is_empty());
}

#[test]
fn the_bundled_kit_is_read_only() {
    let dir = app_data();
    let tongues = kits::list(&dir).unwrap().kits[0].kit.clone();
    assert!(kits::save(&dir, BUNDLED_ID, &tongues)
        .unwrap_err()
        .contains("Duplicate it"));
    assert!(kits::save_code(&dir, BUNDLED_ID, kits::BUNDLED).is_err());
    assert!(kits::delete(&dir, BUNDLED_ID).is_err());
}

#[test]
fn duplicate_edit_and_activate() {
    let dir = app_data();
    let mine = kits::duplicate(&dir, BUNDLED_ID, "My Field Kit!").unwrap();
    assert_eq!(mine.kit.id, "my-field-kit");
    assert!(!mine.bundled);
    let code = kits::code(&dir, "my-field-kit").unwrap();
    assert!(
        code.contains("[templates.semantic-domains]") && code.contains("# A kit is a recipe"),
        "{code}"
    );

    let mut edited = mine.kit.clone();
    edited.subtitle = "Corpus-ish".into();
    edited.tools.retain(|t| t != "elicit");
    edited.layout.retain(|p| p.panel != "elicit");
    let saved = kits::save(&dir, "my-field-kit", &edited).unwrap();
    assert_eq!(saved.kit, edited);

    kits::set_active(&dir, "my-field-kit").unwrap();
    let view = kits::list(&dir).unwrap();
    assert_eq!(view.active, "my-field-kit");
    assert_eq!(
        view.kits
            .iter()
            .map(|k| k.kit.id.as_str())
            .collect::<Vec<_>>(),
        [BUNDLED_ID, "my-field-kit"]
    );

    // A second copy under the same name gets its own id.
    assert_eq!(
        kits::duplicate(&dir, "my-field-kit", "My field kit")
            .unwrap()
            .kit
            .id,
        "my-field-kit-2"
    );
}

#[test]
fn save_refuses_an_id_change() {
    let dir = app_data();
    let mine = kits::duplicate(&dir, BUNDLED_ID, "Mine").unwrap();
    let mut renamed = mine.kit.clone();
    renamed.id = "other".into();
    assert!(kits::save(&dir, "mine", &renamed).is_err());
    let code = kits::code(&dir, "mine")
        .unwrap()
        .replace("id = \"mine\"", "id = \"other\"");
    assert!(kits::save_code(&dir, "mine", &code)
        .unwrap_err()
        .contains("must stay"));
}

#[test]
fn code_saves_keep_the_text_as_typed_and_report_every_problem() {
    let dir = app_data();
    kits::duplicate(&dir, BUNDLED_ID, "Mine").unwrap();
    let typed = format!("{}\n# my own note\n", kits::code(&dir, "mine").unwrap());
    kits::save_code(&dir, "mine", &typed).unwrap();
    assert_eq!(kits::code(&dir, "mine").unwrap(), typed);

    let bad = "[kit]\nid = \"mine\"\n[layout]\nactive = [\"nowhere\"]\n";
    let err = kits::save_code(&dir, "mine", bad).unwrap_err();
    assert!(
        err.contains("kit.name is required") && err.contains("nowhere"),
        "{err}"
    );
    assert_eq!(
        kits::code(&dir, "mine").unwrap(),
        typed,
        "a refused save leaves the file alone"
    );
}

#[test]
fn install_checks_first_and_never_replaces() {
    let dir = app_data();
    let shared = dir.join("shared.toml");
    fs::write(
        &shared,
        "[kit]\nid = \"typology\"\nname = \"Typology\"\n[tools]\nenabled = [\"search\"]\n",
    )
    .unwrap();
    assert_eq!(kits::install(&dir, &shared).unwrap().kit.name, "Typology");
    assert!(kits::install(&dir, &shared)
        .unwrap_err()
        .contains("already installed"));

    fs::write(&shared, kits::BUNDLED).unwrap();
    assert!(kits::install(&dir, &shared)
        .unwrap_err()
        .contains("already installed"));

    fs::write(
        &shared,
        "[kit]\nid = \"s\"\nname = \"S\"\n[scripts]\nx = 1\n",
    )
    .unwrap();
    assert!(kits::install(&dir, &shared)
        .unwrap_err()
        .contains("[scripts]"));
    assert!(!dir.join("kits").join("s.toml").exists());
}

#[test]
fn unusable_files_are_listed_as_broken_not_dropped() {
    let dir = app_data();
    let kits_dir = dir.join("kits");
    fs::create_dir_all(&kits_dir).unwrap();
    fs::write(kits_dir.join("garbled.toml"), "[kit\n").unwrap();
    fs::write(
        kits_dir.join("misnamed.toml"),
        "[kit]\nid = \"other\"\nname = \"O\"\n",
    )
    .unwrap();
    fs::write(kits_dir.join(format!("{BUNDLED_ID}.toml")), kits::BUNDLED).unwrap();
    fs::write(kits_dir.join("notes.txt"), "not a kit").unwrap();
    let view = kits::list(&dir).unwrap();
    assert_eq!(view.kits.len(), 1);
    let files: Vec<_> = view.broken.iter().map(|b| b.file.as_str()).collect();
    assert_eq!(
        files,
        [
            "garbled.toml",
            "misnamed.toml",
            &format!("{BUNDLED_ID}.toml")
        ]
    );
    assert!(
        view.broken[0].problems[0].starts_with("line 1"),
        "{:?}",
        view.broken[0]
    );
}

#[test]
fn deleting_the_active_kit_falls_back_to_tongues_and_drops_its_arrangement() {
    let dir = app_data();
    kits::duplicate(&dir, BUNDLED_ID, "Mine").unwrap();
    kits::set_active(&dir, "mine").unwrap();
    layout::save_layout(&dir, "mine", "{}").unwrap();
    let view = kits::delete(&dir, "mine").unwrap();
    assert_eq!(view.active, BUNDLED_ID);
    assert_eq!(layout::load_layout(&dir, "mine").unwrap(), None);
    assert_eq!(
        fs::read_to_string(dir.join("kits").join("active")).unwrap(),
        BUNDLED_ID
    );
}

#[test]
fn a_missing_active_kit_falls_back_to_tongues() {
    let dir = app_data();
    fs::create_dir_all(dir.join("kits")).unwrap();
    fs::write(dir.join("kits").join("active"), "gone").unwrap();
    assert_eq!(kits::list(&dir).unwrap().active, BUNDLED_ID);
    assert!(kits::set_active(&dir, "gone").is_err());
}

#[test]
fn tongues_reopens_the_arrangement_saved_before_kit_ids() {
    let dir = app_data();
    layout::save_layout(&dir, "tongues", "{\"old\":true}").unwrap();
    assert_eq!(
        layout::load_layout(&dir, BUNDLED_ID).unwrap().as_deref(),
        Some("{\"old\":true}")
    );
    layout::save_layout(&dir, BUNDLED_ID, "{\"new\":true}").unwrap();
    assert_eq!(
        layout::load_layout(&dir, BUNDLED_ID).unwrap().as_deref(),
        Some("{\"new\":true}")
    );
}

#[test]
fn kit_ids_never_escape_the_kits_folder() {
    let dir = app_data();
    for bad in ["../x", "a/b", "..", ""] {
        assert!(kits::code(&dir, bad).is_err(), "{bad}");
        assert!(kits::set_active(&dir, bad).is_err(), "{bad}");
        assert!(layout::load_layout(&dir, bad).is_err(), "{bad}");
    }
}

#[test]
fn the_legacy_tongues_name_is_reserved() {
    let dir = app_data();
    assert_eq!(kits::duplicate(&dir, BUNDLED_ID, "Tongues").unwrap().kit.id, "tongues-2");
    let shared = dir.join("t.toml");
    fs::write(&shared, "[kit]\nid = \"tongues\"\nname = \"T\"\n").unwrap();
    assert!(kits::install(&dir, &shared).is_err());
}

#[test]
fn changing_how_a_kit_starts_drops_its_saved_arrangement() {
    let dir = app_data();
    let mine = kits::duplicate(&dir, BUNDLED_ID, "Mine").unwrap().kit;
    layout::save_layout(&dir, "mine", "{\"saved\":1}").unwrap();
    let mut renamed = mine.clone();
    renamed.name = "Mine, renamed".into();
    kits::save(&dir, "mine", &renamed).unwrap();
    assert!(layout::load_layout(&dir, "mine").unwrap().is_some(), "a rename keeps the arrangement");
    let mut fewer = renamed.clone();
    fewer.layout.truncate(3);
    fewer.active.clear();
    kits::save(&dir, "mine", &fewer).unwrap();
    assert_eq!(layout::load_layout(&dir, "mine").unwrap(), None, "a new start drops it");

    layout::save_layout(&dir, "mine", "{\"saved\":2}").unwrap();
    let code = kits::code(&dir, "mine").unwrap();
    kits::save_code(&dir, "mine", &format!("{code}\n# only a comment\n")).unwrap();
    assert!(layout::load_layout(&dir, "mine").unwrap().is_some(), "a comment keeps it");
}
