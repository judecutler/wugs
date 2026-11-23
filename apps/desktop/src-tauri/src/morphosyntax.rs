//! Tagging and parsing (the `morphosyntax` capability, SCROLLS.md §2):
//! a text's sentences sent, one a line, to an existing tagger, and its
//! CoNLL-U written back on the segments (`wugs_conllu::annotate`), so the
//! Sentence panel, CQP queries and the exports read lemmas, tags and trees.
//!
//! Providers are tools linguists already use, never one hard-coded:
//! - **UDPipe on this computer** (`udpipe` with a `.udpipe` model the
//!   person chose): nothing leaves the computer.
//! - **UDPipe 2 at LINDAT** (ÚFAL's public service): the text goes to
//!   Prague, whose terms let ÚFAL keep what is sent, so a project must allow
//!   it first, and texts that are not open are never sent.
//! - **spaCy on this computer** (the person's Python with spaCy and a
//!   pipeline they installed, `scripts/spacy_tag.py`): nothing leaves the
//!   computer.
//!
//! The session thread gathers and writes; the tagger runs between, off it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::session::{self, Session};

/// The public UDPipe service.
pub const LINDAT: &str = "https://lindat.mff.cuni.cz/services/udpipe/api/process";

/// The project setting that allows sending its open texts to LINDAT.
pub const WEB_OK: &str = "morphosyntax:web-ok";

/// Sentences one request carries, so a long text is never one huge upload.
const BATCH: usize = 200;

// ---------------------------------------------------------------- UDPipe on this computer

/// What this computer uses to run UDPipe: the program (else `udpipe` on
/// PATH) and the model file, kept in `<app data>/udpipe.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSettings {
    pub program: Option<String>,
    pub model: Option<String>,
}

fn settings_path(app_data: &Path) -> PathBuf {
    app_data.join("udpipe.json")
}

pub fn local_settings(app_data: &Path) -> LocalSettings {
    std::fs::read(settings_path(app_data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn set_local_settings(app_data: &Path, s: &LocalSettings) -> Result<(), String> {
    std::fs::create_dir_all(app_data).map_err(|e| e.to_string())?;
    let clean = LocalSettings {
        program: s.program.clone().filter(|p| !p.trim().is_empty()),
        model: s.model.clone().filter(|p| !p.trim().is_empty()),
    };
    std::fs::write(settings_path(app_data), serde_json::to_vec_pretty(&clean).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// UDPipe ready to run here.
#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    pub program: PathBuf,
    pub model: PathBuf,
}

/// `udpipe` on PATH, by name.
fn on_path() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["udpipe.exe", "udpipe"] } else { &["udpipe"] };
    std::env::split_paths(&std::env::var_os("PATH")?).flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// UDPipe and its model as this computer has them, or why not, in words.
pub fn local(app_data: &Path) -> Result<Local, String> {
    let s = local_settings(app_data);
    let program = match &s.program {
        Some(p) if Path::new(p).is_file() => PathBuf::from(p),
        Some(p) => return Err(format!("UDPipe was chosen at {p}, which is not there any more.")),
        None => on_path().ok_or("UDPipe is not installed (download it from ufal.mff.cuni.cz/udpipe/1, then choose it).")?,
    };
    let model = match &s.model {
        Some(m) if Path::new(m).is_file() => PathBuf::from(m),
        Some(m) => return Err(format!("The UDPipe model {m} is not there any more.")),
        None => return Err("UDPipe needs a model for the language (a .udpipe file from the UDPipe models page); choose one.".into()),
    };
    Ok(Local { program, model })
}

/// Run UDPipe 1 on one sentence a line: its own tokenizer told each line
/// is a sentence (`presegmented`), then its tagger and parser.
pub fn run_local(l: &Local, lines: &[String]) -> Result<String, String> {
    let mut child = Command::new(&l.program)
        .arg("--tokenize")
        .arg("--tokenizer=presegmented")
        .arg("--tag")
        .arg("--parse")
        .arg(&l.model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("UDPipe could not start: {e}"))?;
    let input = lines.join("\n") + "\n";
    let mut stdin = child.stdin.take().ok_or("UDPipe took no input")?;
    // Written from another thread, so a large text never deadlocks on full pipes.
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = child.wait_with_output().map_err(|e| format!("UDPipe stopped: {e}"))?;
    writer.join().map_err(|_| "UDPipe's input could not be written".to_string())?.map_err(|e| format!("UDPipe's input could not be written: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(format!("UDPipe failed: {}", why.lines().last().unwrap_or("no message").trim()));
    }
    String::from_utf8(out.stdout).map_err(|_| "UDPipe wrote something that is not UTF-8".into())
}

// ---------------------------------------------------------------- UDPipe 2 at LINDAT

#[derive(Deserialize)]
struct Reply {
    model: Option<String>,
    result: Option<String>,
}

/// Send one sentence a line to the service; `model` is a model name, a
/// prefix of one, or a language name or code (the service picks its newest
/// model for it). Returns the CoNLL-U and the model it used.
pub fn run_lindat(endpoint: &str, model: &str, lines: &[String]) -> Result<(String, String), String> {
    use std::time::Duration;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(300)))
        .https_only(!endpoint.starts_with("http://127.0.0.1:"))
        .http_status_as_error(false)
        .build()
        .into();
    let data = lines.join("\n") + "\n";
    let form = [("model", model), ("tokenizer", "presegmented"), ("tagger", ""), ("parser", ""), ("data", data.as_str())];
    let mut r = agent.post(endpoint).send_form(form).map_err(|e| format!("Could not reach the UDPipe service ({e})."))?;
    let status = r.status().as_u16();
    let body = r.body_mut().read_to_string().map_err(|e| format!("The UDPipe service's answer could not be read: {e}"))?;
    if status != 200 {
        // The service answers a model it does not know with its own message.
        return Err(format!("The UDPipe service refused: {}", body.trim().chars().take(300).collect::<String>()));
    }
    let reply: Reply = serde_json::from_str(&body).map_err(|_| "The UDPipe service answered something wugs cannot read.".to_string())?;
    Ok((reply.result.ok_or("The UDPipe service sent no result.")?, reply.model.unwrap_or_else(|| model.to_string())))
}

// ---------------------------------------------------------------- spaCy on this computer

/// The adapter run inside the person's Python (`python -c`).
const SPACY_SCRIPT: &str = include_str!("../scripts/spacy_tag.py");

/// Loads the pipeline and says what it is, so a choice is checked when it is made.
const SPACY_CHECK: &str = "import sys\nsys.path[:] = [p for p in sys.path if p not in ('', '.')]\nimport spacy\nnlp = spacy.load(sys.argv[1])\nm = nlp.meta\nprint('spaCy %s, %s_%s %s (%s)' % (spacy.__version__, m.get('lang', ''), m.get('name', ''), m.get('version', ''), ', '.join(nlp.pipe_names)))\n";

/// What this computer uses to run spaCy: the Python (else `python3` or
/// `python` on PATH) and the pipeline, kept in `<app data>/spacy.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpacySettings {
    pub python: Option<String>,
    pub pipeline: Option<String>,
    /// What the check said when the pipeline was chosen.
    pub about: Option<String>,
}

fn spacy_path(app_data: &Path) -> PathBuf {
    app_data.join("spacy.json")
}

pub fn spacy_settings(app_data: &Path) -> SpacySettings {
    std::fs::read(spacy_path(app_data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// spaCy ready to run here.
#[derive(Debug, Clone, PartialEq)]
pub struct Spacy {
    pub python: PathBuf,
    pub pipeline: String,
}

/// Python on PATH, by name.
fn python_on_path() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["python.exe", "python3.exe"] } else { &["python3", "python"] };
    std::env::split_paths(&std::env::var_os("PATH")?).flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// A pipeline is an installed package's name (`en_core_web_sm`) or a folder
/// spaCy saved one in; it reaches Python as one argument, never through a shell.
pub fn valid_pipeline(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n.len() <= 300 && !n.chars().any(char::is_control) && (n.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)) || Path::new(n).is_dir())
}

fn python_of(s: &SpacySettings) -> Result<PathBuf, String> {
    match &s.python {
        Some(p) if Path::new(p).is_file() => Ok(PathBuf::from(p)),
        Some(p) => Err(format!("Python was chosen at {p}, which is not there any more.")),
        None => python_on_path().ok_or_else(|| "Python is not installed (spaCy runs in Python: install Python and spaCy, then choose Python).".to_string()),
    }
}

/// spaCy and its pipeline as this computer has them, or why not, in words.
pub fn spacy(app_data: &Path) -> Result<Spacy, String> {
    let s = spacy_settings(app_data);
    let python = python_of(&s)?;
    let pipeline = s.pipeline.filter(|p| valid_pipeline(p)).ok_or("spaCy needs a pipeline for the language (such as en_core_web_sm); type its name.")?;
    Ok(Spacy { python, pipeline })
}

/// What went wrong in Python, in words: the two usual cases named, else its last line.
fn python_failure(python: &Path, pipeline: &str, stderr: &[u8]) -> String {
    let why = String::from_utf8_lossy(stderr);
    let py = python.display();
    if why.contains("No module named 'spacy'") {
        format!("spaCy is not installed for {py}: run `{py} -m pip install spacy`.")
    } else if why.contains("[E050]") || why.contains("[E049]") {
        format!("spaCy has no pipeline called {pipeline} here: run `{py} -m spacy download {pipeline}`, or type the name of one that is installed.")
    } else {
        format!("spaCy failed: {}", why.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no message").trim())
    }
}

/// Run Python with a script and one argument, `input` on stdin.
fn run_python(python: &Path, script: &str, arg: &str, input: String) -> Result<std::process::Output, String> {
    let mut child = Command::new(python)
        .arg("-c")
        .arg(script)
        .arg(arg)
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Python could not start ({}): {e}", python.display()))?;
    let mut stdin = child.stdin.take().ok_or("Python took no input")?;
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = child.wait_with_output().map_err(|e| format!("Python stopped: {e}"))?;
    writer.join().map_err(|_| "Python's input could not be written".to_string())?.map_err(|e| format!("Python's input could not be written: {e}"))?;
    Ok(out)
}

/// Check and keep this computer's spaCy choice: the pipeline is loaded once,
/// so a missing spaCy or pipeline is said now rather than at the first run.
/// An empty pipeline clears it.
pub fn choose_spacy(app_data: &Path, python: Option<String>, pipeline: Option<String>) -> Result<SpacySettings, String> {
    let mut s = SpacySettings { python: python.filter(|p| !p.trim().is_empty()), pipeline: pipeline.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()), about: None };
    if let Some(p) = &s.pipeline {
        if !valid_pipeline(p) {
            return Err(format!("«{p}» is not a spaCy pipeline name (such as en_core_web_sm) or a folder."));
        }
        let py = python_of(&s)?;
        let out = run_python(&py, SPACY_CHECK, p, String::new())?;
        if !out.status.success() {
            return Err(python_failure(&py, p, &out.stderr));
        }
        s.about = Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|a| !a.is_empty());
    }
    std::fs::create_dir_all(app_data).map_err(|e| e.to_string())?;
    std::fs::write(spacy_path(app_data), serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Run spaCy on one sentence a line: the CoNLL-U and the pipeline it used.
pub fn run_spacy(sp: &Spacy, lines: &[String]) -> Result<(String, String), String> {
    let out = run_python(&sp.python, SPACY_SCRIPT, &sp.pipeline, lines.join("\n") + "\n")?;
    if !out.status.success() {
        return Err(python_failure(&sp.python, &sp.pipeline, &out.stderr));
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let used = stderr.lines().rev().find_map(|l| l.strip_prefix("wugs-pipeline: ")).map(str::trim).filter(|u| !u.is_empty()).unwrap_or(&sp.pipeline).to_string();
    Ok((String::from_utf8(out.stdout).map_err(|_| "spaCy wrote something that is not UTF-8".to_string())?, used))
}

// ---------------------------------------------------------------- the run

/// Where a run's sentences go.
pub enum Tagger {
    Local(Local),
    Lindat { endpoint: String, model: String },
    Spacy(Spacy),
}

/// One text's sentences, gathered on the session thread.
#[derive(Debug)]
pub struct Gathered {
    pub text: String,
    pub root: PathBuf,
    /// `(segment, its line)`, in order.
    pub segments: Vec<(String, String)>,
    /// Segments with an analysis already, not sent.
    pub kept: usize,
}

/// The text's sentences on transcription tracks that have no analysis yet,
/// one line each. For the web service, refused unless the project allows
/// it and the text is open.
pub fn gather(s: &mut Session, text_id: &str, web: bool) -> Result<Gathered, String> {
    let root = session::project_root(s)?;
    let p = session::project(s)?;
    if p.proj.entity_kind(text_id)?.as_deref() != Some("text") || p.proj.is_tombstoned(text_id)? {
        return Err("That text is not in this project.".into());
    }
    if web {
        if p.setting(WEB_OK)?.as_deref() != Some("yes") {
            return Err("This project has not allowed sending its texts to the UDPipe web service. Allow it first, or use UDPipe on this computer.".into());
        }
        if p.text_access(text_id)? != "open" {
            return Err("This text is not open, so it is never sent to a web service. Use UDPipe on this computer instead.".into());
        }
    }
    let (_, transcription) = crate::concordance::corpus_tiers(p)?;
    let mut segments = Vec::new();
    let mut kept = 0;
    for seg in p.proj.ordered_children("text", text_id, "segment")? {
        let tier = p.proj.field_str(&seg, "tier")?.unwrap_or_default();
        if !transcription.contains(&tier) || p.proj.is_tombstoned(&seg)? {
            continue;
        }
        let line: String = p.proj.field_str(&seg, "content")?.unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            continue;
        }
        if p.proj.field(&seg, wugs_conllu::F_ANALYSIS)?.is_some() {
            kept += 1;
            continue;
        }
        segments.push((seg, line));
    }
    Ok(Gathered { text: text_id.to_string(), root, segments, kept })
}

/// Run the tagger over the gathered lines, in batches: the CoNLL-U
/// sentences, one per line, and the provenance to record.
pub fn tag(tagger: &Tagger, lines: &[String]) -> Result<(Vec<wugs_conllu::format::Sentence>, String), String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut provenance = String::new();
    for chunk in lines.chunks(BATCH) {
        let (conllu, used) = match tagger {
            Tagger::Local(l) => (run_local(l, chunk)?, format!("udpipe {}", l.model.file_name().and_then(|n| n.to_str()).unwrap_or("model"))),
            Tagger::Lindat { endpoint, model } => {
                let (c, m) = run_lindat(endpoint, model, chunk)?;
                (c, format!("udpipe-lindat {m}"))
            }
            Tagger::Spacy(sp) => {
                let (c, m) = run_spacy(sp, chunk)?;
                (c, format!("spacy {m}"))
            }
        };
        let sentences = wugs_conllu::format::parse(&conllu).map_err(|p| format!("The tagger's CoNLL-U could not be read: {}", p.first().map(ToString::to_string).unwrap_or_default()))?;
        if sentences.len() != chunk.len() {
            return Err(format!("The tagger gave back {} sentences for {} lines, so nothing was written.", sentences.len(), chunk.len()));
        }
        out.extend(sentences);
        provenance = used;
    }
    Ok((out, provenance))
}

/// What a run did, for the panel.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tagged {
    pub annotated: usize,
    pub tokenized: usize,
    pub kept: usize,
    pub differ: usize,
    pub words: usize,
    /// The tagger and model that wrote them.
    pub by: String,
}

/// Write the tagger's sentences on their segments (session thread). The
/// project must still be the one gathered from.
pub fn write(s: &mut Session, g: &Gathered, sentences: Vec<wugs_conllu::format::Sentence>, by: &str) -> Result<Tagged, String> {
    session::require_root(s, &g.root)?;
    let p = session::project(s)?;
    let items: Vec<(String, wugs_conllu::format::Sentence)> = g.segments.iter().map(|(seg, _)| seg.clone()).zip(sentences).collect();
    let r = wugs_conllu::annotate::annotate(p, &g.text, &items, by)?;
    Ok(Tagged { annotated: r.annotated, tokenized: r.tokenized, kept: r.kept + g.kept, differ: r.differ, words: r.words, by: by.to_string() })
}

/// The web service's model for this project: what the person typed, else
/// the project's ISO 639-3 code, else its language's name.
pub fn web_model(s: &mut Session, typed: &str) -> Result<String, String> {
    if !typed.trim().is_empty() {
        return Ok(typed.trim().to_string());
    }
    let l = session::project(s)?.project_language()?;
    if !l.iso.is_empty() {
        Ok(l.iso)
    } else if !l.name.is_empty() {
        Ok(l.name.to_lowercase())
    } else {
        Err("Say which language (a UDPipe model name, or a language name or code), or set the project's language in the Texts panel.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::net::TcpListener;

    const TAGGED: &str = "1\twʌn\twʌn\tNUM\t_\t_\t0\troot\t_\t_\n\n";

    /// A stand-in for the service: answers one POST with a JSON reply naming a model.
    fn fake_service(result: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/process", listener.local_addr().unwrap().port());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).unwrap();
            let json = serde_json::json!({ "model": "english-ewt-ud-2.15-241121", "acknowledgements": [], "result": result }).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}", json.len()).unwrap();
            String::from_utf8(body).unwrap()
        });
        (url, handle)
    }

    #[test]
    fn the_web_service_is_asked_presegmented_and_only_when_allowed() {
        let dir = std::env::temp_dir().join(format!("wugs-ms-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let text = session::list_texts(&mut s).unwrap()[0].id.clone();
        assert!(gather(&mut s, &text, true).unwrap_err().contains("not allowed"), "a project must allow it first");
        session::project(&mut s).unwrap().set_setting(WEB_OK, "yes").unwrap();
        let g = gather(&mut s, &text, true).unwrap();
        assert_eq!(g.segments.len(), 2, "the two transcribed utterances, not the empty one or translations: {:?}", g.segments);
        let lines: Vec<String> = g.segments.iter().map(|(_, l)| l.clone()).collect();
        let (url, server) = fake_service("1\twʌn\twʌn\tNUM\t_\t_\t0\troot\t_\t_\n2\ttuː\ttuː\tNUM\t_\t_\t1\tflat\t_\t_\n3\tθɹiː\tθɹiː\tNUM\t_\t_\t1\tflat\t_\t_\n\n1\tfɔɹ\tfɔɹ\tNUM\t_\t_\t0\troot\t_\t_\n2\tfaɪv\tfaɪv\tNUM\t_\t_\t1\tflat\t_\t_\n\n");
        let (sentences, by) = tag(&Tagger::Lindat { endpoint: url, model: "en".into() }, &lines).unwrap();
        let sent = server.join().unwrap();
        assert!(sent.contains("tokenizer=presegmented") && sent.contains("model=en") && sent.contains("tagger=") && sent.contains("parser="), "{sent}");
        assert_eq!(by, "udpipe-lindat english-ewt-ud-2.15-241121");
        let r = write(&mut s, &g, sentences, &by).unwrap();
        // The tutorial's words are tokenized and glossed already: they keep their tokens.
        assert_eq!((r.annotated, r.differ, r.words), (2, 0, 5), "{r:?}");
        let again = gather(&mut s, &text, true).unwrap();
        assert!(again.segments.is_empty() && again.kept == 2, "a tagged sentence is not sent again");
        // A restricted text is never sent.
        session::project(&mut s).unwrap().set_access(&text, "restricted").unwrap();
        assert!(gather(&mut s, &text, true).unwrap_err().contains("not open"));
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_count_that_does_not_match_writes_nothing() {
        let (url, server) = fake_service(TAGGED);
        let lines = vec!["wʌn".to_string(), "tuː".to_string()];
        let e = tag(&Tagger::Lindat { endpoint: url, model: "en".into() }, &lines).unwrap_err();
        server.join().unwrap();
        assert!(e.contains("1 sentences for 2 lines"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn udpipe_on_this_computer_reads_one_sentence_a_line() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("wugs-udpipe-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        // A stand-in for udpipe: checks its options, then tags each input line as one word.
        let program = dir.join("udpipe");
        std::fs::write(
            &program,
            "#!/bin/sh\n[ \"$1\" = --tokenize ] && [ \"$2\" = --tokenizer=presegmented ] && [ \"$3\" = --tag ] && [ \"$4\" = --parse ] || exit 3\n\
             while IFS= read -r line; do printf '1\\t%s\\t%s\\tNOUN\\t_\\t_\\t0\\troot\\t_\\t_\\n\\n' \"$line\" \"$line\"; done\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let model = dir.join("test.udpipe");
        std::fs::write(&model, b"not a real model").unwrap();
        set_local_settings(&dir, &LocalSettings { program: Some(program.display().to_string()), model: Some(model.display().to_string()) }).unwrap();
        let l = local(&dir).unwrap();
        let (sentences, by) = tag(&Tagger::Local(l), &["kesi".to_string(), "bara".to_string()]).unwrap();
        assert_eq!(sentences.len(), 2);
        assert_eq!(by, "udpipe test.udpipe");
        set_local_settings(&dir, &LocalSettings { program: Some(program.display().to_string()), model: None }).unwrap();
        assert!(local(&dir).unwrap_err().contains("needs a model"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn pipeline_names_are_package_names_or_folders() {
        assert!(valid_pipeline("en_core_web_sm"));
        assert!(valid_pipeline("de_dep_news_trf-3.8.0"));
        assert!(!valid_pipeline(""));
        assert!(!valid_pipeline("en core"));
        assert!(!valid_pipeline("x; rm -rf /"));
        assert!(valid_pipeline(&std::env::temp_dir().display().to_string()), "a folder spaCy saved a pipeline in");
    }

    /// spaCy itself, when this computer has it with the small English
    /// pipeline (`pip install spacy` and `python -m spacy download
    /// en_core_web_sm`); skipped otherwise, like the CLDF validator test.
    #[test]
    fn spacy_tags_one_sentence_a_line() {
        let Some(python) = python_on_path() else { return eprintln!("no Python here: skipped") };
        let ok = Command::new(&python).args(["-c", "import spacy; spacy.load('en_core_web_sm')"]).output().map(|o| o.status.success()).unwrap_or(false);
        if !ok {
            return eprintln!("spaCy with en_core_web_sm is not installed here: skipped");
        }
        let dir = std::env::temp_dir().join(format!("wugs-spacy-{}", wugs_model::ulid()));
        let chosen = choose_spacy(&dir, None, Some("en_core_web_sm".into())).unwrap();
        assert!(chosen.about.as_deref().unwrap_or_default().contains("en_core_web_sm"), "{chosen:?}");
        let missing = choose_spacy(&dir, None, Some("xx_no_such_pipeline".into())).unwrap_err();
        assert!(missing.contains("has no pipeline called xx_no_such_pipeline"), "{missing}");
        assert_eq!(spacy_settings(&dir).pipeline.as_deref(), Some("en_core_web_sm"), "a failed check keeps the last good choice");
        let sp = spacy(&dir).unwrap();
        // Two sentences on one line stay one sentence; extra spaces make no token.
        let lines = ["The dogs saw me. Then they ran.".to_string(), "I  like it, don't you?".to_string()];
        let (sentences, by) = tag(&Tagger::Spacy(sp), &lines).unwrap();
        assert!(by.starts_with("spacy en_core_web_sm "), "{by}");
        assert_eq!(sentences.len(), 2);
        let first = wugs_conllu::format::write(&sentences[..1]);
        assert_eq!(first.lines().filter(|l| l.split('\t').nth(6) == Some("0")).count(), 1, "one root: {first}");
        assert!(first.contains("\tdog\tNOUN\t"), "{first}");
        assert!(first.contains("SpaceAfter=No"), "{first}");
        std::fs::remove_dir_all(dir).ok();
    }
}
