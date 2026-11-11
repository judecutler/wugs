pub mod assist;
pub mod capability;
pub mod capture;
pub mod conflicts;
pub mod corrections;
pub mod elicit;
pub mod gloss;
pub mod identity;
pub mod kits;
pub mod layout;
pub mod lexicon;
pub mod listen;
pub mod praat;
pub mod praat_analysis;
pub mod session;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use capture::Captures;
use gloss::{ApplyAllResult, GlossEdit, GlossView, LexiconMatchView, MorphSpecView};
use identity::{IdentityInfo, KeychainKeyStore};
use serde::Serialize;
use listen::Listening;
use session::{AddedSegment, DeviceRow, DevicesView, EditResult, JoinResult, PairOffer, PeaksInfo, PeaksSlice, ProjectInfo, SearchRow, SearchScope, Session, SourceRow, SyncResult, TextDetail, TextRow, TierRow};
use tauri::{AppHandle, Emitter, Manager, State};
use wugs_auth::DeviceKey;
use wugs_net::discover::{self, Announcement};

type Job = Box<dyn FnOnce(&mut Session) + Send>;

/// `Project` is not `Send` (its validator and text-merger trait objects carry
/// no `Send` bound), so it cannot live in tauri's managed state. One thread
/// owns the session for the app's lifetime and runs commands on it in order.
#[derive(Clone)]
pub struct SessionThread(mpsc::Sender<Job>);

impl SessionThread {
    pub fn spawn() -> SessionThread {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("wugs-session".into())
            .spawn(move || {
                let mut session: Session = None;
                for job in rx {
                    // A panic may leave a savepoint open; closing the project
                    // rolls it back, and the log is untouched by design.
                    if catch_unwind(AssertUnwindSafe(|| job(&mut session))).is_err() {
                        session = None;
                    }
                }
            })
            .expect("spawn the session thread");
        SessionThread(tx)
    }

    pub fn run<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Session) -> Result<R, String> + Send + 'static,
    ) -> Result<R, String> {
        let (reply, result) = mpsc::sync_channel(1);
        self.0
            .send(Box::new(move |s| {
                let _ = reply.send(f(s));
            }))
            .map_err(|_| "the session thread has stopped".to_string())?;
        result
            .recv()
            .map_err(|_| "the command failed unexpectedly and the project was closed".to_string())?
    }
}

/// Let the webview's <audio> element read this project's media over the
/// asset protocol. Only `blobs/` is scoped, never the folder above it, and
/// a scope failure only costs playback, so it is ignored rather than failing
/// the open. Tauri cannot revoke a scope entry: projects opened earlier in
/// the run stay readable until restart.
fn allow_media(app: &AppHandle, project_path: &str) {
    app.asset_protocol_scope()
        .allow_directory(PathBuf::from(project_path).join("blobs"), false)
        .ok();
}

#[tauri::command(async)]
fn open_project(app: AppHandle, s: State<SessionThread>, reach: State<Reachable>, path: String, researcher: String) -> Result<ProjectInfo, String> {
    let p = path.clone();
    let info = s.run(move |s| session::open_project(s, &p, &researcher))?;
    allow_media(&app, &path);
    reach.follow(&s)?;
    Ok(info)
}

#[tauri::command(async)]
fn create_project(app: AppHandle, s: State<SessionThread>, reach: State<Reachable>, path: String, researcher: String) -> Result<ProjectInfo, String> {
    let p = path.clone();
    let info = s.run(move |s| session::create_project(s, &p, &researcher))?;
    allow_media(&app, &path);
    reach.follow(&s)?;
    Ok(info)
}

#[tauri::command(async)]
fn list_texts(s: State<SessionThread>) -> Result<Vec<TextRow>, String> {
    s.run(session::list_texts)
}

#[tauri::command(async)]
fn get_text(s: State<SessionThread>, text_id: String) -> Result<TextDetail, String> {
    s.run(move |s| session::get_text(s, &text_id))
}

#[tauri::command(async)]
fn create_text(s: State<SessionThread>, title: String) -> Result<String, String> {
    s.run(move |s| session::create_text(s, &title))
}

#[tauri::command(async)]
fn add_segment(
    s: State<SessionThread>,
    text_id: String,
    tier_id: Option<String>,
    start_ms: Option<f64>,
    end_ms: Option<f64>,
    content: String,
) -> Result<AddedSegment, String> {
    s.run(move |s| session::add_segment(s, &text_id, tier_id.as_deref(), start_ms, end_ms, &content))
}

#[tauri::command(async)]
fn set_segment_times(
    s: State<SessionThread>,
    text_id: String,
    segment_id: String,
    start_ms: Option<f64>,
    end_ms: Option<f64>,
) -> Result<Vec<String>, String> {
    s.run(move |s| session::set_segment_times(s, &text_id, &segment_id, start_ms, end_ms))
}

#[tauri::command(async)]
fn delete_segment(s: State<SessionThread>, text_id: String, segment_id: String) -> Result<Vec<String>, String> {
    s.run(move |s| session::delete_segment(s, &text_id, &segment_id))
}

#[tauri::command(async)]
fn add_tier(s: State<SessionThread>, template: String, name: Option<String>) -> Result<TierRow, String> {
    s.run(move |s| session::add_tier(s, &template, name.as_deref()))
}

#[tauri::command(async)]
fn import_media(s: State<SessionThread>, path: String) -> Result<SourceRow, String> {
    // The open project's blobs/ is already in the asset scope: every way a
    // project opens (open, create, join) passes through allow_media.
    s.run(move |s| session::import_media(s, &path))
}

#[tauri::command(async)]
fn capture_begin(s: State<SessionThread>, captures: State<Captures>, sample_rate: u32, channels: u16) -> Result<String, String> {
    let root = s.run(session::project_root)?;
    captures.begin(&root, sample_rate, channels)
}

/// The body is the raw PCM16 bytes (no JSON or base64 detour); the capture
/// id rides in a header.
#[tauri::command(async)]
fn capture_chunk(captures: State<Captures>, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(pcm) = request.body() else {
        return Err("capture_chunk expects a binary body".into());
    };
    let id = request
        .headers()
        .get("x-capture-id")
        .and_then(|v| v.to_str().ok())
        .ok_or("capture_chunk is missing the x-capture-id header")?;
    captures.chunk(id, pcm)
}

/// Safe to call again after a failed import: the capture stays tracked,
/// finished, until an import succeeds or the user discards it.
#[tauri::command(async)]
fn capture_finish(s: State<SessionThread>, captures: State<Captures>, capture_id: String, label: String) -> Result<SourceRow, String> {
    let done = captures.finish(&capture_id)?;
    let row = s.run(move |s| session::import_capture(s, done, &label))?;
    captures.forget(&capture_id)?;
    Ok(row)
}

/// The only way to delete a take, called after the user confirmed.
#[tauri::command]
fn capture_discard(captures: State<Captures>, capture_id: String) -> Result<(), String> {
    captures.discard(&capture_id)
}

/// "Keep for later": the take leaves this window but stays in `recordings-pending/`.
#[tauri::command]
fn capture_keep(captures: State<Captures>, capture_id: String) -> Result<(), String> {
    captures.release(&capture_id)
}

/// A fresh page owns no captures: close any the old page left, keeping the files.
#[tauri::command]
fn capture_reset(captures: State<Captures>) -> Result<usize, String> {
    captures.release_all()
}

#[tauri::command(async)]
fn capture_recovered_list(s: State<SessionThread>, captures: State<Captures>) -> Result<Vec<capture::Pending>, String> {
    let root = s.run(session::project_root)?;
    captures.recover_list(&root)
}

#[tauri::command(async)]
fn capture_recovered_import(s: State<SessionThread>, captures: State<Captures>, id: String, label: String) -> Result<SourceRow, String> {
    let root = s.run(session::project_root)?;
    let done = captures.recover_take(&root, &id)?;
    s.run(move |s| session::import_capture(s, done, &label))
}

#[tauri::command(async)]
fn capture_recovered_discard(s: State<SessionThread>, captures: State<Captures>, id: String) -> Result<(), String> {
    let root = s.run(session::project_root)?;
    captures.discard_pending(&root, &id)
}

#[tauri::command(async)]
fn peaks_info(s: State<SessionThread>, source_id: String) -> Result<PeaksInfo, String> {
    s.run(move |s| session::peaks_info(s, &source_id))
}

#[tauri::command(async)]
fn peaks_query(
    s: State<SessionThread>,
    source_id: String,
    channel: u16,
    start_frame: u64,
    end_frame: u64,
    pixels: usize,
) -> Result<PeaksSlice, String> {
    s.run(move |s| session::peaks_query(s, &source_id, channel, start_frame, end_frame, pixels))
}

#[tauri::command(async)]
fn edit_segment_content(
    s: State<SessionThread>,
    text_id: String,
    segment_id: String,
    content: String,
) -> Result<EditResult, String> {
    s.run(move |s| session::edit_segment_content(s, &text_id, &segment_id, &content))
}

#[tauri::command(async)]
fn gloss_get(s: State<SessionThread>, text_id: String, segment_id: String) -> Result<GlossView, String> {
    s.run(move |s| gloss::gloss_get(s, &text_id, &segment_id))
}

#[tauri::command(async)]
fn gloss_tokenize(s: State<SessionThread>, text_id: String, segment_id: String, forms: Vec<String>) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_tokenize(s, &text_id, &segment_id, &forms))
}

#[tauri::command(async)]
fn gloss_retokenize(s: State<SessionThread>, text_id: String, segment_id: String, forms: Vec<String>) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_retokenize(s, &text_id, &segment_id, &forms))
}

#[tauri::command(async)]
fn gloss_apply(s: State<SessionThread>, text_id: String, segment_id: String, token_id: String, morphs: Vec<MorphSpecView>) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_apply(s, &text_id, &segment_id, &token_id, morphs))
}

#[tauri::command(async)]
fn gloss_apply_all(s: State<SessionThread>, text_id: String, segment_id: String, form: String, morphs: Vec<MorphSpecView>) -> Result<ApplyAllResult, String> {
    s.run(move |s| gloss::gloss_apply_all(s, &text_id, &segment_id, &form, morphs))
}

#[tauri::command(async)]
fn gloss_set(s: State<SessionThread>, text_id: String, segment_id: String, morph_id: String, gloss: String, lang: Option<String>) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_set(s, &text_id, &segment_id, &morph_id, &gloss, lang.as_deref()))
}

#[tauri::command(async)]
fn gloss_pick_sense(s: State<SessionThread>, text_id: String, segment_id: String, morph_id: String, sense_id: String, gloss: String) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_pick_sense(s, &text_id, &segment_id, &morph_id, &sense_id, &gloss))
}

#[tauri::command(async)]
fn gloss_add_lexeme(s: State<SessionThread>, text_id: String, segment_id: String, morph_id: String, lemma: String, gloss: String) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_add_lexeme(s, &text_id, &segment_id, &morph_id, &lemma, &gloss))
}

#[tauri::command(async)]
fn gloss_restore(s: State<SessionThread>, text_id: String, segment_id: String, parent_id: String, member_id: String) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_restore(s, &text_id, &segment_id, &parent_id, &member_id))
}

#[tauri::command(async)]
fn gloss_choose_list(s: State<SessionThread>, text_id: String, segment_id: String, parent_id: String, members: Vec<String>) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_choose_list(s, &text_id, &segment_id, &parent_id, &members))
}

#[tauri::command(async)]
fn lexicon_list(s: State<SessionThread>, query: String) -> Result<Vec<lexicon::EntryRow>, String> {
    s.run(move |s| lexicon::lexicon_list(s, &query))
}

#[tauri::command(async)]
fn lexicon_entry(s: State<SessionThread>, id: String) -> Result<lexicon::EntryView, String> {
    s.run(move |s| lexicon::lexicon_entry(s, &id))
}

#[tauri::command(async)]
fn lexicon_add(s: State<SessionThread>, lemma: String, gloss: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_add(s, &lemma, &gloss))
}

#[tauri::command(async)]
fn lexicon_set_field(s: State<SessionThread>, id: String, field: String, value: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_set_field(s, &id, &field, &value))
}

#[tauri::command(async)]
fn lexicon_set_homograph(s: State<SessionThread>, id: String, number: Option<u32>) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_set_homograph(s, &id, number))
}

#[tauri::command(async)]
fn lexicon_number_homographs(s: State<SessionThread>, id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_number_homographs(s, &id))
}

#[tauri::command(async)]
fn lexicon_delete(s: State<SessionThread>, id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_delete(s, &id))
}

#[tauri::command(async)]
fn lexicon_add_sense(s: State<SessionThread>, id: String, gloss: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_add_sense(s, &id, &gloss))
}

#[tauri::command(async)]
fn lexicon_set_sense_gloss(s: State<SessionThread>, sense_id: String, gloss: String, lang: Option<String>) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_set_sense_gloss(s, &sense_id, &gloss, lang.as_deref()))
}

#[tauri::command(async)]
fn lexicon_set_sense_text(s: State<SessionThread>, sense_id: String, field: String, text: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_set_sense_text(s, &sense_id, &field, &text))
}

#[tauri::command(async)]
fn lexicon_delete_sense(s: State<SessionThread>, sense_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_delete_sense(s, &sense_id))
}

#[tauri::command(async)]
fn lexicon_assign_domain(s: State<SessionThread>, sense_id: String, domain_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_assign_domain(s, &sense_id, &domain_id))
}

#[tauri::command(async)]
fn lexicon_unassign_domain(s: State<SessionThread>, sense_id: String, domain_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_unassign_domain(s, &sense_id, &domain_id))
}

#[tauri::command(async)]
fn domains_list(s: State<SessionThread>) -> Result<Vec<lexicon::DomainItem>, String> {
    s.run(move |s| lexicon::domains_list(s))
}

#[tauri::command(async)]
fn domains_load_sil(s: State<SessionThread>) -> Result<lexicon::DomainsLoaded, String> {
    s.run(move |s| lexicon::domains_load_sil(s))
}

#[tauri::command(async)]
fn concept_lists_bundled(s: State<SessionThread>) -> Result<Vec<elicit::BundledList>, String> {
    s.run(move |s| elicit::concept_lists_bundled(s))
}

#[tauri::command(async)]
fn concept_list_load(s: State<SessionThread>, source: String) -> Result<elicit::ListSummary, String> {
    s.run(move |s| elicit::concept_list_load(s, &source))
}

#[tauri::command(async)]
fn concept_lists(s: State<SessionThread>) -> Result<Vec<elicit::ListSummary>, String> {
    s.run(move |s| elicit::concept_lists(s))
}

#[tauri::command(async)]
fn concept_list_view(s: State<SessionThread>, list_id: String) -> Result<elicit::ListView, String> {
    s.run(move |s| elicit::concept_list_view(s, &list_id))
}

#[tauri::command(async)]
fn elicit_answer(s: State<SessionThread>, concept_id: String, form: String, gloss: String) -> Result<elicit::ConceptEdit, String> {
    s.run(move |s| elicit::elicit_answer(s, &concept_id, &form, &gloss))
}

#[tauri::command(async)]
fn elicit_link(s: State<SessionThread>, concept_id: String, sense_id: String) -> Result<elicit::ConceptEdit, String> {
    s.run(move |s| elicit::elicit_link(s, &concept_id, &sense_id))
}

#[tauri::command(async)]
fn elicit_unlink(s: State<SessionThread>, concept_id: String, sense_id: String) -> Result<elicit::ConceptEdit, String> {
    s.run(move |s| elicit::elicit_unlink(s, &concept_id, &sense_id))
}

#[tauri::command(async)]
fn elicit_skip(s: State<SessionThread>, concept_id: String, skipped: bool, note: String) -> Result<elicit::ConceptEdit, String> {
    s.run(move |s| elicit::elicit_skip(s, &concept_id, skipped, &note))
}

#[tauri::command(async)]
fn domain_questions(s: State<SessionThread>, domain_id: String) -> Result<elicit::DomainQuestions, String> {
    s.run(move |s| elicit::domain_questions(s, &domain_id))
}

#[tauri::command(async)]
fn domain_collect(s: State<SessionThread>, domain_id: String, form: String, gloss: String) -> Result<elicit::DomainEdit, String> {
    s.run(move |s| elicit::domain_collect(s, &domain_id, &form, &gloss))
}

#[tauri::command(async)]
fn praat_status(dir: State<AppData>) -> praat::Status {
    praat::status(&dir.0)
}

#[tauri::command(async)]
fn praat_set_path(dir: State<AppData>, path: Option<String>) -> Result<praat::Status, String> {
    praat::set_path(&dir.0, path.as_deref())
}

/// Hand a text to Praat, zoomed to `from_ms`–`to_ms` of `source`'s recording.
#[tauri::command(async)]
fn praat_open(s: State<SessionThread>, dir: State<AppData>, text: String, source: String, from_ms: f64, to_ms: f64) -> Result<praat::Opened, String> {
    let exe = praat::praat(&dir.0)?;
    let opened = s.run(move |s| praat::prepare(s, &text, &source, from_ms, to_ms))?;
    praat::open_in_praat(&exe, &opened)?;
    Ok(opened)
}

#[tauri::command(async)]
fn praat_pending(s: State<SessionThread>, text: String) -> Result<bool, String> {
    s.run(move |s| praat::pending(s, &text))
}

/// Ask Praat to save the text's grid, then list what it would change.
#[tauri::command(async)]
fn praat_bring_back(s: State<SessionThread>, dir: State<AppData>, text: String) -> Result<praat::ReturnView, String> {
    let exe = praat::praat(&dir.0)?;
    let t = text.clone();
    let (grid, script) = s.run(move |s| praat::save_request(s, &t))?;
    // Waiting for Praat happens here, off the session thread.
    praat::ask_praat_to_save(&exe, &grid, &script)?;
    s.run(move |s| praat::review(s, &text))
}

#[tauri::command(async)]
fn praat_apply(s: State<SessionThread>, text: String) -> Result<praat::Applied, String> {
    s.run(move |s| praat::apply(s, &text))
}

/// Pitch, intensity and formants of `source`'s recording for the given
/// 10 s chunks, from whatever powers acoustic analysis (Praat today;
/// cached per chunk).
#[tauri::command]
async fn acoustic_analysis(s: State<'_, SessionThread>, dir: State<'_, AppData>, source: String, chunks: Vec<u32>) -> Result<Vec<praat_analysis::Chunk>, String> {
    let cap = capability::Capability::AcousticAnalysis;
    let (job, pin) = s.run(move |s| {
        let job = praat_analysis::job(s, &source)?;
        Ok((job, capability::project_pin(session::project(s)?, cap)?))
    })?;
    let capability::Provider::Praat(exe) = capability::resolve(cap, pin.as_deref(), &dir.0)? else {
        return Err("Acoustic analysis has no provider here.".into());
    };
    // Praat can take seconds: on a blocking thread, not an async worker or
    // the session thread.
    tauri::async_runtime::spawn_blocking(move || praat_analysis::analyse(&exe, &job, &chunks)).await.map_err(|e| e.to_string())?
}

#[tauri::command(async)]
fn praat_discard(s: State<SessionThread>, text: String) -> Result<(), String> {
    s.run(move |s| praat::discard(s, &text))
}

#[tauri::command(async)]
fn conflicts_list(s: State<SessionThread>) -> Result<Vec<conflicts::ConflictView>, String> {
    s.run(move |s| conflicts::conflicts_list(s))
}

#[tauri::command(async)]
fn conflict_resolve(s: State<SessionThread>, target: String, field: String, raw: String, heads: Vec<String>) -> Result<conflicts::ConflictsResolved, String> {
    s.run(move |s| conflicts::conflict_resolve(s, &target, &field, &raw, &heads))
}

#[tauri::command(async)]
fn gloss_lexicon(s: State<SessionThread>, surface: String) -> Result<Vec<LexiconMatchView>, String> {
    s.run(move |s| gloss::gloss_lexicon(s, &surface))
}

#[tauri::command(async)]
fn conflict_count(s: State<SessionThread>) -> Result<i64, String> {
    s.run(session::conflict_count)
}

struct AppData(PathBuf);

/// The device key, loaded once at start. A failure is kept, not fatal: the
/// shell still opens projects, and every auth command reports why.
struct Identity(Result<(Arc<DeviceKey>, IdentityInfo), String>);

impl Identity {
    fn key(&self) -> Result<Arc<DeviceKey>, String> {
        self.0.as_ref().map(|(k, _)| k.clone()).map_err(Clone::clone)
    }
}

/// The sync listener of the open project and its mDNS announcement.
#[derive(Default)]
struct Reachable(Mutex<Option<Reach>>);

struct Reach {
    // Declared first, so the announcement is withdrawn before the port closes.
    _announcement: Option<Announcement>,
    listening: Listening,
}

impl Reachable {
    /// Stop listening once the session holds another project or another
    /// device id than the listener was started for; the old listener would
    /// otherwise answer for a folder that is no longer open.
    fn follow(&self, s: &SessionThread) -> Result<(), String> {
        let open = s.run(|s| Ok(s.as_ref().map(|p| (p.project_id.clone(), p.device().to_string()))))?;
        let mut reach = self.0.lock().map_err(|_| "listener state poisoned".to_string())?;
        let current = reach.as_ref().map(|r| (r.listening.project_id.clone(), r.listening.device_id.clone()));
        if current.is_some() && current != open {
            *reach = None;
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListenInfo {
    port: u16,
    /// Why peers cannot find this device by discovery; typing the address still works.
    discovery_error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum PairFinished {
    Enrolled { device: DeviceRow },
    Failed { error: String },
}

#[tauri::command(async)]
fn list_sources(s: State<SessionThread>) -> Result<Vec<SourceRow>, String> {
    s.run(session::list_sources)
}

#[tauri::command(async)]
fn search(s: State<SessionThread>, query: String, scope: SearchScope) -> Result<Vec<SearchRow>, String> {
    s.run(move |s| session::search(s, &query, scope))
}

#[tauri::command(async)]
fn save_layout(dir: State<AppData>, kit: String, layout: String) -> Result<(), String> {
    layout::save_layout(&dir.0, &kit, &layout)
}

#[tauri::command(async)]
fn load_layout(dir: State<AppData>, kit: String) -> Result<Option<String>, String> {
    layout::load_layout(&dir.0, &kit)
}

#[tauri::command(async)]
fn assist_models(dir: State<AppData>, downloads: State<assist::Downloads>) -> Vec<assist::ModelRow> {
    assist::rows(&dir.0, &downloads)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadEvent {
    id: String,
    done: u64,
    total: u64,
    /// Set once: `Ok` text "installed" or the error.
    finished: Option<Result<(), String>>,
}

/// Start a model download in the background; progress arrives as
/// `assist-download` events.
#[tauri::command(async)]
fn assist_download(app: AppHandle, dir: State<AppData>, downloads: State<assist::Downloads>, id: String) -> Result<(), String> {
    let spec = capability::manifest(&id).ok_or_else(|| format!("no model {id:?}"))?;
    let cancel = Arc::new(AtomicBool::new(false));
    let token = wugs_model::ulid();
    {
        let mut inflight = downloads.0.lock().expect("downloads lock");
        if inflight.get(&id).is_some_and(|d| d.state.error.is_none()) {
            return Err("That model is already downloading.".into());
        }
        inflight.insert(id.clone(), assist::Inflight { state: assist::DownloadState { done: 0, total: 0, error: None }, cancel: cancel.clone(), token: token.clone() });
    }
    let app_data = dir.0.clone();
    std::thread::spawn(move || {
        let last = std::sync::Mutex::new(std::time::Instant::now());
        let progress = |done: u64, total: u64| {
            let mut at = last.lock().expect("progress clock");
            if at.elapsed() < std::time::Duration::from_millis(200) {
                return;
            }
            *at = std::time::Instant::now();
            // Emitted under the lock, so a Cancel cannot land between the
            // token check and the event and leave a stale progress bar.
            match app.state::<assist::Downloads>().0.lock().expect("downloads lock").get_mut(&spec.id) {
                Some(d) if d.token == token => {
                    d.state.done = done;
                    d.state.total = total;
                    app.emit("assist-download", DownloadEvent { id: spec.id.clone(), done, total, finished: None }).ok();
                }
                // Cancelled or replaced: this transfer reports nothing more.
                _ => {}
            }
        };
        let result = assist::download(&app_data, spec, &cancel, &progress).map(|_| ());
        let downloads = app.state::<assist::Downloads>();
        let mut inflight = downloads.0.lock().expect("downloads lock");
        if !inflight.get(&spec.id).is_some_and(|d| d.token == token) {
            // Cancelled earlier: its slot is gone or belongs to a newer download.
            return;
        }
        match &result {
            Ok(()) => {
                inflight.remove(&spec.id);
            }
            Err(e) => {
                if let Some(d) = inflight.get_mut(&spec.id) {
                    d.state.error = Some(e.clone());
                }
            }
        }
        drop(inflight);
        app.state::<assist::Worker>().forget(&spec.id);
        app.emit("assist-download", DownloadEvent { id: spec.id.clone(), done: 0, total: 0, finished: Some(result) }).ok();
    });
    Ok(())
}

#[tauri::command(async)]
fn assist_cancel(downloads: State<assist::Downloads>, id: String) {
    // Flag the transfer and free its slot now: a read stalled on a dead
    // link may not return for hours, and the person must be able to retry.
    if let Some(d) = downloads.0.lock().expect("downloads lock").remove(&id) {
        d.cancel.store(true, Ordering::Relaxed);
    }
}

#[tauri::command(async)]
fn assist_delete(dir: State<AppData>, downloads: State<assist::Downloads>, worker: State<assist::Worker>, id: String) -> Result<(), String> {
    if downloads.0.lock().expect("downloads lock").get(&id).is_some_and(|d| d.state.error.is_none()) {
        return Err("Cancel the download first.".into());
    }
    worker.forget(&id);
    assist::delete(&dir.0, &id)
}

/// Hear one empty segment with whatever powers phone transcription here
/// (the project's pin, else this computer's choice) and write what it heard.
/// `expect` is the provider the caller showed (or started a batch with): if
/// what powers phone transcription changed since (a pin arrived by sync, a
/// choice changed, a better model finished downloading), nothing is written,
/// so a run never switches models halfway.
#[tauri::command(async)]
fn assist_suggest(s: State<SessionThread>, dir: State<AppData>, worker: State<assist::Worker>, text: String, segment: String, source: String, expect: String) -> Result<assist::Suggested, String> {
    let cap = capability::Capability::PhoneTranscription;
    let (t, seg, src) = (text.clone(), segment.clone(), source.clone());
    let (target, pin) = s.run(move |s| {
        let target = assist::target(s, &t, &seg, &src)?;
        let pin = capability::project_pin(session::project(s)?, cap)?;
        Ok((target, pin))
    })?;
    let capability::Provider::Model(spec) = capability::resolve(cap, pin.as_deref(), &dir.0)? else {
        return Err("Phone transcription needs a model.".into());
    };
    if spec.id != expect {
        return Err(format!("{CHANGED}{}; nothing was written.", spec.name));
    }
    let app_data = dir.0.clone();
    let heard = worker.hear(&spec.id, move || assist::hearer(&app_data, spec), target.wav, target.start_ms, target.end_ms)?;
    s.run(move |s| assist::commit(s, &text, &segment, &source, spec, &heard))
}

/// What a corrections export would hold now.
#[tauri::command(async)]
fn corrections_summary(s: State<SessionThread>, id: State<Identity>) -> Result<corrections::Summary, String> {
    let key = id.key().ok();
    s.run(move |s| {
        let guest = session::is_guest(s, key.as_deref())?;
        corrections::summary(s, guest)
    })
}

/// Export the checked suggestions into a new folder inside `folder`.
/// `unrecorded`: the person confirmed consent where no record says.
#[tauri::command(async)]
fn corrections_export(s: State<SessionThread>, id: State<Identity>, folder: String, unrecorded: bool) -> Result<corrections::Exported, String> {
    let key = id.key().ok();
    let (jobs, sum) = s.run(move |s| {
        let guest = session::is_guest(s, key.as_deref())?;
        corrections::prepare(s, guest, unrecorded)
    })?;
    corrections::write(&jobs, &sum, unrecorded, std::path::Path::new(&folder), &corrections::today())
}

/// The start of `assist_suggest`'s refusal when the model changed under a run.
const CHANGED: &str = "Phone transcription now uses ";


/// Every capability: what can power it here, this computer's choice, the
/// open project's pin, and what runs now.
#[tauri::command(async)]
fn capabilities(s: State<SessionThread>, dir: State<AppData>) -> Result<Vec<capability::CapabilityView>, String> {
    let pins = s.run(|s| {
        let mut pins = std::collections::BTreeMap::new();
        if let Ok(p) = session::project(s) {
            for cap in capability::Capability::ALL {
                pins.insert(cap.id(), capability::project_pin(p, cap)?);
            }
        }
        Ok(pins)
    })?;
    Ok(capability::views(&dir.0, &pins))
}

/// This computer's choice for a capability: "automatic" or a provider id.
#[tauri::command(async)]
fn capability_choose(dir: State<AppData>, capability: String, choice: String) -> Result<(), String> {
    let cap = capability::Capability::parse(&capability)?;
    let choice = capability::Choice::parse(&choice);
    if let capability::Choice::Pick(id) = &choice {
        if !capability::known(cap).contains(id) {
            return Err(format!("{id:?} cannot do {}", cap.title().to_lowercase()));
        }
    }
    capability::set_machine_choice(&dir.0, cap, &choice)
}

/// Pin a provider for everyone on the open project, or unpin with an empty id.
#[tauri::command(async)]
fn capability_pin(s: State<SessionThread>, capability: String, provider: String) -> Result<(), String> {
    let cap = capability::Capability::parse(&capability)?;
    if !provider.is_empty() && !capability::known(cap).contains(&provider) {
        return Err(format!("{provider:?} cannot do {}", cap.title().to_lowercase()));
    }
    s.run(move |s| session::project(s)?.set_setting(&cap.setting_key(), &provider).map(|_| ()))
}

#[tauri::command(async)]
fn kits_list(dir: State<AppData>) -> Result<kits::KitsView, String> {
    kits::list(&dir.0)
}

#[tauri::command(async)]
fn kit_code(dir: State<AppData>, id: String) -> Result<String, String> {
    kits::code(&dir.0, &id)
}

#[tauri::command(async)]
fn kit_save(dir: State<AppData>, id: String, kit: kits::KitDto) -> Result<kits::KitEntry, String> {
    kits::save(&dir.0, &id, &kit)
}

#[tauri::command(async)]
fn kit_save_code(dir: State<AppData>, id: String, code: String) -> Result<kits::KitEntry, String> {
    kits::save_code(&dir.0, &id, &code)
}

#[tauri::command(async)]
fn kit_duplicate(dir: State<AppData>, source: String, name: String) -> Result<kits::KitEntry, String> {
    kits::duplicate(&dir.0, &source, &name)
}

#[tauri::command(async)]
fn kit_delete(dir: State<AppData>, id: String) -> Result<kits::KitsView, String> {
    kits::delete(&dir.0, &id)
}

#[tauri::command(async)]
fn kit_install(dir: State<AppData>, path: String) -> Result<kits::KitEntry, String> {
    kits::install(&dir.0, std::path::Path::new(&path))
}

#[tauri::command(async)]
fn kit_set_active(dir: State<AppData>, id: String) -> Result<(), String> {
    kits::set_active(&dir.0, &id)
}

#[tauri::command(async)]
fn device_identity(id: State<Identity>) -> Result<IdentityInfo, String> {
    id.0.as_ref().map(|(_, info)| info.clone()).map_err(Clone::clone)
}

#[tauri::command(async)]
fn devices_list(s: State<SessionThread>, id: State<Identity>) -> Result<DevicesView, String> {
    let key = id.key()?;
    s.run(move |s| session::devices_list(s, &key))
}

#[tauri::command(async)]
fn device_mark_lost(s: State<SessionThread>, id: State<Identity>, key: String) -> Result<DevicesView, String> {
    let me = id.key()?;
    s.run(move |s| session::device_mark_lost(s, &me, &key))
}

/// Show a code and wait for one joiner in the background; the outcome
/// arrives as a `pair-finished` event.
#[tauri::command(async)]
fn pair_start(app: AppHandle, s: State<SessionThread>, id: State<Identity>, label: String, guest: bool) -> Result<PairOffer, String> {
    let key = id.key()?;
    let k = key.clone();
    let (offer, pairing) = s.run(move |s| session::pair_start(s, &k, &label, guest))?;
    let thread = s.inner().clone();
    std::thread::spawn(move || {
        let result = pairing
            .wait()
            .and_then(|tcp| thread.run(move |s| session::pair_finish(s, &key, pairing, tcp)));
        let event = match result {
            Ok(device) => PairFinished::Enrolled { device },
            Err(error) => PairFinished::Failed { error },
        };
        app.emit("pair-finished", event).ok();
    });
    Ok(offer)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
fn pair_join(
    app: AppHandle,
    s: State<SessionThread>,
    id: State<Identity>,
    reach: State<Reachable>,
    path: String,
    researcher: String,
    addr: String,
    code: String,
    label: String,
) -> Result<JoinResult, String> {
    let key = id.key()?;
    let p = path.clone();
    let joined = s.run(move |s| session::pair_join(s, &key, &p, &researcher, &addr, &code, &label))?;
    allow_media(&app, &path);
    reach.follow(&s)?;
    Ok(joined)
}

#[tauri::command(async)]
fn relocation_accept_move(s: State<SessionThread>) -> Result<(), String> {
    s.run(session::relocation_accept_move)
}

/// The fresh id changes what the listener announces, so it stops; the
/// next "make reachable" starts it under the new id.
#[tauri::command(async)]
fn mint_device_identity(s: State<SessionThread>, id: State<Identity>, reach: State<Reachable>) -> Result<DevicesView, String> {
    let key = id.key()?;
    let view = s.run(move |s| session::mint_device_identity(s, &key))?;
    reach.follow(&s)?;
    Ok(view)
}

/// The unauthenticated path exists for development only: a debug build
/// run with `WUGS_DEV_OPEN_SYNC=1`.
fn dev_open_sync() -> bool {
    cfg!(debug_assertions) && std::env::var("WUGS_DEV_OPEN_SYNC").as_deref() == Ok("1")
}

#[tauri::command(async)]
fn sync_now(s: State<SessionThread>, id: State<Identity>, addr: String) -> Result<SyncResult, String> {
    if dev_open_sync() {
        return s.run(move |s| session::sync_now_open(s, &addr));
    }
    let key = id.key()?;
    s.run(move |s| session::sync_now(s, &key, &addr))
}

/// Make this device reachable for sync: listen on a port, announce it over
/// mDNS, and serve each connection on the session thread. Each served
/// session arrives as a `sync-served` event.
#[tauri::command(async)]
fn sync_listen(app: AppHandle, s: State<SessionThread>, id: State<Identity>, reach: State<Reachable>, label: String) -> Result<ListenInfo, String> {
    let key = id.key()?;
    let k = key.clone();
    let l = s.run(move |s| session::sync_listen(s, &k, &label))?;
    let mut reach = reach.0.lock().map_err(|_| "listener state poisoned".to_string())?;
    if let Some(cur) = reach.as_ref().map(|r| &r.listening).filter(|cur| cur.project_id == l.project_id && cur.device_id == l.device_id) {
        return Ok(ListenInfo { port: cur.port, discovery_error: None });
    }
    // The old listener, if any, closes its port before the new one starts.
    *reach = None;
    let listening = listen::start(s.inner().clone(), key, l, move |served| {
        app.emit("sync-served", served).ok();
    })?;
    let (announcement, discovery_error) = match discover::announce(&listening.project_id, &listening.device_id, listening.port) {
        Ok(a) => (Some(a), None),
        Err(e) => (None, Some(e)),
    };
    let port = listening.port;
    *reach = Some(Reach { _announcement: announcement, listening });
    Ok(ListenInfo { port, discovery_error })
}

/// Devices announcing the open project on the local network.
#[tauri::command(async)]
fn discover_peers(s: State<SessionThread>, reach: State<Reachable>) -> Result<Vec<String>, String> {
    let project_id = s.run(|s| s.as_ref().map(|p| p.project_id.clone()).ok_or_else(|| "no project is open".to_string()))?;
    let own_port = reach.0.lock().ok().and_then(|r| r.as_ref().map(|r| r.listening.port));
    let found = discover::browse(&project_id, Duration::from_secs(2))?;
    Ok(found.into_iter().filter(|a| Some(a.port()) != own_port).map(|a| a.to_string()).collect())
}

pub fn run() {
    tauri::Builder::default()
        // Registered first, as the plugin requires: a second launch hands
        // over to this one, which comes to the front.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                w.unminimize().ok();
                w.show().ok();
                w.set_focus().ok();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        // Size, position and maximized state come back on the next launch; a
        // machine with no saved state keeps the size from tauri.conf.json.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Recording needs the microphone, so the engine's own prompt is
        // skipped, but only for the app's own page: wry hands this handler
        // no origin, so the closure reads the webview's current URL.
        .on_permission_request(|webview, kind| match kind {
            tauri::webview::PermissionKind::Microphone => {
                let dev_url = if cfg!(debug_assertions) { webview.app_handle().config().build.dev_url.clone() } else { None };
                match webview.url() {
                    Ok(page) if capture::is_app_origin(&page, dev_url.as_ref()) => tauri::webview::PermissionResponse::Allow,
                    _ => tauri::webview::PermissionResponse::Deny,
                }
            }
            _ => tauri::webview::PermissionResponse::Default,
        })
        .manage(SessionThread::spawn())
        .manage(Reachable::default())
        .manage(Captures::default())
        .manage(assist::Downloads::default())
        .manage(assist::Worker::spawn())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let identity = identity::device_identity(&dir, &KeychainKeyStore).map(|(k, info)| (Arc::new(k), info));
            app.manage(Identity(identity));
            app.manage(AppData(dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            assist_models,
            assist_download,
            assist_cancel,
            assist_delete,
            assist_suggest,
            corrections_summary,
            corrections_export,
            capabilities,
            capability_choose,
            capability_pin,
            open_project,
            create_project,
            list_texts,
            get_text,
            create_text,
            add_segment,
            edit_segment_content,
            set_segment_times,
            delete_segment,
            add_tier,
            gloss_get,
            gloss_tokenize,
            gloss_retokenize,
            gloss_apply,
            gloss_apply_all,
            gloss_set,
            gloss_pick_sense,
            gloss_add_lexeme,
            gloss_lexicon,
            gloss_restore,
            gloss_choose_list,
            lexicon_list,
            lexicon_entry,
            lexicon_add,
            lexicon_set_field,
            lexicon_set_homograph,
            lexicon_number_homographs,
            lexicon_delete,
            lexicon_add_sense,
            lexicon_set_sense_gloss,
            lexicon_set_sense_text,
            lexicon_delete_sense,
            lexicon_assign_domain,
            lexicon_unassign_domain,
            domains_list,
            domains_load_sil,
            concept_lists_bundled,
            concept_list_load,
            concept_lists,
            concept_list_view,
            elicit_answer,
            elicit_link,
            elicit_unlink,
            elicit_skip,
            domain_questions,
            domain_collect,
            conflict_count,
            conflicts_list,
            praat_status,
            praat_set_path,
            praat_open,
            praat_pending,
            praat_bring_back,
            praat_apply,
            praat_discard,
            acoustic_analysis,
            conflict_resolve,
            list_sources,
            import_media,
            capture_begin,
            capture_chunk,
            capture_finish,
            capture_discard,
            capture_keep,
            capture_reset,
            capture_recovered_list,
            capture_recovered_import,
            capture_recovered_discard,
            peaks_info,
            peaks_query,
            search,
            save_layout,
            load_layout,
            kits_list,
            kit_code,
            kit_save,
            kit_save_code,
            kit_duplicate,
            kit_delete,
            kit_install,
            kit_set_active,
            device_identity,
            devices_list,
            device_mark_lost,
            pair_start,
            pair_join,
            mint_device_identity,
            relocation_accept_move,
            sync_now,
            sync_listen,
            discover_peers,
        ])
        .run(tauri::generate_context!())
        .expect("run the wugs desktop shell");
}
