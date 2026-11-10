use wugs_kit::{parse, update, valid_id, Place, Placement, MAX_BYTES};

const TONGUES: &str = include_str!("../../../kits/tongues/kit.toml");

fn messages(text: &str) -> Vec<String> {
    parse(text)
        .expect_err("should be refused")
        .into_iter()
        .map(|p| p.to_string())
        .collect()
}

#[test]
fn the_bundled_tongues_kit_parses() {
    let kit = parse(TONGUES).unwrap();
    assert_eq!(kit.id, "org.wugs.tongues");
    assert_eq!(
        (kit.name.as_str(), kit.subtitle.as_str()),
        ("Tongues", "Fieldwork")
    );
    assert!(kit.tools.contains(&"lexicon".to_string()));
    assert_eq!(
        kit.layout[0],
        Placement {
            panel: "timeline".into(),
            at: None,
            width: None,
            height: None
        }
    );
    assert_eq!(kit.layout[1].at, Some((Place::Left, "timeline".into())));
    assert_eq!(kit.layout[1].width, Some(260));
    assert_eq!(kit.active, ["texts", "timeline"]);
}

#[test]
fn update_keeps_comments_and_unmodelled_tables() {
    let mut kit = parse(TONGUES).unwrap();
    kit.id = "my-fieldwork".into();
    kit.name = "My fieldwork".into();
    kit.tools.retain(|t| t != "elicit");
    kit.layout.retain(|p| p.panel != "elicit");
    let out = update(TONGUES, &kit).unwrap();
    let back = parse(&out).unwrap();
    assert_eq!(back, kit);
    for kept in [
        "# A kit is a recipe, not a container",
        "[templates.semantic-domains]",
        "tier-overlap = \"hold\"",
        "# panels this kit offers",
    ] {
        assert!(out.contains(kept), "lost {kept:?}:\n{out}");
    }
    assert!(!out.contains("\"elicit\""));
}

#[test]
fn update_of_an_unchanged_kit_changes_nothing() {
    let kit = parse(TONGUES).unwrap();
    assert_eq!(update(TONGUES, &kit).unwrap(), TONGUES);
}

#[test]
fn update_replaces_the_first_sketchs_layout_keys() {
    let old = "[kit]\nid = \"k\"\nname = \"K\"\n[layout]\nprimary = \"timeline\"\nsecondary = [\"lexicon\"]\n";
    let mut kit = parse(old).unwrap();
    assert!(kit.layout.is_empty());
    kit.layout.push(Placement {
        panel: "timeline".into(),
        at: None,
        width: None,
        height: None,
    });
    let out = update(old, &kit).unwrap();
    assert!(
        !out.contains("primary") && !out.contains("secondary"),
        "{out}"
    );
    assert_eq!(parse(&out).unwrap().layout.len(), 1);
}

#[test]
fn update_refuses_a_kit_that_would_not_parse_back() {
    let mut kit = parse(TONGUES).unwrap();
    kit.active.push("nowhere".into());
    assert!(update(TONGUES, &kit).is_err());
}

#[test]
fn syntax_errors_carry_line_and_column() {
    let errs = parse("[kit]\nid = \"a\"\nname = \n").unwrap_err();
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].line, Some(3));
    assert!(errs[0].column.is_some());
}

#[test]
fn every_problem_is_reported_not_just_the_first() {
    let text = "[kit]\nid = \"Bad Id\"\n[tools]\nenabled = [\"a\", \"a\", 3]\n";
    let errs = messages(text);
    assert!(
        errs.iter().any(|e| e.contains("kit.id is lowercase")),
        "{errs:?}"
    );
    assert!(
        errs.iter().any(|e| e.contains("kit.name is required")),
        "{errs:?}"
    );
    assert!(
        errs.iter().any(|e| e.contains("names \"a\" twice")),
        "{errs:?}"
    );
    assert!(errs.iter().any(|e| e.contains("holds names")), "{errs:?}");
    assert!(
        errs.iter().any(|e| e.starts_with("line 2")),
        "id problem should point at its line: {errs:?}"
    );
}

#[test]
fn scripts_are_refused() {
    let errs = messages("[kit]\nid = \"k\"\nname = \"K\"\n[scripts]\non_open = \"x.wasm\"\n");
    assert!(errs[0].contains("[scripts]"), "{errs:?}");
}

#[test]
fn oversized_files_are_refused_unread() {
    let big = format!(
        "[kit]\nid = \"k\"\nname = \"K\"\n#{}\n",
        "x".repeat(MAX_BYTES)
    );
    assert!(messages(&big)[0].contains("at most"));
}

#[test]
fn placement_rules() {
    let kit =
        |panels: &str| format!("[kit]\nid = \"k\"\nname = \"K\"\n[layout]\npanels = [{panels}]\n");
    let refused = |panels: &str, needle: &str| {
        let errs = messages(&kit(panels));
        assert!(
            errs.iter().any(|e| e.contains(needle)),
            "{panels}: wanted {needle:?} in {errs:?}"
        );
    };
    refused(
        r#"{ panel = "a", place = "left", of = "b" }"#,
        "first placement fills",
    );
    refused(
        r#"{ panel = "a" }, { panel = "b" }"#,
        "needs both place and of",
    );
    refused(
        r#"{ panel = "a" }, { panel = "b", place = "left", of = "c" }"#,
        "not placed before it",
    );
    refused(
        r#"{ panel = "a" }, { panel = "b", place = "up", of = "a" }"#,
        "place is one of",
    );
    refused(
        r#"{ panel = "a" }, { panel = "a", place = "tab", of = "a" }"#,
        "twice",
    );
    refused(r#"{ panel = "a", width = 10 }"#, "number of pixels");
    refused(r#"{ panel = "a", colour = "red" }"#, "not one of them");
    refused(r#""a""#, "each entry");
    let ok = parse(&kit(
        r#"{ panel = "a" }, { panel = "b", place = "tab", of = "a" }"#,
    ))
    .unwrap();
    assert_eq!(ok.layout[1].at, Some((Place::Tab, "a".into())));
}

#[test]
fn array_of_tables_placements_read_like_inline_ones() {
    let text = "[kit]\nid = \"k\"\nname = \"K\"\n[[layout.panels]]\npanel = \"a\"\n[[layout.panels]]\npanel = \"b\"\nplace = \"below\"\nof = \"a\"\nheight = 200\n";
    let kit = parse(text).unwrap();
    assert_eq!(
        kit.layout[1],
        Placement {
            panel: "b".into(),
            at: Some((Place::Below, "a".into())),
            width: None,
            height: Some(200)
        }
    );
}

#[test]
fn ids_are_path_safe() {
    for ok in ["tongues", "org.wugs.tongues", "my-kit-2", "console", "com10", "my.con"] {
        assert!(valid_id(ok), "{ok}");
    }
    for bad in [
        "",
        ".hidden",
        "a..b",
        "a/b",
        "a\\b",
        "Upper",
        "trailing.",
        "sp ace",
        &"x".repeat(65),
    ] {
        assert!(!valid_id(bad), "{bad}");
    }
}
