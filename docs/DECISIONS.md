# Decisions

*The choice, the reason and what was rejected, one entry each. How each was built is in the code, `CLAUDE.md` and the PRs, so it is not repeated here. Dated entries are what other docs cite ("DECISIONS 2026-11-11"). Mirror of the Project doc `product/DECISIONS.md`.*

## Product and licensing
- **Names** (2026-11-07). Suite **wugs**, after Berko Gleason's wug test. Kits **Tongues** (fieldwork), **Scrolls** (corpus), **Glyphs** (historical, comparative, decipherment), shown with a plain subtitle ("Glyphs — Historical"). Writing systems are platform-level because Tongues needs them for orthography design.
- **Suite structure** (2026-11-07). Kits on one shared platform; Analysis, Datasets and Publishing are layers in every kit; every feature is labelled Offline or Online.
- **License** (2026-11-07). Code MPL-2.0: file-level copyleft stops a vendor closing the engine and reselling it to the communities whose data it holds, without touching third-party kits or alarming institutions. The archive format and a reference reader are Apache-2.0 OR MIT, so anyone can read a sealed archive in 2056. DCO, never a CLA. Rejected: MIT/Apache (closeable fork), GPL/AGPL (plugin ambiguity, institutional friction), ethical-source licences (not OSI, block funding).
- **Open source, paid hosting** (2026-11-07). No paywalled features, no closed servers. Only services that cost money to run (hosted AI, sync and storage, institutional support) are paid, with a margin that funds the project, and each is self-hostable for free from this repo. The legal form behind them is still open.
- **Public repository, fresh history** (2026-11-10). The pre-release history stays private in `wugs-archive`. Public repositories get the Actions minutes the 3-OS CI needs.
- **Wug points** (2026-11-10, Proposed). Optional credits for contributions that see real use, spent only on hosted services, no cash value.

## Data and sync
- **The op log is the truth** (2026-11-07). An append-only JSONL log of domain ops stamped with HLCs; SQLite is a disposable projection; tombstones, never deletes. Rejected: a generic CRDT document as the store (binary archival lock-in, cannot hold one bad edit out of a batch, provenance bolted on) and cr-sqlite (too little maintenance to bet the format on). Cost: we own sync correctness, so it is property-tested.
- **Checked merges** (2026-11-07). Incoming ops go through quarantine and validators; failures are held. Locally, validators only warn.
- **Character merging only in text** (2026-11-07). Loro runs inside transcription-like fields, and each such op also stores the plain text, so an archive never needs Loro.
- **Seal, don't convert** (2026-11-07). Archiving writes a BagIt package (standard formats, the plain-text log, its spec) and leaves the project live.
- **Bundled SQLite** (2026-11-08). Windows has no system SQLite, and FTS5 must be the same everywhere. Expect `foreign_keys` on by default.
- **Blob refs** (2026-11-08). `blake3:<hex>` is archive format. The GC mark is read from the log, so any blob a log ever named stays.
- **Sharing** (2026-11-08). A pairing code into a flat roster, guest read-only, members see everything, unset access means open. Rejected: stewards, PINs and root keys, as ceremony that makes the tool unusable. Sent bytes cannot be unsent; revocation stops future syncs only.
- **Copied folders** (2026-11-08). A copied project would fork a device's log, so opening one warns. It never blocks.
- **Glossing per occurrence** (2026-11-09). Each occurrence keeps its own analysis, and speed comes from suggestions derived from earlier ones; nothing is applied without a person. Rejected: a shared wordform inventory, where one synced edit silently re-glosses everything. A segment's words and a word's morphs are each one list field, so concurrent analyses conflict instead of interleaving.
- **Conflicts are open versions** (2026-11-09). An op names the versions its writer saw (`heads`); a version nobody named is open. Only a person's direct edit closes a conflict, never a bulk, derived or import write, and closing is always a write, never a local dismiss. Equal values are not a conflict. A change to the op envelope's meaning bumps the sync ALPN, because an older peer would strip it silently.
- **Reference lists are attributed data** (2026-11-09). SIL Semantic Domains and Concepticon lists ship as separately licensed files and load on request, with ids derived from their own, so two loads give one copy.
- **Consent informs, never enforces** (2026-11-11). wugs shows what records and access levels say and uses them for defaults; review boards make the ethical call. Software lacks the facts, and hard rules hurt people in unusual situations.
- **Undo is an inverse write** (2026-11-11). The log is never rewritten, so undo writes the opposite edit, and it is refused when what it would overwrite changed since, anywhere. The first version overwrote later edits and could lose a session of typing.
- **A text remembers its recording** (2026-11-12). Only a person's pick links them, because the wrong audio is worse than none. Linking can change a text's inherited sharing level, so the app says when it does.
- **Track toggles are view state** (2026-11-12). Lock, hide and mute stay on one computer and never become ops; another person's lock must not stop someone who never chose it.

## Models and analysis
- **Features, not models** (2026-11-10). Code asks for a capability; packages provide it. Automatic picks by published score; a project can pin a model through a synced setting so a team uses one; a pick that cannot run is an error, never a silent swap. Large downloads ask first.
- **The marketplace hosts nothing** (2026-11-10). Packages stay on their authors' GitHub or Hugging Face; one index repository pins commits and file hashes. Rejected: topic search, which cannot pin, review or delist. Private indexes are allowed.
- **Local inference with tract** (2026-11-09). Pure Rust, so every OS builds with nothing fetched. Rejected: ONNX Runtime (a native library per platform). Only permissive weights are catalogued (no GPL Allosaurus, no NC ZIPA). A machine suggestion is an ordinary write; "unchecked" is derived from its provenance, never stored.
- **ZIPA** (2026-11-10). The graph is adjusted at load so tract runs it. Inputs are never padded: padding changed a third of the phones.
- **Evaluation** (2026-11-11). PRiSM's public sets and scorer, run in CI and not copied (no licence file). It reproduces PRiSM's published numbers; scores in the manifests drive Automatic.
- **Corrections export** (2026-11-10). Only suggestions a person checked leave, in the Kaldi layout the eval harness reads.
- **Segmentation is a capability** (2026-11-11). A built-in voice-activity provider first. Machine segments are ordinary segments. Snapping moves edges outward only; Alt places freely.
- **Praat is the person's own** (2026-11-10). Driven by `--send` and `--run` scripts. A return is compared with the grid Praat was given, never with the project, so edits made meanwhile are not read as deletions.
- **Compressed audio becomes WAV at import** (2026-11-11). One format for every reader and for archives that ask for uncompressed audio. The compressed original is not kept.
- **Plain text** (2026-11-12). Written rules cut lines and sentences, one txn per document, no tokens on import. Rejected: a sentence-splitter dependency (English-only or large).
- **First impressions, then Scrolls** (2026-11-12). Tongues must show its value at once, then Scrolls, then bundles, then Glyphs. Corpus tooling is crowded, so Scrolls connects to its formats, models and query syntax instead of rebuilding them, and stays out of the ACL paper.
- **Kits and bundles are data** (2026-11-09, 2026-11-12). A kit is a `kit.toml` with no scripts until a sandbox exists; layouts are readable placements, not saved dock state; unknown ids are kept. A kit carries its own wug. A kit file can be a package (`[package]`, `[bundle]`, pinned `[[requires]]`), and a model requirement names a capability.

## Shell, brand and delivery
- **Brand** (2026-11-11). The owner's hand-drawn wug on cream, brand blue for marks only (2:1 on cream) and a deeper blue for text. Wugs are stacked from traced parts, so a new accessory is two layers. A pattern never stops abruptly.
- **Shell look** (2026-11-11). Paper panels, binder-divider tabs that push content down rather than cover it, the app's own menus, themes as token overrides, one Settings sheet. The timeline follows video-editor conventions (2026-11-12) instead of asking the owner each detail.
- **Native drag-and-drop off** (2026-11-11). Required for dock tabs to drag on Windows. Dropped files stream in 8 MiB pieces, and nothing dragged in can navigate the app away.
- **Updates** (2026-11-10). Signed, never downloaded before the person asks. Build, sign and publish are separate jobs, so the key never meets build scripts or a write token.
- **Download page** (2026-11-11). Static, in this repo, reads GitHub Releases. Set in Junicode, not Young Serif, which reads as generated. macOS builds are ad-hoc signed so they open with the ordinary prompt.
- **Feedback** (2026-11-12). A Cloudflare Worker files each report in a private repository and emails it. Rejected: public issues (field data, no accounts), mail from the app, an in-app recorder. Reports wait in an outbox, and every step is idempotent on the report's id.
