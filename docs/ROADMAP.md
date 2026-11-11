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
| 53 | Capabilities: features powered by swappable models (design `docs/design/PACKAGES.md`): the app asks for a job (`phone-transcription`, `acoustic-analysis`), model packages declare which job they serve and which runtime adapter runs them, a per-capability choice (Automatic or a picked model, pinnable per project) | 🟡 | [PR #5](https://github.com/judecutler/wugs/pull/5): `wugs-package` manifests, `catalog/wav2vec2-espeak.toml`, the `onnx-ctc-raw` adapter, Praat as the `acoustic-analysis` provider, the choice and project pin (a synced `Setting`) in Speech assist; Suggest and the overlays no longer name a model. Owner hand check: pick, pin and unpin on a real project |
| 51 | ZIPA phone models for speech assist (CC BY 4.0, 64M CTC, INT8 ONNX): Kaldi filterbank front end, Zipformer graph under tract | 🟡 | [PR #7](https://github.com/judecutler/wugs/pull/7) (stacked on #5): `onnx-ctc-fbank` runtime in `wugs-asr` (filterbank matched to lhotse; tract runs icefall's Zipformer export after load-time graph adjustments; a plan per segment length, about 4 s per 5 s segment on a CI runner; identical phones to ZIPA's own script on its sample), `catalog/zipa-cr-small.toml` (pinned revision and SHA-256), a `real-models` CI workflow that runs the real release against onnxruntime and end to end against ZIPA's own script. Owner hand check: download and Suggest on field recordings; then decide whether ZIPA goes first for Automatic |

## Validate with real users
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 48 | Two-device on-screen run: pairing code, discovery, Sync now, mark-lost, copied-folder banner on both machines | ⬜ | Deferred: owner has one machine; the alpha (29) brings real second devices |
| 28 | Run gate 3: import a real donated FLEx/ELAN project, export, diff | ⬜ | Needs a donated project. Gate 1 passed on a fast cloud machine, not yet on a mid-2010s laptop |
| 29 | Alpha with 5–10 field linguists and one community language worker | ⬜ | |
| 30 | Complete one archive deposit end to end (ELAR/PARADISEC/AILLA) | ⬜ | `wugs_seal::seal` produces the package (PR #11); the deposit itself needs a real archive account |
| 31 | Fix what the alpha surfaces; measure against PRD §9 metrics | ⬜ | |


## Release and open up
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 32 | Package installers for Windows, macOS, Linux | 🟡 | CI builds unsigned installers on tag/dispatch (PR #27); signing + updater later |
| 33 | Write user docs and the plugin/kit authoring guide | ⬜ | |
| 35 | Ship Tongues 1.0 | ⬜ | |

## Sub-priority
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 52 | Conference demo-track paper (ACL / EMNLP / NAACL System Demonstrations; ComputEL or LREC as fallbacks): a short system paper, screencast and live demo | 🟡 | Evaluation harness: [PR #8](https://github.com/judecutler/wugs/pull/8) (stacked on #7): `wugs-asr-eval` transcribes a Kaldi-style test set the way wugs runs a model, and an `eval` workflow scores every catalogued phone model on PRiSM's public multilingual sets (VoxAngeles, CC BY 4.0; Tusom 2021, MIT) with PRiSM's own scorer, plus speed and memory; its first run found the catalogued wav2vec2 file did not load (fixed in #7). Then: for later (advice from the owner's professor: a demo-track acceptance is the strongest route to adoption by linguists). Needs technical rigour above all on the ML side: a reproducible evaluation harness (phone error rate per language on public held-out data, baselines such as Allosaurus and ZIPA, item 51), optimized latency and memory, documented models and data licences, the replay/sync gates written up, and usage evidence from the alpha (29, 31), Human in the loop continuous improvement for WUGS AI features, and  streamlined data exporting. Build new ML features with that evaluation in mind from the start. |
| 57 | Corrections export (human in the loop, part of 52): every machine suggestion a person checked, as a folder of clips with the model's and the person's transcriptions, held back where consent records refuse AI use or access is restricted; a benchmark set for `wugs-asr-eval` and fine-tuning data | 🟡 | [PR #9](https://github.com/judecutler/wugs/pull/9) (stacked on #8; reviewed, consent and access leaks fixed): `Project::corrections`, Speech assist "Share corrections" with a consent confirmation where no record speaks; suggestion records now keep their text, recording and span. Next: per-language sets once texts carry a language, and a corrections leaderboard in the paper |

## Expand the suite
| # | Item | Status | Notes |
| --- | --- | --- | --- |
| 36 | WASM plugin sandbox + permission prompts | ⬜ | |
| 37 | wugs marketplace: a package index (one git repo + GitHub Pages; entries pin each package's commit and file hashes) over packages hosted on their authors' GitHub or Hugging Face; a Packages panel showing your setup by type with drag-to-install, updates and removal; kits, models, datasets and integrations first (code plugins after 36); extra index URLs for private or institutional indexes | ⬜ | Was "Kit Library"; widened by the owner 2026-11-10. Design `docs/design/PACKAGES.md` §4–6 |
| 54 | Publish from wugs: sign in to GitHub (device code) or Hugging Face, push the package, tag a release, open the index pull request | ⬜ | Staged: a ready-to-publish folder first, then GitHub, then Hugging Face. Design §5 |
| 38 | Analysis layer: stats, charts, maps, notebook | ⬜ | |
| 39 | Datasets layer: Glottolog, WALS, Grambank connectors | ⬜ | |
| 40 | Typology starter kit (proves custom kits work) | ⬜ | |
| 41 | Corpus kit (Scrolls): tokenizer, tagger, treebank, concordance | ⬜ | |
| 42 | Historical kit (Glyphs): facsimile viewer, sign/n-gram analysis, cognate aligner | ⬜ | |
| 43 | Publishing layer: dictionaries, grammars, community apps | ⬜ | |
| 44 | Mobile companion (evaluate Tauri 2 mobile first) | ⬜ | |
| 56 | Web version: wugs in a browser tab (We'll build in a seperate Repo), nothing to install, for trying it out, teaching and quick work away from one's own laptop | ⬜ | Owner idea 2026-11-10, for later. Feasible shape: the same TypeScript UI with its Tauri `invoke` layer swapped for a WASM build of the Rust engine (`wugs-model`/`oplog`/`core` and tract compile to WASM; SQLite via its official WASM build; the op log and blobs in the browser's private file storage, OPFS), so a web project is a real wugs project that can be exported and opened on the desktop. Left out on the web: Praat and other installed programs, the OS keychain (WebCrypto keys instead), LAN discovery and direct TCP sync (sync goes through a relay, which ties to 45), large local models where memory is short. Browser storage can be evicted, so the web version keeps prompting to export or sync. Needs a design doc before work starts |
| 45 | Hosted sync and cloud AI, opt-in per project | ⬜ | Paid hosted services; must be self-hostable from the public repo |
| 55 | Optional accounts and wug points: credits earned for referrals that turn into real use, published packages that people keep using, donated corrections (human-in-the-loop data, with consent), translations and bug reports; spent only on hosted services (45) | ⬜ | Owner idea 2026-11-10, for later; proposed guardrails in DECISIONS. Waits on 45 and on the open legal-form decision |
