# The editor pass: the video-editor playbook for a linguistics timeline

*Roadmap item 66. Owner, 2026-11-12, after installing 0.1.0: "take the playbook that CapCut, Adobe, DaVinci etc. have built and use it to make a world class linguistics software." Details follow those programs' conventions; they are not questions for the owner.*

## What people already know

Anyone who has cut a video in CapCut, Premiere Pro or DaVinci Resolve knows these, and wugs should not make them learn anything different:

| Convention | CapCut | Premiere / DaVinci | wugs |
| --- | --- | --- | --- |
| A media bin of cards: picture (or waveform), name, duration; import button and a drop zone *in the bin* | Media panel | Project panel / Media Pool | Media panel |
| Drag a clip from the bin onto the timeline to use it | yes | yes | drag a recording card onto the Timeline loads it for the text |
| A transport row: play/pause, current time / total, start and end | under the player | under the program monitor | a row above the ruler |
| Space plays and pauses; J, K, L shuttle back, stop, forward | Space | Space, J/K/L | Space, J/K/L |
| Home / End go to the start / end | yes | yes | yes |
| Zoom with a slider and − / + buttons, "fit" to see everything | slider | slider, Shift+Z | slider, −/+, Fit (Shift+Z) |
| A header at the left of each track: name, lock, hide (eye), mute for audio | lock, hide, mute | lock, eye, mute, solo | lock and hide on every track, mute on the audio track |
| A locked track shows but cannot be changed; a hidden one keeps its place but shows nothing | yes | yes | the same, and a refused edit says why |
| Split at the playhead (blade), S or Ctrl+B | Ctrl+B | Ctrl+K / Ctrl+B | S (segments, already built); splitting the *recording* makes separate texts (below) |
| Click the ruler to move the playhead | yes | yes | yes |

## Track state is a view, not data

Lock, hide and mute are how one person looks at one text on one computer, the way a video editor's track toggles are part of the project file on that machine and not of the footage. They are kept per text in local storage (`wugs.timeline.tracks.<text id>`), never written to the op log, so they never sync and never conflict. A lock is a guard against slips, not a permission: Transcribe and Gloss can still edit a locked track's text (they edit content, not timing), and the timeline says so when it refuses.

## Split the recording into texts

A long session recording (several stories, or an interview with breaks) is one recording and should become several texts. The editors' answer is a blade on the clip; the wugs answer is **Split recording here** at the playhead, which turns the part of the text after the playhead into a new text. This changes data, so it is built after the view work, as its own step:

- The recording is never cut or copied. Each text points at the same recording and remembers its own span of it (`media-span`: start and end in ms on the text's media link). Times stay recording times, so segments, Praat handoffs and corrections clips need no shifting.
- The new text gets the same tiers (names and roles) and the segments that start at or after the playhead move to it by a resolving write of their `tier`, so their ids, transcriptions, words and glosses stay with them. A segment that crosses the playhead stays with the first text.
- One txn, undoable like other timeline edits (it is the timeline's own edit).
- Exports (ELAN, TextGrid) and seals of a span text use its span; the DATA-MODEL change is written before the code.

## Media bin

- Cards in a grid: a small waveform drawn from the recording's peaks (the same pyramid the timeline uses, coarsest level), the name, the duration, and "In this text" on the recording the open text uses.
- Files are dropped **on the Media panel** (recordings) or **on the Texts panel** (ELAN, FLEx, Praat, Toolbox), each panel highlighting itself while a drag carries files it takes. Anywhere else a file drag is refused with the cursor, and a single line under the cursor says where to drop it. The whole-window overlay goes.
- A card can be dragged onto the Timeline to load it for the text; double-click does the same.
- Recording in the app stays at the top of the bin.

## Order of work

1. Transport row, keys, zoom slider, ruler seek; track headers with lock, hide, mute. (View only.)
2. Media bin cards and per-panel drop zones; drag a card onto the Timeline.
3. Split recording into texts (data model first).
