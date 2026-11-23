//! A .wugskit written, opened, and refused when it breaks a rule.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use wugs_bundle::{open, valid_entry, write, LIMITS};
use wugs_package::ListedFile;

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wugs-bundle-{name}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn sha(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A synthetic package folder and the kit that pins it.
fn fixture(dir: &Path) -> (String, PathBuf, PathBuf) {
    let pkg = dir.join("pkg");
    fs::create_dir_all(pkg.join("data")).unwrap();
    fs::write(pkg.join("data/tags.tsv"), b"NOUN\tnoun\nVERB\tverb\n").unwrap();
    fs::write(pkg.join("README.md"), b"A synthetic tagset.\n").unwrap();
    let list = vec![
        ListedFile { path: "README.md".into(), size: b"A synthetic tagset.\n".len() as u64, sha256: sha(b"A synthetic tagset.\n") },
        ListedFile { path: "data/tags.tsv".into(), size: b"NOUN\tnoun\nVERB\tverb\n".len() as u64, sha256: sha(b"NOUN\tnoun\nVERB\tverb\n") },
    ];
    let pin = wugs_package::file_list_sha256(&list);
    let examples = dir.join("ex");
    fs::create_dir_all(examples.join("texts")).unwrap();
    fs::write(examples.join("texts/story.txt"), "Once there was a wug.\n").unwrap();
    let kit = format!(
        "[package]\ntype = \"kit\"\nlicense = \"MPL-2.0\"\n\n[bundle]\nauthors = [\"A. Tester\"]\nsummary = \"A test kit.\"\n\n\
         [[requires]]\ntype = \"dataset\"\nid = \"test-tagset\"\nversion = \"1.0\"\nfiles_sha256 = \"{pin}\"\nwhy = \"Tags\"\n\n\
         [[requires]]\ntype = \"model\"\nid = \"some-model\"\nversion = \"2.0\"\nfiles_sha256 = \"{}\"\ncapability = \"morphosyntax\"\n\n\
         [examples]\nproject = \"examples/sample/\"\n\n\
         [kit]\nid = \"org.test.kit\"\nname = \"Test\"\nversion = \"0.1.0\"\n\n[tools]\nenabled = [\"texts\"]\n\n[layout]\npanels = [{{ panel = \"texts\" }}]\n",
        "0".repeat(64)
    );
    (kit, pkg, examples)
}

fn requirement(kit: &str, id: &str) -> wugs_kit::Requirement {
    wugs_kit::parse(kit).unwrap().bundle.unwrap().requires.into_iter().find(|r| r.id == id).unwrap()
}

#[test]
fn a_kit_file_goes_out_and_comes_back() {
    let dir = temp("rt");
    let (kit, pkg, examples) = fixture(&dir);
    let out = dir.join("test.wugskit");
    let w = write(&out, &kit, Some(&examples), &[(requirement(&kit, "test-tagset"), pkg.clone())]).unwrap();
    assert_eq!((w.files, w.packages), (4, 1));
    assert!(write(&out, &kit, None, &[]).is_err(), "never replaces a file");
    let opened = open(&out, &dir.join("stage"), LIMITS).unwrap();
    assert_eq!(opened.kit.id, "org.test.kit");
    assert!(opened.listed && opened.examples);
    assert_eq!(opened.included.len(), 1);
    assert_eq!(opened.missing.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["some-model"], "the model is fetched by reference later");
    assert_eq!(fs::read_to_string(dir.join("stage/examples/sample/texts/story.txt")).unwrap(), "Once there was a wug.\n");
    // A package whose files changed no longer matches its pin.
    fs::write(pkg.join("README.md"), b"changed\n").unwrap();
    assert!(write(&dir.join("bad.wugskit"), &kit, None, &[(requirement(&kit, "test-tagset"), pkg)]).unwrap_err().contains("do not match"));
    fs::remove_dir_all(dir).ok();
}

/// A zip with the given entries, written by hand so it can break the rules.
fn raw_zip(path: &Path, entries: &[(&str, &[u8], Option<u32>)]) {
    let f = fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    for (name, data, mode) in entries {
        let mut o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        if let Some(m) = mode {
            o = o.unix_permissions(*m);
        }
        if mode.is_some_and(|m| m & 0o170000 == 0o120000) {
            z.add_symlink(*name, std::str::from_utf8(data).unwrap(), o).unwrap();
        } else {
            z.start_file(*name, o).unwrap();
            z.write_all(data).unwrap();
        }
    }
    z.finish().unwrap();
}

#[test]
fn a_file_that_breaks_a_rule_is_refused_whole() {
    let dir = temp("refuse");
    let (kit, _, _) = fixture(&dir);
    let kit = kit.as_bytes();
    let cases: Vec<(&str, Vec<(&str, &[u8], Option<u32>)>, &str)> = vec![
        ("dotdot", vec![("kit.toml", kit, None), ("../evil.txt", b"x", None)], "plain relative path"),
        ("absolute", vec![("kit.toml", kit, None), ("/etc/evil", b"x", None)], "plain relative path"),
        ("case", vec![("kit.toml", kit, None), ("examples/a.txt", b"x", None), ("examples/A.txt", b"y", None)], "apart from case"),
        ("device", vec![("kit.toml", kit, None), ("con.txt", b"x", None)], "plain relative path"),
        ("symlink", vec![("kit.toml", kit, None), ("link", b"/etc/passwd", Some(0o120777))], "symbolic link"),
        ("nokit", vec![("other.toml", b"x", None)], "no kit.toml"),
        ("stray", vec![("kit.toml", kit, None), ("packages/dataset/other/1.0/a.txt", b"x", None)], "does not require"),
        ("pin", vec![("kit.toml", kit, None), ("packages/dataset/test-tagset/1.0/a.txt", b"x", None)], "does not match the files_sha256"),
        ("list", vec![("kit.toml", kit, None), ("files.sha256", b"0000 1 kit.toml\n", None)], "own list"),
    ];
    for (name, entries, want) in cases {
        let path = dir.join(format!("{name}.wugskit"));
        raw_zip(&path, &entries);
        let stage = dir.join(format!("stage-{name}"));
        let e = open(&path, &stage, LIMITS).unwrap_err();
        assert!(e.contains(want), "{name}: {e}");
        assert!(!stage.exists(), "{name}: nothing is left staged");
    }
    // A zip bomb: a megabyte and more of zeros packed tiny.
    let zeros = vec![0u8; 3 << 20];
    let path = dir.join("bomb.wugskit");
    raw_zip(&path, &[("kit.toml", kit, None), ("examples/zeros.bin", &zeros, None)]);
    assert!(open(&path, &dir.join("stage-bomb"), LIMITS).unwrap_err().contains("far more"));
    fs::remove_dir_all(dir).ok();
}

#[test]
fn entry_names_follow_the_package_rules() {
    assert!(valid_entry("examples/sample/texts/a-1_b.txt"));
    for bad in ["", "../x", "a/../b", "/x", "C:/x", "a\\b", "a/./b", "nul", "x.", "é.txt"] {
        assert!(!valid_entry(bad), "{bad}");
    }
}
