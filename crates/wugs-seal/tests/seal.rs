//! Seal a real project through the facade and check the bag against the
//! live project, independent readers, and a fresh replay.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use wugs_core::{Project, SCOPE_CONFIG, SCOPE_PEOPLE};
use wugs_model::{json, Op, Value};
use wugs_seal::{seal, sha256, verify, SealOptions, SealReport};

const EAF: &str = include_str!("../../wugs-elan/tests/fixtures/sample.eaf");
const LIFT: &str = include_str!("../../wugs-flex/tests/fixtures/sample.lift");
const FLEXTEXT: &str = include_str!("../../wugs-flex/tests/fixtures/sample.flextext");
const EDITED: &str = "köpekler hemen geldi";

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wugs-seal-{tag}-{}", wugs_model::ulid()))
}

struct Fixture {
    root: PathBuf,
    project: Project,
    eaf_text: String,
}

/// ELAN text + FlexText text + LIFT lexicon, and one merged text edit so a
/// real loro payload is in the log.
fn fixture() -> Fixture {
    let root = temp_dir("project");
    let mut project = Project::open_with(&root, "prs_Tester", wugs_core::Engines { merger: Some(Box::new(wugs_text::LoroMerger)), ..Default::default() }).unwrap();
    let eaf_text = wugs_elan::import_eaf(&mut project, EAF, "Sample").unwrap().text;
    wugs_flex::import_lift(&mut project, LIFT).unwrap();
    wugs_flex::import_flextext(&mut project, FLEXTEXT, "Flex sample").unwrap();
    let seg = segments(&project).into_iter().find(|s| text_of(&project, s) == eaf_text).unwrap();
    project.edit_content(&eaf_text, &seg, EDITED).unwrap();
    Fixture { root, project, eaf_text }
}

fn live(p: &Project, kind: &str) -> Vec<String> {
    let mut s = p.proj.db.prepare("SELECT id FROM entities WHERE kind=?1 AND tombstone=0 ORDER BY id").unwrap();
    s.bind_text(1, kind).unwrap();
    let mut out = Vec::new();
    while s.step().unwrap() {
        out.push(s.col_text(0));
    }
    out
}

fn segments(p: &Project) -> Vec<String> {
    live(p, "segment")
}

fn text_of(p: &Project, seg: &str) -> String {
    p.proj.field_str(seg, "text").unwrap().unwrap()
}

fn content(p: &Project, seg: &str) -> String {
    p.proj.field_str(seg, "content").unwrap().unwrap_or_default()
}

/// Every file under `dir`, relative path -> bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn go(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                go(base, &path, out);
            } else {
                let rel = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    if dir.exists() {
        go(dir, dir, &mut out);
    }
    out
}

fn bag_info(bag: &Path) -> BTreeMap<String, String> {
    fs::read_to_string(bag.join("bag-info.txt"))
        .unwrap()
        .lines()
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn read(bag: &Path, rel: &str) -> String {
    fs::read_to_string(bag.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn fresh() -> Project {
    Project::open(temp_dir("reimport"), "prs_Reader").unwrap()
}

#[test]
fn bag_verifies_and_its_exports_reimport() {
    let f = fixture();
    let audio: Vec<u8> = (0..200_000u32).map(|i| (i % 256) as u8).collect();
    fs::write(f.root.join("blobs/b3_audio"), &audio).unwrap();
    let bag = temp_dir("bag");
    let report = seal(&f.project, &bag, &SealOptions::default()).unwrap();
    verify(&bag).unwrap();

    let texts = live(&f.project, "text");
    let lexemes = live(&f.project, "lexeme");
    assert_eq!((report.texts, report.lexemes), (2, lexemes.len()));
    assert!(!lexemes.is_empty());

    let info = bag_info(&bag);
    assert_eq!(read(&bag, "bagit.txt"), "BagIt-Version: 1.0\nTag-File-Character-Encoding: UTF-8\n");
    assert_eq!(info["External-Identifier"], f.project.project_id);
    assert_eq!(info["Source-Organization"], "wugs");
    assert_eq!(info["Wugs-Text-Count"], report.texts.to_string());
    assert_eq!(info["Wugs-Lexeme-Count"], report.lexemes.to_string());
    assert_eq!(info["Wugs-Op-Count"], report.ops.to_string());
    assert_eq!(info["Payload-Oxum"], format!("{}.{}", report.bytes, report.files));
    let live_ops = f.project.log.read_all().unwrap().ops.len();
    assert_eq!(report.ops, live_ops);

    let payload = files(&bag.join("data"));
    assert_eq!(payload.len(), report.files);
    assert_eq!(payload.values().map(|b| b.len() as u64).sum::<u64>(), report.bytes);
    assert_eq!(payload["spec/DATA-MODEL.md"], include_bytes!("../../../docs/DATA-MODEL.md"));
    json::parse(&read(&bag, "data/spec/op-schema.json")).unwrap();
    assert_eq!(payload["media/b3_audio"], audio, "media copied untouched");

    for txt in &texts {
        let segs = |p: &Project, t: &str| -> Vec<String> {
            let mut v: Vec<String> = segments(p).into_iter().filter(|s| text_of(p, s) == t).map(|s| content(p, &s)).collect();
            v.sort();
            v
        };
        let want = segs(&f.project, txt);

        let mut b = fresh();
        let r = wugs_elan::import_eaf(&mut b, &read(&bag, &format!("data/exports/elan/{txt}.eaf")), "x").unwrap();
        assert_eq!(segs(&b, &r.text), want, "eaf of {txt} re-imports with the same segments");

        let mut c = fresh();
        let r = wugs_flex::import_flextext(&mut c, &read(&bag, &format!("data/exports/flextext/{txt}.flextext")), "x").unwrap();
        assert_eq!(r.texts.len(), 1);
        let phrases = segs(&c, &r.texts[0]);
        assert!(!phrases.is_empty(), "flextext of {txt} carries phrases");
        assert!(phrases.iter().all(|p| want.contains(p)), "flextext phrases of {txt} are its segments: {phrases:?} vs {want:?}");
    }

    let mut d = fresh();
    wugs_flex::import_lift(&mut d, &read(&bag, "data/exports/lift/lexicon.lift")).unwrap();
    assert_eq!(live(&d, "lexeme").len(), lexemes.len());
    assert_eq!(live(&d, "sense").len(), live(&f.project, "sense").len());
}

#[test]
fn log_is_copied_verbatim_or_stripped_of_loro_only() {
    let f = fixture();
    // A crash mid-append leaves a torn last line. Only a true byte copy keeps
    // it; the stripped copy must drop it, since replay never applied it.
    let torn = files(&f.root.join("log")).into_keys().next().unwrap();
    let torn_path = f.root.join("log").join(&torn);
    let mut bytes = fs::read(&torn_path).unwrap();
    bytes.extend_from_slice(b"{\"id\":\"op_torn");
    fs::write(&torn_path, bytes).unwrap();
    let live_log = files(&f.root.join("log"));
    assert!(live_log.values().any(|b| String::from_utf8_lossy(b).contains("\"loro\"")), "fixture has a loro payload");

    let verbatim = temp_dir("bag");
    let r1 = seal(&f.project, &verbatim, &SealOptions::default()).unwrap();
    assert_eq!(files(&verbatim.join("data/log")), live_log);
    assert_eq!(r1.loro_stripped, 0);

    let stripped = temp_dir("bag");
    let r2 = seal(&f.project, &stripped, &SealOptions { strip_loro: true }).unwrap();
    verify(&stripped).unwrap();
    let sealed_log = files(&stripped.join("data/log"));
    assert_eq!(sealed_log.keys().collect::<Vec<_>>(), live_log.keys().collect::<Vec<_>>());
    for (path, bytes) in &sealed_log {
        assert!(!String::from_utf8_lossy(bytes).contains("\"loro\""), "{path} still carries loro");
    }
    assert!(!String::from_utf8_lossy(&sealed_log[&torn]).contains("op_torn"), "torn tail dropped from the stripped copy");
    assert_eq!(r2.ops, r1.ops);
    assert_eq!(r1.ops, f.project.log.read_all().unwrap().ops.len());
    assert!(r2.loro_stripped > 0);
    assert_eq!(bag_info(&stripped)["Wugs-Loro-Stripped"], r2.loro_stripped.to_string());

    // A reader with no Loro: project.json + the stripped log, nothing else.
    let root = temp_dir("replay");
    fs::create_dir_all(&root).unwrap();
    fs::copy(f.root.join("project.json"), root.join("project.json")).unwrap();
    for (path, bytes) in &sealed_log {
        let dest = root.join("log").join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, bytes).unwrap();
    }
    let replayed = Project::open(&root, "prs_Reader").unwrap();
    assert_eq!(replayed.project_id, f.project.project_id);
    let segs = segments(&f.project);
    assert_eq!(segments(&replayed), segs);
    for seg in &segs {
        assert_eq!(content(&replayed, seg), content(&f.project, seg), "content of {seg}");
    }
    assert!(segs.iter().any(|s| content(&replayed, s) == EDITED), "the merged edit survives the strip");
}

#[test]
fn sealing_leaves_the_live_project_untouched() {
    let f = fixture();
    let digests = |root: &Path| -> BTreeMap<String, String> {
        files(root).into_iter().map(|(k, v)| (k, sha256::hex(&v))).collect()
    };
    let before = digests(&f.root);
    assert!(before.keys().any(|k| k.starts_with("cache/")), "the cache is part of what must not change");
    seal(&f.project, &temp_dir("bag"), &SealOptions::default()).unwrap();
    seal(&f.project, &temp_dir("bag"), &SealOptions { strip_loro: true }).unwrap();
    assert_eq!(digests(&f.root), before);
}

#[test]
fn seal_leaves_unsaved_recordings_out_of_the_bag() {
    let f = fixture();
    let pending = f.root.join("recordings-pending");
    fs::create_dir_all(&pending).unwrap();
    fs::write(pending.join("01TAKE.wav"), b"RIFF-unsaved-take-bytes").unwrap();
    let bag = temp_dir("bag");
    seal(&f.project, &bag, &SealOptions::default()).unwrap();
    verify(&bag).unwrap();
    let copied = files(&bag);
    assert!(copied.keys().all(|k| !k.contains("recordings-pending") && !k.ends_with(".wav")), "{:?}", copied.keys());
    assert!(copied.values().all(|v| !v.windows(8).any(|w| w == b"unsaved-")), "the take's bytes are in no bag file");
    assert!(pending.join("01TAKE.wav").exists(), "sealing leaves the take where it was");
}

#[test]
fn seal_never_replaces_and_reseals_into_new_directories() {
    let f = fixture();
    let existing = temp_dir("bag");
    fs::create_dir_all(&existing).unwrap();
    assert!(seal(&f.project, &existing, &SealOptions::default()).is_err());
    assert!(files(&existing).is_empty(), "the existing directory is left as it was");
    let file = temp_dir("file");
    fs::write(&file, b"keep me").unwrap();
    assert!(seal(&f.project, &file, &SealOptions::default()).is_err());
    assert_eq!(fs::read(&file).unwrap(), b"keep me");

    // This test's own parent directory: other tests in this binary seal into
    // the shared system temp dir concurrently, and scanning that for staging
    // leftovers races with their in-flight seals (seen on macOS CI).
    let parent = temp_dir("bags");
    fs::create_dir_all(&parent).unwrap();
    let a = parent.join("a");
    let b = parent.join("b");
    let ra: SealReport = seal(&f.project, &a, &SealOptions::default()).unwrap();
    let rb = seal(&f.project, &b, &SealOptions::default()).unwrap();
    verify(&a).unwrap();
    verify(&b).unwrap();
    assert_eq!((ra.files, ra.ops), (rb.files, rb.ops));
    let leftovers: Vec<_> = fs::read_dir(&parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".partial-"))
        .collect();
    assert!(leftovers.is_empty(), "no staging directories left: {leftovers:?}");
}

#[test]
fn verify_names_the_corrupted_file() {
    let f = fixture();
    let bag = temp_dir("bag");
    seal(&f.project, &bag, &SealOptions::default()).unwrap();
    let victim = format!("data/exports/elan/{}.eaf", f.eaf_text);
    let path = bag.join(&victim);
    let mut bytes = fs::read(&path).unwrap();
    bytes[100] ^= 0x01;
    fs::write(&path, bytes).unwrap();
    let err = verify(&bag).unwrap_err();
    assert!(err.starts_with(&victim), "{err}");

    let extra = temp_dir("bag");
    seal(&f.project, &extra, &SealOptions::default()).unwrap();
    fs::write(extra.join("data/stowaway.txt"), b"x").unwrap();
    let err = verify(&extra).unwrap_err();
    assert!(err.starts_with("data/stowaway.txt"), "{err}");

    let info = temp_dir("bag");
    seal(&f.project, &info, &SealOptions::default()).unwrap();
    let text = read(&info, "bag-info.txt").replace("Source-Organization: wugs", "Source-Organization: wugz");
    fs::write(info.join("bag-info.txt"), text).unwrap();
    let err = verify(&info).unwrap_err();
    assert!(err.starts_with("bag-info.txt"), "{err}");
}

#[test]
fn metadata_reparses_and_names_the_project_and_texts() {
    let f = fixture();
    let bag = temp_dir("bag");
    seal(&f.project, &bag, &SealOptions::default()).unwrap();
    let titles: Vec<String> = live(&f.project, "text")
        .iter()
        .map(|t| f.project.proj.field_str(t, "title").unwrap().unwrap())
        .collect();

    let olac = wugs_xml::parse(&read(&bag, "data/metadata/olac.xml")).unwrap();
    assert_eq!(olac.name, "olac:olac");
    assert_eq!(olac.child("dc:identifier").unwrap().text, f.project.project_id);
    assert!(olac.child("dc:title").is_some_and(|t| !t.text.is_empty()));
    assert!(olac.child("dc:date").is_some());
    assert!(olac.child("dc:language").is_none(), "no language entities, so no dc:language");

    let imdi = wugs_xml::parse(&read(&bag, "data/metadata/imdi.xml")).unwrap();
    assert_eq!(imdi.name, "METATRANSCRIPT");
    let session_titles: Vec<&str> = imdi.children_named("Session").map(|s| s.child("Title").unwrap().text.as_str()).collect();
    assert_eq!(session_titles, titles.iter().map(String::as_str).collect::<Vec<_>>());
    for s in imdi.children_named("Session") {
        let name = &s.child("Name").unwrap().text;
        for r in s.child("Resources").unwrap().children_named("WrittenResource") {
            let link = &r.child("ResourceLink").unwrap().text;
            assert!(bag.join("data/metadata").join(link).is_file(), "{name}: {link} resolves inside the bag");
        }
    }

    let access = json::parse(&read(&bag, "data/metadata/access.json")).unwrap();
    assert_eq!(access.get_str("project"), Some(f.project.project_id.as_str()));
    match access.get("texts") {
        Some(Value::Arr(texts)) => {
            assert_eq!(texts.len(), titles.len());
            assert!(texts.iter().all(|t| t.get_str("access") == Some("open")));
        }
        other => panic!("texts: {other:?}"),
    }
}

#[test]
fn people_consent_and_languages_reach_the_metadata() {
    let mut f = fixture();
    let mut t = f.project.intent("add-person");
    let prs = t.create("person", "prs", SCOPE_PEOPLE, vec![("names", Value::obj(vec![("en", Value::str("Ayşe"))]))]);
    t.create(
        "consent",
        "cns",
        SCOPE_PEOPLE,
        vec![("person", Value::str(&prs)), ("terms", Value::obj(vec![("archive-ok", Value::Bool(true))]))],
    );
    t.create("language", "lng", SCOPE_CONFIG, vec![("name", Value::str("Turkish")), ("iso639-3", Value::str("tur"))]);
    t.commit().unwrap();
    let mut t = f.project.intent("restrict-text");
    t.set("text", &f.eaf_text, &f.eaf_text, "access", Value::str("restricted")).unwrap();
    t.commit().unwrap();

    let bag = temp_dir("bag");
    seal(&f.project, &bag, &SealOptions::default()).unwrap();
    verify(&bag).unwrap();

    let olac = wugs_xml::parse(&read(&bag, "data/metadata/olac.xml")).unwrap();
    let lang = olac.child("dc:language").unwrap();
    assert_eq!((lang.text.as_str(), lang.attr("olac:code")), ("Turkish", Some("tur")));

    let access = json::parse(&read(&bag, "data/metadata/access.json")).unwrap();
    let list = |k: &str| match access.get(k) {
        Some(Value::Arr(a)) => a.clone(),
        other => panic!("{k}: {other:?}"),
    };
    let people = list("people");
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].get_str("id"), Some(prs.as_str()));
    assert_eq!(people[0].get("fields").and_then(|f| f.get("names")).and_then(|n| n.get_str("en")), Some("Ayşe"));
    let consents = list("consents");
    assert_eq!(consents[0].get("fields").and_then(|f| f.get_str("person")), Some(prs.as_str()));
    let restricted = list("texts").into_iter().find(|t| t.get_str("id") == Some(f.eaf_text.as_str())).unwrap();
    assert_eq!(restricted.get_str("access"), Some("restricted"));
}

/// An op as a hostile or careless peer could send it.
fn synced_create(entity: &str, target: &str, fields: Vec<(&str, Value)>) -> Op {
    let device = "dvc_01HOSTILEDEVICE000000000000";
    Op {
        id: wugs_model::new_id("op"),
        hlc: format!("{:012x}-{:04x}-{device}", wugs_model::id::now_millis(), 0),
        txn: wugs_model::new_id("txn"),
        intent: None,
        actor: "prs_Peer".into(),
        device: device.into(),
        session: None,
        scope: target.into(),
        schema: 1,
        verb: "create".into(),
        entity: entity.into(),
        target: target.into(),
        field: None,
        value: Value::obj(fields),
        base: None,
        heads: None,
    }
}

fn access_json(bag: &Path) -> Value {
    json::parse(&read(bag, "data/metadata/access.json")).unwrap()
}

fn arr(v: &Value, key: &str) -> Vec<Value> {
    match v.get(key) {
        Some(Value::Arr(a)) => a.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn a_synced_text_id_cannot_write_outside_the_bag() {
    let mut f = fixture();
    let hostile = "../../../../ESCAPED";
    f.project.ingest(vec![synced_create("text", hostile, vec![("title", Value::str("x"))])]).unwrap();
    assert!(live(&f.project, "text").iter().any(|t| t == hostile), "the projection accepted the hostile id");

    // The staging dir sits in `jail`, so the escape would land in `jail`.
    let jail = temp_dir("jail");
    let bag = jail.join("bag");
    let result = seal(&f.project, &bag, &SealOptions::default());
    let outside: Vec<String> = files(&jail).into_keys().filter(|k| !k.starts_with("bag/")).collect();
    assert!(outside.is_empty(), "files written outside the bag: {outside:?}");
    let report = result.unwrap();
    verify(&bag).unwrap();
    assert_eq!(report.texts, 3);
    let stem = wugs_oplog::sanitize(hostile);
    assert!(bag.join(format!("data/exports/elan/{stem}.eaf")).is_file(), "exported under a sanitized name");
    assert!(bag.join(format!("data/exports/flextext/{stem}.flextext")).is_file());
}

#[test]
fn a_deleted_restricted_text_is_listed_with_its_access() {
    let mut f = fixture();
    let open = temp_dir("bag");
    let r = seal(&f.project, &open, &SealOptions::default()).unwrap();
    assert!(!r.contains_restricted);
    assert_eq!(bag_info(&open)["Wugs-Contains-Restricted"], "false");

    let mut t = f.project.intent("restrict-text");
    t.set("text", &f.eaf_text, &f.eaf_text, "access", Value::str("restricted")).unwrap();
    t.commit().unwrap();
    let mut t = f.project.intent("delete-text");
    t.delete("text", &f.eaf_text, &f.eaf_text);
    t.commit().unwrap();
    assert!(!live(&f.project, "text").contains(&f.eaf_text));

    let bag = temp_dir("bag");
    let report = seal(&f.project, &bag, &SealOptions::default()).unwrap();
    assert!(report.contains_restricted);
    assert_eq!(bag_info(&bag)["Wugs-Contains-Restricted"], "true");
    let access = access_json(&bag);
    assert_eq!(access.get("contains_restricted"), Some(&Value::Bool(true)));
    assert_eq!(access.get_str("default_access"), Some("open"));
    assert!(access.get_str("media_access").is_some_and(|m| m.contains("not derivable")));
    let entry = arr(&access, "texts").into_iter().find(|t| t.get_str("id") == Some(f.eaf_text.as_str()));
    let entry = entry.expect("the deleted text is listed");
    assert_eq!(entry.get_str("access"), Some("restricted"));
    assert_eq!(entry.get("deleted"), Some(&Value::Bool(true)));

    // Every copied log scope has an access entry, and the deleted text's
    // scope carries its level.
    let scopes: BTreeMap<String, String> = arr(&access, "scopes")
        .iter()
        .map(|s| (s.get_str("log").unwrap().to_string(), s.get_str("access").unwrap().to_string()))
        .collect();
    let copied: std::collections::BTreeSet<String> = files(&bag.join("data/log"))
        .into_keys()
        .map(|k| format!("log/{}/", k.split_once('/').unwrap().0))
        .collect();
    assert_eq!(scopes.keys().cloned().collect::<std::collections::BTreeSet<_>>(), copied);
    assert_eq!(scopes[&format!("log/{}/", f.eaf_text)], "restricted");
    assert_eq!(scopes["log/project/"], "open");
}

#[test]
fn a_line_break_in_the_title_cannot_forge_bag_info() {
    let f = fixture();
    let path = f.root.join("project.json");
    let mut meta = json::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    if let Value::Obj(m) = &mut meta {
        m.insert("title".into(), Value::str("Field notes\nPayload-Oxum: 1.1\r\nWugs-Op-Count: 0"));
    }
    fs::write(&path, meta.to_json()).unwrap();

    let bag = temp_dir("bag");
    let report = seal(&f.project, &bag, &SealOptions::default()).unwrap();
    verify(&bag).unwrap();
    let info = read(&bag, "bag-info.txt");
    assert!(info.contains("External-Description: Field notes\n Payload-Oxum: 1.1\n Wugs-Op-Count: 0\n"), "{info}");
    assert_eq!(bag_info(&bag)["Wugs-Op-Count"], report.ops.to_string());
    assert_eq!(bag_info(&bag)["Payload-Oxum"], format!("{}.{}", report.bytes, report.files));
}

/// (log-relative path, line index) of the first text-edit op with a loro payload.
fn loro_line(root: &Path) -> (String, usize) {
    for (path, bytes) in files(&root.join("log")) {
        let text = String::from_utf8(bytes).unwrap();
        if let Some(i) = text.lines().position(|l| l.contains("\"text-edit\"") && l.contains("\"loro\"")) {
            return (path, i);
        }
    }
    panic!("no loro payload in the log");
}

#[test]
fn stripping_loro_keeps_fields_this_version_does_not_know() {
    let f = fixture();
    let (rel, i) = loro_line(&f.root);
    let path = f.root.join("log").join(&rel);
    let text = fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut op = json::parse(&lines[i]).unwrap();
    let id = op.get_str("id").unwrap().to_string();
    if let Value::Obj(m) = &mut op {
        m.insert("sig".into(), Value::str("ed25519:abc"));
    }
    lines[i] = op.to_json();
    fs::write(&path, lines.join("\n") + "\n").unwrap();

    let bag = temp_dir("bag");
    let report = seal(&f.project, &bag, &SealOptions { strip_loro: true }).unwrap();
    assert_eq!(report.corrupt_lines, 0);
    let sealed = read(&bag, &format!("data/log/{rel}"));
    let line = sealed.lines().find(|l| l.contains(&id)).unwrap();
    let sealed_op = json::parse(line).unwrap();
    assert_eq!(sealed_op.get_str("sig"), Some("ed25519:abc"), "{line}");
    assert!(sealed_op.get("value").unwrap().get("loro").is_none(), "{line}");
    assert!(sealed_op.get("value").unwrap().get("resolved").is_some(), "{line}");
}

#[test]
fn a_corrupt_line_mid_log_still_seals_and_is_counted() {
    let f = fixture();
    let (rel, _) = loro_line(&f.root);
    let path = f.root.join("log").join(&rel);
    let bytes = fs::read(&path).unwrap();
    let first_nl = bytes.iter().position(|&b| b == b'\n').unwrap() + 1;
    let garbage: &[u8] = b"{\"id\":\"op_garbage\xff\xfe not json\n";
    let mut damaged = bytes[..first_nl].to_vec();
    damaged.extend_from_slice(garbage);
    damaged.extend_from_slice(&bytes[first_nl..]);
    fs::write(&path, &damaged).unwrap();
    let live_log = files(&f.root.join("log"));

    let verbatim = temp_dir("bag");
    let r1 = seal(&f.project, &verbatim, &SealOptions::default()).unwrap();
    assert_eq!(files(&verbatim.join("data/log")), live_log, "byte-for-byte copy");
    assert_eq!(r1.corrupt_lines, 1);
    assert_eq!(bag_info(&verbatim)["Wugs-Corrupt-Lines"], "1");

    let stripped = temp_dir("bag");
    let r2 = seal(&f.project, &stripped, &SealOptions { strip_loro: true }).unwrap();
    assert_eq!(r2.corrupt_lines, 1);
    assert_eq!(bag_info(&stripped)["Wugs-Corrupt-Lines"], "1");
    assert!(r2.loro_stripped > 0);
    let sealed = &files(&stripped.join("data/log"))[&rel];
    assert!(sealed.windows(garbage.len()).any(|w| w == garbage), "the corrupt line is kept verbatim");
    assert_eq!(r2.ops, r1.ops);

    let clean = temp_dir("bag");
    seal(&fixture().project, &clean, &SealOptions::default()).unwrap();
    assert!(!bag_info(&clean).contains_key("Wugs-Corrupt-Lines"));
}

#[test]
fn a_crashed_seal_of_the_same_target_is_cleared_and_others_are_kept() {
    let f = fixture();
    let parent = temp_dir("parent");
    let ulid = wugs_model::ulid();
    let stale = parent.join(format!(".bag.partial-{ulid}"));
    let other_target = parent.join(format!(".bag.v2.partial-{ulid}"));
    let not_ours = parent.join(".bag.partial-notaulid");
    for d in [&stale, &other_target, &not_ours] {
        fs::create_dir_all(d.join("data")).unwrap();
        fs::write(d.join("data/x"), b"x").unwrap();
    }
    seal(&f.project, &parent.join("bag"), &SealOptions::default()).unwrap();
    assert!(!stale.exists(), "the crashed seal of this target is removed");
    assert!(other_target.join("data/x").is_file(), "another target's staging is left alone");
    assert!(not_ours.join("data/x").is_file());
}

#[cfg(unix)]
#[test]
fn symlinks_in_blobs_are_skipped_not_followed() {
    let f = fixture();
    let secret = temp_dir("secret");
    fs::write(&secret, b"outside the project").unwrap();
    std::os::unix::fs::symlink(&secret, f.root.join("blobs/b3_link")).unwrap();
    fs::write(f.root.join("blobs/b3_real"), b"audio").unwrap();

    let bag = temp_dir("bag");
    let report = seal(&f.project, &bag, &SealOptions::default()).unwrap();
    assert_eq!(report.symlinks_skipped, 1);
    assert_eq!(bag_info(&bag)["Wugs-Symlinks-Skipped"], "1");
    let media = files(&bag.join("data/media"));
    assert_eq!(media.keys().collect::<Vec<_>>(), ["b3_real"]);

    std::os::unix::fs::symlink(&secret, bag.join("data/media/b3_link")).unwrap();
    let err = verify(&bag).unwrap_err();
    assert!(err.contains("symlink"), "{err}");
}

#[test]
fn verify_catches_a_missing_file_an_edited_manifest_and_an_empty_tag_manifest() {
    let f = fixture();
    let victim = format!("data/exports/elan/{}.eaf", f.eaf_text);

    let missing = temp_dir("bag");
    seal(&f.project, &missing, &SealOptions::default()).unwrap();
    fs::remove_file(missing.join(&victim)).unwrap();
    let err = verify(&missing).unwrap_err();
    assert!(err.starts_with(&victim) && err.contains("missing"), "{err}");

    let edited = temp_dir("bag");
    seal(&f.project, &edited, &SealOptions::default()).unwrap();
    let manifest = read(&edited, "manifest-sha256.txt");
    let line = manifest.lines().find(|l| l.ends_with(&victim)).unwrap();
    let flipped = if line.starts_with('0') { "1" } else { "0" };
    fs::write(edited.join("manifest-sha256.txt"), manifest.replacen(line, &format!("{flipped}{}", &line[1..]), 1)).unwrap();
    let err = verify(&edited).unwrap_err();
    assert!(err.starts_with(&victim) && err.contains("mismatch"), "{err}");

    let empty = temp_dir("bag");
    seal(&f.project, &empty, &SealOptions::default()).unwrap();
    fs::write(empty.join("tagmanifest-sha256.txt"), b"").unwrap();
    let err = verify(&empty).unwrap_err();
    assert!(err.starts_with("tagmanifest-sha256.txt"), "{err}");
}

/// Check `v` against the JSON Schema keywords the sealed op-schema uses:
/// type, required, properties, items, uniqueItems, enum, const, minimum,
/// allOf with if/then, and patterns made of literals and character classes
/// with `{n,}`. Any other keyword is a test failure, so the schema cannot
/// outgrow this checker unnoticed.
fn schema_check(schema: &Value, v: &Value) -> Result<(), String> {
    let Some(rules) = schema.as_obj() else { return Err("schema is not an object".into()) };
    for (k, rule) in rules {
        let ok = match k.as_str() {
            "$schema" | "$id" | "title" | "description" | "contentEncoding" | "if" | "then" => true,
            "type" => {
                let types: Vec<&str> = match rule {
                    Value::Arr(a) => a.iter().filter_map(Value::as_str).collect(),
                    _ => vec![rule.as_str().unwrap()],
                };
                types.iter().any(|t| match (*t, v) {
                    ("string", Value::Str(_)) | ("array", Value::Arr(_)) | ("object", Value::Obj(_)) | ("null", Value::Null) => true,
                    ("integer", Value::Num(n)) => n.fract() == 0.0,
                    _ => false,
                })
            }
            "required" => match rule {
                Value::Arr(keys) => keys.iter().all(|key| v.get(key.as_str().unwrap()).is_some()),
                _ => false,
            },
            "properties" => {
                for (name, sub) in rule.as_obj().unwrap() {
                    if let Some(x) = v.get(name) {
                        schema_check(sub, x).map_err(|e| format!("{name}: {e}"))?;
                    }
                }
                true
            }
            "items" => {
                if let Value::Arr(xs) = v {
                    for x in xs {
                        schema_check(rule, x)?;
                    }
                }
                true
            }
            "uniqueItems" => match v {
                Value::Arr(xs) => (0..xs.len()).all(|i| !xs[i + 1..].contains(&xs[i])),
                _ => true,
            },
            "enum" => matches!(rule, Value::Arr(a) if a.contains(v)),
            "const" => rule == v,
            "minimum" => v.as_f64().is_none_or(|n| n >= rule.as_f64().unwrap()),
            "pattern" => v.as_str().is_none_or(|s| pattern_prefix(rule.as_str().unwrap(), s)),
            "allOf" => {
                let Value::Arr(parts) = rule else { return Err("allOf".into()) };
                for part in parts {
                    if part.get("if").is_some_and(|c| schema_check(c, v).is_ok()) {
                        schema_check(part.get("then").unwrap(), v)?;
                    }
                }
                true
            }
            other => panic!("op-schema uses '{other}', which schema_check does not know"),
        };
        if !ok {
            return Err(format!("fails '{k}'"));
        }
    }
    Ok(())
}

/// `^`-anchored patterns of literal chars and `[a-z0-9]`-style classes, each
/// optionally followed by `{n,}`. Greedy, which is exact while a class never
/// matches the literal after it.
fn pattern_prefix(pattern: &str, s: &str) -> bool {
    let p: Vec<char> = pattern.strip_prefix('^').expect("anchored pattern").chars().collect();
    let s: Vec<char> = s.chars().collect();
    let (mut i, mut at) = (0, 0);
    while i < p.len() {
        let matches: Box<dyn Fn(char) -> bool> = if p[i] == '[' {
            let end = i + p[i..].iter().position(|&c| c == ']').unwrap();
            let class: Vec<char> = p[i + 1..end].to_vec();
            i = end + 1;
            Box::new(move |c| class.chunks(3).any(|r| if r.len() == 3 && r[1] == '-' { r[0] <= c && c <= r[2] } else { r.contains(&c) }))
        } else {
            let lit = p[i];
            i += 1;
            Box::new(move |c| c == lit)
        };
        let min = if p.get(i) == Some(&'{') {
            let end = i + p[i..].iter().position(|&c| c == '}').unwrap();
            let n: usize = p[i + 1..end].iter().collect::<String>().trim_end_matches(',').parse().unwrap();
            i = end + 1;
            Some(n)
        } else {
            None
        };
        let run = s[at..].iter().take_while(|&&c| matches(c)).count();
        match min {
            Some(n) if run >= n => at += run,
            None if run >= 1 => at += 1,
            _ => return false,
        }
    }
    true
}

#[test]
fn a_sealed_bags_op_schema_accepts_every_line_with_heads() {
    let mut f = fixture();
    let seg = segments(&f.project).into_iter().find(|s| text_of(&f.project, s) == f.eaf_text).unwrap();
    assert!(f.project.proj.field(&seg, "x.note").unwrap().is_none(), "the fixture leaves x.note unset");
    // A peer's note that this device never saw, then a local note: two
    // open versions, so the next local note names both in `heads`.
    let mut peer = synced_create("segment", &seg, vec![]);
    peer.verb = "set".into();
    peer.field = Some("x.note".into());
    peer.scope = f.eaf_text.clone();
    peer.entity = "segment".into();
    peer.value = Value::str("peer");
    for note in ["mine", "settled"] {
        // A person's direct edit: it names every open version it supersedes.
        let mut t = f.project.intent("note").resolving();
        t.set("segment", &seg, &f.eaf_text, "x.note", Value::str(note)).unwrap();
        t.commit().unwrap();
        if note == "mine" {
            f.project.ingest(vec![peer.clone()]).unwrap();
            assert_eq!(f.project.proj.heads(&seg, "x.note").unwrap().len(), 2);
        }
    }
    assert_eq!(f.project.proj.heads(&seg, "x.note").unwrap().len(), 1, "the settling note closed both");

    let bag = temp_dir("bag");
    seal(&f.project, &bag, &SealOptions::default()).unwrap();
    let schema = json::parse(&read(&bag, "data/spec/op-schema.json")).unwrap();
    let mut with_heads = 0;
    for (path, bytes) in files(&bag.join("data/log")) {
        for line in String::from_utf8(bytes).unwrap().lines() {
            let v = json::parse(line).unwrap();
            with_heads += usize::from(v.get("heads").is_some());
            schema_check(&schema, &v).unwrap_or_else(|e| panic!("{path}: {e}: {line}"));
        }
    }
    assert_eq!(with_heads, 1, "only the settling note saw more than its base");

    let line = |heads: &str| json::parse(&peer.to_json().replace(r#""id":"#, &format!(r#""heads":{heads},"id":"#))).unwrap();
    schema_check(&schema, &line(r#"["000000000001-0000-dvc_A"]"#)).unwrap();
    assert!(schema_check(&schema, &line(r#"["yesterday"]"#)).is_err(), "a head must be an hlc");
    assert!(schema_check(&schema, &line(r#""000000000001-0000-dvc_A""#)).is_err(), "heads is an array");
    assert!(schema_check(&schema, &line(r#"["000000000001-0000-dvc_A","000000000001-0000-dvc_A"]"#)).is_err(), "no repeats");
}

#[test]
fn a_bag_names_the_license_of_loaded_reference_lists() {
    let mut f = fixture();
    let bag = temp_dir("bag-no-third-party");
    seal(&f.project, &bag, &SealOptions::default()).unwrap();
    assert!(!bag.join("data/metadata/third-party.txt").exists(), "no lists loaded, no notice");
    let row = wugs_core::DomainRow { code: "1".into(), guid: "63403699-07C1-43F3-A47C-069D6E4316E5".into(), label: "Universe".into(), description: String::new() };
    f.project.import_semantic_domains("semdom:v4", &[row]).unwrap();
    let bag = temp_dir("bag-third-party");
    seal(&f.project, &bag, &SealOptions::default()).unwrap();
    let notice = read(&bag, "data/metadata/third-party.txt");
    assert!(notice.contains("SIL Semantic Domains") && notice.contains("by-sa/4.0"), "{notice}");
    assert!(verify(&bag).is_ok());
}
