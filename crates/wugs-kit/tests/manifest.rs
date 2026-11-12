use std::collections::BTreeMap;

use wugs_kit::{parse, update, valid_colour, valid_id, Place, Placement, Wug, MAX_BYTES};

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
    assert_eq!(
        kit.wug,
        Some(Wug {
            body: "#f08060".into(),
            accessories: vec!["hat".into(), "recorder".into()],
            colours: BTreeMap::new(),
        })
    );
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
    for ok in [
        "tongues",
        "org.wugs.tongues",
        "my-kit-2",
        "console",
        "com10",
        "my.con",
    ] {
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

const PLAIN: &str = "[kit]\nid = \"k\"\nname = \"K\"\n";

#[test]
fn a_kit_without_a_wug_has_none() {
    assert_eq!(parse(PLAIN).unwrap().wug, None);
}

#[test]
fn a_wug_reads_colours_inline_or_as_a_table() {
    let inline = format!("{PLAIN}[wug]\nbody = \"#ABCDEF\"\naccessories = [\"crown\"]\ncolours = {{ \"crown-shape\" = \"#00ff00\" }}\n");
    let table = format!("{PLAIN}[wug]\nbody = \"#ABCDEF\"\naccessories = [\"crown\"]\n[wug.colours]\ncrown-shape = \"#00ff00\"\n");
    let a = parse(&inline).unwrap().wug.unwrap();
    assert_eq!(a, parse(&table).unwrap().wug.unwrap());
    assert_eq!(a.body, "#ABCDEF");
    assert_eq!(a.colours["crown-shape"], "#00ff00");
}

#[test]
fn unknown_accessories_and_keys_are_kept_so_a_newer_kit_loads() {
    let text = format!(
        "{PLAIN}[wug]\nbody = \"#000000\"\naccessories = [\"jetpack\"]\neyes = \"sleepy\"\n"
    );
    assert_eq!(parse(&text).unwrap().wug.unwrap().accessories, ["jetpack"]);
}

#[test]
fn wug_problems_carry_line_and_column() {
    let text = format!(
        "{PLAIN}[wug]\nbody = \"coral\"\naccessories = [\"hat\", \"hat\", \"../x\"]\ncolours = {{ hat-shape = \"red\", \"Bad Part\" = \"#000000\" }}\n"
    );
    let errs = parse(&text).unwrap_err();
    let msgs: Vec<String> = errs.iter().map(ToString::to_string).collect();
    let at = |needle: &str| {
        errs.iter()
            .find(|p| p.message.contains(needle))
            .unwrap_or_else(|| panic!("no {needle:?} in {msgs:?}"))
    };
    assert_eq!(at("six hex digits").line, Some(5), "{msgs:?}");
    assert_eq!(at("names \"hat\" twice").line, Some(6));
    assert!(
        msgs.iter()
            .any(|m| m.contains("wug.accessories holds names")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("line 7") && m.contains("six hex digits")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("names drawing parts")),
        "{msgs:?}"
    );
    assert!(at("six hex digits").column.is_some());
}

#[test]
fn a_wug_needs_a_body_and_a_table_shape() {
    let errs = messages(&format!("{PLAIN}[wug]\naccessories = []\n"));
    assert!(
        errs.iter().any(|e| e.contains("wug.body is required")),
        "{errs:?}"
    );
    let errs = messages(&format!("wug = \"blue\"\n{PLAIN}"));
    assert!(
        errs.iter().any(|e| e.contains("[wug] must be a table")),
        "{errs:?}"
    );
    let errs = messages(&format!(
        "{PLAIN}[wug]\nbody = \"#000000\"\ncolours = [\"#000000\"]\n"
    ));
    assert!(
        errs.iter()
            .any(|e| e.contains("wug.colours must be a table")),
        "{errs:?}"
    );
}

#[test]
fn wug_counts_are_bounded() {
    let many: Vec<String> = (0..65).map(|i| format!("p{i} = \"#000000\"")).collect();
    let errs = messages(&format!(
        "{PLAIN}[wug]\nbody = \"#000000\"\ncolours = {{ {} }}\n",
        many.join(", ")
    ));
    assert!(
        errs.iter().any(|e| e.contains("more than 64 parts")),
        "{errs:?}"
    );
    let many: Vec<String> = (0..65).map(|i| format!("\"a{i}\"")).collect();
    let errs = messages(&format!(
        "{PLAIN}[wug]\nbody = \"#000000\"\naccessories = [{}]\n",
        many.join(", ")
    ));
    assert!(errs.iter().any(|e| e.contains("more than 64")), "{errs:?}");
}

#[test]
fn colours_are_hash_and_six_hex_digits() {
    for ok in ["#000000", "#f08060", "#ABCdef"] {
        assert!(valid_colour(ok), "{ok}");
    }
    for bad in [
        "",
        "#fff",
        "f08060",
        "#f0806",
        "#f080600",
        "#g08060",
        "#f0806\u{e9}",
        "red",
        "#f08060\"><x",
    ] {
        assert!(!valid_colour(bad), "{bad}");
    }
}

#[test]
fn update_writes_a_wug_in_place_and_keeps_its_comments() {
    let mut kit = parse(TONGUES).unwrap();
    let wug = kit.wug.as_mut().unwrap();
    wug.body = "#336699".into();
    wug.accessories = vec!["crown".into(), "recorder".into()];
    wug.colours.insert("crown-shape".into(), "#aa00aa".into());
    let out = update(TONGUES, &kit).unwrap();
    assert_eq!(parse(&out).unwrap(), kit);
    for kept in [
        "# the kit's mascot",
        "# coral; colours are",
        "# one per slot",
        "[templates.semantic-domains]",
    ] {
        assert!(out.contains(kept), "lost {kept:?}:\n{out}");
    }
    assert!(out.contains("crown-shape = \"#aa00aa\""), "{out}");
}

#[test]
fn update_adds_and_removes_a_wug() {
    let mut kit = parse(PLAIN).unwrap();
    kit.wug = Some(Wug {
        body: "#123456".into(),
        accessories: vec!["hat".into()],
        colours: BTreeMap::from([("hat-shape".to_string(), "#654321".to_string())]),
    });
    let out = update(PLAIN, &kit).unwrap();
    assert_eq!(parse(&out).unwrap(), kit);
    kit.wug.as_mut().unwrap().colours.clear();
    let cleared = update(&out, &kit).unwrap();
    assert!(!cleared.contains("colours"), "{cleared}");
    kit.wug = None;
    let gone = update(&cleared, &kit).unwrap();
    assert!(!gone.contains("[wug]"), "{gone}");
    assert_eq!(parse(&gone).unwrap(), kit);
}

#[test]
fn update_refuses_a_wug_colour_that_is_not_hex() {
    let mut kit = parse(TONGUES).unwrap();
    kit.wug.as_mut().unwrap().body = "\"><script>".into();
    assert!(update(TONGUES, &kit).is_err());
    let mut kit = parse(TONGUES).unwrap();
    kit.wug
        .as_mut()
        .unwrap()
        .colours
        .insert("hat-shape".into(), "url(x)".into());
    assert!(update(TONGUES, &kit).is_err());
}

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn bundled(extra: &str) -> String {
    format!(
        "[package]\ntype = \"kit\"\nid = \"my.kit\"\nlicense = \"MPL-2.0\"\nsource = \"https://github.com/someone/kit\"\n\n[kit]\nid = \"my.kit\"\nname = \"Mine\"\nversion = \"1.0.0\"\n\n[bundle]\nauthors = [\"A. Linguist\"]\nsummary = \"Corpus tools\"\n\n{extra}"
    )
}

#[test]
fn a_bundle_names_what_it_needs() {
    let text = bundled(&format!(
        "[[requires]]\ntype = \"model\"\nid = \"udpipe1-ud-2.5_english\"\nversion = \"1.0.0\"\nfiles_sha256 = \"{SHA}\"\ncapability = \"morphosyntax\"\nrecommended = true\nwhy = \"Tags English\"\n\n[[requires]]\ntype = \"dataset\"\nid = \"ud-tagset\"\nversion = \"2.5\"\nfiles_sha256 = \"{SHA}\"\n\n[examples]\nproject = \"examples/sample/\"\n"
    ));
    let kit = parse(&text).unwrap();
    let b = kit.bundle.expect("a bundle");
    assert_eq!(b.requires[0].id, "udpipe1-ud-2.5_english");
    assert_eq!((b.license.as_str(), b.authors.len(), b.summary.as_str()), ("MPL-2.0", 1, "Corpus tools"));
    assert_eq!(b.requires.len(), 2);
    assert_eq!(b.requires[0].capability.as_deref(), Some("morphosyntax"));
    assert!(b.requires[0].recommended && !b.requires[1].recommended);
    assert_eq!(b.examples.as_deref(), Some("examples/sample/"));
    // A form edit keeps every bundle table.
    let mut edited = parse(&text).unwrap();
    edited.name = "Renamed".into();
    let out = update(&text, &edited).unwrap();
    assert_eq!(parse(&out).unwrap().bundle, parse(&text).unwrap().bundle);
    // A new id or version in the form reaches a `[package]` that repeats them.
    let with_version = text.replace("license", "version = \"1.0.0\"\nlicense");
    let mut moved = parse(&with_version).unwrap();
    moved.id = "again".into();
    moved.version = "2.0.0".into();
    let out = update(&with_version, &moved).unwrap();
    let back = parse(&out).unwrap();
    assert_eq!((back.id.as_str(), back.version.as_str()), ("again", "2.0.0"));
    assert!(out.contains("[package]\ntype = \"kit\"\nid = \"again\"\nversion = \"2.0.0\""), "{out}");
}

#[test]
fn a_bundle_is_checked_like_any_kit_input() {
    let m = messages(&bundled(&format!(
        "[[requires]]\ntype = \"script\"\nid = \"../x\"\nversion = \"1\"\nfiles_sha256 = \"ABC\"\n\n[[requires]]\ntype = \"model\"\nid = \"m\"\nversion = \"1\"\nfiles_sha256 = \"{SHA}\"\n\n[examples]\nproject = \"../../etc\"\n"
    )))
    .join("\n");
    for needle in ["model, dataset, integration or plugin", "requires.id is lowercase letters", "64 lowercase hex", "names its capability", "examples.project is a folder inside"] {
        assert!(m.contains(needle), "{needle}: {m}");
    }
    let wrong = bundled("").replace("type = \"kit\"", "type = \"model\"").replace("id = \"my.kit\"\nlicense", "id = \"other\"\nlicense");
    let m = messages(&wrong).join("\n");
    assert!(m.contains("package.type of a kit file") && m.contains("differs from kit.id"), "{m}");
    assert!(messages(&bundled("").replace("https://github.com", "http://github.com")).join("\n").contains("https://"));
    assert!(messages(&bundled("").replace("https://github.com/someone/kit", "https://")).join("\n").contains("https://"));
    for bad in ["https://.", "https://github.com@evil.com/x", "https://a..b/x"] {
        assert!(messages(&bundled("").replace("https://github.com/someone/kit", bad)).join("\n").contains("https://"), "{bad}");
    }
    assert!(messages(&bundled("").replace("version = \"1.0.0\"\n\n[bundle]", "version = \"../../evil\"\n\n[bundle]")).join("\n").contains("kit.version of a shared kit"));
    // Versions and ids name folders; a misspelt key is named.
    let m = messages(&bundled(&format!(
        "[[requires]]\ntype = \"dataset\"\nid = \"Upper\"\nversion = \"../../x\"\nfiles_sha256 = \"{SHA}\"\nrecomended = true\n\n[examples]\nproject = \"a./b\"\n"
    )))
    .join("\n");
    for needle in ["requires.id is lowercase", "requires.version is letters", "no key \"recomended\"", "examples.project is a folder inside"] {
        assert!(m.contains(needle), "{needle}: {m}");
    }
    assert!(messages(&format!("requires = [1]\n{}", bundled(""))).join("\n").contains("[[requires]] tables"));
    let m = messages(&bundled("").replace("version = \"1.0.0\"\n\n[bundle]", "\n[bundle]").replace("license", "version = \"1.0.0\"\nlicense")).join("\n");
    assert!(m.contains("set kit.version to the same"), "{m}");
}
