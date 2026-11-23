//! Capabilities (roadmap 53, `docs/design/PACKAGES.md`): the jobs the app
//! asks for, and what powers each on this machine. Feature code asks
//! `resolve` for a capability and gets a provider; it never names a model
//! or a program. A provider is either a model package (a manifest in the
//! catalogue, run by one of this build's runtime adapters) or a program the
//! person installed (Praat), or one built into wugs (voice activity for
//! segmentation).
//!
//! The choice per capability is Automatic or a named provider. This
//! computer keeps its own choice in `<app data>/capabilities.json`; a
//! project may pin a provider for everyone on it (a `Setting` in the
//! project config scope, `capability:<id>`), and the pin wins, so a long
//! annotation campaign is not switched halfway by one person's update.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use wugs_package::Manifest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    PhoneTranscription,
    AcousticAnalysis,
    Segmentation,
    Morphosyntax,
}

impl Capability {
    pub const ALL: [Capability; 4] = [Capability::PhoneTranscription, Capability::AcousticAnalysis, Capability::Segmentation, Capability::Morphosyntax];

    pub fn id(self) -> &'static str {
        match self {
            Capability::PhoneTranscription => "phone-transcription",
            Capability::AcousticAnalysis => "acoustic-analysis",
            Capability::Segmentation => "segmentation",
            Capability::Morphosyntax => "morphosyntax",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Capability::PhoneTranscription => "Phone transcription",
            Capability::AcousticAnalysis => "Acoustic analysis",
            Capability::Segmentation => "Segmentation",
            Capability::Morphosyntax => "Tagging and parsing",
        }
    }

    pub fn parse(id: &str) -> Result<Capability, String> {
        Capability::ALL.into_iter().find(|c| c.id() == id).ok_or_else(|| format!("no capability {id:?}"))
    }

    /// The project setting that pins it.
    pub fn setting_key(self) -> String {
        format!("capability:{}", self.id())
    }
}

/// The runtime adapters this build has. A manifest naming another one is
/// listed but never used.
pub const RUNTIMES: &[&str] = &["onnx-ctc-raw", "onnx-ctc-fbank"];

/// Praat, as a provider id. Not a package: the person's own program.
pub const PRAAT: &str = "praat";

/// Built-in voice-activity detection (`wugs_asr::vad`), as a provider id:
/// where speech starts and ends, from loudness over the recording's own
/// noise floor. Not a package and always ready; segmentation models can
/// join it as packages later.
pub const VOICE_ACTIVITY: &str = "voice-activity";

/// UDPipe 1 on this computer, with a model the person chose: the person's
/// own program, nothing leaves the computer.
pub const UDPIPE: &str = "udpipe";

/// UDPipe 2 at ÚFAL's LINDAT service: a web service, so never Automatic's
/// pick (text would leave the computer without anyone choosing it), and a
/// project must allow it before anything is sent (`morphosyntax::WEB_OK`).
pub const UDPIPE_LINDAT: &str = "udpipe-lindat";

/// spaCy in the person's own Python, with a pipeline they installed:
/// nothing leaves the computer.
pub const SPACY: &str = "spacy";

/// In catalogue order, which is Automatic's last tie-break. wav2vec2 stays
/// first until ZIPA has been checked on real speech by hand: downloading
/// ZIPA must not silently change what Automatic runs. wugs's own scores
/// will decide once the evaluation harness exists.
const BUNDLED: &[&str] = &[include_str!("../../../../catalog/wav2vec2-espeak.toml"), include_str!("../../../../catalog/zipa-cr-small.toml")];

/// The bundled catalogue (the remote index replaces it in item 37).
pub fn catalogue() -> &'static [Manifest] {
    static CAT: OnceLock<Vec<Manifest>> = OnceLock::new();
    CAT.get_or_init(|| BUNDLED.iter().map(|t| wugs_package::parse(t).expect("the bundled catalogue parses (tested)")).collect())
}

pub fn manifest(id: &str) -> Option<&'static Manifest> {
    catalogue().iter().find(|m| m.id == id)
}

/// Automatic, or one provider by id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Automatic,
    Pick(String),
}

impl Choice {
    pub fn parse(s: &str) -> Choice {
        if s.is_empty() || s == "automatic" { Choice::Automatic } else { Choice::Pick(s.to_string()) }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Choice::Automatic => "automatic",
            Choice::Pick(id) => id,
        }
    }
}

// ---------------------------------------------------------------- this computer's choices

#[derive(Default, Serialize, Deserialize)]
struct Choices(BTreeMap<String, String>);

fn choices_path(app_data: &Path) -> PathBuf {
    app_data.join("capabilities.json")
}

pub fn machine_choice(app_data: &Path, cap: Capability) -> Choice {
    let saved: Choices = fs::read(choices_path(app_data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    saved.0.get(cap.id()).map(|s| Choice::parse(s)).unwrap_or(Choice::Automatic)
}

pub fn set_machine_choice(app_data: &Path, cap: Capability, choice: &Choice) -> Result<(), String> {
    let path = choices_path(app_data);
    let mut saved: Choices = fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    saved.0.insert(cap.id().to_string(), choice.as_str().to_string());
    fs::create_dir_all(app_data).map_err(|e| e.to_string())?;
    crate::praat::write_atomic(&path, &serde_json::to_vec_pretty(&saved).expect("serializable"))
}

/// A project's pin: a provider id, or None when it has none (an empty
/// value unpins; `automatic` is read the same way, since pinning Automatic
/// would pin nothing).
pub fn project_pin(project: &wugs_core::Project, cap: Capability) -> Result<Option<String>, String> {
    Ok(project.setting(&cap.setting_key())?.filter(|v| !v.is_empty() && v != "automatic"))
}

/// Every provider id that can serve `cap`, ready here or not.
pub fn known(cap: Capability) -> Vec<String> {
    let mut ids: Vec<String> = catalogue().iter().filter(|m| m.model.capability == cap.id()).map(|m| m.id.clone()).collect();
    match cap {
        Capability::AcousticAnalysis => ids.push(PRAAT.into()),
        Capability::Segmentation => ids.push(VOICE_ACTIVITY.into()),
        Capability::Morphosyntax => ids.extend([UDPIPE.to_string(), SPACY.to_string(), UDPIPE_LINDAT.to_string()]),
        Capability::PhoneTranscription => {}
    }
    ids
}

// ---------------------------------------------------------------- what can power a capability

/// One provider of a capability, as this computer sees it.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    /// "model", "program", "built-in" or "web service".
    pub kind: &'static str,
    /// Ready to run here now.
    pub ready: bool,
    /// Why not, or how it was found, in words.
    pub note: String,
    /// Published error rate (lower is better), when evaluated.
    pub score: Option<f64>,
}

/// Facts about this computer one capability's choice depends on.
pub struct Here {
    /// Ids of this capability's models whose files are downloaded.
    pub installed: Vec<String>,
    /// Praat's path, or why there is none (looked up only for acoustic
    /// analysis: it runs `praat --version`).
    pub praat: Result<PathBuf, String>,
    /// UDPipe and its model, or why not (looked up only for morphosyntax).
    pub udpipe: Result<crate::morphosyntax::Local, String>,
    /// spaCy and its pipeline, or why not (looked up only for morphosyntax;
    /// Python is not run, so a missing spaCy shows when the pipeline is chosen or run).
    pub spacy: Result<crate::morphosyntax::Spacy, String>,
}

impl Here {
    pub fn look(app_data: &Path, cap: Capability) -> Here {
        Here {
            installed: catalogue().iter().filter(|m| m.model.capability == cap.id() && crate::assist::installed(app_data, m).is_some()).map(|m| m.id.clone()).collect(),
            praat: if cap == Capability::AcousticAnalysis { crate::praat::praat(app_data) } else { Err("not needed".into()) },
            udpipe: if cap == Capability::Morphosyntax { crate::morphosyntax::local(app_data) } else { Err("not needed".into()) },
            spacy: if cap == Capability::Morphosyntax { crate::morphosyntax::spacy(app_data) } else { Err("not needed".into()) },
        }
    }
}

pub fn options(cap: Capability, here: &Here) -> Vec<ProviderRow> {
    let mut out: Vec<ProviderRow> = catalogue()
        .iter()
        .filter(|m| m.model.capability == cap.id())
        .map(|m| {
            let runs = RUNTIMES.contains(&m.model.runtime.as_str());
            let installed = here.installed.contains(&m.id);
            let note = if !runs {
                format!("Needs a newer wugs (runtime {}).", m.model.runtime)
            } else if installed {
                "Downloaded.".to_string()
            } else {
                "Not downloaded.".to_string()
            };
            ProviderRow { id: m.id.clone(), name: m.name.clone(), kind: "model", ready: runs && installed, note, score: m.score(None) }
        })
        .collect();
    if cap == Capability::AcousticAnalysis {
        let (ready, note) = match &here.praat {
            Ok(p) => (true, format!("Found at {}.", p.display())),
            Err(e) => (false, e.clone()),
        };
        out.push(ProviderRow { id: PRAAT.into(), name: "Praat".into(), kind: "program", ready, note, score: None });
    }
    if cap == Capability::Segmentation {
        out.push(ProviderRow {
            id: VOICE_ACTIVITY.into(),
            name: "Voice activity (built in)".into(),
            kind: "built-in",
            ready: true,
            note: "Built in: finds pauses in the recording, offline.".into(),
            score: None,
        });
    }
    if cap == Capability::Morphosyntax {
        let (ready, note) = match &here.udpipe {
            Ok(l) => (true, format!("Runs here with {}; nothing leaves this computer.", l.model.file_name().and_then(|n| n.to_str()).unwrap_or("its model"))),
            Err(e) => (false, e.clone()),
        };
        out.push(ProviderRow { id: UDPIPE.into(), name: "UDPipe on this computer".into(), kind: "program", ready, note, score: None });
        let (ready, note) = match &here.spacy {
            Ok(sp) => (true, format!("Runs here with the {} pipeline; nothing leaves this computer.", sp.pipeline)),
            Err(e) => (false, e.clone()),
        };
        out.push(ProviderRow { id: SPACY.into(), name: "spaCy on this computer".into(), kind: "program", ready, note, score: None });
        out.push(ProviderRow {
            id: UDPIPE_LINDAT.into(),
            name: "UDPipe 2 (LINDAT web service)".into(),
            kind: "web service",
            ready: true,
            note: "Sends the text to ÚFAL in Prague, who may keep it; models are CC BY-NC-SA. Each project must allow it, and texts that are not open are never sent.".into(),
            score: None,
        });
    }
    out
}

/// Automatic's pick among ready providers: the best published score, then
/// unscored ones (until the evaluation harness scores the catalogue, an
/// unscored model is still better than none), then catalogue order.
fn automatic(options: &[ProviderRow]) -> Option<&ProviderRow> {
    // A web service is used only when someone chose it.
    options.iter().filter(|o| o.ready && o.kind != "web service").min_by(|a, b| match (a.score, b.score) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    })
}

/// What powers a capability once chosen.
#[derive(Debug)]
pub enum Provider {
    Model(&'static Manifest),
    Praat(PathBuf),
    VoiceActivity,
    Udpipe(crate::morphosyntax::Local),
    UdpipeLindat,
    Spacy(crate::morphosyntax::Spacy),
}

impl Provider {
    pub fn id(&self) -> &str {
        match self {
            Provider::Model(m) => &m.id,
            Provider::Praat(_) => PRAAT,
            Provider::VoiceActivity => VOICE_ACTIVITY,
            Provider::Udpipe(_) => UDPIPE,
            Provider::UdpipeLindat => UDPIPE_LINDAT,
            Provider::Spacy(_) => SPACY,
        }
    }
}

/// The provider in effect: the project's pin if it has one, else this
/// computer's choice. An unready pick is an error naming it, never a
/// silent switch to something else.
pub fn effective(cap: Capability, pin: Option<&str>, machine: &Choice, options: &[ProviderRow]) -> Result<String, String> {
    let (choice, pinned) = match pin {
        Some(id) => (Choice::Pick(id.to_string()), true),
        None => (machine.clone(), false),
    };
    match choice {
        Choice::Pick(id) => match options.iter().find(|o| o.id == id) {
            Some(o) if o.ready => Ok(o.id.clone()),
            Some(o) => Err(format!(
                "{} {} {} for {}, and it is not ready here: {}",
                if pinned { "This project uses" } else { "You chose" },
                o.name,
                match o.kind {
                    "model" => "(a model)",
                    "program" => "(a program)",
                    "web service" => "(a web service)",
                    _ => "(built in)",
                },
                cap.title().to_lowercase(),
                o.note
            )),
            None => Err(format!(
                "{} {id:?} for {}, which this version of wugs does not know.",
                if pinned { "This project uses" } else { "You chose" },
                cap.title().to_lowercase()
            )),
        },
        Choice::Automatic => automatic(options).map(|o| o.id.clone()).ok_or_else(|| match cap {
            Capability::PhoneTranscription => "No phone transcription model is downloaded yet; Speech assist lists the ones you can download.".to_string(),
            Capability::AcousticAnalysis => options.iter().find(|o| o.id == PRAAT).map(|o| o.note.clone()).unwrap_or_else(|| "Nothing on this computer can analyse audio.".into()),
            Capability::Segmentation => "Nothing here can segment a recording.".to_string(),
            Capability::Morphosyntax => options
                .iter()
                .find(|o| o.id == UDPIPE)
                .map(|o| format!("{} Or set up spaCy, or choose the UDPipe 2 web service.", o.note))
                .unwrap_or_else(|| "Nothing on this computer can tag and parse.".into()),
        }),
    }
}

/// Resolve to a runnable provider.
pub fn resolve(cap: Capability, pin: Option<&str>, app_data: &Path) -> Result<Provider, String> {
    let here = Here::look(app_data, cap);
    let id = effective(cap, pin, &machine_choice(app_data, cap), &options(cap, &here))?;
    if id == PRAAT {
        return here.praat.map(Provider::Praat);
    }
    if id == VOICE_ACTIVITY {
        return Ok(Provider::VoiceActivity);
    }
    if id == UDPIPE {
        return here.udpipe.map(Provider::Udpipe);
    }
    if id == UDPIPE_LINDAT {
        return Ok(Provider::UdpipeLindat);
    }
    if id == SPACY {
        return here.spacy.map(Provider::Spacy);
    }
    manifest(&id).map(Provider::Model).ok_or_else(|| format!("no package {id:?}"))
}

// ---------------------------------------------------------------- the panel's view

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityView {
    pub id: &'static str,
    pub title: &'static str,
    /// This computer's choice: "automatic" or a provider id.
    pub machine: String,
    /// The open project's pin, if any.
    pub pin: Option<String>,
    /// What runs now, or why nothing can.
    pub effective: Option<String>,
    pub problem: Option<String>,
    pub options: Vec<ProviderRow>,
}

pub fn views(app_data: &Path, pins: &BTreeMap<&'static str, Option<String>>) -> Vec<CapabilityView> {
    Capability::ALL
        .into_iter()
        .map(|cap| {
            let here = Here::look(app_data, cap);
            let options = options(cap, &here);
            let machine = machine_choice(app_data, cap);
            let pin = pins.get(cap.id()).cloned().flatten();
            let (effective, problem) = match effective(cap, pin.as_deref(), &machine, &options) {
                Ok(id) => (Some(id), None),
                Err(e) => (None, Some(e)),
            };
            CapabilityView { id: cap.id(), title: cap.title(), machine: machine.as_str().to_string(), pin, effective, problem, options }
        })
        .collect()
}

/// A provider's name for people, for any id a project may hold.
pub fn provider_name(id: &str) -> String {
    match id {
        "" => "not pinned".into(),
        PRAAT => "Praat".into(),
        VOICE_ACTIVITY => "Voice activity (built in)".into(),
        UDPIPE => "UDPipe on this computer".into(),
        UDPIPE_LINDAT => "UDPipe 2 (LINDAT web service)".into(),
        SPACY => "spaCy on this computer".into(),
        "automatic" => "Automatic".into(),
        _ => manifest(id).map(|m| m.name.clone()).unwrap_or_else(|| id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(id: &str, ready: bool, score: Option<f64>) -> ProviderRow {
        ProviderRow { id: id.into(), name: id.into(), kind: "model", ready, note: if ready { "Downloaded.".into() } else { "Not downloaded.".into() }, score }
    }

    #[test]
    fn the_bundled_catalogue_parses_and_names_a_runtime_this_build_has() {
        assert!(!catalogue().is_empty());
        for m in catalogue() {
            assert!(RUNTIMES.contains(&m.model.runtime.as_str()), "{} needs {}", m.id, m.model.runtime);
            Capability::parse(&m.model.capability).unwrap();
        }
    }

    #[test]
    fn automatic_prefers_the_best_ready_score_then_unscored() {
        let os = [opt("a", true, None), opt("b", true, Some(0.3)), opt("c", true, Some(0.2)), opt("d", false, Some(0.1))];
        assert_eq!(effective(Capability::PhoneTranscription, None, &Choice::Automatic, &os).unwrap(), "c");
        let os = [opt("a", true, None), opt("d", false, Some(0.1))];
        assert_eq!(effective(Capability::PhoneTranscription, None, &Choice::Automatic, &os).unwrap(), "a");
        let err = effective(Capability::PhoneTranscription, None, &Choice::Automatic, &[opt("d", false, None)]).unwrap_err();
        assert!(err.contains("ones you can download"), "{err}");
    }

    #[test]
    fn a_pin_wins_and_an_unready_pick_is_named_not_swapped() {
        let os = [opt("a", true, Some(0.1)), opt("b", false, None)];
        assert_eq!(effective(Capability::PhoneTranscription, Some("a"), &Choice::Pick("b".into()), &os).unwrap(), "a");
        let err = effective(Capability::PhoneTranscription, Some("b"), &Choice::Automatic, &os).unwrap_err();
        assert!(err.starts_with("This project uses b"), "{err}");
        let err = effective(Capability::PhoneTranscription, None, &Choice::Pick("b".into()), &os).unwrap_err();
        assert!(err.starts_with("You chose b"), "{err}");
        let err = effective(Capability::PhoneTranscription, Some("zipa"), &Choice::Automatic, &os).unwrap_err();
        assert!(err.contains("does not know"), "{err}");
    }

    #[test]
    fn providers_are_known_only_for_their_own_capability() {
        assert!(known(Capability::PhoneTranscription).contains(&"wav2vec2-espeak".to_string()));
        assert!(!known(Capability::PhoneTranscription).contains(&PRAAT.to_string()));
        assert_eq!(known(Capability::AcousticAnalysis), [PRAAT]);
        assert_eq!(known(Capability::Segmentation), [VOICE_ACTIVITY]);
        assert!(!known(Capability::PhoneTranscription).contains(&VOICE_ACTIVITY.to_string()));
    }

    #[test]
    fn segmentation_is_built_in_and_always_ready() {
        let dir = std::env::temp_dir().join(format!("wugs-cap-seg-{}", wugs_model::ulid()));
        assert_eq!(Capability::parse("segmentation").unwrap(), Capability::Segmentation);
        let here = Here::look(&dir, Capability::Segmentation);
        let os = options(Capability::Segmentation, &here);
        assert_eq!(os.iter().map(|o| (o.id.as_str(), o.kind, o.ready)).collect::<Vec<_>>(), [(VOICE_ACTIVITY, "built-in", true)]);
        assert_eq!(effective(Capability::Segmentation, None, &Choice::Automatic, &os).unwrap(), VOICE_ACTIVITY);
        assert!(matches!(resolve(Capability::Segmentation, None, &dir).unwrap(), Provider::VoiceActivity));
        let err = effective(Capability::Segmentation, Some("some-model"), &Choice::Automatic, &os).unwrap_err();
        assert!(err.contains("does not know"), "{err}");
        assert_eq!(provider_name(VOICE_ACTIVITY), "Voice activity (built in)");
    }

    #[test]
    fn machine_choices_round_trip_per_capability() {
        let dir = std::env::temp_dir().join(format!("wugs-cap-test-{}", wugs_model::ulid()));
        assert_eq!(machine_choice(&dir, Capability::PhoneTranscription), Choice::Automatic);
        set_machine_choice(&dir, Capability::PhoneTranscription, &Choice::Pick("wav2vec2-espeak".into())).unwrap();
        set_machine_choice(&dir, Capability::AcousticAnalysis, &Choice::Automatic).unwrap();
        assert_eq!(machine_choice(&dir, Capability::PhoneTranscription), Choice::Pick("wav2vec2-espeak".into()));
        assert_eq!(machine_choice(&dir, Capability::AcousticAnalysis), Choice::Automatic);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn praat_powers_acoustic_analysis_only_when_found() {
        let here = Here { installed: vec![], praat: Err("Praat was not found.".into()), udpipe: Err("not needed".into()), spacy: Err("not needed".into()) };
        let os = options(Capability::AcousticAnalysis, &here);
        assert_eq!(os.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), [PRAAT]);
        assert_eq!(effective(Capability::AcousticAnalysis, None, &Choice::Automatic, &os).unwrap_err(), "Praat was not found.");
        let here = Here { installed: vec![], praat: Ok(PathBuf::from("/usr/bin/praat")), udpipe: Err("not needed".into()), spacy: Err("not needed".into()) };
        assert_eq!(effective(Capability::AcousticAnalysis, None, &Choice::Automatic, &options(Capability::AcousticAnalysis, &here)).unwrap(), PRAAT);
        assert!(options(Capability::PhoneTranscription, &here).iter().all(|o| o.kind == "model"));
    }

    #[test]
    fn the_web_tagger_runs_only_when_chosen() {
        let missing = Here { installed: vec![], praat: Err("not needed".into()), udpipe: Err("UDPipe is not installed.".into()), spacy: Err("Python is not installed.".into()) };
        let os = options(Capability::Morphosyntax, &missing);
        assert_eq!(os.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), [UDPIPE, SPACY, UDPIPE_LINDAT]);
        let auto = effective(Capability::Morphosyntax, None, &Choice::Automatic, &os).unwrap_err();
        assert!(auto.starts_with("UDPipe is not installed.") && auto.contains("web service"), "Automatic never sends text away: {auto}");
        assert_eq!(effective(Capability::Morphosyntax, None, &Choice::Pick(UDPIPE_LINDAT.into()), &os).unwrap(), UDPIPE_LINDAT);
        let here = Here { installed: vec![], praat: Err("not needed".into()), udpipe: Ok(crate::morphosyntax::Local { program: "/usr/bin/udpipe".into(), model: "/m/english.udpipe".into() }), spacy: Err("not needed".into()) };
        assert_eq!(effective(Capability::Morphosyntax, None, &Choice::Automatic, &options(Capability::Morphosyntax, &here)).unwrap(), UDPIPE);
        assert!(known(Capability::Morphosyntax).contains(&UDPIPE_LINDAT.to_string()));
        // spaCy is Automatic's pick when UDPipe is not set up, and UDPipe stays first when both are.
        let sp = crate::morphosyntax::Spacy { python: "/usr/bin/python3".into(), pipeline: "en_core_web_sm".into() };
        let only_spacy = Here { installed: vec![], praat: Err("not needed".into()), udpipe: Err("UDPipe is not installed.".into()), spacy: Ok(sp.clone()) };
        assert_eq!(effective(Capability::Morphosyntax, None, &Choice::Automatic, &options(Capability::Morphosyntax, &only_spacy)).unwrap(), SPACY);
        let both = Here { spacy: Ok(sp), ..here };
        assert_eq!(effective(Capability::Morphosyntax, None, &Choice::Automatic, &options(Capability::Morphosyntax, &both)).unwrap(), UDPIPE);
        assert!(known(Capability::Morphosyntax).contains(&SPACY.to_string()));
        assert_eq!(provider_name(SPACY), "spaCy on this computer");
    }
}
