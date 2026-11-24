pub mod abbreviations;
pub mod assist;
pub mod capability;
pub mod clips;
pub mod capture;
pub mod chat;
pub mod exmaralda;
pub mod like_sentences;
pub mod worklog;
pub mod hit_categories;
pub mod mlu;
pub mod wikipedia;
pub mod characters;
pub mod concordance;
pub mod cqp;
pub mod cwb;
pub mod dictionary;
pub mod flashcards;
pub mod archive_sheet;
pub mod typing;
pub mod dispersion;
pub mod collocates;
pub mod similar;
pub mod frequencies;
pub mod keywords;
pub mod elan_glossing;
pub mod topics;
pub mod conflicts;
pub mod consistency;
pub mod convert;
pub mod corrections;
pub mod drops;
pub mod elicit;
pub mod feedback;
pub mod help;
pub mod glottolog;
pub mod gloss;
pub mod gloss_types;
pub mod identity;
pub mod interchange;
pub mod kits;
pub mod layout;
pub mod lexicon;
pub mod lexicon_table;
pub mod duplicates;
pub mod levels;
pub mod meanings;
pub mod measure;
pub mod media_bin;
pub mod metadata;
pub mod morphosyntax;
pub mod people;
pub mod field_sessions;
pub mod plaintext;
pub mod replace;
pub mod website;
pub mod speech_dataset;
pub mod spelling;
pub mod parallel;
pub mod facsimile;
pub mod stylometry;
pub mod subtitles;
pub mod table_text;
pub mod listen;
pub mod praat;
pub mod affixes;
pub mod progress;
pub mod praat_analysis;
pub mod sample;
pub mod segmentation;
pub mod sentence;
pub mod notes_overview;
pub mod growth;
pub mod wordsketch;
pub mod soundchanges;
pub mod audacity;
pub mod cognates;
pub mod sentences;
pub mod timing;
pub mod compare;
pub mod phoible;
pub mod word_order;
pub mod session;
pub mod sounds;
pub mod typology;
pub mod orthography;
pub mod paradigm_templates;
pub mod wordlist;
pub mod spectrogram;
pub mod updater;
pub mod voice_analysis;

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

/// The tutorial project (roadmap 68; once called the sample, whose old
/// `sample-project` folder is left as it was): built in this computer's app data the
/// first time, opened as it was left after that.
#[tauri::command(async)]
fn open_sample(app: AppHandle, s: State<SessionThread>, reach: State<Reachable>, researcher: String) -> Result<ProjectInfo, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("tutorial-project");
    let path = dir.to_string_lossy().to_string();
    let who = if researcher.trim().is_empty() { "me".to_string() } else { researcher };
    let info = s.run(move |s| sample::open_sample(s, &dir, &who))?;
    allow_media(&app, &path);
    reach.follow(&s)?;
    Ok(info)
}

/// Every occurrence of a word, morph, gloss or translation word, in context (roadmap 71).
#[tauri::command(async)]
fn concordance(s: State<SessionThread>, query: String, kind: concordance::Kind, exact: Option<bool>, text_id: Option<String>, within: Option<concordance::Where>) -> Result<concordance::Concordance, String> {
    let how = if exact == Some(true) { concordance::Match::Exact } else { concordance::Match::Glob };
    s.run(move |s| concordance::concordance_in(s, &query, kind, how, &concordance::Scope::of(text_id, within)))
}

/// Where an item occurs, text by text, and how evenly (AntConc's Plot; `src/dispersion.rs`).
#[tauri::command(async)]
fn dispersion(s: State<SessionThread>, query: String, kind: concordance::Kind, exact: Option<bool>, text_id: Option<String>, within: Option<concordance::Where>) -> Result<dispersion::Dispersion, String> {
    let how = if exact == Some(true) { concordance::Match::Exact } else { concordance::Match::Glob };
    s.run(move |s| dispersion::dispersion(s, &query, kind, how, &concordance::Scope::of(text_id, within)))
}

/// Morphs glossed more than one way across the project (`src/consistency.rs`).
#[tauri::command(async)]
fn gloss_consistency(s: State<SessionThread>) -> Result<Vec<consistency::Inconsistent>, String> {
    s.run(consistency::glosses)
}

/// Gloss every morph of `key` and `kind` that reads `from` as `to` instead.
#[tauri::command(async)]
fn gloss_unify(s: State<SessionThread>, key: String, kind: String, from: String, to: String) -> Result<usize, String> {
    s.run(move |s| consistency::unify(s, &key, &kind, &from, &to))
}

/// A search's categories and the lines sorted into them.
#[tauri::command(async)]
fn hit_categories(s: State<SessionThread>, kind: String, query: String) -> Result<hit_categories::Sorted, String> {
    s.run(move |s| hit_categories::get(s, &kind, &query))
}

/// Name a search's categories.
#[tauri::command(async)]
fn set_hit_categories(s: State<SessionThread>, kind: String, query: String, categories: Vec<String>) -> Result<hit_categories::Sorted, String> {
    s.run(move |s| hit_categories::set_categories(s, &kind, &query, &categories))
}

/// Put a line of a search in a category (empty takes it out).
#[tauri::command(async)]
fn set_hit_category(s: State<SessionThread>, kind: String, query: String, segment_id: String, at: usize, category: String) -> Result<hit_categories::Sorted, String> {
    s.run(move |s| hit_categories::set_line(s, &kind, &query, &segment_id, at, &category))
}

/// What was done in the project day by day, read from the op log.
#[tauri::command(async)]
fn work_log(s: State<SessionThread>, offset_minutes: i32, days: usize) -> Result<Vec<worklog::Day>, String> {
    s.run(move |s| worklog::worklog(s, offset_minutes, days))
}

/// The project's sentences most like this one, with their translations.
#[tauri::command(async)]
fn like_sentences(s: State<SessionThread>, segment_id: String) -> Result<Vec<like_sentences::Like>, String> {
    s.run(move |s| like_sentences::like(s, &segment_id))
}

/// Word forms analysed with more than one breakdown (the Checks panel).
#[tauri::command(async)]
fn gloss_segmentations(s: State<SessionThread>) -> Result<Vec<consistency::Segmented>, String> {
    s.run(consistency::segmentations)
}

/// Give every analysed word of this token's form its analysis.
#[tauri::command(async)]
fn gloss_use_breakdown(s: State<SessionThread>, token_id: String) -> Result<usize, String> {
    s.run(move |s| consistency::use_breakdown(s, &token_id))
}

/// Glossed roots with no lexicon entry (the Checks panel).
#[tauri::command(async)]
fn consent_gaps(s: State<SessionThread>) -> Result<people::ConsentGaps, String> {
    s.run(people::consent_gaps)
}

#[tauri::command(async)]
fn gloss_sense_drift(s: State<SessionThread>) -> Result<Vec<consistency::Drift>, String> {
    s.run(consistency::sense_drift)
}

#[tauri::command(async)]
fn gloss_settle_drift(s: State<SessionThread>, sense_id: String, text_gloss: String, use_lexicon: bool) -> Result<usize, String> {
    s.run(move |session| consistency::settle_drift(session, &sense_id, &text_gloss, use_lexicon))
}

#[tauri::command(async)]
fn gloss_missing(s: State<SessionThread>) -> Result<Vec<consistency::Missing>, String> {
    s.run(consistency::missing)
}

/// Make entries for these roots (every one when `keys` is empty) and link their morphs.
#[tauri::command(async)]
fn gloss_add_missing(s: State<SessionThread>, keys: Vec<String>) -> Result<consistency::Added, String> {
    s.run(move |s| consistency::add_missing(s, &keys))
}

/// The project's own meanings of these gloss abbreviations (`src/abbreviations.rs`).
#[tauri::command(async)]
fn abbreviation_meanings(s: State<SessionThread>, abbreviations: Vec<String>) -> Result<Vec<Option<String>>, String> {
    s.run(move |s| abbreviations::meanings(s, &abbreviations))
}

/// Write (or clear, when empty) what a gloss abbreviation means in this project.
#[tauri::command(async)]
fn set_abbreviation_meaning(s: State<SessionThread>, abbreviation: String, meaning: String) -> Result<(), String> {
    s.run(move |s| abbreviations::set(s, &abbreviation, &meaning))
}

/// A CQP-style query over the sentences' tokens and their annotation (`src/cqp.rs`).
#[tauri::command(async)]
fn concordance_query(s: State<SessionThread>, query: String, text_id: Option<String>) -> Result<concordance::Concordance, String> {
    s.run(move |s| cqp::run(s, &query, text_id.as_deref()))
}

/// A text's document metadata and the keys the project uses.
#[tauri::command(async)]
fn text_metadata(s: State<SessionThread>, text_id: String) -> Result<metadata::MetaView, String> {
    s.run(move |s| metadata::text_metadata(s, &text_id))
}

/// Set one metadata key of a text; an empty value clears it.
#[tauri::command(async)]
fn set_text_metadata(s: State<SessionThread>, text_id: String, key: String, value: String) -> Result<metadata::MetaView, String> {
    s.run(move |s| metadata::set_text_metadata(s, &text_id, &key, &value))
}

/// Every detail's values across the project, for picking texts by one.
#[tauri::command(async)]
fn metadata_values(s: State<SessionThread>) -> Result<Vec<metadata::MetaValues>, String> {
    s.run(metadata::metadata_values)
}

/// Bring in a table of document metadata (one row per text).
#[tauri::command(async)]
fn import_metadata(s: State<SessionThread>, path: String) -> Result<metadata::MetaImport, String> {
    s.run(move |s| metadata::import_metadata(s, std::path::Path::new(&path)))
}

/// One sentence's words, tags and tree, for the Sentence panel.
#[tauri::command(async)]
fn sentence_get(s: State<SessionThread>, text_id: String, segment_id: String) -> Result<sentence::SentenceView, String> {
    s.run(move |s| sentence::sentence_get(s, &text_id, &segment_id))
}

/// Correct one cell of a sentence's annotation (one resolving write).
#[tauri::command(async)]
fn sentence_set(s: State<SessionThread>, text_id: String, segment_id: String, token_id: String, column: String, value: String) -> Result<sentence::SentenceView, String> {
    s.run(move |s| sentence::sentence_set(s, &text_id, &segment_id, &token_id, &column, &value))
}

/// How often each word, morph, gloss or translation word occurs (Scrolls' frequency list).
#[tauri::command(async)]
fn frequencies(s: State<SessionThread>, kind: concordance::Kind, text_id: Option<String>, within: Option<concordance::Where>, size: Option<usize>) -> Result<frequencies::Frequencies, String> {
    s.run(move |s| frequencies::frequencies_of(s, kind, &concordance::Scope::of(text_id, within), size.unwrap_or(1)))
}

/// Which words one text, or the texts a detail picks, use more or less than the rest, or than another text, detail or track (Scrolls' keywords).
#[tauri::command(async)]
fn keywords(s: State<SessionThread>, kind: concordance::Kind, text_id: Option<String>, within: Option<concordance::Where>, against_text: Option<String>, against: Option<concordance::Where>, min_count: usize, size: Option<usize>) -> Result<keywords::Keywords, String> {
    s.run(move |s| keywords::keywords_against(s, kind, &concordance::Scope::of(text_id, within), &concordance::Scope::of(against_text, against), min_count, size.unwrap_or(1)))
}

/// How alike the texts are in their most frequent words (Cosine Delta).
#[tauri::command(async)]
fn texts_alike(s: State<SessionThread>, kind: concordance::Kind) -> Result<stylometry::Alike, String> {
    s.run(move |s| stylometry::alike(s, kind))
}

/// Which words keep company with a word (Scrolls' collocates).
#[tauri::command(async)]
fn collocates(s: State<SessionThread>, node: String, kind: concordance::Kind, left: usize, right: usize, min_together: usize, text_id: Option<String>, within: Option<concordance::Where>) -> Result<collocates::Collocates, String> {
    s.run(move |s| collocates::collocates(s, &node, kind, left, right, min_together, &concordance::Scope::of(text_id, within)))
}

/// The words used in the same contexts as a word (src/similar.rs).
#[tauri::command(async)]
fn similar_words(s: State<SessionThread>, word: String, kind: concordance::Kind, window: usize, min_freq: usize, text_id: Option<String>, within: Option<concordance::Where>) -> Result<similar::Similar, String> {
    s.run(move |s| similar::similar(s, &word, kind, window, min_freq, &concordance::Scope::of(text_id, within)))
}

/// Remember which recording a text was transcribed from, or forget it (roadmap 72).
#[tauri::command(async)]
fn link_recording(s: State<SessionThread>, text_id: String, source_id: Option<String>) -> Result<(), String> {
    s.run(move |s| session::link_recording(s, &text_id, source_id.as_deref()))
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

/// Import an ELAN, FLEx, Praat or Toolbox file into the open project.
#[tauri::command(async)]
fn import_file(s: State<SessionThread>, path: String) -> Result<interchange::Imported, String> {
    s.run(move |s| interchange::import_file(s, &path))
}

/// Export one text (`elan`, `flextext`, `textgrid`) into `folder`.
#[tauri::command(async)]
fn export_text(s: State<SessionThread>, text_id: String, format: String, folder: String) -> Result<interchange::Exported, String> {
    let format = interchange::Format::parse(&format)?;
    s.run(move |s| interchange::export_text(s, &text_id, format, std::path::Path::new(&folder)))
}

/// Export the lexicon (`lift`, `toolbox`) into `folder`.
#[tauri::command(async)]
fn export_lexicon(s: State<SessionThread>, format: String, folder: String) -> Result<interchange::Exported, String> {
    let format = interchange::Format::parse(&format)?;
    s.run(move |s| interchange::export_lexicon(s, format, std::path::Path::new(&folder)))
}

/// Read a lexicon table (CSV or tab-separated, a row per sense) into the lexicon.
#[tauri::command(async)]
fn import_lexicon_table(s: State<SessionThread>, path: String) -> Result<lexicon_table::TableImport, String> {
    s.run(move |s| lexicon_table::import(s, std::path::Path::new(&path)))
}

/// Write the lexicon as a CSV table into `folder`; the file's path back.
#[tauri::command(async)]
fn export_lexicon_table(s: State<SessionThread>, folder: String) -> Result<String, String> {
    s.run(move |s| lexicon_table::export(s, std::path::Path::new(&folder)))
}

/// The project's affixes and clitics with their slots (`src/affixes.rs`).
#[tauri::command(async)]
fn project_affixes(s: State<SessionThread>) -> Result<affixes::Affixes, String> {
    s.run(affixes::affixes)
}

/// How far the documentation has come, text by text (`src/progress.rs`).
#[tauri::command(async)]
fn project_progress(s: State<SessionThread>) -> Result<progress::Progress, String> {
    s.run(progress::progress)
}

/// The language the project documents (name, Glottocode, ISO 639-3).
#[tauri::command(async)]
fn project_language(s: State<SessionThread>) -> Result<interchange::LanguageDto, String> {
    s.run(interchange::project_language)
}

#[tauri::command(async)]
fn set_project_language(s: State<SessionThread>, language: interchange::LanguageDto) -> Result<interchange::LanguageDto, String> {
    s.run(move |s| interchange::set_project_language(s, &language))
}

/// How far a text's machine tags are checked.
#[tauri::command(async)]
fn sentence_check_status(s: State<SessionThread>, text_id: String) -> Result<sentence::CheckView, String> {
    s.run(move |s| sentence::check_status(s, &text_id))
}

/// Mark machine tags checked without changing them (the whole text when no sentence is named).
#[tauri::command(async)]
fn sentence_mark_checked(s: State<SessionThread>, text_id: String, segment_ids: Vec<String>) -> Result<sentence::CheckView, String> {
    s.run(move |s| sentence::mark_checked(s, &text_id, &segment_ids))
}

/// Every character the project's transcriptions and lexicon use, with what may be a slip.
#[tauri::command(async)]
fn compare_read(path: String) -> Result<compare::OtherList, String> {
    compare::read_list(std::path::Path::new(&path))
}

#[tauri::command(async)]
fn sound_changes(s: State<SessionThread>) -> Result<String, String> {
    s.run(soundchanges::get)
}

#[tauri::command(async)]
fn set_sound_changes(s: State<SessionThread>, rules: String) -> Result<String, String> {
    s.run(move |session| soundchanges::set(session, &rules))
}

#[tauri::command(async)]
fn word_sketch(s: State<SessionThread>, lemma: String) -> Result<wordsketch::WordSketch, String> {
    s.run(move |session| wordsketch::word_sketch(session, &lemma))
}

#[tauri::command(async)]
fn vocabulary_growth(s: State<SessionThread>) -> Result<growth::Growth, String> {
    s.run(growth::vocabulary_growth)
}

/// Read Wikipedia articles (named, and some at random) off the session
/// thread, then bring each in as a text with its source.
#[tauri::command(async)]
async fn wikipedia_import(s: State<'_, SessionThread>, wiki: String, titles: Vec<String>, random: usize, date: String) -> Result<wikipedia::Gathered, String> {
    let base = wikipedia::base_of(&wiki)?;
    let (articles, missing) = tauri::async_runtime::spawn_blocking(move || wikipedia::fetch(&base, &titles, random)).await.map_err(|e| e.to_string())??;
    s.run(move |s| wikipedia::import(s, &wiki, &articles, &missing, &date))
}

#[tauri::command(async)]
fn corpus_tracks(s: State<SessionThread>) -> Result<Vec<concordance::Track>, String> {
    s.run(concordance::tracks)
}

#[tauri::command(async)]
fn speaker_lengths(s: State<SessionThread>) -> Result<Vec<mlu::SpeakerRow>, String> {
    s.run(mlu::speakers)
}

#[tauri::command(async)]
fn all_notes(s: State<SessionThread>) -> Result<notes_overview::NotesView, String> {
    s.run(notes_overview::all_notes)
}

#[tauri::command(async)]
fn tree_problems(s: State<SessionThread>) -> Result<sentence::TreeProblems, String> {
    s.run(sentence::tree_problems)
}

#[tauri::command(async)]
fn character_inventory(s: State<SessionThread>) -> Result<characters::Inventory, String> {
    s.run(characters::inventory)
}

/// The word types on a transcription track (or all of them), for the Sounds panel.
#[tauri::command(async)]
fn sound_words(s: State<SessionThread>, tier_id: Option<String>, text_id: Option<String>) -> Result<sounds::SoundWords, String> {
    s.run(move |s| sounds::sound_words(s, tier_id.as_deref(), text_id))
}

/// Sentences written twice or more (src/duplicates.rs).
#[tauri::command(async)]
fn duplicate_sentences(s: State<SessionThread>) -> Result<duplicates::Duplicates, String> {
    s.run(duplicates::duplicates)
}

/// Segments that overlap on a track, or are very short or very long.
#[tauri::command(async)]
fn segment_timing(s: State<SessionThread>) -> Result<timing::Timing, String> {
    s.run(timing::timing)
}

/// The project's prompt lists for sentence elicitation (src/sentences.rs).
#[tauri::command(async)]
fn sentence_lists(s: State<SessionThread>) -> Result<Vec<sentences::PromptList>, String> {
    s.run(sentences::lists)
}

/// Keeps a prompt list read from a text or table file.
#[tauri::command(async)]
fn sentence_list_import(s: State<SessionThread>, path: String) -> Result<sentences::PromptList, String> {
    s.run(move |session| sentences::import(session, std::path::Path::new(&path)))
}

/// Keeps a prompt list made in the app (frames filled with lexicon entries).
#[tauri::command(async)]
fn sentence_list_make(s: State<SessionThread>, name: String, prompts: Vec<sentences::Prompt>) -> Result<sentences::PromptList, String> {
    s.run(move |session| sentences::make(session, &name, prompts))
}

/// A prompt list with each prompt's answers.
#[tauri::command(async)]
fn sentence_list_view(s: State<SessionThread>, list: String) -> Result<sentences::PromptView, String> {
    s.run(move |session| sentences::view(session, &list))
}

/// Writes an answer to a prompt as a sentence with the prompt as its translation.
#[tauri::command(async)]
fn sentence_answer(s: State<SessionThread>, list: String, prompt: String, sentence: String) -> Result<sentences::PromptView, String> {
    s.run(move |session| sentences::answer(session, &list, &prompt, &sentence))
}

/// The project's cognate judgements (src/cognates.rs).
#[tauri::command(async)]
fn cognates(s: State<SessionThread>) -> Result<String, String> {
    s.run(cognates::get)
}

/// Keeps the project's cognate judgements, returning them as kept.
#[tauri::command(async)]
fn set_cognates(s: State<SessionThread>, table: String) -> Result<String, String> {
    s.run(move |session| cognates::set(session, &table))
}

/// Word order counted from the parsed sentences, by WALS feature (src/word_order.rs).
#[tauri::command(async)]
fn word_order(s: State<SessionThread>) -> Result<word_order::WordOrder, String> {
    s.run(word_order::word_order)
}

/// PHOIBLE's inventories of a language and its relatives' phonemes (src/phoible.rs).
/// Which of a language's relatives have one phoneme, on a map (src/phoible.rs).
#[tauri::command(async)]
fn phoible_map(glottocode: String, phoneme: String) -> Result<typology::FeatureMap, String> {
    phoible::map(&glottocode, &phoneme)
}

#[tauri::command(async)]
fn phoible_compare(glottocode: String) -> Result<phoible::Compared, String> {
    phoible::compare(&glottocode)
}

/// What Grambank or WALS says about a language, beside its relatives (src/typology.rs).
#[tauri::command(async)]
fn typology_profile(glottocode: String, source: typology::Source) -> Result<typology::Profile, String> {
    typology::profile(&glottocode, source)
}

/// Where a language's relatives answer one Grambank or WALS feature, with the land to draw them on (src/typology.rs).
#[tauri::command(async)]
fn typology_map(glottocode: String, source: typology::Source, feature: String) -> Result<typology::FeatureMap, String> {
    typology::map(&glottocode, source, &feature)
}

/// The project's own answers to Grambank or WALS features (src/typology.rs).
#[tauri::command(async)]
fn typology_answers(s: State<SessionThread>, source: typology::Source) -> Result<Vec<typology::Ours>, String> {
    s.run(move |s| typology::answers(s, source))
}

/// The project's typology answers as a CLDF StructureDataset in a new folder.
#[tauri::command(async)]
fn export_typology(app: AppHandle, s: State<SessionThread>, source: typology::Source, folder: String, date: String) -> Result<typology::StructureExport, String> {
    let generator = format!("wugs {}", app.package_info().version);
    s.run(move |s| typology::export_structure(s, source, std::path::Path::new(&folder), &date, &generator))
}

/// Write (or clear) the project's answer to one feature.
#[tauri::command(async)]
fn set_typology_answer(s: State<SessionThread>, source: typology::Source, id: String, value: String, note: String) -> Result<typology::Ours, String> {
    s.run(move |s| typology::set_answer(s, source, &id, &value, &note))
}

/// The project's orthography profile, what it says, and the tracks to convert (src/orthography.rs).
#[tauri::command(async)]
fn orthography_get(s: State<SessionThread>) -> Result<orthography::Stored, String> {
    s.run(orthography::stored)
}

/// Keep a profile as the project's.
#[tauri::command(async)]
fn orthography_save(s: State<SessionThread>, profile: String) -> Result<orthography::Check, String> {
    s.run(move |s| orthography::save(s, &profile))
}

/// A starting profile from a track's own letters.
#[tauri::command(async)]
fn orthography_draft(s: State<SessionThread>, tier_id: String, text_id: Option<String>, ignore_case: bool) -> Result<String, String> {
    s.run(move |s| orthography::draft(s, &tier_id, text_id, ignore_case))
}

/// What converting a track with a profile would write.
#[tauri::command(async)]
fn orthography_preview(s: State<SessionThread>, profile: String, tier_id: String, text_id: Option<String>, column: String, ignore_case: bool) -> Result<orthography::Preview, String> {
    s.run(move |s| orthography::preview(s, &profile, &tier_id, text_id, &column, ignore_case))
}

/// The lexicon's headwords through a profile as written pronunciations (counted, or written).
#[tauri::command(async)]
fn orthography_pronounce(s: State<SessionThread>, profile: String, column: String, ignore_case: bool, write: bool) -> Result<orthography::Pronounced, String> {
    s.run(move |s| orthography::pronounce(s, &profile, &column, ignore_case, write))
}

/// A track's segments with their transcriptions, for a tool that converts them itself.
#[tauri::command]
fn track_sources(s: State<SessionThread>, tier_id: String, text_id: Option<String>) -> Result<Vec<orthography::TrackLine>, String> {
    s.run(move |s| orthography::track_sources(s, &tier_id, text_id))
}

/// Rows another tool converted from a track, written onto a transcription track.
#[tauri::command(async)]
fn write_converted_track(s: State<SessionThread>, tier_id: String, target: String, rows: Vec<orthography::TrackLine>, by: String) -> Result<orthography::Converted, String> {
    s.run(move |s| orthography::write_rows(s, &tier_id, &target, rows, &by))
}

/// Convert a track with a profile onto a transcription track.
#[tauri::command(async)]
fn orthography_convert(s: State<SessionThread>, profile: String, tier_id: String, text_id: Option<String>, column: String, ignore_case: bool, target: String) -> Result<orthography::Converted, String> {
    s.run(move |s| orthography::convert(s, &profile, &tier_id, text_id, &column, ignore_case, &target))
}

/// Where Glottolog places a language and its nearest languages, for a grammar sketch's locator map.
#[tauri::command(async)]
fn glottolog_place(glottocode: String) -> Option<glottolog::Place> {
    glottolog::place(&glottocode, 8)
}

/// Glottolog's languages and dialects a typed name or code fits, best first.
#[tauri::command(async)]
fn glottolog_find(query: String) -> Vec<glottolog::Languoid> {
    glottolog::find(&query, 8)
}

/// The lexicon as a dictionary to print or share (one HTML file in `folder`).
#[tauri::command(async)]
fn export_dictionary(s: State<SessionThread>, folder: String, date: String, include_restricted: bool) -> Result<dictionary::Dictionary, String> {
    s.run(move |s| dictionary::export_dictionary(s, std::path::Path::new(&folder), &date, include_restricted))
}

/// Every text as one CWB corpus (vertical file, encode.sh, README) in a new folder inside `folder`.
#[tauri::command(async)]
fn export_corpus(s: State<SessionThread>, folder: String, date: String, include_restricted: bool) -> Result<cwb::CorpusExported, String> {
    s.run(move |s| cwb::export_corpus(s, std::path::Path::new(&folder), &date, include_restricted))
}

/// Publish the lexicon and glossed sentences as a CLDF dataset in a new folder inside `folder`.
#[tauri::command(async)]
fn export_cldf(app: AppHandle, s: State<SessionThread>, folder: String, date: String, include_restricted: bool, audio: Option<bool>) -> Result<interchange::Published, String> {
    let generator = format!("wugs {}", app.package_info().version);
    s.run(move |s| interchange::export_cldf(s, std::path::Path::new(&folder), &date, include_restricted, audio.unwrap_or(false), &generator))
}

/// Export the elicited word lists (one, or all) as a CLDF Wordlist with an EDICTOR table, in a new folder inside `folder`.
#[tauri::command(async)]
fn export_wordlist(app: AppHandle, s: State<SessionThread>, folder: String, date: String, list_id: Option<String>) -> Result<wordlist::WordlistExport, String> {
    let generator = format!("wugs {}", app.package_info().version);
    s.run(move |s| wordlist::export(s, std::path::Path::new(&folder), &date, list_id.as_deref(), &generator))
}

/// Save a document the frontend built (interlinear examples) into `folder`.
#[tauri::command(async)]
fn save_document(s: State<SessionThread>, folder: String, title: String, extension: String, contents: String) -> Result<interchange::Exported, String> {
    s.run(move |s| interchange::save_document(s, std::path::Path::new(&folder), &title, &extension, &contents))
}

/// Seal the project as an archive package in a new folder inside `folder`.
#[tauri::command(async)]
fn seal_project(s: State<SessionThread>, folder: String, date: String) -> Result<interchange::Sealed, String> {
    s.run(move |s| interchange::seal_project(s, std::path::Path::new(&folder), &date))
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
fn add_translation(s: State<SessionThread>, text_id: String, tier_id: Option<String>, segment_id: String, content: String) -> Result<String, String> {
    s.run(move |s| session::add_translation(s, &text_id, tier_id.as_deref(), &segment_id, &content))
}

#[tauri::command(async)]
fn set_segment_times(
    s: State<SessionThread>,
    text_id: String,
    segment_id: String,
    start_ms: Option<f64>,
    end_ms: Option<f64>,
    expect: Option<Vec<session::SegmentCheckView>>,
) -> Result<Vec<String>, String> {
    s.run(move |s| match expect {
        Some(e) => session::set_segment_times_checked(s, &text_id, &segment_id, start_ms, end_ms, &e),
        None => session::set_segment_times(s, &text_id, &segment_id, start_ms, end_ms),
    })
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
    let file = std::path::Path::new(&path);
    let label = file.file_name().and_then(|n| n.to_str()).unwrap_or("Recording").to_string();
    import_recording(&s, file, label, None)
}

/// A recording picked or dropped: a WAV goes in as it is, anything else is
/// decoded to a WAV first. `expected_root`: the project a dropped file was
/// meant for, checked in the same session-thread call that imports, so a
/// project opened during a long decode gets nothing.
fn import_recording(s: &SessionThread, file: &std::path::Path, label: String, expected_root: Option<std::path::PathBuf>) -> Result<SourceRow, String> {
    // The open project's blobs/ is already in the asset scope: every way a
    // project opens (open, create, join) passes through allow_media.
    if convert::is_wav(file) {
        let (wav, label, expected) = (file.to_path_buf(), label.clone(), expected_root.clone());
        match s.run(move |s| session::import_media_named(s, &wav, &label, expected.as_deref())) {
            // 8-bit, A-law, μ-law and ADPCM WAVs are decoded like compressed audio.
            Err(e) if e.contains("unsupported WAV") => {}
            done => return done,
        }
    }
    // Compressed audio is decoded to a WAV in the project's cache, off the
    // session thread, then imported like any WAV under its own file name.
    // A drop meant for a project that is no longer open is refused before
    // minutes of decoding; the check inside the import below stays the guard.
    let root = match expected_root.clone() {
        Some(expected) => {
            s.run(move |s| session::require_root(s, &expected))?;
            s.run(session::project_root)?
        }
        None => s.run(session::project_root)?,
    };
    let dir = root.join("cache").join("convert");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    convert::sweep(&dir);
    let wav = dir.join(format!("{}.wav", wugs_model::ulid()));
    convert::to_wav(file, &wav)?;
    let converted = wav.clone();
    let row = s.run(move |s| session::import_media_named(s, &converted, &label, expected_root.as_deref()));
    std::fs::remove_file(&wav).ok();
    row
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Dropped {
    Recording { source: SourceRow },
    File { imported: interchange::Imported },
}

/// Start copying a dropped file into `project`, the project open when it
/// was dropped; refused if another one is open now. Returns the drop's id.
#[tauri::command(async)]
fn drop_begin(s: State<SessionThread>, drops: State<drops::Drops>, project: String, name: String, size: u64) -> Result<String, String> {
    let root = s.run(move |s| session::require_root(s, std::path::Path::new(&project)).and_then(|()| session::project_root(s)))?;
    drops.begin(&root, &name, size)
}

/// The body is the next piece of the file, raw; the drop id rides in a header.
#[tauri::command(async)]
fn drop_chunk(drops: State<drops::Drops>, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("drop_chunk expects a binary body".into());
    };
    let id = request
        .headers()
        .get("x-drop-id")
        .and_then(|v| v.to_str().ok())
        .ok_or("drop_chunk is missing the x-drop-id header")?;
    drops.chunk(id, bytes)
}

/// Import the whole dropped file the way a picked one is imported, then
/// remove the staged copy whatever happened.
#[tauri::command(async)]
fn drop_finish(s: State<SessionThread>, drops: State<drops::Drops>, drop_id: String) -> Result<Dropped, String> {
    let staged = drops.finish(&drop_id)?;
    let dir = staged.dir.clone();
    let result = import_dropped(&s, staged);
    std::fs::remove_dir_all(&dir).ok();
    result
}

/// Every import below checks, in the call that writes, that the project
/// the file was dropped into is still the open one.
fn import_dropped(s: &SessionThread, staged: drops::Staged) -> Result<Dropped, String> {
    match drops::route(&staged.path) {
        drops::Route::Recording => {
            import_recording(s, &staged.path, staged.name.clone(), Some(staged.root.clone())).map(|source| Dropped::Recording { source })
        }
        drops::Route::File => s.run(move |s| drops::import_file(s, &staged)).map(|imported| Dropped::File { imported }),
    }
}

#[tauri::command(async)]
fn drop_cancel(drops: State<drops::Drops>, drop_id: String) {
    drops.cancel(&drop_id);
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

/// A recording's waveform overview for its media bin card, made off the session thread.
#[tauri::command]
async fn media_overview(s: State<'_, SessionThread>, source_id: String) -> Result<media_bin::Overview, String> {
    let Some(at) = s.run(move |s| media_bin::locate(s, &source_id))? else {
        return Ok(media_bin::Overview::Missing);
    };
    tauri::async_runtime::spawn_blocking(move || media_bin::overview(&at)).await.map_err(|e| e.to_string())?
}

/// A recording's peak, loudness, noise floor and clips, read off the
/// session thread; None when its audio is not on this computer.
#[tauri::command]
async fn recording_levels(s: State<'_, SessionThread>, source_id: String) -> Result<Option<levels::Levels>, String> {
    let Some(at) = s.run(move |s| media_bin::locate(s, &source_id))? else {
        return Ok(None);
    };
    tauri::async_runtime::spawn_blocking(move || levels::levels(&at.media)).await.map_err(|e| e.to_string())?.map(Some)
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
    expect: Option<Vec<session::SegmentCheckView>>,
) -> Result<EditResult, String> {
    s.run(move |s| match expect {
        Some(e) => session::edit_segment_content_checked(s, &text_id, &segment_id, &content, &e),
        None => session::edit_segment_content(s, &text_id, &segment_id, &content),
    })
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
fn gloss_word_types(s: State<SessionThread>, text_id: Option<String>) -> Result<gloss_types::WordTypes, String> {
    s.run(move |s| gloss_types::word_types(s, text_id.as_deref()))
}

#[tauri::command(async)]
fn likely_meanings(s: State<SessionThread>, forms: Vec<String>) -> Result<Vec<Vec<meanings::Meaning>>, String> {
    s.run(move |s| {
        let p = session::project(s)?;
        meanings::likely_meanings(p, &concordance::Scope::all(), &forms)
    })
}

#[tauri::command(async)]
fn gloss_break_all(s: State<SessionThread>, text_id: Option<String>) -> Result<usize, String> {
    s.run(move |s| gloss_types::break_all(s, text_id.as_deref()))
}

#[tauri::command(async)]
fn gloss_apply_type(s: State<SessionThread>, form: String, morphs: Vec<gloss::MorphSpecView>) -> Result<gloss_types::TypeApplied, String> {
    s.run(move |s| gloss_types::apply_type(s, &form, morphs))
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
fn gloss_add_topic(s: State<SessionThread>, text_id: String, segment_id: String, name: String) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_add_topic(s, &text_id, &segment_id, &name))
}

#[tauri::command(async)]
fn gloss_remove_topic(s: State<SessionThread>, text_id: String, segment_id: String, name: String) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_remove_topic(s, &text_id, &segment_id, &name))
}

/// Every topic with its sentences (`topics.rs`).
#[tauri::command(async)]
fn topics_overview(s: State<SessionThread>) -> Result<topics::Topics, String> {
    s.run(topics::overview)
}

/// File the sentences of the Concordance's lines under a topic.
#[tauri::command(async)]
fn file_under_topic(s: State<SessionThread>, lines: Vec<topics::LineRef>, name: String) -> Result<topics::Filed, String> {
    s.run(move |s| topics::file_lines(s, &lines, &name))
}

/// Rename a topic everywhere, or fold it into another.
#[tauri::command(async)]
fn rename_topic(s: State<SessionThread>, from: String, to: String) -> Result<topics::Topics, String> {
    s.run(move |s| topics::rename(s, &from, &to))
}

#[tauri::command(async)]
fn gloss_add_note(s: State<SessionThread>, text_id: String, segment_id: String, body: String) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_add_note(s, &text_id, &segment_id, &body))
}

#[tauri::command(async)]
fn gloss_set_note(s: State<SessionThread>, text_id: String, segment_id: String, note_id: String, body: String) -> Result<GlossEdit, String> {
    s.run(move |s| gloss::gloss_set_note(s, &text_id, &segment_id, &note_id, &body))
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

/// The project's own paradigm templates (`src/paradigm_templates.rs`).
#[tauri::command(async)]
fn paradigm_templates(s: State<SessionThread>) -> Result<Vec<paradigm_templates::Template>, String> {
    s.run(paradigm_templates::templates)
}

/// Keep a paradigm template for the project; the whole list back.
#[tauri::command(async)]
fn save_paradigm_template(s: State<SessionThread>, template: paradigm_templates::Template) -> Result<Vec<paradigm_templates::Template>, String> {
    s.run(move |s| paradigm_templates::save(s, template))
}

/// Remove one of the project's paradigm templates; the whole list back.
#[tauri::command(async)]
fn delete_paradigm_template(s: State<SessionThread>, id: String) -> Result<Vec<paradigm_templates::Template>, String> {
    s.run(move |s| paradigm_templates::delete(s, &id))
}

#[tauri::command(async)]
fn lexicon_paradigm(s: State<SessionThread>, id: String, axes: Vec<lexicon::Axis>) -> Result<lexicon::ParadigmView, String> {
    s.run(move |s| lexicon::lexicon_paradigm(s, &id, &axes))
}

#[tauri::command(async)]
fn lexicon_fill_cell(
    s: State<SessionThread>,
    id: String,
    form_id: Option<String>,
    features: std::collections::BTreeMap<String, String>,
    surface: String,
    axes: Vec<lexicon::Axis>,
) -> Result<lexicon::ParadigmEdit, String> {
    s.run(move |s| lexicon::lexicon_fill_cell(s, &id, form_id.as_deref(), &features, &surface, &axes))
}

#[tauri::command(async)]
fn lexicon_delete_form(s: State<SessionThread>, id: String, form_id: String, axes: Vec<lexicon::Axis>) -> Result<lexicon::ParadigmEdit, String> {
    s.run(move |s| lexicon::lexicon_delete_form(s, &id, &form_id, &axes))
}

// People and consent (roadmap 61): src/people.rs. Each returns the whole view.
#[tauri::command(async)]
fn people_view(s: State<SessionThread>) -> Result<people::PeopleView, String> {
    s.run(people::view)
}
#[tauri::command(async)]
fn person_add(s: State<SessionThread>, name: String, role: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::add_person(s, &name, &role))
}
#[tauri::command(async)]
fn person_set_name(s: State<SessionThread>, person: String, name: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::set_name(s, &person, &name))
}
#[tauri::command(async)]
fn person_set_roles(s: State<SessionThread>, person: String, roles: Vec<String>) -> Result<people::PeopleView, String> {
    s.run(move |s| people::set_roles(s, &person, &roles))
}
#[tauri::command(async)]
fn field_sessions_view(s: State<SessionThread>) -> Result<field_sessions::SessionsView, String> {
    s.run(field_sessions::view)
}
#[tauri::command(async)]
fn field_session_add(s: State<SessionThread>, date: String) -> Result<field_sessions::SessionsView, String> {
    s.run(move |s| field_sessions::add(s, &date))
}
#[tauri::command(async)]
fn field_session_set(s: State<SessionThread>, session: String, field: String, value: String) -> Result<field_sessions::SessionsView, String> {
    s.run(move |s| field_sessions::set(s, &session, &field, &value))
}
#[tauri::command(async)]
fn field_session_person(s: State<SessionThread>, session: String, person: String, add: bool) -> Result<field_sessions::SessionsView, String> {
    s.run(move |s| field_sessions::person(s, &session, &person, add))
}
#[tauri::command(async)]
fn field_session_delete(s: State<SessionThread>, session: String) -> Result<field_sessions::SessionsView, String> {
    s.run(move |s| field_sessions::delete(s, &session))
}
#[tauri::command(async)]
fn person_set_detail(s: State<SessionThread>, person: String, key: String, value: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::set_detail(s, &person, &key, &value))
}
#[tauri::command(async)]
fn person_delete(s: State<SessionThread>, person: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::delete_person(s, &person))
}
#[tauri::command(async)]
fn consent_add(s: State<SessionThread>, person: String, covers: String, date: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::add_consent(s, &person, &covers, &date))
}
#[tauri::command(async)]
fn consent_set_term(s: State<SessionThread>, consent: String, term: String, answer: Option<bool>) -> Result<people::PeopleView, String> {
    s.run(move |s| people::set_term(s, &consent, &term, answer))
}
#[tauri::command(async)]
fn consent_set_covers(s: State<SessionThread>, consent: String, covers: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::set_covers(s, &consent, &covers))
}
#[tauri::command(async)]
fn consent_set_date(s: State<SessionThread>, consent: String, date: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::set_date(s, &consent, &date))
}
#[tauri::command(async)]
fn consent_attach(s: State<SessionThread>, consent: String, path: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::attach(s, &consent, &path))
}
#[tauri::command(async)]
fn consent_detach(s: State<SessionThread>, consent: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::detach(s, &consent))
}
#[tauri::command(async)]
fn consent_delete(s: State<SessionThread>, consent: String) -> Result<people::PeopleView, String> {
    s.run(move |s| people::delete_consent(s, &consent))
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

/// Merge one entry into another: senses, forms, pronunciations and relations move over.
#[tauri::command(async)]
fn lexicon_merge(s: State<SessionThread>, keep: String, other: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_merge(s, &keep, &other))
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

/// Copy a picture into the project and show it with a sense (a picture dictionary).
#[tauri::command(async)]
fn lexicon_set_picture(s: State<SessionThread>, sense: String, path: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_set_picture(s, &sense, &path))
}

#[tauri::command(async)]
fn lexicon_clear_picture(s: State<SessionThread>, sense: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_clear_picture(s, &sense))
}

#[tauri::command(async)]
fn lexicon_add_allomorph(s: State<SessionThread>, id: String, form: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_add_allomorph(s, &id, &form))
}

#[tauri::command(async)]
fn lexicon_add_example(s: State<SessionThread>, sense_id: String, sentence: String, translation: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_add_example(s, &sense_id, &sentence, &translation))
}

#[tauri::command(async)]
fn lexicon_example_from(s: State<SessionThread>, sense_id: String, segment_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_example_from(s, &sense_id, &segment_id))
}

#[tauri::command(async)]
fn lexicon_set_example(s: State<SessionThread>, example_id: String, field: String, text: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_set_example(s, &example_id, &field, &text))
}

#[tauri::command(async)]
fn lexicon_delete_example(s: State<SessionThread>, example_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_delete_example(s, &example_id))
}

#[tauri::command(async)]
fn lexicon_delete_allomorph(s: State<SessionThread>, id: String, form_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_delete_allomorph(s, &id, &form_id))
}

#[tauri::command(async)]
fn lexicon_add_pronunciation(s: State<SessionThread>, id: String, form: String, segment_id: Option<String>) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_add_pronunciation(s, &id, &form, segment_id.as_deref()))
}

#[tauri::command(async)]
fn lexicon_set_pronunciation(s: State<SessionThread>, id: String, pronunciation_id: String, form: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_set_pronunciation(s, &id, &pronunciation_id, &form))
}

#[tauri::command(async)]
fn lexicon_pronunciation_audio(s: State<SessionThread>, id: String, pronunciation_id: String, segment_id: Option<String>) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_pronunciation_audio(s, &id, &pronunciation_id, segment_id.as_deref()))
}

#[tauri::command(async)]
fn lexicon_delete_pronunciation(s: State<SessionThread>, id: String, pronunciation_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_delete_pronunciation(s, &id, &pronunciation_id))
}

#[tauri::command(async)]
fn lexicon_add_relation(s: State<SessionThread>, id: String, relation: String, target: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_add_relation(s, &id, &relation, &target))
}

#[tauri::command(async)]
fn lexicon_delete_relation(s: State<SessionThread>, id: String, relation_id: String) -> Result<lexicon::EntryEdit, String> {
    s.run(move |s| lexicon::lexicon_delete_relation(s, &id, &relation_id))
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

/// Load a word list of one's own from a CSV or tab-separated table.
#[tauri::command(async)]
fn concept_list_import(s: State<SessionThread>, path: String) -> Result<elicit::OwnList, String> {
    s.run(move |s| elicit::concept_list_import(s, std::path::Path::new(&path)))
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
fn elicit_answer(s: State<SessionThread>, concept_id: String, form: String, gloss: String, segment_id: Option<String>) -> Result<elicit::ConceptEdit, String> {
    s.run(move |s| elicit::elicit_answer(s, &concept_id, &form, &gloss, segment_id.as_deref()))
}

#[tauri::command(async)]
fn elicit_link(s: State<SessionThread>, concept_id: String, sense_id: String, segment_id: Option<String>) -> Result<elicit::ConceptEdit, String> {
    s.run(move |s| elicit::elicit_link(s, &concept_id, &sense_id, segment_id.as_deref()))
}

#[tauri::command(async)]
fn elicit_unlink(s: State<SessionThread>, concept_id: String, sense_id: String) -> Result<elicit::ConceptEdit, String> {
    s.run(move |s| elicit::elicit_unlink(s, &concept_id, &sense_id))
}

/// Entries glossed like a concept no one has answered yet, offered for linking.
#[tauri::command(async)]
fn elicit_gloss_matches(s: State<SessionThread>, list_id: String) -> Result<Vec<elicit::GlossMatch>, String> {
    s.run(move |s| elicit::gloss_matches(s, &list_id))
}

/// Link the chosen (sense, concept) pairs in one txn.
#[tauri::command(async)]
fn elicit_link_matches(s: State<SessionThread>, list_id: String, pairs: Vec<(String, String)>) -> Result<elicit::ListView, String> {
    s.run(move |s| elicit::link_matches(s, &list_id, &pairs))
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
/// 10 s chunks, from whatever powers acoustic analysis (Praat, cached per
/// chunk, else the built-in pitch and intensity).
#[tauri::command]
async fn acoustic_analysis(s: State<'_, SessionThread>, dir: State<'_, AppData>, source: String, chunks: Vec<u32>) -> Result<Vec<praat_analysis::Chunk>, String> {
    let cap = capability::Capability::AcousticAnalysis;
    let (job, pin) = s.run(move |s| {
        let job = praat_analysis::job(s, &source)?;
        Ok((job, capability::project_pin(session::project(s)?, cap)?))
    })?;
    // Either can take seconds: on a blocking thread, not an async worker or
    // the session thread.
    let provider = capability::resolve(cap, pin.as_deref(), &dir.0)?;
    if chunks.len() > praat_analysis::MAX_CHUNKS {
        return Err(format!("at most {} chunks at a time", praat_analysis::MAX_CHUNKS));
    }
    tauri::async_runtime::spawn_blocking(move || measure::analyse(&provider, &job, &chunks)).await.map_err(|e| e.to_string())?
}

/// A recording's voice for acoustic analysis (pitch range, formant ceiling).
#[tauri::command(async)]
fn acoustic_voice(s: State<SessionThread>, source: String) -> Result<praat_analysis::VoiceView, String> {
    s.run(move |s| praat_analysis::voice_view(s, &source))
}

#[tauri::command(async)]
fn set_acoustic_voice(s: State<SessionThread>, source: String, voice: String) -> Result<praat_analysis::VoiceView, String> {
    s.run(move |s| praat_analysis::set_voice(s, &source, &voice))
}

/// The texts and dictionary as a folder of web pages (`website.rs`).
#[tauri::command(async)]
fn export_website(s: State<SessionThread>, folder: String, date: String, include_restricted: bool, audio: bool) -> Result<website::Site, String> {
    s.run(move |s| website::export_website(s, std::path::Path::new(&folder), &date, include_restricted, audio))
}

/// Senses with no example sentence, and how many could take one from a text.
#[tauri::command(async)]
fn lexicon_example_gaps(s: State<SessionThread>) -> Result<lexicon::ExampleGaps, String> {
    s.run(lexicon::example_gaps)
}

/// Give each sense with no example its first use in an open text.
#[tauri::command(async)]
fn lexicon_examples_from_texts(s: State<SessionThread>) -> Result<usize, String> {
    s.run(lexicon::examples_from_texts)
}

/// The project's typing shortcuts (`typing.rs`).
#[tauri::command(async)]
fn typing_shortcuts(s: State<SessionThread>) -> Result<String, String> {
    s.run(typing::get)
}

#[tauri::command(async)]
fn set_typing_shortcuts(s: State<SessionThread>, table: String) -> Result<String, String> {
    s.run(move |s| typing::set(s, &table))
}

/// What each text still lacks for an archive deposit (`archive_sheet::readiness`).
#[tauri::command(async)]
fn archive_readiness(s: State<SessionThread>) -> Result<archive_sheet::Readiness, String> {
    s.run(archive_sheet::readiness)
}

/// One catalogue row per text for an archive's item forms (`archive_sheet.rs`).
#[tauri::command(async)]
fn export_archive_sheet(s: State<SessionThread>, folder: String) -> Result<archive_sheet::Sheet, String> {
    s.run(move |s| archive_sheet::export_sheet(s, std::path::Path::new(&folder)))
}

/// The lexicon as flashcards for Anki and Quizlet (`flashcards.rs`).
#[tauri::command(async)]
fn export_flashcards(s: State<SessionThread>, folder: String, date: String, include_restricted: bool, order: Option<flashcards::Order>) -> Result<flashcards::Flashcards, String> {
    s.run(move |s| flashcards::export_flashcards(s, std::path::Path::new(&folder), &date, include_restricted, order.unwrap_or(flashcards::Order::Alphabet)))
}

/// A text's page images and the areas its segments stand on (`facsimile.rs`).
#[tauri::command]
fn facsimile_view(s: State<SessionThread>, text: String) -> Result<facsimile::FacsimileView, String> {
    s.run(move |s| facsimile::view(s, &text))
}

#[tauri::command(async)]
fn facsimile_add_page(s: State<SessionThread>, text: String, path: String) -> Result<facsimile::FacsimileView, String> {
    s.run(move |s| facsimile::add_page(s, &text, &path))
}

#[tauri::command(async)]
fn facsimile_page_image(s: State<SessionThread>, text: String, source: String, path: String) -> Result<facsimile::FacsimileView, String> {
    s.run(move |s| facsimile::set_page_image(s, &text, &source, &path))
}

#[tauri::command]
fn facsimile_remove_page(s: State<SessionThread>, text: String, source: String) -> Result<facsimile::FacsimileView, String> {
    s.run(move |s| facsimile::remove_page(s, &text, &source))
}

#[tauri::command]
fn facsimile_move_page(s: State<SessionThread>, text: String, source: String, by: i32) -> Result<facsimile::FacsimileView, String> {
    s.run(move |s| facsimile::move_page(s, &text, &source, by))
}

#[tauri::command]
fn facsimile_add_line(s: State<SessionThread>, text: String, area: facsimile::Area) -> Result<facsimile::NewLine, String> {
    s.run(move |s| facsimile::add_line(s, &text, area))
}

#[tauri::command]
fn facsimile_set_area(s: State<SessionThread>, text: String, segment: String, area: Option<facsimile::Area>) -> Result<facsimile::FacsimileView, String> {
    s.run(move |s| facsimile::set_area(s, &text, &segment, area))
}

/// Word, morph and gloss tiers an ELAN file brought, which could become the text's glossing (`elan_glossing.rs`).
#[tauri::command(async)]
fn tier_glossing(s: State<SessionThread>, text_id: String) -> Result<Vec<elan_glossing::TierGlossing>, String> {
    s.run(move |s| elan_glossing::find(s, &text_id))
}

/// Read one set of those tiers as the text's tokens and morphs, and take the tiers away.
#[tauri::command(async)]
fn read_tier_glossing(s: State<SessionThread>, text_id: String, index: usize) -> Result<elan_glossing::ReadGlossing, String> {
    s.run(move |s| elan_glossing::read(s, &text_id, index))
}

/// Sentences and translations as a parallel corpus (`parallel.rs`).
#[tauri::command(async)]
fn export_parallel(s: State<SessionThread>, folder: String, date: String, include_restricted: bool) -> Result<parallel::Parallel, String> {
    s.run(move |s| parallel::export_parallel(s, std::path::Path::new(&folder), &date, include_restricted))
}

/// A Hunspell spelling dictionary of the language (`spelling.rs`).
#[tauri::command(async)]
fn export_spelling(s: State<SessionThread>, folder: String, date: String) -> Result<spelling::Spelling, String> {
    s.run(move |s| spelling::export_spelling(s, std::path::Path::new(&folder), &date))
}

/// Every sentence a find and replace would change, before and after.
#[tauri::command(async)]
fn replace_preview(s: State<SessionThread>, find: replace::Find) -> Result<replace::Preview, String> {
    s.run(move |s| replace::preview(s, &find))
}

/// Write the chosen changes, each only where its sentence still reads as previewed.
#[tauri::command(async)]
fn replace_apply(s: State<SessionThread>, changes: Vec<replace::Change>) -> Result<replace::Applied, String> {
    s.run(move |s| replace::apply(s, changes))
}

/// Measure every timed segment of one track of a text against `source`'s
/// recording (duration, pitch, intensity, formants) and write the table as a
/// CSV file into `folder`; the analysis runs off the session thread.
#[tauri::command]
async fn measure_track(s: State<'_, SessionThread>, dir: State<'_, AppData>, text_id: String, tier_id: String, source: String, folder: String) -> Result<measure::MeasureReport, String> {
    let into = std::path::PathBuf::from(&folder);
    let prepared = s.run(move |s| {
        interchange::outside_project(s, std::path::Path::new(&folder))?;
        measure::prepare(s, &text_id, &tier_id, &source)
    })?;
    let app_data = dir.0.clone();
    tauri::async_runtime::spawn_blocking(move || measure::run(prepared, &app_data, &into)).await.map_err(|e| e.to_string())?
}

/// Save each timed segment of a track as its own WAV, with a `clips.csv`
/// index, in a new folder inside `folder`; the cutting runs off the session thread.
#[tauri::command]
async fn save_clips(s: State<'_, SessionThread>, text_id: String, tier_id: String, source: String, folder: String) -> Result<clips::ClipsReport, String> {
    let into = std::path::PathBuf::from(&folder);
    let prepared = s.run(move |s| {
        interchange::outside_project(s, std::path::Path::new(&folder))?;
        clips::prepare(s, &text_id, &tier_id, &source)
    })?;
    tauri::async_runtime::spawn_blocking(move || clips::write(prepared, &into)).await.map_err(|e| e.to_string())?
}

/// Save the sentences behind concordance lines as WAVs, each cut from its
/// text's recording, with a `clips.csv` index; the cutting runs off the session thread.
#[tauri::command]
async fn save_hit_clips(s: State<'_, SessionThread>, query: String, hits: Vec<clips::HitRef>, folder: String) -> Result<clips::ClipsReport, String> {
    let into = std::path::PathBuf::from(&folder);
    let prepared = s.run(move |s| {
        interchange::outside_project(s, std::path::Path::new(&folder))?;
        clips::prepare_hits(s, &query, &hits)
    })?;
    tauri::async_runtime::spawn_blocking(move || clips::write(prepared, &into)).await.map_err(|e| e.to_string())?
}

/// The F1/F2 of every timed segment of a track, for the Vowel space panel.
#[tauri::command]
async fn measure_points(s: State<'_, SessionThread>, dir: State<'_, AppData>, text_id: String, tier_id: String, source: String) -> Result<measure::PointsView, String> {
    let prepared = s.run(move |s| measure::prepare(s, &text_id, &tier_id, &source))?;
    let app_data = dir.0.clone();
    tauri::async_runtime::spawn_blocking(move || measure::points(&prepared, &app_data)).await.map_err(|e| e.to_string())?
}

/// The spectrogram of ten seconds of `source`'s recording (chunk `index`),
/// as raw bytes (`src/spectrogram.rs` says the layout); computed off the
/// session thread.
#[tauri::command]
async fn spectrogram(s: State<'_, SessionThread>, source: String, index: u32, narrow: Option<bool>) -> Result<tauri::ipc::Response, String> {
    let job = s.run(move |s| praat_analysis::job(s, &source))?;
    let narrow = narrow.unwrap_or(false);
    let bytes = tauri::async_runtime::spawn_blocking(move || spectrogram::chunk(&job.wav, index, narrow)).await.map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Resolve what powers segmentation here (the project's pin, else this
/// computer's choice) and find the speech spans of `source`'s recording off
/// the session thread (cached per recording).
async fn segmentation_spans(s: &SessionThread, dir: &AppData, source: String, granularity: &str) -> Result<Vec<(f64, f64)>, String> {
    let granularity = segmentation::granularity(granularity)?;
    let cap = capability::Capability::Segmentation;
    let (job, pin) = s.run(move |s| {
        let job = segmentation::job(s, &source)?;
        Ok((job, capability::project_pin(session::project(s)?, cap)?))
    })?;
    let provider = capability::resolve(cap, pin.as_deref(), &dir.0)?;
    tauri::async_runtime::spawn_blocking(move || segmentation::spans(&provider, &job, granularity)).await.map_err(|e| e.to_string())?
}

/// Where speech starts and ends in `source`'s recording, at `granularity`
/// (`word` for edge snapping, `utterance`).
#[tauri::command]
async fn speech_spans(s: State<'_, SessionThread>, dir: State<'_, AppData>, source: String, granularity: String) -> Result<Vec<[f64; 2]>, String> {
    Ok(segmentation_spans(&s, &dir, source, &granularity).await?.into_iter().map(|(a, b)| [a, b]).collect())
}

/// What tagging needs from this computer and this project: UDPipe's
/// program and model as chosen, and whether the project allows the web service.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MorphosyntaxSetup {
    local: morphosyntax::LocalSettings,
    spacy: morphosyntax::SpacySettings,
    stanza: morphosyntax::SpacySettings,
    web_ok: bool,
}

#[tauri::command(async)]
fn morphosyntax_setup(s: State<SessionThread>, dir: State<AppData>) -> Result<MorphosyntaxSetup, String> {
    let web_ok = s.run(|s| Ok(session::project(s).ok().map(|p| p.setting(morphosyntax::WEB_OK)).transpose()?.flatten().as_deref() == Some("yes")))?;
    Ok(MorphosyntaxSetup { local: morphosyntax::local_settings(&dir.0), spacy: morphosyntax::spacy_settings(&dir.0), stanza: morphosyntax::py_settings(&dir.0, morphosyntax::PyTool::Stanza), web_ok })
}

/// Choose spaCy's Python (None: python3 or python on PATH) and pipeline on
/// this computer; the pipeline is loaded once to check it (an empty one clears it).
#[tauri::command]
async fn morphosyntax_choose_spacy(dir: State<'_, AppData>, python: Option<String>, pipeline: Option<String>) -> Result<morphosyntax::SpacySettings, String> {
    let app = dir.0.clone();
    tauri::async_runtime::spawn_blocking(move || morphosyntax::choose_spacy(&app, python, pipeline)).await.map_err(|e| e.to_string())?
}

/// Choose Stanza's Python and language on this computer; the models are
/// loaded once to check they are downloaded (an empty language clears it).
#[tauri::command]
async fn morphosyntax_choose_stanza(dir: State<'_, AppData>, python: Option<String>, language: Option<String>) -> Result<morphosyntax::SpacySettings, String> {
    let app = dir.0.clone();
    tauri::async_runtime::spawn_blocking(move || morphosyntax::choose_py(&app, morphosyntax::PyTool::Stanza, python, language)).await.map_err(|e| e.to_string())?
}

/// Choose UDPipe's program and model on this computer (None keeps the PATH lookup or clears the model).
#[tauri::command(async)]
fn morphosyntax_choose(dir: State<AppData>, program: Option<String>, model: Option<String>) -> Result<(), String> {
    morphosyntax::set_local_settings(&dir.0, &morphosyntax::LocalSettings { program, model })
}

/// Allow (or stop allowing) this project's open texts to go to the UDPipe web service.
#[tauri::command(async)]
fn morphosyntax_allow_web(s: State<SessionThread>, allow: bool) -> Result<(), String> {
    s.run(move |s| session::project(s)?.set_setting(morphosyntax::WEB_OK, if allow { "yes" } else { "" }).map(|_| ()))
}

/// Tag and parse a text's untagged sentences with the provider the
/// `morphosyntax` capability resolves to; `model` names the web service's
/// model or language (else the project's language).
#[tauri::command]
async fn morphosyntax_run(s: State<'_, SessionThread>, dir: State<'_, AppData>, text_id: String, model: String) -> Result<morphosyntax::Tagged, String> {
    let cap = capability::Capability::Morphosyntax;
    let pin = s.run(move |s| capability::project_pin(session::project(s)?, cap))?;
    let provider = capability::resolve(cap, pin.as_deref(), &dir.0)?;
    let web = matches!(provider, capability::Provider::UdpipeLindat);
    let id = text_id.clone();
    let (gathered, web_model) = s.run(move |s| {
        let g = morphosyntax::gather(s, &id, web)?;
        let m = if web { Some(morphosyntax::web_model(s, &model)?) } else { None };
        Ok((g, m))
    })?;
    if gathered.segments.is_empty() {
        return Ok(morphosyntax::Tagged { kept: gathered.kept, ..Default::default() });
    }
    let tagger = match (provider, web_model) {
        (capability::Provider::Udpipe(l), _) => morphosyntax::Tagger::Local(l),
        (capability::Provider::Python(sp), _) => morphosyntax::Tagger::Spacy(sp),
        (capability::Provider::UdpipeLindat, Some(model)) => morphosyntax::Tagger::Lindat { endpoint: morphosyntax::LINDAT.to_string(), model },
        _ => return Err("Nothing here can tag and parse.".into()),
    };
    let lines: Vec<String> = gathered.segments.iter().map(|(_, l)| l.clone()).collect();
    let (sentences, by) = tauri::async_runtime::spawn_blocking(move || morphosyntax::tag(&tagger, &lines)).await.map_err(|e| e.to_string())??;
    s.run(move |s| morphosyntax::write(s, &gathered, sentences, &by))
}

/// One machine segment per stretch of speech (per utterance or per word, by
/// `granularity`) in `source`'s recording on `tier` (the utterance tier
/// when None), skipping what is already covered.
#[tauri::command]
async fn auto_segment(s: State<'_, SessionThread>, dir: State<'_, AppData>, text_id: String, source: String, tier_id: Option<String>, granularity: String) -> Result<segmentation::AutoSegmented, String> {
    let spans = segmentation_spans(&s, &dir, source, &granularity).await?;
    s.run(move |s| segmentation::commit(s, &text_id, tier_id.as_deref(), &spans))
}

#[tauri::command(async)]
fn restore_segments(s: State<SessionThread>, text_id: String, segment_ids: Vec<String>) -> Result<session::SegmentsEdit, String> {
    s.run(move |s| session::restore_segments(s, &text_id, &segment_ids))
}

#[tauri::command(async)]
fn delete_segments(s: State<SessionThread>, text_id: String, expect: Vec<session::SegmentCheckView>, keep_changed: bool) -> Result<session::DeletedSegments, String> {
    s.run(move |s| session::delete_segments(s, &text_id, &expect, keep_changed))
}

#[tauri::command(async)]
fn split_segment(s: State<SessionThread>, text_id: String, segment_id: String, at_ms: f64) -> Result<AddedSegment, String> {
    s.run(move |s| session::split_segment(s, &text_id, &segment_id, at_ms))
}

#[tauri::command(async)]
fn merge_segments(s: State<SessionThread>, text_id: String, first_id: String, second_id: String, expect: Option<Vec<session::SegmentCheckView>>) -> Result<session::SegmentsEdit, String> {
    s.run(move |s| session::merge_segments(s, &text_id, &first_id, &second_id, &expect.unwrap_or_default()))
}

#[tauri::command(async)]
fn unmerge_segments(
    s: State<SessionThread>,
    text_id: String,
    first_id: String,
    second_id: String,
    first_end_ms: f64,
    first_content: Option<String>,
    expect: Vec<session::SegmentCheckView>,
) -> Result<session::SegmentsEdit, String> {
    s.run(move |s| session::unmerge_segments(s, &text_id, &first_id, &second_id, first_end_ms, first_content.as_deref(), &expect))
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

/// Set (or clear) one of a recording's details (date, place, recorded by…).
#[tauri::command(async)]
fn source_set_detail(s: State<SessionThread>, source: String, key: String, value: String) -> Result<Vec<SourceRow>, String> {
    s.run(move |session| session::set_source_detail(session, &source, &key, &value))
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

/// The technical details a feedback report would carry: the page's own
/// (kit, screen) plus the app version and system. Shown before sending.
#[tauri::command]
fn feedback_details(page: serde_json::Value) -> serde_json::Value {
    feedback::details(page)
}

/// Check a feedback report and keep it in the outbox (copying a video can
/// take a moment, so off the async workers). Returns its id; the form closes
/// and `feedback_deliver` sends it.
#[tauri::command]
async fn feedback_keep(
    dir: State<'_, AppData>,
    message: String,
    contact: String,
    details: Option<serde_json::Value>,
    attachment: Option<String>,
) -> Result<String, String> {
    let dir = dir.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let details = details.map(feedback::details);
        feedback::enqueue(&dir, &message, &contact, details, attachment.as_deref().map(std::path::Path::new))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ask the help service a question about wugs (Online). `None` when this
/// build has no service to ask, so the sheet can say so.
#[tauri::command]
async fn help_ask(question: String, history: Vec<help::Turn>, share: bool) -> Result<Option<help::Answer>, String> {
    if help::ENDPOINT.is_empty() {
        return Ok(None);
    }
    tauri::async_runtime::spawn_blocking(move || help::ask(help::ENDPOINT, &question, &history, share).map(Some))
        .await
        .map_err(|e| e.to_string())?
}

/// Send everything waiting and say what became of report `id`. Online;
/// offline, it waits and goes with the next send.
#[tauri::command]
async fn feedback_deliver(app: AppHandle, dir: State<'_, AppData>, id: String) -> Result<feedback::Outcome, String> {
    send_feedback(app, dir.0.clone(), Some(id)).await
}

/// Send reports that are waiting (at start, and when the computer comes online).
#[tauri::command]
async fn feedback_retry(app: AppHandle, dir: State<'_, AppData>) -> Result<feedback::Outcome, String> {
    send_feedback(app, dir.0.clone(), None).await
}

async fn send_feedback(app: AppHandle, dir: PathBuf, focus: Option<String>) -> Result<feedback::Outcome, String> {
    if feedback::ENDPOINT.is_empty() {
        return Ok(feedback::Outcome {
            sent: false,
            waiting: feedback::waiting(&dir).len(),
            problem: None,
            configured: false,
        });
    }
    tauri::async_runtime::spawn_blocking(move || {
        let sending = app.state::<feedback::Sending>();
        let _one_at_a_time = sending.0.lock().unwrap_or_else(|p| p.into_inner());
        feedback::send_waiting(&dir, &feedback::Https { endpoint: feedback::ENDPOINT.into() }, focus.as_deref())
    })
    .await
    .map_err(|e| e.to_string())
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
/// `flagged`: also include those the project's records speak against.
#[tauri::command(async)]
fn corrections_export(s: State<SessionThread>, id: State<Identity>, folder: String, flagged: bool, date: String) -> Result<corrections::Exported, String> {
    let key = id.key().ok();
    let (jobs, sum) = s.run(move |s| {
        let guest = session::is_guest(s, key.as_deref())?;
        corrections::prepare(s, guest, flagged)
    })?;
    corrections::write(&jobs, &sum, std::path::Path::new(&folder), &corrections::date_or_today(&date))
}

/// What a speech dataset export would hold now.
#[tauri::command(async)]
fn speech_dataset_summary(s: State<SessionThread>, id: State<Identity>) -> Result<speech_dataset::Summary, String> {
    let key = id.key().ok();
    s.run(move |s| {
        let guest = session::is_guest(s, key.as_deref())?;
        speech_dataset::summary(s, guest)
    })
}

/// Export every transcribed, timed segment as a speech dataset in a new
/// folder inside `folder`; the cutting runs off the session thread.
#[tauri::command]
async fn speech_dataset_export(s: State<'_, SessionThread>, id: State<'_, Identity>, folder: String, flagged: bool, date: String) -> Result<speech_dataset::Exported, String> {
    let key = id.key().ok();
    let into = std::path::PathBuf::from(&folder);
    let (jobs, sum) = s.run(move |s| {
        interchange::outside_project(s, std::path::Path::new(&folder))?;
        let guest = session::is_guest(s, key.as_deref())?;
        speech_dataset::prepare(s, guest, flagged)
    })?;
    let date = corrections::date_or_today(&date);
    tauri::async_runtime::spawn_blocking(move || speech_dataset::write(&jobs, &sum, &into, &date)).await.map_err(|e| e.to_string())?
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
    kits::install_file(&dir.0, std::path::Path::new(&path))
}

/// What installing a kit file would add, before anything is written.
#[tauri::command(async)]
fn kit_inspect(dir: State<AppData>, path: String) -> Result<kits::KitFileView, String> {
    kits::inspect_file(&dir.0, std::path::Path::new(&path))
}

/// Save a kit as a self-contained `.wugskit` in `folder`.
#[tauri::command(async)]
fn kit_share(dir: State<AppData>, id: String, folder: String) -> Result<String, String> {
    kits::share_file(&dir.0, &id, std::path::Path::new(&folder))
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
        // Copy example (roadmap 69) puts an HTML table and plain text on the clipboard.
        .plugin(tauri_plugin_clipboard_manager::init())
        // Size, position and maximized state come back on the next launch; a
        // machine with no saved state keeps the size from tauri.conf.json.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Online: checks GitHub Releases only when the webview asks (the
        // person's setting), downloads only on a click, verifies the
        // minisign signature against tauri.conf.json's key. The webview has
        // no updater permission; it goes through `updater::update_*`.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // The webview never leaves the app's own page. With native
        // drag-and-drop off (tauri.conf.json), WebView2 accepts a link
        // dragged in from a browser and would navigate to it, taking the
        // whole UI (and any take being recorded) with it.
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("stay-in-app")
                .on_navigation(|webview, url| {
                    let dev_url = if cfg!(debug_assertions) { webview.app_handle().config().build.dev_url.clone() } else { None };
                    capture::is_app_origin(url, dev_url.as_ref())
                })
                .build(),
        )
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
        .manage(drops::Drops::default())
        .manage(updater::Staged::default())
        .manage(assist::Downloads::default())
        .manage(feedback::Sending::default())
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
            feedback_details,
            feedback_keep,
            feedback_deliver,
            feedback_retry,
            help_ask,
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
            open_sample,
            concordance,
            concordance_query,
            dispersion,
            abbreviation_meanings,
            gloss_consistency,
            gloss_unify,
            gloss_segmentations,
            like_sentences,
            work_log,
            hit_categories,
            set_hit_categories,
            set_hit_category,
            gloss_use_breakdown,
            gloss_missing,
            gloss_sense_drift,
            gloss_settle_drift,
            consent_gaps,
            gloss_add_missing,
            set_abbreviation_meaning,
            text_metadata,
            set_text_metadata,
            import_metadata,
            metadata_values,
            morphosyntax_setup,
            morphosyntax_choose,
            morphosyntax_choose_spacy,
            morphosyntax_choose_stanza,
            glottolog_find,
            glottolog_place,
            typology_profile,
            typology_map,
            phoible_compare,
            phoible_map,
            cognates,
            sentence_lists,
            segment_timing,
            duplicate_sentences,
            sentence_list_import,
            sentence_list_make,
            sentence_list_view,
            sentence_answer,
            set_cognates,
            word_order,
            typology_answers,
            orthography_get,
            orthography_save,
            orthography_draft,
            orthography_preview,
            orthography_convert,
            track_sources,
            write_converted_track,
            orthography_pronounce,
            export_wordlist,
            set_typology_answer,
            character_inventory,
        all_notes,
        vocabulary_growth,
        speaker_lengths,
        corpus_tracks,
        export_typology,
        wikipedia_import,
        word_sketch,
        sound_changes,
        compare_read,
        set_sound_changes,
            tree_problems,
            sound_words,
            sentence_check_status,
            sentence_mark_checked,
            morphosyntax_allow_web,
            morphosyntax_run,
            sentence_get,
            sentence_set,
            list_texts,
            link_recording,
            media_overview,
            frequencies,
            collocates,
            similar_words,
            keywords,
            tier_glossing,
            read_tier_glossing,
            get_text,
            create_text,
            import_file,
            export_text,
            export_lexicon,
            import_lexicon_table,
            export_lexicon_table,
            project_progress,
            project_affixes,
            seal_project,
            project_language,
            set_project_language,
            export_cldf,
            export_corpus,
            export_dictionary,
            save_document,
            add_segment,
            add_translation,
            edit_segment_content,
            set_segment_times,
            delete_segment,
            restore_segments,
            delete_segments,
            split_segment,
            merge_segments,
            unmerge_segments,
            speech_spans,
            auto_segment,
            add_tier,
            gloss_get,
            gloss_tokenize,
            gloss_retokenize,
            gloss_apply,
            gloss_apply_all,
            gloss_word_types,
            likely_meanings,
            speech_dataset_summary,
            speech_dataset_export,
            recording_levels,
            export_spelling,
            export_parallel,
            facsimile_view,
            facsimile_add_page,
            facsimile_page_image,
            facsimile_remove_page,
            facsimile_move_page,
            facsimile_set_area,
            facsimile_add_line,
            save_hit_clips,
            texts_alike,
            gloss_break_all,
            gloss_apply_type,
            gloss_set,
            gloss_pick_sense,
            gloss_add_note,
            gloss_add_topic,
            gloss_remove_topic,
            topics_overview,
            rename_topic,
            file_under_topic,
            gloss_set_note,
            gloss_add_lexeme,
            gloss_lexicon,
            gloss_restore,
            gloss_choose_list,
            people_view,
            person_add,
            person_set_name,
            person_set_roles,
            person_set_detail,
            field_sessions_view,
            field_session_add,
            field_session_set,
            field_session_person,
            field_session_delete,
            person_delete,
            consent_add,
            consent_set_term,
            consent_set_covers,
            consent_set_date,
            consent_attach,
            consent_detach,
            consent_delete,
            lexicon_list,
            lexicon_entry,
            lexicon_add,
            lexicon_set_field,
            lexicon_set_homograph,
            lexicon_number_homographs,
            lexicon_delete,
            lexicon_merge,
            lexicon_add_sense,
            lexicon_set_sense_gloss,
            lexicon_set_sense_text,
            lexicon_delete_sense,
            lexicon_add_allomorph,
            lexicon_set_picture,
            lexicon_clear_picture,
            lexicon_add_example,
            lexicon_example_from,
            lexicon_set_example,
            lexicon_delete_example,
            lexicon_delete_allomorph,
            lexicon_add_pronunciation,
            lexicon_set_pronunciation,
            lexicon_pronunciation_audio,
            lexicon_delete_pronunciation,
            lexicon_add_relation,
            lexicon_delete_relation,
            lexicon_assign_domain,
            lexicon_unassign_domain,
            lexicon_paradigm,
            paradigm_templates,
            save_paradigm_template,
            delete_paradigm_template,
            lexicon_fill_cell,
            lexicon_delete_form,
            domains_list,
            domains_load_sil,
            concept_lists_bundled,
            concept_list_load,
            concept_list_import,
            concept_lists,
            concept_list_view,
            elicit_answer,
            elicit_link,
            elicit_unlink,
            elicit_skip,
            elicit_gloss_matches,
            elicit_link_matches,
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
            measure_track,
            export_flashcards,
            export_archive_sheet,
            archive_readiness,
            typing_shortcuts,
            lexicon_example_gaps,
            lexicon_examples_from_texts,
            set_typing_shortcuts,
            save_clips,
            replace_preview,
            export_website,
            replace_apply,
            acoustic_voice,
            set_acoustic_voice,
            measure_points,
            spectrogram,
            conflict_resolve,
            list_sources,
            source_set_detail,
            import_media,
            capture_begin,
            capture_chunk,
            drop_begin,
            drop_chunk,
            drop_finish,
            drop_cancel,
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
            kit_inspect,
            kit_share,
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
            updater::update_check,
            updater::update_download,
            updater::update_install,
        ])
        .run(tauri::generate_context!())
        .expect("run the wugs desktop shell");
}
