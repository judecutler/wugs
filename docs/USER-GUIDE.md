# Tongues user guide

Tongues is the fieldwork kit of wugs. It takes you from a recording to a transcribed, glossed text, a lexicon, and files that ELAN, FLEx, Praat, and language archives accept. Everything in this guide works offline on your own computer, unless a step says **Online**.

This guide follows the usual order of fieldwork. Each section starts with what it lets you do. To change which panels Tongues shows, or to make a kit of your own, read [Kit authoring](KIT-AUTHORING.md).

A few words appear throughout:

- A **project** is one folder on your computer. It holds everything about one language or one corpus.
- A **text** is one recording session or document inside a project.
- A **track** is one row of annotations under the waveform, such as the utterances or their translation. ELAN calls a track a tier.
- A **segment** is one annotation on a track: a stretch of time with its transcription.

## Install wugs and create a project

To install wugs, follow [Install a build](INSTALL.md). The installers are not code-signed yet, so Windows and macOS warn you the first time. That page tells you how to get past the warning.

To create your first project:

1. Start wugs. With no project open, it shows the welcome screen.
2. Under **Your researcher id**, type your name or initials. wugs records this id on every edit you make.
3. Click **Create a project**.
4. Choose an empty folder. The project lives in that folder.

To open a project that already exists, click **Open a project folder** and choose its folder. wugs opens the last project again the next time it starts; if it cannot, the welcome screen offers **Reopen** with the folder's name. With a project open, the **Project** menu in the top bar opens or creates another one, and the top bar shows the project's folder and your researcher id.

To start a text, open the **Texts** panel, type a title in **New text title**, and click **New text**.

## Try the sample project

On the welcome screen, click **Open the sample project**. It holds a short made-up recording of someone counting to ten in an invented language, already segmented, transcribed, translated, glossed and in the lexicon. The last segment is left empty so you can try transcribing it.

The first time you open it, a short tour shows what each panel is for. Press **Escape** or click **Skip the tour** to leave it at any point. To see it again, press **Ctrl+K** and choose **Take the tour**.

The sample is an ordinary project kept in wugs's own folder on this computer. Change anything you like. It is never mixed into your own projects.

## Switch between Tongues and Scrolls

wugs comes with two kits: **Tongues** for fieldwork (recordings, transcription, glossing, the dictionary) and **Scrolls** for corpus work (every occurrence in context, searching, glossing sentences). Click the kit's name at the top left, such as **Tongues**, and choose the other kit.

Your project stays open and nothing in it changes. Only the panels and their arrangement change, and each kit remembers its own arrangement. You can also press **Ctrl+K** and type "switch". **Manage kits…** in the same menu opens the Kits panel, where you can make your own kit.

## Arrange the workspace

The workspace is made of panels: Timeline, Media, Texts, Transcribe, Gloss, Lexicon, Elicit, Search, Speech assist, People, Devices, and Conflicts. Panels that share a space are stacked like pages behind binder dividers. The dividers rest as a thin coloured edge along the top of each panel and rise when you move the pointer to that edge, pushing the panel down a little so nothing is covered. Click a divider to bring its panel to the front. Drag a divider to move its panel, or drop it on another panel to stack them. wugs remembers your arrangement.

Click **Settings** in the top bar for the rest. A panel slides in from the right; Escape or **Close** puts it away.

- **Colours:** each choice shows a small picture of the workspace in its colours. Click one to use it: **Cream** (the default), **Seafoam**, **Clay** or **Night**. The choice is kept on this computer.
- **Patterns:** each family shows its three patterns: the one for the page behind the panels, the one across the side panels, and the one for emphasised strips (the tier names beside the timeline, the IPA palette, Import and export). Click a family to use it, or **No pattern**. The strip pattern stays off until you turn on **Pattern on emphasised strips**.
- **Workspace:** **Always show panel tabs** keeps the dividers up. Now and then the kit's wug peeks over the top of the panel you are working in; it never comes while you type, hold the mouse button, drag, play audio or record, and it hides when you touch it. To stop its visits, turn off **Let the wug visit**. A panel you closed is listed under **Closed panels**; click it to bring it back. **Reset layout** puts every panel back where Tongues starts.
- **Updates:** see [Keep wugs up to date](#keep-wugs-up-to-date).

### Find any action

Press **Ctrl+K** (**⌘K** on a Mac), or click **Find an action** in the top bar. Type a few letters of what you want, such as "split", "night" or "lexicon", then press **Enter**. The list covers opening panels, colours and patterns, timeline actions, copying an example, and the Project menu. Each row shows its key, if it has one. An action that cannot run yet stays in the list, greyed, and says why.

Press **?** anywhere outside a text field to see every keyboard shortcut.

## Bring in a recording

### Import an audio file

To work on an existing recording:

1. Select a text in the **Texts** panel.
2. In the **Timeline** toolbar, click **Import audio…**.
3. Choose the file.

You can also drag the file from your file manager and drop it anywhere on the wugs window. The window says what a drop will do while you drag. You can drop several files at once; wugs imports them one after another and shows its progress. If you open another project before they are done, the rest are not imported; drop them again. Only files are imported: a link dragged from a web browser does nothing.

wugs reads WAV, M4A (AAC or Apple Lossless), MP3, FLAC, Ogg Vorbis, and AIFF. It decodes compressed files to WAV, the format archives ask for, and keeps the WAV in the project. It does not keep the original compressed file. The recording loads on the timeline when the import finishes.

wugs cannot read Opus, WMA, AMR, or video files other than MP4. Convert those to WAV first. A compressed recording with three or more channels is mixed to mono when it is decoded; a WAV keeps its channels, and the timeline draws the first two.

To show a recording that is already in the project, pick it from the recording list in the Timeline toolbar.

### Record in the app

To record a speaker directly into the project:

1. Open the **Media** panel.
2. If you have more than one microphone, choose one from the **Microphone** list.
3. Click **Record**.
4. Click **Stop** when the speaker finishes.
5. Type a name for the recording and click **Save**.

If you are not ready to name it, click **Keep for later**. To throw the take away, click **Discard**, then click **Really discard?**.

wugs saves the recording to disk every few seconds while you record. If the computer crashes or loses power, you lose at most the last five seconds. While a recording is in progress or unsaved, wugs does not let you open another project or close the window.

Recording is meant to work on Windows and macOS. On Linux, the system's web view may not allow recording. In that case the Media panel says "This system's web view cannot record audio", and you import a file instead.

### The media bin

Below the Record button, the **Media** panel shows each recording as a card: its waveform, name, length, sharing level and the texts that use it. The card of the text on screen has a blue outline and says **In this text**. To use a recording for the text on screen, drag its card onto the Timeline, double-click it, or click **Use for this text**. The text remembers it, so every panel can play its segments.

## Segment the recording

A segment marks where one utterance, or one word, starts and ends. The Transcribe and Gloss panels work segment by segment.

### Let wugs find the speech

To make segments automatically:

1. Load the recording on the **Timeline**.
2. In the toolbar list beside **Auto-segment**, choose **by utterance** or **by word**. Use **by word** for word lists and counting.
3. Click **Auto-segment**.

wugs finds where speech starts and ends and adds one segment for each stretch. It never changes segments you drew yourself. A message tells you how many segments it added and offers **Undo**.

Auto-segment measures loudness only. Wind, a generator, or other people talking can count as speech, and very quiet speech can be missed. If the recording has almost no pauses, you may get one long segment. Switch to **by word**, or draw the segments by hand.

### Draw and change segments by hand

- To draw a segment, drag across a track lane under the waveform.
- To move an edge, drag it. A drawn edge, or an edge you drag outward, snaps to the nearest place where speech starts or ends, within a quarter of a second. To place an edge freely, hold **Alt** while you drag.
- To type in a segment directly on the timeline, double-click it.
- To delete a segment, select it and press **Delete**, then click **Delete** in the box that opens. Click **Keep** to cancel.
- To add a track, choose a kind from the **Add track…** list: transcription, translation, notes, or custom. A custom track asks for a name.

### Play, zoom, and move around

The row under the Timeline toolbar works like the transport in a video editor. It has buttons to go to the start, play or pause, and go to the end. Beside them is the playhead time and the recording's length. At the right are the zoom controls: **−**, a slider, **+**, and **Fit**, which shows the whole recording.

Click the ruler or the waveform to move the playhead there. While the timeline has focus, these keys also work:

| Key | Action |
| --- | --- |
| **Space** | Play or pause. |
| **J** | Jump back two seconds. |
| **K** | Stop. |
| **L** | Play. |
| **Home** and **End** | Go to the start or the end. |
| **+** and **-** | Zoom in or out. |
| **Shift+Z** | Fit the whole recording on screen. |

### Lock, hide, and mute tracks

Each track has a header at the left of the timeline, as in a video editor:

- The lock button stops a track's segments from changing. You can still select them and play them, but drawing, dragging, typing, splitting, merging, nudging, deleting and Auto-segment are refused with a message. Click the lock again to unlock the track. Changes brought back from Praat are not stopped by a lock.
- The eye button hides a track's segments, so you can work on another track without them in the way. A hidden track cannot be edited either. Hiding the track of the selected segment clears the selection.
- The speaker button beside **Recording** mutes the Timeline's playing. The playhead still moves, so you can follow along silently. Replaying a segment from Transcribe still plays aloud.

These settings belong to one text on this computer. They are not shared with other devices and they do not change the project.

### Split, merge, and nudge

Select a segment first. Then use the toolbar buttons or these keys while the timeline has focus:

| Key | Action |
| --- | --- |
| **S** | Split the selected segment at the playhead. The text stays on the first part. |
| **M** | Merge the selected segment with the next one on its track. The texts are joined with a space. |
| **[** and **]** | Move the start edge 10 ms earlier or later. |
| **Shift+[** and **Shift+]** | Move the end edge 10 ms earlier or later. |
| **Space** | Play or pause. |
| **Delete** or **Backspace** | Ask to delete the selected segment. |
| **Escape** | Clear the selection. |
| Mouse wheel | Zoom in or out around the pointer. |
| **Shift** + mouse wheel | Scroll the timeline sideways. |

To split, first click the waveform inside the selected segment to put the playhead there.

### Undo a timeline edit

Click **Undo** or **Redo** in the Timeline toolbar. You can also press **Ctrl+Z** to undo, and **Ctrl+Shift+Z** or **Ctrl+Y** to redo, while the timeline has focus. On a Mac, use **Cmd** in place of **Ctrl**.

Undo covers the timeline's own edits: drawing, dragging, nudging, typing on the timeline, deleting, splitting, merging, and Auto-segment. It does not cover edits made in other panels, or changes brought back from Praat.

Undo never overwrites later work. If the segment changed since the edit, on this computer or another, wugs refuses the undo and tells you why. Undoing an Auto-segment takes back only the segments nobody edited since, and says how many it kept.

The undo history belongs to one text and lasts until you close wugs or switch projects.

## Transcribe

### Type transcriptions with the keyboard

The **Transcribe** panel lists the segments of one track. It follows the Timeline: select a segment there and Transcribe moves to it.

1. Load the text's recording on the Timeline, so you can hear the segments.
2. Click a segment in the list, and press **Tab** to hear it.
3. Type what you hear.
4. Press **Enter**. wugs saves the text, moves to the next segment, and plays it.

| Key | Action |
| --- | --- |
| **Enter** | Save, go to the next segment, and play it. |
| **Shift+Enter** | Go to the previous segment. |
| **Tab** or **Ctrl+Enter** | Play the segment again. |
| **Esc** | Undo your typing in this segment. |
| **Shift+Tab** | Leave the field. |

If a save fails, the segment is marked and your typing stays. Select the segment and press **Enter** to try again.

### Type IPA symbols

Click **IPA** in the Transcribe panel to show or hide the IPA palette. It has groups for vowels, consonants, diacritics, suprasegmentals, and tone. Click a symbol to insert it.

### Get machine suggestions

Speech assist suggests a phonetic transcription, in IPA, for empty segments. The model runs on your computer. Your recordings are never sent anywhere.

Before the first use, download a model:

1. Open the **Speech assist** panel.
2. Under **Models**, click **Download** beside a model. This step is **Online**. The wav2vec2 model is about 640 MB.

To fill every empty segment of the open text:

1. Load the text's recording on the **Timeline**.
2. In **Speech assist**, click **Suggest for N empty segments**.

Each segment takes a few seconds. Click **Stop** at any time. The segments already filled stay filled.

To fill one segment, select the empty segment in **Transcribe** and click **Suggest**.

Suggestions only ever fill empty segments. They never replace text a person typed.

### Read the machine tints

A segment that holds a machine suggestion is tinted on the timeline and in Transcribe:

- **Yellow** means a person has not checked the suggestion yet.
- **Red** means the model was less than 50% sure.

Edit the text to mark it as checked. The tint then goes away. In Transcribe, hover over a tinted row to see which model wrote it and how sure it was.

### Choose or pin a model

wugs asks for a job, phone transcription, and not for one model. In **Speech assist**, the **Which model** list shows the choice:

- **Automatic** picks the downloaded model with the best published score. The list shows which model that is.
- Any downloaded model, if you want that one.

This choice applies to this computer. To make everyone on the project use the same model, click **Pin for this project**. A long transcription campaign then never switches model halfway. Click **Unpin** to let each computer choose again.

If a pinned model is not downloaded on this computer, wugs says so by name. It never uses a different model in its place.

To remove a model's files, click **Delete** beside it, then click **Delete? Click again**. Transcriptions the model wrote stay.

### Share your corrections

Each suggestion you checked pairs what the model heard with what you wrote. These pairs measure how well models work for your language, and can help train better ones.

1. In **Speech assist**, go to **Share corrections**.
2. Read what is ready and what is left out.
3. Click **Export N corrections…**.
4. Choose a folder.

wugs writes a folder named `wugs-corrections-<date>` with one audio clip per segment, the two transcriptions, and a README datasheet. Nothing is sent anywhere.

By default, wugs leaves out clips that your People records speak against, such as a recorded "No" for AI use or a restricted access level. To include them anyway, select **Include them**. The datasheet records your choice.

## Gloss

The **Gloss** panel shows one segment as interlinear glossed text, in Leipzig style. Select a segment on the Timeline or in Transcribe to gloss it.

### Break a segment into words

Click **Break into words**. wugs splits the transcription on spaces and keeps punctuation out of the words.

If you change the transcription later, the panel says "Transcription changed since these words were broken out." Click **Retokenize**. Words that did not change keep their morphs and glosses.

### Write the morphs and glosses

1. In the breakdown field under a word, type the word's morphs with the usual markers.
2. Type a gloss for each morph in the cell under it.
3. Press **Tab** or **Enter** to move to the next cell.

The breakdown field takes these markers:

| You type | Meaning |
| --- | --- |
| `-ler` | suffix |
| `ka-` | prefix |
| `=ki` | clitic |
| `re~` | reduplication |
| `<in>` | infix |
| `∅` | zero morph |

Grammatical glosses in capitals, such as `LOC` or `PL`, show in small caps.

The gloss cells suggest senses from the lexicon as you type. Press **Enter** on a match to link the morph to that lexicon sense.

### Add unknown words to the lexicon

When a root is not in the lexicon, the panel offers **Add «form» 'gloss' to the lexicon**. Click it to make the entry. The Lexicon panel shows it at once.

### Reuse earlier analyses

When you meet a word you analysed before, its breakdown field shows the earlier analysis in grey. Press **Enter** on the empty field to accept it. wugs never applies an analysis unless a person accepts it.

To give the same analysis to every other unanalysed copy of a word, click **Apply to N other unanalysed «form»**. When more than 20 words would change, click a second time to confirm. Words that already have an analysis are left alone.

### Clear an analysis

Emptying the breakdown field does not remove the analysis. To remove it, click **Clear analysis**. Nothing is deleted from the project's history.

### Add a second gloss language

Click **+ language** and type a language tag, such as `es` or `tpi`. A second gloss row appears for that language.

### Add a free translation

A free translation lives on its own track, lined up in time with the utterance.

1. If the text has no translation track, click **Add translation track**.
2. Type the translation under **Free translation**.

The segment needs start and end times for this. If it has none, give it times on the Timeline first.

### Copy an example into a paper

Click **Copy example** at the top of the Gloss panel. The segment is copied as a numbered example: the words split into morphs, the glosses under them with grammatical glosses in small capitals, and the free translation in quotes. Then paste it into your paper.

Choose where it will go in the list beside the button:

- **Word / Google Docs** pastes as a table that lines the morphs up with their glosses. It also works in LibreOffice and email.
- **LaTeX (gb4e)** and **LaTeX (ExPex)** paste the lines those packages expect. They number the example themselves. With pdflatex, add `\usepackage[T1]{fontenc}` so IPA tildes print as tildes; XeLaTeX and LuaLaTeX need nothing extra.
- **Plain text** lines the columns up with spaces.

wugs remembers the choice on this computer. The number is the segment's place in its text. A morph with no gloss yet shows **?**, so you can see the gap before you send the paper. Grammatical glosses paste as slightly smaller capitals, which read as small capitals in every word processor.

## Build the lexicon

The **Lexicon** panel has a list of entries beside an editor for the selected entry.

### Add and edit entries

1. Type a word in **Lemma** and its meaning in **Gloss**.
2. Click **Add entry**.

To find an entry, type part of its lemma or gloss in **Find an entry by lemma or gloss**.

In the entry editor you can set the lemma, the citation form, the category (part of speech), and the homograph number. Each sense has a gloss, an optional gloss in the Gloss panel's second language, a definition, and a usage note. Click **Add a sense** for another sense. Fields save when you press **Enter** or leave the field.

When two entries share a lemma, the editor says so. Click **Number them** to give each one a homograph number.

The editor shows how many words in your texts use the entry.

To delete a sense or an entry, click **Delete sense** or **Delete entry**, then click again to confirm.

### Give entries semantic domains

1. If the project has no domains yet, click **Load SIL Semantic Domains**.
2. In a sense, type a word or an outline number, such as `moon` or `1.1.1`, in the domain picker.
3. Use the arrow keys to move through the matches, and press **Enter** to pick one.

Each domain shows as a chip on the sense. Click its remove button to take it off.

## Elicit words

The **Elicit** panel collects words with a speaker in two ways. Every answer becomes a lexicon entry, so you never copy a word list over by hand.

### Work through a word list

1. In **Elicit**, open **Word lists**.
2. Click **Add a word list**, then click **Load** beside Swadesh 100, Swadesh 200, or Leipzig-Jakarta.
3. Type the speaker's word in the word field of the current concept. Add a gloss if you want one.
4. Press **Enter**. wugs saves the answer and moves to the next concept still to do.

The progress bar counts answered concepts. Use **To do**, **Done**, and **All** to filter the list.

- If the word already exists in the lexicon, wugs offers **Use** with the entry's lemma and gloss. Click it to link the existing entry instead of adding a second one.
- To take an answer back, click the × on its chip. The entry stays in the lexicon.
- If a concept does not apply to the language, click **N/A**. Any text in the gloss field is kept as the reason.
- Press **Enter** on an empty word field to skip ahead without answering.

The concept lists come from Concepticon. Each answer stays linked to its Concepticon concept, so the list can later be compared across languages.

### Collect words by meaning

1. In **Elicit**, open **Semantic domains**.
2. If the project has no domains, click **Load SIL Semantic Domains**.
3. Find a domain by word or outline number.
4. Ask the speaker the SIL questions the panel shows.
5. Type each word and its gloss, then press **Enter** or click **Add**.

Each word becomes a lexicon entry in that domain. The field keeps focus for the next word.

## Record people and consent

The **People** panel records who speaks in your recordings and what each person agreed to. It is optional.

These records inform and never block. The corrections export and sealed archive packages report what the records say. Nothing in wugs is blocked by them. Your ethics board's rules are the ones that count.

To add a person:

1. Type a name in **Name**.
2. Choose a role, such as speaker or consultant.
3. Click **Add person**.

To record what they agreed to:

1. Click **Record what they agreed to**.
2. Under **Covers**, choose what the record covers: the whole project, one recording, or one text.
3. Set the date.
4. For each term, click **Yes**, **No**, or **Not asked**. The terms are "Deposit in an archive", "Publish online", "Share within the community only", and "Use to build or test speech tools (AI)".
5. To keep the signed form with the record, click **Attach the signed form…** and choose a scan, photo, or PDF.

Click **Add another record** for a second record. To delete a person or a record, click **Delete** or **Delete record**, then click again to confirm. Deleting a person also deletes their records.

An attached form is stored in the project, so a sealed package contains it. Treat sealed packages as personal data.

## Work with Praat

wugs works with your own installed copy of Praat, version 6 or newer. It finds Praat on the system path, in the usual install places, and in `Praat` folders in Desktop, Downloads, or Documents. If wugs cannot find it, the Timeline toolbar shows **Praat…**. Click it and choose the Praat program.

### Show pitch, intensity, and formants

Click **Pitch**, **Intensity**, or **Formants** in the Timeline toolbar. Your Praat analyses the recording, and wugs draws the curves over the waveform.

Praat analyses at most one minute of view at a time. If more is on screen, zoom in. wugs keeps the results, so a part you already viewed shows at once next time.

The analysis uses Praat's standard speech settings. Pitch is tracked between 75 and 600 Hz, so a voice outside that range shows no pitch.

### Edit in Praat and bring the changes back

1. Select a segment, or scroll to the part you want.
2. Click **Open in Praat**. Praat opens the text and the recording, zoomed to the selected segment with a second on each side, or to what is on screen.
3. Edit boundaries and labels in Praat. Keep the TextGrid open in Praat.
4. In wugs, click **Bring back from Praat**.
5. Read the summary of what changed and what is not brought back.
6. Click **Apply** to write the changes, or **Not now** to wait.

Moved boundaries, changed labels, and new labelled intervals come back. Clearing an interval's label in Praat removes that segment. Merging two intervals in Praat keeps the first segment and removes the second.

Tiers you add in Praat, and changes Praat cannot represent, are reported and left alone. To stop the handoff, click **Discard**, then click **Discard: sure?**. Changes already brought back stay.

Bring back needs the TextGrid still open in Praat. If you closed Praat, nothing comes back. Click **Discard** and start again.

A change brought back from Praat replaces another device's edit to the same segment, if that edit arrived since you opened Praat.

## Import, export, and seal

All of these are in the **Texts** panel, under **Import and export**.

### Import files from other tools

1. Click **Import a file…**.
2. Choose the file.

Or drop the file anywhere on the wugs window.

wugs reads these formats:

- ELAN `.eaf`
- FLEx `.flextext` and LIFT `.lift`
- Praat `.TextGrid`
- Toolbox `.db`, `.sfm`, and `.lex`, and `.txt` files in Toolbox format
- Plain text `.txt`: any other text file becomes a text named after the file, and the message says how it was cut. A file whose lines mostly end in a full stop, with no blank lines, comes in a segment per line. Otherwise blank lines separate paragraphs, a line ending in a full stop ends its sentence, and inside a line a sentence ends after `.`, `!`, `?` or the full stop of another script when the next word starts with a capital, a digit or a quote; "e.g. the" stays together, but "Dr. Smith" is cut. Thai and Lao paragraphs come in whole, and an orthography written all in lower case is cut only at line ends and blank lines. The file must be UTF-8 or UTF-16; a damaged one is refused. Importing the same file again adds nothing.

A message reports what wugs imported, and the new text is selected. The import report also counts anything the format holds that wugs does not keep. For FLEx, that includes LIFT examples, notes, pronunciations, relations, and etymologies.

An imported ELAN or Praat text is not linked to its recording. Import the audio with **Import audio…** on the Timeline and pick it there.

### Export a text or the lexicon

To export a text, select it, choose ELAN (.eaf), FLEx (.flextext), Praat (.TextGrid), or plain text, and click **Export…**. Plain text writes one track a segment per line, in time order, which AntConc and most corpus tools read.

To export the lexicon, choose LIFT or Toolbox beside **Lexicon as** and click **Export…**. FLEx and WeSay read LIFT.

wugs names the file after the text's title and never replaces a file that exists. It adds " (2)" to the name instead. You cannot export into the project folder itself.

### Seal a version for the archive

1. Click **Seal a version for the archive…**.
2. Choose a folder outside the project folder.

wugs writes a BagIt package named `wugs-sealed-<date>`. It holds ELAN, FLEx, and LIFT copies, OLAC and IMDI metadata, every recording, the full history, and checksums for every file. ELAR, PARADISEC, and AILLA accept BagIt deposits.

If any of the project is restricted, the message says so. The package then holds restricted data, so treat the whole package as restricted.

Sealing copies every recording. Other actions wait until it finishes.

## Search the project

Type in the **Search** panel. Choose **All**, **Texts**, or **Lexicon** to narrow the results. Results are grouped by text. Click a result, or select it and press **Enter**, to jump to that segment.

Search ignores accents in both directions, so `e` finds `é`.


## See every occurrence in context

The **Concordance** panel lists every place a word is used, one line per occurrence, with the word in the middle and what was said before and after it. It is a tab beside Lexicon. If your layout predates it, press **Ctrl+K** and choose **Show Concordance**.

1. Choose what to look for: **Words** as transcribed, **Morphs**, **Glosses** (such as PL or 1SG), or words of **Translations**.
2. Type it. Capital letters do not matter, but diacritics do: ŋa never finds na. Morph markers do not matter either: `-ra` and `ra` find the same morph. Use `*` for any letters (`ni*` finds every word starting with ni) and `?` for one letter.
3. Sort the lines **By the word before** or **By the word after** to see patterns, as in a concordancer such as AntConc.

Click a line to open its segment on the Timeline and in Transcribe and Gloss. Click ▶ to play it. **Copy rows** copies every line as a table you can paste into a spreadsheet. A morph or gloss line also shows the whole word's analysis under it.

## Count words, morphs and glosses

The **Frequencies** panel lists the words of the project with how often each occurs, most frequent first. It is a tab beside Search in Scrolls; in Tongues, press **Ctrl+K** and choose **Show Frequencies**.

1. Choose what to count: **Words**, **Morphs**, **Glosses** or words of **Translations**.
2. Count in the **Whole project** or pick one text.
3. Sort by count, alphabetically, or by how many texts a word is in, and type in **Filter the list** to find one.

**Per 1000** is how often it occurs in every thousand. Capital letters count together (Kesi and kesi are one word, shown as the spelling used most); diacritics do not (ŋa and na are two). A morph or gloss counts once per word that has it, so a word with PL twice counts once. Click a row to see its occurrences in the Concordance, looked up exactly (a `?` in a word is a letter there, not a wildcard) and in the chosen text: the number of lines matches the count. The list counts again when the project syncs or texts change; after glossing or typing, click **Recount**. Up to 5,000 items are listed and the first 500 drawn; filter to find the rest. The **#** column is each item's frequency rank, whatever the order shown. **Copy table** copies the list to paste into a spreadsheet.

## Hear any segment from anywhere

Every segment has a ▶ beside it: the lines in Transcribe, the top of the Gloss panel, each line of the Concordance, each segment found by Search, and the words a lexicon entry is used in. In the Lexicon, click **used in 3 words** (or however many) under a sense to list those words, then ▶ to hear each one.

▶ plays from the text's own recording. A text learns its recording the first time you pick a recording for it in the Timeline's list (or import or drop one while the text is open), and from then on the Timeline opens the text with it and ▶ works from every panel, even while another text is on the Timeline. A text whose recording was never picked plays only while it is the text on screen and the recording loaded is not another text's; elsewhere ▶ says to pick its recording first. The sample project comes with its recording picked.

A text with no sharing level of its own shares like its recording, so picking a restricted recording makes the text restricted too; wugs says so when a pick changes this.

## Work with other devices

Two or more computers can share one project. They sync directly over the local network, with no server.

### Pair a new device

On a computer that already has the project:

1. Open the **Devices** panel.
2. To let the new device read but never send changes, select **Guest (reads open material only, never sends)**.
3. Click **Show pairing code**. The panel shows an eight-digit code, the network port, and when the code expires.

On the new computer:

1. On the welcome screen, type your **researcher id** and click **Join a project from another device**. wugs shows the **Devices** panel.
2. In the **Devices** panel, go to **Join a project**.
3. Under **Address**, type the first computer's network address and the port it shows, as `host:port`, for example `192.168.1.20:41234`.
4. Under **Code**, type the eight-digit code.
5. Click **Choose a folder and join**, and choose an empty folder.
6. Sync with the first computer to bring the project over.

A guest sees only material whose access is open, and never sends changes back.

### Sync

1. On one computer, click **Make this device reachable**.
2. On the other, click **Find devices**, then click **Sync** beside the device it finds.

If **Find devices** finds nothing, type the other computer's address as `host:port` in the field under **Sync** and click **Sync now**.

If a computer is lost or stolen, click **Mark lost…** beside it on another computer, then **Mark lost**. A device marked lost can never sync again.

### Settle conflicts

When two people change the same thing before they sync, wugs keeps both versions and shows one. The top bar shows how many conflicts are open; click that count to open the **Conflicts** panel, which lists them.

Each card says what changed, where it is, and every version with its device and time. "shown now" marks the version the project shows.

- To choose a version, click **Keep this** beside it.
- To write a new value instead, type it in **Or type the right value** and click **Use this**. This is offered for plain-text fields only.
- To see a segment's context, click **Show**.

If someone else settled the conflict while you were reading, wugs refuses your choice and reloads the card. Choose again.

In the Gloss panel, a word that another device analysed differently shows the other analysis with **Use that instead**.

## Keep wugs up to date

wugs checks GitHub once at start for a newer release. This is **Online**. When one exists, a line under the top bar offers **Install and restart**, **Later**, and **What's new**. Nothing downloads until you click **Install and restart**.

To stop the check, open **Settings** in the top bar and, under **Updates**, turn off **Check when wugs starts**. For details, read the updates section of [Install a build](INSTALL.md#updates).

## Fix problems

| Problem | What to do |
| --- | --- |
| A panel is missing. | Open **Settings** in the top bar and click the panel under **Closed panels**, or click **Reset layout**. |
| An error message appears. | Read it. It says what failed. Edits already saved are kept, because wugs writes each edit to the project when you make it. Click **Dismiss** to close the message. |
| **Recovered recordings** appears in the Media panel. | wugs found recordings that were still on disk when the app last closed. Type a name and click **Import** to keep one. Click **Discard**, then **Really discard?**, to throw it away. Nothing happens until you choose. |
| wugs says the project folder was moved or copied. | If you moved the folder, click **I moved it**. If it is a copy and the original is still in use, click **Create fresh device identity**, so the two folders sync as two devices. |
| Auto-segment makes one long segment. | The recording has few pauses. Choose **by word**, or drag on a track lane to draw segments. |
| **Import audio…** refuses a file. | Convert it to WAV and import the WAV. |
| **Suggest** or **Suggest for N empty segments** is not available. | Download a model in **Speech assist**, and load the text's recording on the Timeline. |
| An undo is refused. | The segment changed since that edit. Make the change by hand. |
| A Praat button is missing. | wugs has not found Praat. Click **Praat…** and choose the Praat program. |
| **Keep this** on a conflict card is refused. | Another device settled the conflict meanwhile. The card reloads. Choose again. |

## Send feedback

When something goes wrong or could work better, tell the people who make wugs. This needs an internet connection; without one, wugs keeps your feedback and sends it the next time it can.

1. Click **Feedback** at the top right.
2. Under **What happened?**, write what you were doing and what went wrong.
3. Optional: add your email address if you would like a reply.
4. Optional: click **Attach a screenshot or video…** and choose the file. The form shows the keys that record the screen on your computer. The file can be up to 95 MB, which is a few minutes of screen recording.
5. Check what the attachment shows. A recording of the screen shows your project too: names, recordings and transcriptions.
6. Leave **Include technical details** ticked unless you prefer not to send them. **See exactly what is included** shows every line: the wugs version, your system and its web view, the kit (its id is the name you gave it, if you made your own), the theme and pattern, the window size, and whether a project is open. Nothing from inside your project is sent, and an attachment goes by its file name only, never its folder.
7. Click **Send feedback**. The form closes once your feedback is saved on this computer, and a message says when it has been sent.

Your feedback goes to the wugs maintainers privately. It is never posted publicly. Escape closes the form only while it is empty; **Cancel** closes it and discards what you wrote.

## Known limits

wugs is young. These limits are known and stated so that you can plan around them:

- The installers are not code-signed, so first installs show a warning.
- Recording in the app is not verified on Linux.
- wugs decodes compressed audio to WAV on import. It does not read Opus, WMA, AMR, or most video files.
- Auto-segment uses loudness only and has no sensitivity setting.
- On Linux, some window managers take Alt+drag for moving windows, so free edge placement may need a different setup there.
- Timeline undo is per text and lasts only until wugs closes.
- The Speech assist models take no language hint and no phone set for your language.
- The Praat overlays use fixed settings and draw no spectrogram.
- The lexicon has no editor yet for variants, examples, or cross-references.
- The Elicit panel has no paradigms, no session planning, no audio per answer, and no lists of your own.
- Devices appear as "this device" and "another device" in the Conflicts panel.

## How wugs keeps your work

wugs never overwrites your work. Every edit you make, by any person on any device, is written as a new line in a journal in the project folder, the op log. wugs adds to the journal and never changes or removes a line. A deleted segment is marked deleted, and a restore brings it back.

What you see on screen is built from the journal. wugs keeps a cache in the project's `cache` folder so it opens fast, but the cache is disposable. If it is lost or damaged, wugs rebuilds it from the journal. This is why a crash or an error message cannot lose saved work, and why the sealed package can include the project's full history.

The journal also makes sync safe. Each device writes only its own journal file and reads the others. When two people change the same thing, both versions stay in the journal, and the Conflicts panel asks a person to choose. A choice adds a new line to the journal and removes nothing, so an earlier version can always be recovered.
