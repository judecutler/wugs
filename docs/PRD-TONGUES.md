# PRD: Tongues — the Fieldwork Kit (v2)

*Supersedes PRD-v1. Suite structure: `SUITE-CONTEXT.md`. Architecture decisions: `DECISIONS.md`.*

## 1. Summary

Field linguistics software is fragmented across aging desktop tools (ELAN, FLEx, Toolbox, Praat, SayMore) that don't talk to each other, don't sync, and don't survive fieldwork conditions gracefully. **Tongues** is the fieldwork kit of the suite: a fast, local-first environment for language documentation — record → slice → transcribe → gloss → build lexicon — that runs for weeks offline on low-end hardware and keeps the recording, its transcription, its glosses and the dictionary one connected body of data.

Tongues is a **kit**: a preset over shared platform tools (lexicon, IGT, media timeline, search, writing systems) plus its own tools (waveform canvas, elicitation tracker). The platform — op-log data core, sync, ethics and access control, import/export — is shared with every other kit and specified in `DECISIONS.md`, not here.

**Signed languages are in scope from the start:** video is as first-class as audio throughout (capture, timeline annotation, slicing).

## 2. Target users

- **The field linguist** — documents an under-described language; offline for weeks; owns the analysis.
- **The community consultant / language worker** — records, checks transcriptions and glosses, builds the dictionary; needs a simple UI in their language; often on modest hardware.
- **The lab collaborator** — receives synced data, runs deeper analysis (Analysis layer), prepares archive deposits.

## 3. Release-1 scope

**In:** desktop (Windows/macOS/Linux); record and import audio+video; waveform/timeline annotation; transcription; interlinear glossing linked to the lexicon; lexicon with semantic domains; elicitation tracking; search; local + same-network P2P sync; ELAN/FLEx/Toolbox/TextGrid import; sealed archive export.

**Out (later):** mobile companion; AI copilot features; forced alignment; advanced phonetics (Praat-level acoustics); hypothesis generation; kit library.

**Non-goals (never in Tongues):** corpus-scale tagging/treebanking (Scrolls); comparative reconstruction (Glyphs); experiment running.

**Dropped claims from v1:** Bluetooth mesh (replaced by local Wi-Fi/mDNS and hotspot sync); acoustic-to-articulatory mapping (research-grade, not a feature); "bypasses Chromium" (Tauri uses the system webview — WebView2 *is* Chromium; the real claim is a fraction of Electron's footprint); separate native Kotlin/Swift apps (evaluate Tauri 2 mobile first — one UI, not three).

## 4. Features (each labelled Offline / Online)

| Feature | Behavior | Mode |
| --- | --- | --- |
| **Recording & media** | Multi-track audio and video capture; content-addressed storage; waveform peaks computed in Rust (not the webview) | Offline |
| **Waveform/timeline canvas** | Millisecond annotation tiers (ELAN-style tier types); smooth on a 2-hour WAV on old hardware | Offline |
| **Transcription** | Tier-based, with per-field collaborative text editing | Offline |
| **Lightning capture** | Hotkey scratchpad; `[köpek-ler = dog-PL]` parses into structured records | Offline |
| **Interlinear glossing** | Morpheme breakdown linked to lexicon entries; Leipzig Glossing Rules defaults | Offline |
| **Lexicon** | Entries → senses → forms; semantic domains; instant search | Offline |
| **Elicitation tracker** | Swadesh lists, paradigm coverage, session planning | Offline |
| **P2P sync** | Same-room sync over local Wi-Fi (mDNS discovery) or phone hotspot; quarantine + validators + conflict prompts per `DECISIONS.md` | Offline |
| **Power management** | Suppress background work on battery | Offline |
| **Local speech aids** | Downloadable phone-recognizer models (multilingual wav2vec2-class, *not* Whisper, which fails on unseen languages); optional offline extension, installed online, never bundled | Offline ext. |
| **Gloss suggestions** | From the project's own lexicon locally; cloud-model assists only if the project opted in | Offline / Online |
| **Cloud sync & backup** | To institutional servers / repositories | Online |
| **Batch processing, AI copilot** | Deferred to a later release | Online |

## 5. Import & export (adoption-critical)

No one abandons twenty years of data. Release 1 must **round-trip**, not just read:
- **Import:** ELAN `.eaf`, FLEx (LIFT + FlexText), Toolbox/Shoebox SFM, Praat TextGrid
- **Export:** the same formats, plus CLDF and **sealed archive packages** (BagIt with ELAN/LIFT/CLDF, OLAC/IMDI metadata, plain-text op log — see `DECISIONS.md`)
- Import fidelity is a test suite: real donated FLEx/ELAN projects, imported, exported, diffed.

## 6. Ethics & data sovereignty

- Access levels **per recording and per speaker** (open / restricted / community-only), enforced at sync boundaries: restricted material never leaves the device toward an unauthorized peer.
- Consent records attached to sources and speakers; carried into sealed packages.
- **Cloud AI is off by default and opt-in per project.** No language data reaches any cloud model without an explicit project-level decision. Local models never phone home.
- CARE principles guide defaults; deletion requests are honored via tombstones + redacted seals.

## 7. Writing systems & input

- Unicode normalization (NFC) enforced at entry, or search silently breaks.
- Custom keyboards/input methods per writing system; custom sort orders; RTL and complex scripts; bundled fallback fonts.
- Orthography sandbox: test a proposed orthography against existing data before adopting it.

## 8. Architecture (summary — details in DATA-MODEL.md)

Rust core (op log, SQLite projection, validators, media segmentation) · Tauri + TypeScript UI · per-field Loro text CRDTs for collaborative text · content-addressed blob store · merkle-verified P2P sync.

## 9. Success metrics

- A real FLEx or ELAN project imported and **still in active use 3 months later**
- One archive deposit (ELAR/PARADISEC/AILLA) completed end-to-end
- Transcription-to-gloss loop measurably faster than ELAN+FLEx round-tripping
- Runs a full workday on a mid-2010s laptop battery without fans

## 10. Risks & prototype gates (before full build)

1. **Op-log replay:** ~500k ops → SQLite on an old laptop; measure replay time, incremental latency, memory.
2. **Waveform in Tauri on Linux** (WebKitGTK): 2-hour WAV, smooth scroll, peaks from Rust.
3. **FLEx import fidelity:** one real donated project through import→export→diff.

Open: Tauri 2 mobile evaluation. (Names and license are settled: see `DECISIONS.md` and `LICENSING.md`.)
