# wugs

> *wug* /wʌɡ/ — the imaginary bird from Jean Berko Gleason's 1958 wug test. "This is a wug. Now there are two of them. There are two ___."

The platform core for a modular, local-first linguistics tool suite. One shared data engine; **kits** (preset tool environments) for fieldwork, corpus work and historical linguistics on top of it.

**Status: pre-alpha.** The data core is built and benchmarked. The desktop app runs the first kit, **Tongues** (fieldwork), with panels for recording and the waveform timeline, transcription, interlinear glossing, the lexicon, elicitation, search, sync between devices, conflict review and kits. Not ready for real field data yet. What's next is in `docs/ROADMAP.md`.

## Why

Field linguistics software is fragmented across aging tools (ELAN, FLEx, Toolbox, Praat) that don't talk to each other, don't sync, and don't survive fieldwork conditions. wugs keeps a recording, its transcription, its glosses and the dictionary one connected body of data — offline-first, on low-end hardware, with the researcher's edit history preserved as scholarly provenance.

## Design in one paragraph

The source of truth is an **append-only log of linguistic operations** (JSONL, documented, replayable forever), merged with per-field last-write-wins over hybrid logical clocks, validated by configurable *linguistic* checks before any merge commits, and projected into SQLite for instant queries. Media is content-addressed and immutable. Access control follows sync boundaries. The log doubles as the archive format. See `docs/DATA-MODEL.md`.

## Repo map

- `docs/` — vision, PRD, decision log, data-model spec (**start here**)
- `crates/` — the Rust engine. The format-bearing core (`wugs-model`, `wugs-oplog`, `wugs-xml`) has no dependencies; edge crates use maintained libraries (bundled SQLite, Loro, rustls, tract…)
- `kits/` — kit manifests (`kits/tongues/kit.toml`) and the data they bundle
- `apps/desktop/` — the desktop app (Tauri 2 + TypeScript)

## Build

```
cargo build --workspace
cargo test --workspace
cargo run -p wugs-bench --release -- 500000   # op-log replay benchmark
```

Requires Rust (stable). SQLite is bundled and built from source, so a C compiler is needed.

The desktop app is its own workspace (on Linux it needs the WebKitGTK development packages Tauri lists):

```
cd apps/desktop
npm install
npm run tauri dev
```

## Install a build

Download an unsigned installer for Windows, macOS or Linux from Releases or from a CI run's artifacts. Steps for the SmartScreen and Gatekeeper warnings, and how to cut a release: `docs/INSTALL.md`.

## License

Code: **MPL-2.0** (`LICENSE`). File-level copyleft, so improvements to wugs files stay published, while your own kits, plugins and integrations stay under your own terms.

Archive format spec and future reference reader: **Apache-2.0 OR MIT**, so anyone can write a reader for a sealed wugs archive with no license question to answer.

Details and reasoning: `LICENSING.md`. Contributions take a DCO sign-off, not a CLA: `CONTRIBUTING.md`.
