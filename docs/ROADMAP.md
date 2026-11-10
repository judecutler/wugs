# Roadmap: from here to a shipped suite

*What's left, in order. Status: 🟡 in progress or partial · ⬜ not started. **When an item is finished, delete its row** — decisions are recorded in `DECISIONS.md`, and what was built is recorded in `CLAUDE.md` and the PR. Item numbers stay fixed, so gaps mean finished items. Mirror of the claude.ai Project doc `product/ROADMAP.md`.*

*Last updated: 2026-11-10.*

*PR numbers up to #38 refer to the private pre-release repository [wugs-archive](https://github.com/judecutler/wugs-archive); the public repository started from one commit on 2026-11-10 and numbers its PRs afresh, so new references are written as full links.*

**The platform-core block is done** except item 8's typed tables, which wait for a real need. The Tongues shell (item 16) is finished; its two-device checks wait on item 48.

*Finished so far: 1–4 (license, names, open-source / paid-hosting model, package installs; see `DECISIONS.md`), 5 (FLEx importer, `wugs-flex`, PR #4), 6 (Toolbox SFM + Praat TextGrid, `wugs-legacy`, PR #14; reviewed, three TextGrid data-loss fixes), 7 (sync wire protocol, `wugs-sync::wire` + `wugs-net`, PR #8 — reviewed, six findings fixed; no auth yet), 9 (Loro text CRDTs, `wugs-text`, PR #5), 10 (validators #5–#8, PR #6), 11 (consent/ACL/Person intents PR #10 + sync auth v1: hash-chained roster, CPace pairing, TLS raw pubkeys, guest mode, `wugs-auth`, PR #20; security-reviewed, takeover blocker fixed; pairing UI lands with the shell), 12 (blob store + log-marked GC, `wugs-blob`, PR #13; reviewed, GC mark moved to the op log), 13 (writing systems: NFC at entry, WS intents, alphabet sort keys, `wugs-unicode` + `wugs_model::sortkey`, PR #12; reviewed, NFC-bypass blocker fixed; keyboards/ICU/RTL are UI-side, items 20/26), 14 (sealing: BagIt deposit packages, `wugs-seal`, PR #11; reviewed, path-traversal and access-listing blockers fixed; CLDF and XSD-valid IMDI are follow-ups), 15 (clearable follow-ups: wugs-elan one-scope-per-txn + create_scope_root, PR #15; the remaining CLAUDE.md follow-ups are each gated on later work and tracked there), 16 (Tauri shell + dock + auth screens, PR #7/#22; single-machine on-screen run passed on Windows 2026-11-08 — layout persistence incl. close-time flush, keychain key store, single-instance guard, create project; two-device checks moved to item 48; window-state restore and the relocation banner get a hand check by the owner (no agent screen access)), 26 (kits: `wugs-kit` manifests, Kit Builder with View code, per-kit layouts, PR #37), 34 (license, contributing guide, issue templates; repo public since 2026-11-10 with a fresh history, the pre-release history kept privately as `wugs-archive`), 46 (Praat with the user's own install: TextGrid handoff with a reviewed round trip, [PR #2](https://github.com/judecutler/wugs/pull/2), passed on-screen on Windows with Praat 7.0.03; pitch, intensity and formant overlays drawn by the Timeline from cached `praat --run` analysis, [PR #3](https://github.com/judecutler/wugs/pull/3)) and 47 (bundled SQLite + 3-OS CI, PR #24; all six legs green).*

## Finish the platform core
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 49 | Causal conflict detection: decide "had seen" by walking each field's write lineage (per device), one conflict row per unseen loser, computed so replay equals every arrival order (incl. sets mixed with text-edits) | 🟡 | PR #32: ops carry `heads` (schema 2, ALPN `wugs/3`); conflicts = open heads; only direct edits resolve; causal-oracle property test incl. degraded text-edits; reviewed, review fixes in |
| 8 | Typed projections + FTS5 search | 🟡 | FTS5 search + the hot ref index done (PR #9); gates unchanged (p95 87µs). Typed tables deferred until a tool query or projection size (643 MB @ 500k ops) hurts |

## Build Tongues (fieldwork kit)
*UI direction: `docs/design/TONGUES-UI.md` (dock panels = kit presets, canvas timeline on the tier model, per-segment transcription assist with provenance + confidence, lexicon-looped glossing, interop first).*

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 17 | Waveform canvas, peaks in Rust (gate 2: 2-hour WAV on Linux) | 🟡 | Canvas timeline built (PR #26): strip-cached waveform, zoom/pan, playback cursor, perf overlay; owner's on-screen run on Windows pending; WebKitGTK half of gate 2 still open (no Linux machine). Engine half done and reviewed (`wugs-peaks`, PR #17): 2 h WAV → peaks in 1.1 s cold, 22 MB, queries µs-scale |
| 18 | Media capture, audio and video | 🟡 | In-app audio recording (PR #28): uncompressed PCM via AudioWorklet, level meter, mic picker; video later; owner hand check pending; reviewed; recording-loss paths fixed |
| 19 | Annotation timeline with ELAN tier types | 🟡 | Tier lanes + segment create/resize/edit/delete on canvas (PR #26); ELAN symbolic tier TYPES (subdivision/association) still data-only |
| 20 | Transcription editor | 🟡 | Keyboard transcription loop + IPA palette (PR #29); owner hand check pending |
| 21 | Interlinear glossing UI linked to the lexicon | 🟡 | Gloss panel on the per-occurrence model (docs/design/GLOSSING-MODEL.md): tokenize/retokenize with carry-over, FLEx-style suggestions from prior analyses, apply-to-unanalysed, Leipzig glosses, lexicon autocomplete + add-to-lexicon, multi-language glosses, free translation line (PR #31); owner hand check pending; reviewed; concurrent-edit loss fixed (atomic membership lists); re-review fixes: losing whole analyses reported, hidden edits charged to the list that removed them |
| 22 | Lexicon UI with semantic domains | 🟡 | PR #33 (stacked on #32): Lexicon panel (find, add, edit lemma/citation/category/homograph, senses with two-language glosses, definition, usage, delete), semantic-domain picker, on-demand "Load SIL Semantic Domains" (v4, CC BY-SA, bundled as attributed data); lexicon intents return findings; owner hand check pending |
| 23 | Elicitation tracker (Swadesh, paradigms) | 🟡 | PR #34 (stacked on #33): Elicit panel with Concepticon word lists (Swadesh 100/200, Leipzig-Jakarta; CC BY) and SIL domain questions; each answer is a lexicon entry linked to its concept or domain; coverage derived; design `docs/design/ELICITATION.md`. Paradigms split out to row 50; owner hand check pending |
| 50 | Paradigm elicitation: inflected `Form`s with features, a paradigm grid per lexeme (person × number …), coverage per cell | ⬜ | Split from 23; needs a Form editor in the Lexicon panel |
| 24 | Conflict-resolution UI ("Choose version or edit manually") | 🟡 | PR #35 (stacked on #34): Conflicts panel queue: every open conflict described in words (field of what, where), its versions side by side (whose, when, which is shown), "Keep this" or type the right value; `resolve_field` writes a resolving set or text-edit in the winner's scope, so the conflict closes everywhere. Glossing slice ("Use that instead") stays in the Gloss panel. Owner hand check needs two devices (item 48) |
| 25 | Search UI | 🟡 | Search panel: jump to segment, kind filter, grouped results, keyboard nav (PR #30); owner hand check pending |
| 27 | Downloadable local phone recognizer as an optional offline extension | 🟡 | [PR #1](https://github.com/judecutler/wugs/pull/1) (archive #38): `wugs-asr` (tract, CTC phone models, per-phone confidence), `machine-transcribe` intent with derived verification, Speech assist panel (download, Suggest, fill empty segments of a tier), yellow/red machine tints; first model wav2vec2 espeak (Apache-2.0); owner hand check with the real model pending |
| 51 | ZIPA phone models for speech assist (CC BY 4.0, 64M CTC, INT8 ONNX): Kaldi filterbank front end, Zipformer graph under tract | ⬜ | Split from 27: smaller and more accurate than wav2vec2 (ZIPA, ACL 2025); needs the fbank front end checked against lhotse |

## Validate with real users
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 48 | Two-device on-screen run: pairing code, discovery, Sync now, mark-lost, copied-folder banner on both machines | ⬜ | Deferred: owner has one machine; the alpha (29) brings real second devices |
| 28 | Run gate 3: import a real donated FLEx/ELAN project, export, diff | ⬜ | Needs a donated project. Gate 1 passed on a fast cloud machine, not yet on a mid-2010s laptop |
| 29 | Alpha with 5–10 field linguists and one community language worker | ⬜ | |
| 30 | Complete one archive deposit end to end (ELAR/PARADISEC/AILLA) | ⬜ | `wugs_seal::seal` produces the package (PR #11); the deposit itself needs a real archive account |
| 31 | Fix what the alpha surfaces; measure against PRD §9 metrics | ⬜ | |
| 52 | Conference demo-track paper (ACL / EMNLP / NAACL System Demonstrations; ComputEL or LREC as fallbacks): a short system paper, screencast and live demo | ⬜ | Owner sub-priority, for later (advice from the owner's professor: a demo-track acceptance is the strongest route to adoption by linguists). Needs technical rigour above all on the ML side: a reproducible evaluation harness (phone error rate per language on public held-out data, baselines such as Allosaurus and ZIPA, item 51), latency and memory on a mid-2010s laptop, documented models and data licences, the replay/sync gates written up, and usage evidence from the alpha (29, 31). Build new ML features with that evaluation in mind from the start |

## Release and open up
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 32 | Package installers for Windows, macOS, Linux | 🟡 | CI builds unsigned installers on tag/dispatch (PR #27); signing + updater later |
| 33 | Write user docs and the plugin/kit authoring guide | ⬜ | |
| 35 | Ship Tongues 1.0 | ⬜ | |

## Expand the suite
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 36 | WASM plugin sandbox + permission prompts | ⬜ | |
| 37 | Kit Library (GitHub-backed index) | ⬜ | |
| 38 | Analysis layer: stats, charts, maps, notebook | ⬜ | |
| 39 | Datasets layer: Glottolog, WALS, Grambank connectors | ⬜ | |
| 40 | Typology starter kit (proves custom kits work) | ⬜ | |
| 41 | Corpus kit (Scrolls): tokenizer, tagger, treebank, concordance | ⬜ | |
| 42 | Historical kit (Glyphs): facsimile viewer, sign/n-gram analysis, cognate aligner | ⬜ | |
| 43 | Publishing layer: dictionaries, grammars, community apps | ⬜ | |
| 44 | Mobile companion (evaluate Tauri 2 mobile first) | ⬜ | |
| 45 | Hosted sync and cloud AI, opt-in per project | ⬜ | Paid hosted services; must be self-hostable from the public repo |
