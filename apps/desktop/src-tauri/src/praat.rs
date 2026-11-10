//! Praat handoff (roadmap 46): the person's own Praat, found on this
//! machine, opens a text's TextGrid over its recording, and what they change
//! there comes back as ops after they look at the list of changes.
//!
//! Praat cannot be embedded, but it takes scripts from outside: `praat
//! --send script` runs a script in the Praat already open (starting one if
//! none is). Opening sends a script that loads the recording as a LongSound
//! and the grid, opens them together and zooms to the selection. Bringing
//! back sends a script that saves that TextGrid object over our working
//! file; the engine (`wugs_legacy::praat`) compares it with the baseline and
//! the person applies the result.
//!
//! Everything for one text lives in `<project>/cache/praat/<text id>/`: the
//! audio (a hard link to the blob, so a long recording is not copied), the
//! working grid Praat saves into, `handoff.json` (the baseline grid it is
//! compared with, the manifest naming the segment behind each item, and the
//! object name Praat knows it by, replaced together in one rename), and the
//! two scripts. Only cache: discarding it loses nothing that was applied.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wugs_legacy::praat::{apply_return, hand_out, plan_return, Plan};

use crate::session::{self, Session};

/// `--send` runs scripts in a running Praat, which came with Praat 6.
const MIN_MAJOR: u32 = 6;
/// How long to wait for Praat to save after asking it to.
pub const SAVE_WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- finding

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The Praat that will be used, if one was found.
    pub path: Option<String>,
    pub version: Option<String>,
    /// Set when the person chose the path (not found automatically).
    pub chosen: bool,
    /// Why there is no usable Praat, in words.
    pub problem: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    path: Option<String>,
}

fn settings_file(app_data: &Path) -> PathBuf {
    app_data.join("praat.json")
}

fn load_settings(app_data: &Path) -> Settings {
    fs::read(settings_file(app_data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Where Praat usually is, after PATH.
fn usual_places() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(windows) {
        // Praat for Windows is one Praat.exe, often left where it was
        // unzipped.
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
                out.push(base.join("Praat").join("Praat.exe"));
                out.push(base.join("Praat.exe"));
                out.push(base.join("Programs").join("Praat").join("Praat.exe"));
            }
        }
        if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            for sub in ["Desktop", "Downloads", "Documents"] {
                out.push(home.join(sub).join("Praat.exe"));
                out.push(home.join(sub).join("Praat").join("Praat.exe"));
            }
        }
    } else if cfg!(target_os = "macos") {
        out.push(PathBuf::from("/Applications/Praat.app/Contents/MacOS/Praat"));
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            out.push(home.join("Applications/Praat.app/Contents/MacOS/Praat"));
        }
    } else {
        for p in ["/usr/bin/praat", "/usr/local/bin/praat", "/snap/bin/praat"] {
            out.push(PathBuf::from(p));
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            out.push(home.join("bin/praat"));
            out.push(home.join(".local/bin/praat"));
        }
    }
    out
}

fn on_path() -> Vec<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["Praat.exe", "praat.exe"] } else { &["praat", "Praat"] };
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).flat_map(|d| names.iter().map(move |n| d.join(n))).collect())
        .unwrap_or_default()
}

/// The Praat to use: the chosen one when set (even if it no longer works,
/// so the problem is reported, not silently replaced), else the first found.
pub fn status(app_data: &Path) -> Status {
    if let Some(chosen) = load_settings(app_data).path {
        return match version_of(Path::new(&chosen)) {
            Ok(v) => Status { path: Some(chosen), version: Some(v), chosen: true, problem: None },
            Err(e) => Status { path: Some(chosen), version: None, chosen: true, problem: Some(e) },
        };
    }
    let mut last_problem = None;
    for candidate in on_path().into_iter().chain(usual_places()) {
        if !candidate.is_file() {
            continue;
        }
        match version_of(&candidate) {
            Ok(v) => return Status { path: Some(candidate.to_string_lossy().into_owned()), version: Some(v), chosen: false, problem: None },
            Err(e) => last_problem = Some(e),
        }
    }
    Status {
        path: None,
        version: None,
        chosen: false,
        problem: Some(last_problem.unwrap_or_else(|| "Praat was not found on this computer. Install it from praat.org, or choose where it is.".into())),
    }
}

/// Use this Praat from now on (`None` goes back to finding it). Refused when
/// the file does not run as Praat 6 or newer.
pub fn set_path(app_data: &Path, path: Option<&str>) -> Result<Status, String> {
    if let Some(p) = path {
        version_of(Path::new(p))?;
    }
    let body = serde_json::to_vec_pretty(&Settings { path: path.map(str::to_string) }).map_err(|e| e.to_string())?;
    fs::create_dir_all(app_data).map_err(|e| e.to_string())?;
    let tmp = settings_file(app_data).with_extension("json.tmp");
    fs::write(&tmp, body).map_err(|e| e.to_string())?;
    fs::rename(&tmp, settings_file(app_data)).map_err(|e| e.to_string())?;
    Ok(status(app_data))
}

/// "Praat 6.4.06 (February 25 2024)" from `praat --version`, checked to be
/// new enough. Praat on Windows prints UTF-16 by default.
pub fn version_of(path: &Path) -> Result<String, String> {
    let out = run_quietly(Command::new(path).arg("--version"), Duration::from_secs(10)).map_err(|e| format!("{} does not run: {e}", path.display()))?;
    parse_version(&out)
}

pub fn parse_version(bytes: &[u8]) -> Result<String, String> {
    let text = if bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count() * 2 >= bytes.len() / 2 {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    let line = text.trim_start_matches('\u{feff}').lines().next().unwrap_or("").trim().to_string();
    let Some(rest) = line.strip_prefix("Praat ") else {
        return Err("this program did not answer as Praat".into());
    };
    let major: u32 = rest.split('.').next().and_then(|m| m.trim().parse().ok()).ok_or("this program did not answer as Praat")?;
    if major < MIN_MAJOR {
        return Err(format!("{line} is too old; wugs needs Praat {MIN_MAJOR} or newer."));
    }
    Ok(line)
}

/// Run to completion with a deadline, returning stdout. The child is killed
/// at the deadline.
fn run_quietly(cmd: &mut Command, deadline: Duration) -> Result<Vec<u8>, String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).ok();
        buf
    });
    let started = std::time::Instant::now();
    loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            let out = reader.join().unwrap_or_default();
            return if st.success() { Ok(out) } else { Err(format!("it exited with {st}")) };
        }
        if started.elapsed() > deadline {
            child.kill().ok();
            child.wait().ok();
            return Err("it did not answer in time".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `praat --send script`: run it in the Praat already open, or a new one.
/// Praat stays open after; this returns once the script has been handed
/// over, and a thread reaps the launcher.
fn send(praat: &Path, script: &Path) -> Result<(), String> {
    let mut child = Command::new(praat)
        .arg("--send")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Praat did not start: {e}"))?;
    std::thread::spawn(move || {
        child.wait().ok();
    });
    Ok(())
}

/// The Praat to run, or why not.
pub fn praat(app_data: &Path) -> Result<PathBuf, String> {
    let st = status(app_data);
    match (st.path, st.problem) {
        (Some(p), None) => Ok(PathBuf::from(p)),
        (_, Some(problem)) => Err(problem),
        (None, None) => Err("Praat was not found on this computer.".into()),
    }
}

// ---------------------------------------------------------------- scripts

/// A Praat string literal: double quotes doubled.
pub fn praat_string(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn open_script(audio: &Path, grid: &Path, name: &str, from_s: f64, to_s: f64) -> String {
    format!(
        "# Written by wugs: open a text with its recording.\n\
         sound = Open long sound file: {audio}\n\
         Rename: {name}\n\
         duration = Get total duration\n\
         grid = Read from file: {grid}\n\
         Rename: {name}\n\
         # Cover the whole recording, not only up to the last annotation.\n\
         gridEnd = Get end time\n\
         if duration > gridEnd\n\
         \x20   Extend time: duration - gridEnd, \"End\"\n\
         endif\n\
         selectObject: sound, grid\n\
         View & Edit\n\
         editor: grid\n\
         \x20   Zoom: {from_s:.3}, {to_s:.3}\n\
         endeditor\n",
        audio = praat_string(&audio.to_string_lossy()),
        grid = praat_string(&grid.to_string_lossy()),
        name = praat_string(name),
    )
}

pub fn save_script(grid: &Path, name: &str) -> String {
    format!(
        "# Written by wugs: save the TextGrid back for wugs to read.\n\
         selectObject: {object}\n\
         Save as text file: {grid}\n",
        object = praat_string(&format!("TextGrid {name}")),
        grid = praat_string(&grid.to_string_lossy()),
    )
}

// ---------------------------------------------------------------- handoff

struct Files {
    dir: PathBuf,
}

/// The state of one handoff, always written whole.
#[derive(Serialize, Deserialize)]
struct State {
    name: String,
    baseline: String,
    manifest: String,
}

impl Files {
    fn of(root: &Path, text: &str) -> Result<Files, String> {
        // Text ids are ULID-based (`txt_…`); anything else must not name a
        // folder.
        if text.is_empty() || !text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("not a text id: {text:?}"));
        }
        Ok(Files { dir: root.join("cache").join("praat").join(text) })
    }
    fn grid(&self) -> PathBuf {
        self.dir.join("grid.TextGrid")
    }
    fn state_file(&self) -> PathBuf {
        self.dir.join("handoff.json")
    }
    fn exists(&self) -> bool {
        self.state_file().is_file()
    }
    fn state(&self) -> Result<State, String> {
        let bytes = fs::read(self.state_file()).map_err(|_| "Nothing from this text is open in Praat. Use Open in Praat first.".to_string())?;
        serde_json::from_slice(&bytes).map_err(|_| "This text's Praat handoff is damaged; discard it and open the text in Praat again.".to_string())
    }
    fn write_state(&self, st: &State) -> Result<(), String> {
        write_atomic(&self.state_file(), &serde_json::to_vec(st).map_err(|e| e.to_string())?)
    }
}

fn write_atomic(path: &Path, body: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, body).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// The recording as a `.wav` beside the grid: a hard link to the blob when
/// the filesystem allows, else a symbolic link (Unix), else the blob itself
/// (Praat reads WAV by its header, not its name).
fn audio_for_praat(blob: &Path, dir: &Path, hex: &str) -> PathBuf {
    let link = dir.join(format!("{hex}.wav"));
    if link.is_file() {
        return link;
    }
    if fs::hard_link(blob, &link).is_ok() {
        return link;
    }
    #[cfg(unix)]
    if std::os::unix::fs::symlink(blob, &link).is_ok() {
        return link;
    }
    blob.to_path_buf()
}

/// What opening wrote.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// The script to send to Praat.
    pub script: String,
    /// Segments the grid cannot hold (overlaps on one tier, untimed), so
    /// Praat cannot show or change them.
    pub not_shown: usize,
}

/// Write a text's handoff and the script that opens it in Praat, zoomed to
/// `from_ms`–`to_ms`. A handoff already waiting for this text is replaced:
/// what was not brought back from it is given up, which the panel warns
/// about before calling this.
pub fn prepare(session: &mut Session, text: &str, source: &str, from_ms: f64, to_ms: f64) -> Result<Opened, String> {
    let p = session::project(session)?;
    let files = Files::of(&p.root, text)?;
    let blob = session::audio_blob(p, source)?;
    let hex = wugs_blob::parse_ref(&blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?.to_string();
    let wav = p.root.join("blobs").join(&hex);
    if !wav.is_file() {
        return Err("The recording's audio file is missing from this project folder.".into());
    }
    let out = hand_out(p, text)?;
    if !out.textgrid.contains("tiers? <exists>") {
        return Err("This text has nothing on a timed tier yet. Add a segment on the timeline first, then open it in Praat.".into());
    }
    fs::create_dir_all(&files.dir).map_err(|e| e.to_string())?;
    let name = format!("wugs_{}", wugs_model::ulid().to_lowercase().replace('_', ""));
    write_atomic(&files.grid(), out.textgrid.as_bytes())?;
    files.write_state(&State { name: name.clone(), baseline: out.textgrid, manifest: out.manifest })?;
    let audio = audio_for_praat(&wav, &files.dir, &hex);
    let (from_s, to_s) = (from_ms.max(0.0) / 1000.0, to_ms.max(from_ms + 1.0) / 1000.0);
    let script = files.dir.join("open.praat");
    write_atomic(&script, open_script(&audio, &files.grid(), &name, from_s, to_s).as_bytes())?;
    Ok(Opened { script: script.to_string_lossy().into_owned(), not_shown: out.not_exported.total() })
}

pub fn open_in_praat(praat: &Path, opened: &Opened) -> Result<(), String> {
    send(praat, Path::new(&opened.script))
}

/// Whether a text has a handoff waiting to be brought back.
pub fn pending(session: &mut Session, text: &str) -> Result<bool, String> {
    let p = session::project(session)?;
    Ok(Files::of(&p.root, text)?.exists())
}

/// The working grid's path and the save script for a pending handoff.
pub fn save_request(session: &mut Session, text: &str) -> Result<(PathBuf, PathBuf), String> {
    let p = session::project(session)?;
    let files = Files::of(&p.root, text)?;
    let st = files.state()?;
    let script = files.dir.join("save.praat");
    write_atomic(&script, save_script(&files.grid(), &st.name).as_bytes())?;
    Ok((files.grid(), script))
}

/// Ask Praat to save, then wait for the file. The working grid is moved
/// aside first, so a save is seen by the file appearing (modification times
/// are 2 s coarse on FAT sticks, and an unchanged grid saves the same
/// bytes), and it is read until two reads agree, so a half-written file is
/// never taken. If Praat never saves, the old grid is put back.
pub fn ask_praat_to_save(praat: &Path, grid: &Path, script: &Path) -> Result<(), String> {
    let aside = grid.with_extension("previous");
    if grid.is_file() {
        fs::rename(grid, &aside).map_err(|e| format!("could not set the old TextGrid aside: {e}"))?;
    }
    let restore = || {
        if aside.is_file() && !grid.is_file() {
            fs::rename(&aside, grid).ok();
        }
    };
    if let Err(e) = send(praat, script) {
        restore();
        return Err(e);
    }
    let started = std::time::Instant::now();
    let mut last: Option<Vec<u8>> = None;
    while started.elapsed() < SAVE_WAIT {
        std::thread::sleep(Duration::from_millis(150));
        let now = fs::read(grid).ok().filter(|b| !b.is_empty());
        if now.is_some() && now == last {
            fs::remove_file(&aside).ok();
            return Ok(());
        }
        last = now;
    }
    restore();
    Err("Praat did not save the TextGrid. If you closed it in Praat, its changes are not in wugs; if it is still open, try again.".into())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnView {
    pub retimed: usize,
    pub relabelled: usize,
    pub added: usize,
    pub removed: usize,
    /// What Praat changed that is not brought back, in words, with counts.
    pub not_brought_back: Vec<(String, usize)>,
}

fn plan_for(session: &mut Session, text: &str) -> Result<(Files, State, Plan), String> {
    let p = session::project(session)?;
    let files = Files::of(&p.root, text)?;
    let st = files.state()?;
    let edited = fs::read(files.grid()).map_err(|e| format!("could not read Praat's TextGrid: {e}"))?;
    let plan = plan_return(p, text, &st.baseline, &st.manifest, &edited)?;
    Ok((files, st, plan))
}

/// What Praat's saved grid would change, for the person to look at.
pub fn review(session: &mut Session, text: &str) -> Result<ReturnView, String> {
    let (_, _, plan) = plan_for(session, text)?;
    let c = plan.counts();
    Ok(ReturnView {
        retimed: c.retimed,
        relabelled: c.relabelled,
        added: c.added,
        removed: c.removed,
        not_brought_back: plan.skipped.0.into_iter().collect(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub changes: usize,
    pub findings: Vec<String>,
}

/// Plan again from the files as they are now (so what is written is what
/// the grid says at this moment) and apply it as one txn. The grid Praat
/// saved becomes the baseline, so the window can stay open in Praat and be
/// brought back again later without repeating anything.
pub fn apply(session: &mut Session, text: &str) -> Result<Applied, String> {
    let (files, st, plan) = plan_for(session, text)?;
    let p = session::project(session)?;
    let applied = apply_return(p, text, &plan)?;
    let (baseline, manifest) = plan.rebased(&applied)?;
    // A crash after the commit and before this write leaves the old
    // baseline. The next return then rewrites the same retimes and labels
    // (same values) and skips adds already in wugs, unless Praat moved one
    // of those again meanwhile, which is added a second time.
    files.write_state(&State { name: st.name, baseline, manifest })?;
    Ok(Applied {
        changes: plan.changes.len(),
        findings: applied.commit.as_ref().map(session::finding_lines).unwrap_or_default(),
    })
}

/// Forget a text's handoff. Changes already applied stay; the rest are
/// given up. The state file goes first, so the handoff is over even when
/// Praat still holds the audio open (Windows refuses to delete it); the
/// leftovers are cache, cleared on the next try or the next open.
pub fn discard(session: &mut Session, text: &str) -> Result<(), String> {
    let p = session::project(session)?;
    let files = Files::of(&p.root, text)?;
    match fs::remove_file(files.state_file()) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    fs::remove_dir_all(&files.dir).ok();
    Ok(())
}
