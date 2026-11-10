use wugs_package::parse;

const BUNDLED: &str = include_str!("../../../catalog/wav2vec2-espeak.toml");

#[test]
fn the_bundled_catalogue_parses() {
    let m = parse(BUNDLED).unwrap();
    assert_eq!(m.id, "wav2vec2-espeak");
    assert_eq!(m.model.capability, "phone-transcription");
    assert_eq!(m.model.runtime, "onnx-ctc-raw");
    assert_eq!(m.param_str("onnx").unwrap(), "model.onnx");
    assert_eq!(m.param_bool("normalize"), Some(true));
    assert_eq!(m.download_mb(), Some(640));
    assert_eq!(m.files.len(), 2);
    assert_eq!(m.score(None), None);
}

fn with(replace: &str, by: &str) -> String {
    assert!(BUNDLED.contains(replace), "fixture lost {replace:?}");
    BUNDLED.replacen(replace, by, 1)
}

#[test]
fn every_catalogue_file_parses() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalog");
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        if path.extension().is_some_and(|x| x == "toml") {
            parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            n += 1;
        }
    }
    assert!(n >= 1);
}

#[test]
fn unsafe_or_insecure_entries_are_refused_together() {
    let text = with("id = \"wav2vec2-espeak\"", "id = \"../evil\"")
        .replacen(
            "https://huggingface.co/sadda-speech/wav2vec2-espeak-ctc/resolve/main/model.onnx",
            "http://example.com/model.onnx",
            1,
        )
        .replacen("path = \"vocab.json\"", "path = \"../vocab.json\"", 1);
    let err = parse(&text).unwrap_err();
    assert!(err.contains("package id"), "{err}");
    assert!(err.contains("model.onnx: url must be an https://"), "{err}");
    assert!(err.contains("plain file name"), "{err}");
    assert_eq!(err.lines().count(), 3, "{err}");
}

#[test]
fn reserved_and_duplicate_names_are_refused() {
    assert!(parse(&with("id = \"wav2vec2-espeak\"", "id = \"con\""))
        .unwrap_err()
        .contains("package id"));
    assert!(
        parse(&with("path = \"vocab.json\"", "path = \"model.onnx\""))
            .unwrap_err()
            .contains("listed twice")
    );
    assert!(parse(&with("path = \"vocab.json\"", "path = \"nul.json\""))
        .unwrap_err()
        .contains("plain file name"));
}

#[test]
fn unknown_keys_and_types_are_refused() {
    assert!(parse(&with(
        "approx_mb = 640",
        "approx_mb = 640\nscripts = \"run.sh\""
    ))
    .is_err());
    assert!(parse(&with("type = \"model\"", "type = \"plugin\""))
        .unwrap_err()
        .contains("only \"model\""));
    assert!(parse(&with(
        "source = \"hf:sadda-speech/wav2vec2-espeak-ctc\"",
        "source = \"https://evil\""
    ))
    .unwrap_err()
    .contains("source"));
}

#[test]
fn checksums_sizes_and_scores() {
    let sha = "AB".repeat(32);
    let text = with("resolve/main/model.onnx\"", &format!("resolve/main/model.onnx\"\nsha256 = \"{sha}\"\nbytes = 1500000"))
        .replacen("resolve/main/vocab.json\"", "resolve/main/vocab.json\"\nbytes = 2000", 1)
        + "\n[evaluation]\nbenchmark = \"wugs-phone-bench@1\"\nper = { \"*\" = 0.3, \"tr\" = 0.2 }\n";
    let m = parse(&text).unwrap();
    assert_eq!(m.files[0].sha256.as_deref(), Some("ab".repeat(32).as_str()));
    assert_eq!(m.download_mb(), Some(2));
    assert_eq!(m.score(Some("tr")), Some(0.2));
    assert_eq!(m.score(Some("fi")), Some(0.3));
    assert!(parse(&with(
        "resolve/main/model.onnx\"",
        "resolve/main/model.onnx\"\nsha256 = \"abc\""
    ))
    .unwrap_err()
    .contains("64 hex"));
    let bad = BUNDLED.to_string() + "\n[evaluation]\nbenchmark = \"b\"\nper = { \"*\" = 7.0 }\n";
    assert!(parse(&bad).unwrap_err().contains("error rate"));
}

#[test]
fn oversized_manifests_are_refused_before_parsing() {
    let big = format!("{BUNDLED}\n#{}", "x".repeat(wugs_package::MAX_BYTES));
    assert!(parse(&big).unwrap_err().contains("at most"));
}
