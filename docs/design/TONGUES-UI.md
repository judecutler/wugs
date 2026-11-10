# Tongues UI: design direction

*Decided direction (owner, 2026-11-08), shaped against real fieldwork workflows. Status: guides items 16–21 and 27; adjust as the on-screen work teaches us. Mirrored at the claude.ai Project doc `product/design/TONGUES-UI.md`.*

## The one-line pitch

ELAN's track paradigm with an editor that feels like a modern media tool: a waveform on top, annotation tracks stacked beneath it, a media bin at the side, and panels the linguist arranges to fit their work. "ELAN, but it doesn't feel like 2006."

## Interop first (owner principle)

**wugs wins by being the best citizen of the existing ecosystem, not a replacement for it.** Without incorporation of the tools linguists already use, wugs is just another tool in the mix. Three tiers of incorporation, in order of preference:

1. **Files round-trip losslessly** — shipped: ELAN `.eaf`, FLEx LIFT/FlexText, Toolbox SFM, Praat TextGrid, each with a fidelity harness and nothing lost silently.
2. **Running tools get live handoffs** — see Praat integration below.
3. **Installed tools become compute engines** — headless invocation of what the user already has, results rendered in our UI.

Never require a tool the user doesn't have; always degrade to the file tier.

## Layout: dockable panels, saved as layouts

- Panels (media bin, timeline, lexicon, text list, search, conflict queue…) are dockable and rearrangeable, Chrome-tabs style. **We do not build the docking system**: use a maintained library (e.g. dockview, MIT) per the prefer-existing-tools rule. Layout state serializes to JSON.
- A saved layout is a **kit preset**: "tools = views over shared data; kits = manifests" was the architecture all along. Tongues ships as the default layout; a rearranged layout is a user preset, and the Kit Builder (item 26) later edits the same shape.
- The media bin lists Sources (speaker, date, access level); dragging a recording to the timeline calls the shipped `add_source`/`add_media_track` intents.

## Timeline: waveform + tracks = the tier model

- **Waveform is ours**, canvas-rendered from `wugs-peaks` (2 h WAV → peaks in ~1.1 s, µs queries; gate 2's engine half). Praat is an exchange partner, not the renderer.
- **A "track" is a TierDef** — the data layer already implements ELAN's tier types (time-aligned, symbolic subdivision, symbolic association). "Add a track" offers tier templates: transcription (IPA or orthography), translation, notes, custom. Each carries its tier role (`transcription`, `translation`, `note`). There is no gloss template: glosses live on morphs, under the words of a transcription, and the Gloss panel shows them.
- Everything in the timeline and track lanes is **canvas, not DOM**. Tongues runs on WebKitGTK on Linux — the weakest webview — and gate 2's smooth-scroll half is measured there, on-screen.
- Visual identity: the hand-drawn warmth (the wug) lives in chrome, empty states and onboarding. The waveform, track text and IPA rendering stay crisp and dense — transcribers stare at this screen for hundreds of hours.

## Transcription assist (machine IPA), item 27

- **Assist per segment, never transcribe-the-file**: the linguist selects a segment and asks for a suggestion, or queues segments as a background job with progress. Fieldworkers correct machine output; they don't accept whole-file passes.
- The recognizer is a **pluggable optional extension** (universal phone recognizers: Allosaurus/MMS-family). The track UI ships first and works fully by hand; the model drops in later. Conversion settings (model, language hint, phone set) are adjustable per project.
- **Machine output enters the op log as ops with machine provenance** and a stored confidence (e.g. `x.asr.confidence`). Provenance is scholarly data: "which transcriptions did a human verify" must be answerable forever.
- **Confidence coloring**: segments tint yellow (unsure) or red (very unsure) from the stored confidence. A human edit clears the tint — the color means *unverified*, not merely uncertain. Must stay visually distinct from validator warnings.

## Gloss track and the lexicon loop (items 21–22)

The gloss track **autocompletes from the project lexicon**, and glossing an unknown form offers to add it — the FLEx loop that makes the lexicon grow as a side effect of transcription. Morphs, glosses and sense links are already in the core; Leipzig abbreviations become project config (DATA-MODEL §13.2) before this ships.

## Praat integration (the user's installed Praat; new roadmap row 46)

Praat cannot be embedded (monolithic GUI, no embedding API; X11 reparenting is dead on Wayland/macOS). Two doors it deliberately provides:

1. **Open selection in Praat** (the ELAN-beloved button, plus the loop ELAN never closed): write a clipped WAV + TextGrid of the current tiers, open Praat zoomed to the selection via the `sendpraat` remote protocol (tiny public C source; implement or bundle). On save, **re-import the TextGrid** — the importer's origin dedupe merges boundary edits as ops. Fallback everywhere the remote protocol is flaky (Wayland): launch a fresh Praat with the files as arguments.
2. **Headless analysis overlays**: `praat --run` with our scripts computes pitch, intensity and formants using the user's own binary; wugs parses the output and draws the curves **in our waveform panel**, cached in `cache/` keyed by (blob, params) like peaks. Praat's numbers, our canvas, no second window.

Detection: PATH + standard install locations, overridable in settings. No Praat installed → both features hide; the file tier still works.

## Deferred, deliberately

- **Syntax track**: automatic parsing of under-documented languages isn't real, and manual tree annotation is a minority fieldwork workflow — Scrolls (corpus kit) territory. The panel architecture lets it arrive later as just another tool.
- Full-file batch recognition, keyboard layouts/RTL rendering (with items 20/26), ICU tailored sort (edge upgrade).

## Build order (reshapes items 16–21)

1. Dock shell + saved layouts (item 16 grows this scope, plus the auth screens from PR #20's follow-ups)
2. Timeline: canvas waveform + tier lanes + segment editing (items 17/19; closes gate 2 on-screen)
3. Transcription editor on the tracks (item 20)
4. Gloss track + lexicon loop (item 21)
5. Praat handoff + overlays (row 46)
6. Recognizer plug-in (item 27)
