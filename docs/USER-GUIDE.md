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

To open a project that already exists, click **Open a project folder** and choose its folder. wugs opens the last project again the next time it starts; if it cannot, the welcome screen offers **Reopen** with the folder's name. With a project open, the **Project** menu in the top bar opens or creates another one, and the top bar shows the project's folder and your researcher id. Under **Recent**, the Project menu lists the last six project folders opened on this computer, each with the folder it sits in; click one to open it. The welcome screen lists a few of them too.

To start a text, open the **Texts** panel, type a title in **New text title**, and click **New text**.

## Try the tutorial project

The first time you start wugs, click **Tutorial** in the top bar (or **Open the tutorial project** on the welcome screen). It holds a short recording of someone counting from one to six in English, already segmented, transcribed in IPA, translated, glossed and in the lexicon. The last segment, «six», is left empty so you can try transcribing it. After the first time, the Tutorial button moves into the **Project** menu, as **Open the tutorial project**.

The first time you open it, a short tour shows what each panel is for. Press **Escape** or click **Skip the tour** to leave it at any point. To see it again, press **Ctrl+K** and choose **Take the tour**.

The tutorial is an ordinary project kept in wugs's own folder on this computer. Change anything you like. It is never mixed into your own projects.

## Switch between Tongues and Scrolls

wugs comes with two kits: **Tongues** for fieldwork (recordings, transcription, glossing, the dictionary) and **Scrolls** for corpus work (every occurrence in context, searching, glossing sentences). Click the kit's name at the top left, such as **Tongues**, and choose the other kit.

Your project stays open and nothing in it changes. Only the panels and their arrangement change, and each kit remembers its own arrangement. You can also press **Ctrl+K** and type "switch". **Manage kits…** in the same menu opens the Kits panel, where you can make your own kit.

### Share a kit as one file

In the Kits panel, press **Save as a file…** beside a kit. wugs saves one `.wugskit` file with the kit and its example project. You can put it on a USB stick or a shared drive, or send it by email. To install a kit file, press **Install a kit file…**. Before anything is installed, wugs shows what the kit holds, what it relies on, and whether the file was damaged on the way. Kit files are not signed yet, so install kits only from people you trust.

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

Under each card's text line, a link names the recording's details (**No details yet** at first). Click it to open them under the cards: date, place, who recorded it, participants, genre, equipment and a description, what an archive asks about a recording. Each saves when you press Enter or leave the field. The **Catalogue sheet…** uses a recording's date, place, genre and description for a text linked to it that has none of its own.

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

To hear fast speech word by word, choose a slower speed in the menu beside the clock: **0.5× speed** or **0.75× speed**. Speech keeps its pitch, so voices sound like themselves, only slower. The same menu is in the Transcribe toolbar; both show the one speed, which every segment, replay and play button in the window uses. wugs remembers it on this computer. Pronunciations in the Lexicon always play at normal speed.

To hear a segment over and over while you write it down, turn on **Loop** in the Transcribe toolbar. Each segment you play then repeats, with a short pause between plays, until you move to another segment (**Enter** or **Shift+Enter**), play something else, or pause. Turn **Loop** off to stop at once. wugs remembers the choice on this computer.

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

### Cut segments by ear as the recording plays

To segment a recording while you listen, as ELAN's segmentation mode does: click the timeline, press **Space** to play, and press **Enter** where a stretch of speech starts. Then press **Enter** at the end of each stretch as it plays: every press makes a segment from the last press to that moment, so pressing in rhythm cuts the recording into consecutive segments. Edges snap to where speech starts and stops within a quarter of a second, which makes up for the delay of a key press. Segments go on the selected segment's track, else the utterance track. **Escape** forgets the last press, so the next press starts afresh; each segment can be undone like one you drew.

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

Click **IPA** in the Transcribe panel to show or hide the IPA palette. It has groups for vowels, consonants, diacritics, suprasegmentals, and tone. Click a symbol to insert it. Its first row, **This project**, holds the letters and accents your project already uses that a plain keyboard does not type, commonest first, so the special letters of a practical orthography are one click away too. A letter of another script (such as a Cyrillic «а» typed among Latin letters) is left out of that row, since it is usually a mistake; the Checks panel's Characters lists those.

If you type IPA in X-SAMPA, click **X-SAMPA** in the Transcribe panel. Type `S` for ʃ, `N` for ŋ, `@` for ə, `t_h` for tʰ, `r\` for ɹ, `"` for the primary stress mark and `:` for length. Beside the field, wugs shows the IPA it will save. When you press Enter or leave the field, the text becomes that IPA. Characters with no X-SAMPA meaning stay as typed, so IPA you pasted or picked from the palette is kept. X-SAMPA reads digits and capital letters as symbols (`1` is ɨ, `E` is ɛ), so turn it off for an orthographic track. wugs remembers the setting on this computer.

For the letters of a practical orthography that a keyboard lacks, make typing shortcuts, as Keyman users do. Click **Shortcuts…** in the Transcribe toolbar and write a line for each: what you type, a space, and what it becomes, such as `;n ŋ` or `b' ɓ`. **Start from an example** fills in a few common ones. Click **Save**. From then on, typing `;n` writes ŋ at once wherever you type the language: the transcription field, a word's breakdown in the Gloss panel, a headword, citation form, other form, pronunciation or example sentence in the Lexicon, and the word fields in Elicit. The table belongs to the project, so everyone working on it, on any device, types the same way. Lines wugs cannot read are named under the table; a line starting with `#` is a comment.

As you type, wugs checks each finished word against the words already in the project. A word used once or twice that is only accents, or one letter, away from a word used far more often shows up beside the field as a suggestion, such as **toɾuma → toɾumo?**. Click it to write the common spelling instead, or ignore it: it is only a hint, and a word you keep using soon counts as the project's own. It uses the same rules as **Spellings to check** in the Checks panel, which lists the slips already saved.

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

Grammatical glosses in capitals, such as `LOC` or `PL`, show in small caps. While you type one, a line under the cell says what each abbreviation means, from the project's own list (**Abbreviations** in the Interlinear tab) or the Leipzig Glossing Rules: `3SG.PST` shows "3 third person · SG singular · PST past", and an abbreviation with no meaning anywhere shows **?**, so a slip such as `PTS` stands out at once.

The gloss cells suggest senses from the lexicon as you type. Press **Enter** on a match to link the morph to that lexicon sense.

### Add unknown words to the lexicon

When a root is not in the lexicon, the panel offers **Add «form» 'gloss' to the lexicon**. Click it to make the entry. The Lexicon panel shows it at once.

### Reuse earlier analyses

When you meet a word you analysed before, its breakdown field shows the earlier analysis in grey. Press **Enter** on the empty field to accept it. wugs never applies an analysis unless a person accepts it.

A word you have never analysed can still get a suggestion, marked **guess**: wugs reads it from the morphs you glossed in other words and from your lexicon. Once you have glossed `ni-kani` '1SG-count' and `teka=ma` 'ten=FOC', a new word `nitekama` is offered as `ni-teka=ma` '1SG-ten=FOC'. A guess is made of prefixes and clitics, one or two roots, then suffixes and clitics; shorter breakdowns and morphs you use often come first. An affix in the lexicon counts when its headword carries its marker, such as `-ler` or `ni-`. Check a guess before you accept it, as you would a suggestion from a colleague.

When sentences have translations, a word with no analysis yet also shows up to two meanings the translations suggest, such as ‘walks’?: the translation words that stand most often in the translations of the sentences the word is in (the same counts as **Maybe** in Words to gloss). Click one to gloss the word as one root with that meaning, then break it into morphs if it has more.

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

### Keep notes on a sentence

Under **Notes**, at the bottom of the Gloss panel, type anything you want to remember about the sentence: "the speaker hesitated here", "check this form with M.", "compare text 3". Press Enter or leave the field to keep it. A sentence can have as many notes as you like. Retype a note to change it, or empty it to delete it.

Notes belong to the sentence's text, so they have its sharing level, and they sync with the project. Two people can add notes to the same sentence at once, and both notes stay. The **Interlinear** tab shows each note under its sentence, and **Search** finds words in notes too: the hit opens the sentence and says "note". Notes go in and out of FlexText as FLEx's Note line.

### Go through every note before a session

The **Notes** panel lists every note in the project in one place: the notes under sentences, grouped by text, and the notes on lexicon entries and senses. Press **Ctrl+K** and choose **Show Notes** (it is not placed in the workspace at first).

- Type in **Find in the notes** to look for a word in the notes, their sentences or headwords and glosses, ignoring capitals and accents. **Sentences** and **Lexicon** show one kind only.
- **Only questions** keeps the notes with a question mark: the things to ask the speaker next time.
- Click a sentence to open it in the Gloss panel, or a headword to open its entry in the Lexicon, and change or delete the note there. Click **Read again** after adding notes elsewhere.
- **Copy list** copies the notes shown as plain text. **Print a list…** saves them as a page to print, with a box to tick and room to write the answer under each.

### Gloss a word once for all its uses

The **Words to gloss** panel (open it from Settings, Closed panels, or **Ctrl+K**) lists every word form with uses not yet broken into morphs, the most frequent first, so a few minutes there gloss a large share of a corpus (FLEx calls this word analyses). For each form, type its morphs as in the Gloss panel (`ni- kani` or `ni-kani`) and their glosses in the same order (`1SG count` or `1SG-count`), then press **Enter** or **Apply**: every use not yet analysed gets that analysis, in every text. A word analysed before, or one wugs can guess from the morphs it knows, shows its likely analysis on a button; click it, or press **Enter** in the empty field, to use it. Uses already analysed are never changed. A morph whose spelling and gloss match exactly one sense in the Lexicon (`kani` glossed `count`) is linked to that sense, as picking it in the Gloss panel would. Click a form to see its lines in the Concordance.

When sentences have translations, a line under a word offers **Maybe** ‘…’: the translation words that stand most often in the translations of the sentences the word is in (counted by Dice's coefficient, so a word in every translation, such as *the*, scores low), each with how many of those sentences share it (`3/4`). Click one to gloss the word's root with it; with nothing typed yet, the word becomes one root with that gloss. Check before you apply: these are guesses from co-occurrence, and a word met in only one translated sentence gets none.

Only sentences already broken into words are listed. When some are not (a text imported from plain text, say), **Break N sentences into words** does it for all of them at once, as **Break into words** does for one in the Gloss panel.

### Check that each morph is glossed one way

Over months of glossing, the same suffix can end up as `PL` in one text and `pl.` in another. The **Checks** panel finds them. Press **Ctrl+K** and choose **Show Checks** (it is not placed in the workspace at first).

It lists every morph glossed more than one way across the project, the ones that differ only in capitals, spaces or dots (almost surely slips) first. Under each morph are its glosses, how many times each is used, and a link to a sentence that uses it. Click **Use «PL» for all**, then click again to confirm, and every use of that morph glossed otherwise is changed to it.

Two morphs that sound alike but mean different things (a plural `-s` and a third-person `-s`) show up too. Leave them, or link each one to its own lexicon entry in the Gloss panel, and they drop off the list.

### Grow the lexicon from your glossed texts

Under **Not in the lexicon**, the Checks panel lists every root you have glossed in your texts that has no lexicon entry yet, most used first. Each shows its glosses with how often each is used, and a link to a sentence that uses it. This happens a lot after importing glossed texts from Toolbox or FLEx without their lexicon.

- Click **Add to lexicon** to make its entry: the spelling used most often becomes the headword, and the gloss becomes the sense. A root glossed two ways gets a sense for each (**Add with 2 senses**).
- Every use of the root in your texts is linked to its new sense, so **used in N words**, the Concordance and the dictionary see them.
- **Add all**, then a second click, does this for every root on the list.

Then fill in the entries in the Lexicon panel: part of speech, definitions, examples.

### Keep the texts and the lexicon agreeing

A word linked to a lexicon sense keeps its own gloss in the text, so the two can drift apart: a gloss retyped in one sentence, or a sense renamed in the Lexicon after the texts were glossed. Under **Unlike the lexicon**, the Checks panel lists each sense whose linked words are glossed otherwise, with the gloss the texts use, how often, and a sentence that uses it. A gloss that is one of several the sense lists (`hound` for a sense glossed `dog; hound`) is not counted as different.

- **Use «river» in the texts** glosses those words as the lexicon does (click twice when there are several).
- **Make «stream» the lexicon's gloss** changes the sense in the lexicon instead.
- Click the headword to open the entry in the Lexicon.

### Check segment times

Under **Segment timing**, the Checks panel lists timed segments that overlap the one before them on the same track, are under 0.15 seconds long (often a stray click while segmenting), or are over 30 seconds long. Overlaps on one track confuse ELAN and Praat when you export, and speech tools take at most 30 seconds at a time. Click a segment's text to open it on the Timeline and fix it there by dragging its edges, merging or splitting. Segments without times are not checked.

### Find entries typed twice

A lexicon gathered from imports, quick elicitation and several people often holds one word twice. Under **Entries that may be one word**, the Checks panel lists pairs of entries that share a meaning (a gloss, or one part of a gloss written with `;` or `,`, with a leading "to" ignored) and whose headwords are the same, differ only in accents (`kesi`, `kési`), or are one letter apart (`liwa`, `lima`, for headwords of four letters or more). Two numbered homographs of one headword are taken as told apart on purpose and left out.

Click a headword to open its entry. If the two are one word, **Merge «…» into «…»** (click twice) moves the second entry's senses, forms, pronunciations and relations to the first and deletes the second; the one used in a text is kept. Two different words that happen to look alike, such as a real minimal pair, are left as they are.

### Find spelling slips

Under **Spellings to check**, the Checks panel lists rare words that look like slips for a common word: a word used once or twice beside one used at least four times as often that has the same letters with other accents (`kési` beside `kesi`), or one letter added, left out or changed (`tekma` beside `tekama`, for words of four letters or more). Capitals do not count as a difference. Click either word to see its lines in the Concordance. Some pairs are real words, such as a rare minimal pair, so check each one, and fix the slips with **Replace…** in the Search panel.

Under **Unusual sound sequences**, it lists rare words (used once or twice) with two sounds side by side, or a sound at the start or end of a word, that no other word in the project has, though both sounds are common: `tʰakka` is listed when it is the only word with `kk`. Such a word may be a slip of the ear or keyboard, a loanword, or a real rarity, so open it in the Concordance and decide. Sounds are cut as the Sounds panel cuts them, and the check waits until the transcriptions hold at least 500 words.

### Check the characters you type

Many typing slips are invisible: a Cyrillic «а» from the wrong keyboard among Latin letters, an ASCII apostrophe `'` beside the glottal stop's `ʼ`, `g` beside IPA `ɡ`, or a no-break space pasted from a document. Words with them look right but never match in a search or the lexicon.

Under **Characters**, the Checks panel counts every character your transcriptions and lexicon headwords use. It lists first the ones worth a look, saying why:

- a letter from another script than most of your letters;
- a character that looks like another one the project also uses;
- an invisible character;
- a letter or accent used only once or twice.

Each card shows a sentence that uses the character, and **Every word with it** opens the Concordance on every word containing it. Fix a slip in Search's **Replace…**. **Every character** shows the whole inventory with each character's code, Unicode name, script and counts, a quick way to check an orthography.

### See which texts have consent records

Under **Consent records**, the Checks panel lists the texts that no consent record covers (no record for the text, its recording or the whole project), then the texts covered by a record that limits them: a no to publishing online, depositing or speech tools, or sharing within the community only. Click a text to open it, or **Open People** to add or edit records. Records inform what you share; nothing in wugs waits on them.

Under **Ready for the archive**, the Checks panel lists each text that still lacks something an archive's item form asks for: a date, place, speakers, genre or description (in the text's details, or its recording's in the Media panel), a recording, a consent record, sentences still to transcribe, and transcribed sentences without a translation. It also says when the project's language has no name, Glottocode or ISO 639-3 code. These are the questions **Catalogue sheet…** answers in its columns, asked before you write it. Nothing here stops a deposit.

### Check the dependency trees

After tagging and parsing, or importing CoNLL-U or TEI, some trees may be broken: two words both marked as the root, a word whose head is itself through a loop, or a head pointing to a word that is gone. Under **Dependency trees**, the Checks panel lists every parsed sentence with such a problem and says what it is. Click a sentence to open it in the **Sentence** panel and correct its heads there. When every tree is well formed, the section says so.

### Copy an example into a paper

Click **Copy example** at the top of the Gloss panel. The segment is copied as a numbered example: the words split into morphs, the glosses under them with grammatical glosses in small capitals, and the free translation in quotes. Then paste it into your paper.

Choose where it will go in the list beside the button:

- **Word / Google Docs** pastes as a table that lines the morphs up with their glosses. It also works in LibreOffice and email.
- **LaTeX (gb4e)** and **LaTeX (ExPex)** paste the lines those packages expect. They number the example themselves. With pdflatex, add `\usepackage[T1]{fontenc}` so IPA tildes print as tildes; XeLaTeX and LuaLaTeX need nothing extra.
- **Plain text** lines the columns up with spaces.

wugs remembers the choice on this computer. The number is the segment's place in its text. A morph with no gloss yet shows **?**, so you can see the gap before you send the paper. Grammatical glosses paste as slightly smaller capitals, which read as small capitals in every word processor.

### Read a whole text glossed

The **Interlinear** tab, beside Gloss, shows the selected text the way you would print it: each sentence with its number, its words, the morph breakdown, the glosses with grammatical glosses in small capitals, and the free translation in quotes. Sentences not glossed yet show their transcription in grey.

- The line at the top says how many sentences and words are glossed so far. A sentence counts as glossed once every word in it is.
- Choose **Sentences still to gloss** in the menu above the list to hide the finished ones, or **Sentences without a translation** to list the transcribed sentences no one has translated yet; the button beside it then reads **Next to translate** and opens the next one in the Gloss panel, where its translation line is.
- Click **Next to gloss** (or choose **Go to the next sentence to gloss** with **Ctrl+K**) to open the next sentence with a word still to gloss in the Gloss panel.
- Click a sentence, or move to it with the arrow keys and press Enter, to open it in the Gloss panel.
- Click ▶ to hear a sentence that has times.

Click **Abbreviations** to list every grammatical abbreviation the text's glosses use (`SG`, `PST`, `3` in `3SG`), the list a paper prints with its examples. Each one has a field for what it means:

- An empty field uses the meaning from the Leipzig Glossing Rules' standard list, shown in grey. `N` before a listed abbreviation reads as "non-" (`NPST` nonpast).
- Type a meaning and press Enter (or leave the field) to use your own. It is kept for the whole project and syncs to everyone in it. Empty the field to go back to the Leipzig meaning.
- An abbreviation that is not in the Leipzig list and has no meaning yet is marked, and a note names it. That is also how a typing slip shows up, such as `PLU` beside `PL`.

To get the whole text into a paper or handout, use **Glossed examples for** under the Texts panel (see "Export a text or the lexicon").

## Build the lexicon

The **Lexicon** panel has a list of entries beside an editor for the selected entry.

### Add and edit entries

1. Type a word in **Lemma** and its meaning in **Gloss**.
2. Click **Add entry**.

To find an entry, type part of its lemma or gloss in **Find an entry by lemma or gloss**.

The list beside it shows **Every entry**, or only the entries with work left: **Needs a gloss** (a sense with no gloss yet, such as a word added from the Frequencies list), **Needs a category**, or **Not in any text** (no glossed word in a text is linked to it yet). Each choice shows how many entries it would list. Under **Category**, the same list offers each part of speech the entries use, commonest first (`n (40)`, `v (25)`), to see only the nouns, only the verbs, or only the affixes. **Print…** beside it saves the entries listed (and found by the search) as a checklist to print for a session with a speaker: each headword with its category and meaning, a box to tick and room for a correction, under lines for who checked it, when and where.

In the entry editor you can set the lemma, the citation form, the category (part of speech), and the homograph number. Each sense has a gloss, an optional gloss in the Gloss panel's second language, a definition, a usage note and your own notes. The entry has a **Notes** field too, for etymology, doubts or what to ask next time. Notes come in from LIFT (`<note>`) and Toolbox (`\nt`) files and go back out with your edits. A LIFT file from FLEx also brings each sense's semantic domains: wugs loads the SIL Semantic Domains if the project has none yet and puts each sense in its domains, and a LIFT export writes them back for FLEx. Click **Add a sense** for another sense. Fields save when you press **Enter** or leave the field (the larger fields save when you leave them, or on **Ctrl+Enter**).

When two entries share a lemma, the editor says so. Click **Number them** to give each one a homograph number. If they are really one word, entered twice (by two imports, say), click **Merge … into this one** and then click again: the other entry's senses, other forms, pronunciations and related words move into the entry on screen, and the other entry is removed. Words in your texts glossed with its senses stay linked to them.

**Other forms** lists the entry's allomorphs, the other shapes it takes: `-lar` beside `-ler`, `ew` beside `ev`. Type one and press **Enter** to add it; click × to remove it. In the Gloss panel a morph written in any of these forms finds the entry, and guesses for new words use them too. Variants and allomorphs from FLEx (LIFT) and Toolbox (`\va`, `\a`) files show here too, and go back out in those exports.

The editor shows how many words in your texts use the entry.

To delete a sense or an entry, click **Delete sense** or **Delete entry**, then click again to confirm.

### Add how a word is said

The **Pronunciation** row under an entry's category holds how the word is said, usually in IPA. An entry can have several, for dialects, speakers, or careful and fast speech.

- To type one, fill in the field at the end of the row and press **Enter**. Add slashes or brackets yourself if you want them shown.
- To hear one, select the segment where the word is said, on the Timeline or in Transcribe, then click **From the selected segment**. Auto-segment **by word** cuts a recording of a word list into such segments. If the segment's transcription is one word, it becomes the written form. A ▶ then plays the word from its recording, whatever text is on screen.
- To add the sound to a pronunciation you typed, select the segment and click **+ sound** beside it.

Empty a pronunciation's field to delete it, or click × and then **Delete?**. Pronunciations print after the headword in **Dictionary to print…**, and on a published website the dictionary plays each headword heard in a recording that is open. LIFT files carry them both ways (`<pronunciation>`, with its media file name kept as written), and so do Toolbox files (`\ph`).

To show what a sense means with a picture, click **+ Picture** under its examples and choose a PNG, JPEG, GIF or WebP file (at most 20 MB): a photo of the plant, a drawing of the tool. wugs copies the file into the project, so it goes with the project when it syncs or is sealed. The picture shows under the sense, in **Dictionary to print…** (inside the one HTML file), on a published website's dictionary and on the back of **Flashcards…**. **Change…** picks another, and **Remove picture** takes it off the sense.

Pictures travel with LIFT files too. FLEx exports each sense's picture into a `pictures` folder beside the `.lift` file; when you import that file, wugs looks there and gives each sense its picture, and the message says how many it found. A picture it cannot find keeps its name and goes back out in the next LIFT export. When you export the lexicon as LIFT, wugs copies the pictures into a `pictures` folder beside the file, where FLEx looks for them.

### Fill in a paradigm

The **Paradigm** section at the bottom of an entry lays out its inflected forms as a grid. Choose a template: **Person × number**, **Tense × person** or **Singular and plural** are built in. Type a form in a cell and press **Enter** or leave the field. The line beside the template says how many cells are filled. Forms that fit no cell of the grid are listed under it. **Copy table** copies the grid for a paper or grammar in the format chosen for **Copy example** in the Gloss panel: a table for Word and Google Docs, a LaTeX `tabular`, or columns lined up for plain text. Empty cells show a dash. **Print a sheet…** saves the grid as a form to print for a session without a laptop: lines for the speaker, date, place and recording, the forms already known in grey to check, room in every cell to write, and lines for notes.

To see which forms your texts already show, click **Forms in the texts** under the grid. It lists every word in the texts with a morph linked to one of the entry's senses, grouped by form and analysis (`nikani ni-kani 1SG-count ×3`), commonest first, and greys out the forms the paradigm already holds. Click a form to see its lines in the Concordance.

To make a grid for your language, such as case × number or noun class × number, choose **New template…** in the template list:

1. Give it a name, such as `Case × number`.
2. Under **Rows: feature**, name what the rows stand for (`Case`). Under **Rows: values**, list the values separated by commas. Write `value=meaning` to show the meaning in the grid: `nom=nominative, acc=accusative, gen`.
3. Fill in the columns the same way, or leave them empty for a grid of one column.
4. Click **Add template**.

The template is kept with the project, so everyone working on it can choose it. Choose it and click **Edit template** to change it, or **Delete template** and then click again to remove it. Removing a template keeps the forms: they show in any grid with the same features, and under the grid otherwise.

### Relate entries to each other

The **Related** row links an entry to its synonyms, antonyms and other words a reader should see. Pick a type (**Synonym**, **Antonym**, **See also**, **Part of**, **Has part**), type the other entry's headword and press **Enter**. When several entries share a headword, add the homograph number: `tela2`.

A relation shows on both entries: a synonym is a synonym from either side, and «finger» **Part of** «hand» shows on «hand» as **Has part** «finger». Click the other headword to open it, or × to remove the relation. The printed dictionary lists them after the senses (**Syn:**, **Ant:**, **See also:**) as links, and LIFT files carry them both ways, including the types FLEx names that wugs has no word for. Toolbox files carry synonyms, antonyms and see-also as `\sy`, `\an` and `\cf` lines; a line naming a headword that is neither in the file nor in the project stays as written.

### Add example sentences

Each sense can have example sentences with their translations, as a printed dictionary shows under a meaning.

- To type one, fill in **Add an example sentence** and **Its translation** under the sense, then press **Enter**.
- To take one from your texts, click **used in N words** under the sense, then **Use as example** beside an occurrence. The sentence and its first translation are copied in, with a link back: **from «text»** opens the sentence, and ▶ plays it.
- To give every sense that has none an example at once, open the **Checks** panel: **Example sentences** says how many senses have none and how many are glossed in a text that is open, and **Add N from the texts** (click twice) takes each one's first use, a translated sentence when there is one. Texts that are not open are never used. Edit or delete the examples in the Lexicon as usual.

Each example's sentence and translation are fields you can edit like any other. Editing an example does not change the text it came from. Click × and then **Delete?** to remove one.

The lexicon is shared with everyone the project reaches. A sentence from a text that is not open to everyone cannot be copied in, so type an example that may be shared instead.

Examples go out with the lexicon: in LIFT files (FLEx's `<example>` with its translation and source), in Toolbox files (`\xv` with its `\xe`), in the CLDF dataset (listed under their sense), in the printed dictionary (in place of the sense's first use in a text), and in Search, which finds them under **Lexicon**. Examples in a LIFT or Toolbox file you import come in the same way.

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

To work through a questionnaire of your own, click **Add a word list**, then **Load a table…** under "A list of your own". Choose a CSV or tab-separated table, or an Excel or LibreOffice spreadsheet (`.xlsx`, `.xls`, `.ods`) as it is. The table needs a column of concepts headed **Gloss**, **English** or **Concept**. A **No.** column numbers the concepts, and a **Concepticon_ID** column links each one to its [Concepticon](https://concepticon.clld.org) concept set, so the list can be compared with other languages. The list takes the file's name. Loading the table again after you add rows adds only the new rows; answers already given stay.

- If the word already exists in the lexicon, wugs offers **Use** with the entry's lemma and gloss. Click it to link the existing entry instead of adding a second one.
- To take an answer back, click the × on its chip. The entry stays in the lexicon.
- If a concept does not apply to the language, click **N/A**. Any text in the gloss field is kept as the reason.
- Press **Enter** on an empty word field to skip ahead without answering.

#### Keep the speaker's voice with each answer

If you recorded the session, each answer can keep the word as the speaker said it.

1. Load the recording in the Timeline and press **Auto-segment** with **by word**, so each word said is its own segment.
2. Select the first word's segment.
3. In **Elicit**, tick **Sound from the Timeline**. The line under it says which segment the next answer will be heard in.
4. Type each answer as before. When you press **Enter**, the answer is heard in the selected segment, and the next segment on that track is selected and played, so you hear the next word as you type it.

An answer heard this way has a ▶ on its chip. In the Lexicon its entry has the sound under **Pronunciation**, and a published website plays it beside the headword. If the speaker said a word twice, or a segment holds something else, click on the Timeline to select the right segment before you press Enter.

The concept lists come from Concepticon. Each answer stays linked to its Concepticon concept, so the list can later be compared across languages.

#### Take a word list into a session on paper

When a laptop would be in the way, print the list. **Print a sheet…** beside the list saves an HTML file to open in a browser or Word and print. At the top are lines for the speaker, date, place, recording, who elicited and the contact language. Below them is a row per concept, with its number, gloss and Concepticon gloss, a wide space for the word as heard and a space for notes. With **To do** chosen, the sheet holds only the concepts still to ask. With **All** or **Done** chosen, it holds every concept, and the answers already in the project are printed in grey so you can check them with the speaker. After the session, type the answers in as usual.

#### Share a word list for comparative work

**Export for comparison…** beside the list writes its answers into a new folder in two forms:

- a CLDF Wordlist (`forms.csv`, `parameters.csv`, `languages.csv` and `Wordlist-metadata.json`), the format Lexibank and CLICS collect;
- `edictor.tsv`, a table that [EDICTOR](https://digling.org/edictor/) and LingPy open for cognate judgements.

Each form keeps its Concepticon concept and is cut into sound segments. If the project has an orthography profile (see [Write a track in IPA from a practical spelling](#write-a-track-in-ipa-from-a-practical-spelling)), the profile cuts the segments, so `ng` counts as one sound and a practical spelling comes out in IPA. Without a profile, each letter with its accents and length marks is one segment. Set the project's Glottocode in the Texts panel first, so the list can join other languages' lists.

### Collect words by meaning

1. In **Elicit**, open **Semantic domains**.
2. If the project has no domains, click **Load SIL Semantic Domains**.
3. Find a domain by word or outline number.
4. Ask the speaker the SIL questions the panel shows.
5. Type each word and its gloss, then press **Enter** or click **Add**.

Each word becomes a lexicon entry in that domain. The field keeps focus for the next word.

Above the list, a line says how many words the project holds in how many domains, and each domain shows how many words it has. To work through the domains in order, as a Rapid Word Collection session does, click **Next empty domain**: it opens the next domain in the outline that has no words yet. Tick **Only domains with no words yet** to list just those.

### Elicit sentences from a list of prompts

The **Elicit sentences** panel works through a questionnaire: sentences in a language you share with the speaker (such as a valency or tense–aspect questionnaire, or your own list) to say in the language you are documenting. Open it with **Ctrl+K**, **Show Elicit sentences**.

1. Press **Load prompts…** and choose the list. It can be a `.txt` with one sentence a line, or a table with a header: CSV, tab-separated, or an Excel or LibreOffice spreadsheet. The prompt column is named prompt, sentence, stimulus, english, translation, text, question or item, and is otherwise the first column that is not numbers. An id or number column keeps each prompt's own number. The list is kept with the project. Loading a file of the same name again replaces its prompts and keeps the answers.
2. Type the answer under a prompt (typing shortcuts work here) and press **Enter**. The cursor moves to the next prompt without an answer.
3. Each answer is saved as a sentence of the list's own text, named after the list, with the prompt as its translation. Open it in the Gloss panel by clicking it, to gloss it like any other sentence. A prompt can have several answers (another way to say it, another speaker); type the next one under the first.

The line at the top counts the prompts answered. **Only prompts to do** hides the rest. **Print a sheet…** saves the prompts as a form to print, with a header for the speaker, date and place and room for each sentence and notes, for a session without the laptop; with **Only prompts to do** ticked it holds only those, else every prompt with the answers already here in grey. Since the answers are ordinary sentences, they appear in the Concordance, the exports and the Interlinear tab; record the session and line the sentences up on the Timeline to hear them.

**From frames…** makes a list from your lexicon instead, for frame elicitation: the frames tone and noun-class work uses, where each word is said in the same few sentences. Write one frame a line in the language you share with the speaker, with `___` where the word goes (`I see the ___.`, `The ___ is big.`; `___` alone asks for the word on its own), choose **Every entry** or one category, give the list a name and press **Make the list**. Each entry's gloss goes in each frame, entry by entry, so a word's frames come one after another. The entry's headword shows beside its prompt as a reminder, and is not saved with the answer: the translation is the frame as filled in. Entries without a gloss are left out, and the line under the frames says how many prompts that makes. Making a list of the same name again keeps the answers of the words and frames still in it.

## Record people and consent

The **People** panel records who speaks in your recordings and what each person agreed to. It is optional.

These records inform and never block. The corrections export and sealed archive packages report what the records say. Nothing in wugs is blocked by them. Your ethics board's rules are the ones that count.

To add a person:

1. Type a name in **Name**.
2. Choose a role, such as speaker or consultant.
3. Click **Add person**.

Under each person, **Details** opens fields for what an archive's list of participants asks for: the year they were born, sex or gender, the languages they speak, where they are from and where they live, ethnic group, education and notes. Each saves when you press Enter or leave the field, and the line shows how many are filled. Details are kept in the project's people records, the most restricted part of a project. **Copy as a table** above the list copies everyone, with their roles, details and what their consent records say, to paste into an archive's deposit form or a spreadsheet.

**Print a consent form…** above the list saves a form to print as a page you open in a browser: each use the People panel records (an archive, a website, the community only, speech tools) in plain words with Yes and No boxes, what will be recorded, how the person wants to be named, a line saying they can change their mind, signature lines, and room to note consent given aloud (read or translated, the language used, the recording of it, a witness). **Print a form…** under a person fills in their name and role. It is a starting point: your ethics board's approved wording comes first, and the form is best translated into a language the person reads well. Afterwards, type their answers into the record and attach a scan of the signed page.

To record what they agreed to:

1. Click **Record what they agreed to**.
2. Under **Covers**, choose what the record covers: the whole project, one recording, or one text.
3. Set the date.
4. For each term, click **Yes**, **No**, or **Not asked**. The terms are "Deposit in an archive", "Publish online", "Share within the community only", and "Use to build or test speech tools (AI)".
5. To keep the signed form with the record, click **Attach the signed form…** and choose a scan, photo, or PDF.

Click **Add another record** for a second record. To delete a person or a record, click **Delete** or **Delete record**, then click again to confirm. Deleting a person also deletes their records.

An attached form is stored in the project, so a sealed package contains it. Treat sealed packages as personal data.

### Keep a fieldwork log

The **Sessions** panel is the project's field notebook: one entry per sitting, with when and where it was, how you worked (elicitation, recording, transcription with a speaker, checking…), who took part, and notes on what you covered and what to ask next time. Open it from Settings (Closed panels) or with **Find an action**.

1. Click **New session**. It is dated today and its notes are ready to type in.
2. Change the **Date**, **Place** or **Method** if you need to. Each saves when you leave the field.
3. Under the fields, choose people from **+ Person…** (people come from the People panel). Click × beside a name to take them off.
4. Type the notes. They save when you click elsewhere.

**Texts that day** lists the texts dated on the session's date, by their own date detail or their recording's, so a log entry leads to what was recorded. **Find in the log** looks through dates, places, methods, people, texts and notes; **Everyone** narrows the log to one person's sessions, and the line above says who you have worked with and how often. **Copy table** copies the log for a spreadsheet, and **Print the log…** saves it as one page, oldest first, to print or to keep with the recordings. To delete a session, click **Delete**, then click again.

Sessions are kept with the people records, the most restricted part of a project, because a log speaks of the people in it. Two people adding the same person to a session at once keep one; notes typed on two devices merge.

## Work with Praat

wugs works with your own installed copy of Praat, version 6 or newer. It finds Praat on the system path, in the usual install places, and in `Praat` folders in Desktop, Downloads, or Documents. If wugs cannot find it, the Timeline toolbar shows **Praat…**. Click it and choose the Praat program.

### Show pitch, intensity, and formants

Click **Pitch**, **Intensity**, or **Formants** in the Timeline toolbar. Your Praat analyses the recording, and wugs draws the curves over the waveform.

Without Praat, all three still work: wugs analyses the recording itself with Praat's own methods and settings, so the curves match Praat's closely. Where a vowel's formants are hard to tell apart, the two can differ by a frame or two.

The analysis covers at most one minute of view at a time. If more is on screen, zoom in. wugs keeps the results, so a part you already viewed shows at once next time.

The analysis uses Praat's standard speech settings unless you choose otherwise. Once an overlay is on, a **voice** menu appears beside the toggles. Choose the one that fits the speaker in this recording:

- **Typical**: pitch 75–600 Hz, formants up to 5500 Hz (Praat's defaults).
- **Low voice**: pitch 50–300 Hz, formants up to 5000 Hz, for a deep or creaky voice.
- **High voice**: pitch 100–600 Hz, formants up to 5500 Hz.
- **Child**: pitch 150–800 Hz, formants up to 8000 Hz.

The choice is saved with the recording for everyone on the project. **Measure…** and the **Vowel space** panel use it too, and the table names it in its last column. A voice outside the chosen pitch range shows no pitch, so try another voice when the pitch line has gaps or jumps an octave.

### Measure a track

Click **Measure…** in the Timeline toolbar to measure every timed segment of a track and save the numbers as a CSV table, ready for R, Python or a spreadsheet. wugs measures the track of the selected segment, or the utterance track when nothing is selected. Choose a folder outside the project, and the table goes there.

Each row is one segment: its label, start and end, duration, mean, lowest and highest pitch, how much of it is voiced, mean intensity, F1 to F3 at its middle, F1 to F3 averaged over its middle half (the steady part of a vowel), and its pitch at ten points spread evenly through it (5% to 95% of the way), for drawing tone and intonation contours. A value is left empty where there is nothing to measure, such as pitch in a voiceless stretch. The last column says what analysed it: your Praat when wugs finds it, else wugs's own analysis. The numbers are the same ones the Pitch, Intensity and Formants overlays draw.

Segments without times are left out, and the message after measuring says how many.

### Save segments as sound files

Click **Save clips…** in the Timeline toolbar to save every timed segment of a track as its own WAV file, the way Praat's "Extract all intervals" does. wugs uses the same track as **Measure…**. Choose a folder outside the project. wugs makes a folder named after the text and track, and puts these files in it:

- one WAV per segment, at the recording's own quality, numbered in time order and named after its transcription, such as `007 kesi mopa.wav`;
- `clips.csv`, with each file's name, start, end, duration, transcription and translation.

Use the clips in a Praat script that runs over a folder, as stimuli for a listening task, in flash cards, or to send a speaker the words to check. Saving again makes a second folder beside the first.

### Plot the vowel space

The **Vowel space** panel puts every timed segment of a track at its F1 and F2, the chart phoneticians draw for a vowel system: F2 runs from right to left and F1 from top to bottom, so front vowels sit on the left and high vowels at the top. Open it from Settings (Closed panels) or with **Find an action**.

Segment the recording so each vowel (or each word you want to compare) is one segment, and label each segment with its vowel. Choose the track and click **Plot**. wugs measures each segment's middle half, the steady part of a vowel, and draws a dot for it. Each label gets its mean, written in large letters, and an ellipse one standard deviation wide.

- **Normalize** shows each formant as standard deviations from this speaker's mean (Lobanov normalization), so recordings of different speakers can be compared.
- **Every token** shows or hides the dots, leaving only the means and ellipses.
- **Trajectories** draws an arrow for each label from its mean F1 and F2 at 20% of its segments to its mean at 80%: a diphthong such as `ai` shows as a long arrow from the [a] region to the [i] region, and a monophthong's short arrow shows how much it moves (vowel-inherent spectral change).
- Click a dot to hear its segment and open it on the Timeline.
- **Copy table** copies every segment's label, times, pitch, F1 to F3, and F1 and F2 at 20% and 80% for a spreadsheet or R. **Measure…** on the Timeline writes the same four values, in the columns `f1_20pct_hz` to `f2_80pct_hz`.

### Draw pitch contours

The **Pitch contours** panel draws how the pitch of each timed segment moves from its start to its end, the picture tone and intonation studies draw. Open it from Settings (Closed panels) or with **Find an action**.

Segment the recording one syllable, word or phrase to a segment, and label each segment with its tone or tune, such as `H`, `L`, `HL` or `rise`. Choose the track and click **Draw**. wugs measures each segment's pitch at ten points spread evenly through it (5%, 15% … 95% of the way), so short and long segments line up, and draws:

- a faint line for every segment, broken where the voice stops; click one to hear it;
- each label's mean contour in a heavy line, named at its right end with how many segments it averages.

**Units** shows semitones from the speaker's median pitch, which lets you compare speakers with different voices, or plain Hz. **Copy table** copies each segment's label, times and the ten pitch values in Hz for R or a spreadsheet, and **Save picture…** saves the plot as an SVG file. **Measure…** on the Timeline writes the same ten values into its table, in the columns `f0_05pct_hz` to `f0_95pct_hz`.
- **Save picture…** saves the plot as an SVG file for a paper or slides. Word, Inkscape, Illustrator and web browsers open it.

The numbers come from the same analysis as the overlays and **Measure…**: your Praat when wugs finds it, else wugs's own.

### Compare how long sounds last

The **Durations** panel shows how long each label's segments last on one track of the text on screen: the numbers a study of vowel length, gemination or speaking rate starts from. Open it from Settings (Closed panels) or with **Find an action**. It reads the segments' own start and end times, so nothing is measured and no recording is needed.

Segment the recording one sound, syllable or word to a segment and type a label in each, such as `a` and `aː`, or `t` and `tː`. Choose the **Track**. The panel shows:

- a strip plot with a row per label, a dot for every segment (click one to hear it) and a bar for the mean and one standard deviation either side;
- a table with each label's count, mean, standard deviation, median, shortest and longest, in milliseconds;
- **Compare two labels**: pick two, and it says how many times as long one is as the other on average, with Welch's t-test (which does not assume the two vary alike), such as "«aː» is 1.86 times as long as «a» on average (152 ms against 82 ms): Welch's t(17.3) = 9.41, p < 0.001". Each label needs at least two segments.

**Show** leaves labels with few segments out of the plot. **Copy table** copies the summary, and **Copy every segment** copies one row per segment (label, start, end, duration) for R or a spreadsheet. **Save picture…** saves the plot as an SVG file.

Under the comparison, **Speaking rate** reads the same track as running speech: the articulation rate (syllables a second inside the segments, pauses left out), the median and range per segment, and the speech rate with the pauses between segments. A syllable is counted for each vowel the Sounds panel finds, so it is read best from an IPA track; a long vowel counts once, a diphthong written as two vowels twice, and a segment with no vowel wugs can read is left out and counted.

### Compare two transcriptions of one recording

The **Agreement** panel compares two transcription tracks of the text on screen segment by segment: two people's transcriptions of the same recording (inter-annotator agreement), a machine track against the one a person corrected, or a converted IPA track against a hand transcription. Open it from Settings (Closed panels) or with **Find an action**. To compare two annotators, give each a track on the Timeline (**Add track**), or import an ELAN file that holds both annotators' tiers.

Choose the **Reference** track and the **Compared** track (**Swap** turns them round). Two segments are matched when they overlap by 60% or more of their combined extent, as in ELAN's reliability calculation; each segment is matched at most once, the closest first. The panel says:

- how many segments are matched, and how many on each track have no partner;
- how often matched labels are identical;
- the sound error rate: the sounds inserted, left out or changed, against the reference track's sounds (a letter with its marks counts as one sound, a word boundary as one);
- Cohen's kappa with Landis and Koch's words for it (slight, fair, moderate, substantial, almost perfect), counting a segment with no partner against an empty label; kappa is given only when there are 40 different labels or fewer, since transcriptions of whole sentences are not categories;
- how far apart the boundaries are at the start and the end, on average and as the median.

The table lists every match in time order, with the sounds apart and how much later (+) or earlier (−) the compared track's start and end are, in milliseconds; rows with different labels and segments with no partner are shaded, and ▶ plays each. **Only differences** lists just those. **Copy table** copies every row, with times in seconds, for a spreadsheet or R.

### See how speakers take turns

The **Turn-taking** panel reads a conversation transcribed with one track per speaker (the usual ELAN layout) and measures how quickly each speaker takes the floor from the last: the *floor transfer offset*, the new turn's start minus the previous turn's end. A positive offset is a gap, a negative one an overlap; across many languages most fall within a few hundred milliseconds of zero. Open it from Settings (Closed panels) or with **Find an action**.

Untick a track to leave it out (a translation or a notes track is left out already). The panel says how many changes of speaker there are, the median and mean offset, and what share start after a gap, overlap the previous turn, or come within 200 ms of its end. A segment that starts and ends inside another speaker's turn (a backchannel such as *mm*) is not a change of speaker and is counted apart, as is a speaker going on with another segment. A histogram shows the offsets from two seconds of overlap to two seconds of gap, and a table gives each direction (one speaker to another) its count, median and mean. **Copy every change** copies each one with its times for R or a spreadsheet; **Save picture…** saves the histogram as SVG.

### Write a track in IPA from a practical spelling

Many projects transcribe in a practical orthography. The **Orthography** panel writes such a track in another spelling, usually IPA, with an *orthography profile*: a table that lists the letters and letter groups of the spelling in a **Grapheme** column and what each is in IPA in an **IPA** column. These are the profiles of *The Unicode Cookbook for Linguists* (Moran & Cysouw 2018), which the `segments` package and CLDF datasets also read, so a profile made here works there too. Open the panel from Settings (Closed panels) or with **Find an action**.

1. Choose the **Track** to convert, and **Every text** or only the text on screen.
2. Press **Start from this track's letters**. The table then lists every letter the track uses, commonest first, each written as itself, with how often it occurs.
3. Correct the **IPA** column where the spelling differs, such as `y` → `j`. Press **Add a row** for letter groups, such as `sh` → `ʃ` or `ng` → `ŋ`. Longer groups win, so `sh` is used before `s` and `h`.
4. Press **Preview** to see the first sentences before and after. The preview also lists any letter that no row covers. Those letters stay as written until you add a row for them.
5. Press **Write the «IPA» track**. The converted text goes on a new transcription track, which the Timeline, Transcribe, the Sounds panel and the corpus tools read like any other.

Some rules for the table:

- Write `^` before a grapheme for "only at the start of a word" (`^h` → `ʔ`), and `$` after it for "only at the end".
- `NULL` writes nothing.
- An empty cell keeps the letter as it is.
- **Read capitals as small letters** (on by default) reads a capital as its small letter, so `Sh` at the start of a sentence converts like `sh`.

The profile is kept with the project and syncs to its members. **Edit as text** shows the profile as text, so you can paste one from a spreadsheet or a `.tsv` file. **Copy profile** copies it back out. You can correct the converted track by hand. When you change the profile and convert again, only segments that nobody has edited since the last conversion are rewritten, and the report says how many were kept. wugs never writes into a track that someone transcribed by hand.

### See the sound inventory on the IPA chart

The **Sounds** panel cuts every word of a transcription track into sounds and puts them where the IPA chart puts them: pulmonic consonants by place and manner, voiceless on the left of each cell and voiced on the right, and vowels on the vowel trapezoid, unrounded on the left of each point and rounded on the right. Affricates and double articulations, clicks, implosives and other consonants are listed under the chart. Each sound shows how many times it occurs. Open the panel from Settings (Closed panels) or with **Find an action**.

A sound is a letter with every diacritic and modifier written after it, so `tʰ`, `kʼ`, `aː` and `ã` each count as one sound, beside `t`, `k` and `a`. A tie bar joins two letters into one sound (`t͡ʃ`), and a superscript nasal before a consonant marks it as prenasalized (`ᵐb`). Stress marks, tone letters and syllable dots belong to no sound.

- Choose the track to read. wugs starts with a track named IPA or phonetic when there is one, else the fullest.
- Choose the whole project or only the text on screen.
- **Whole IPA chart** shows every row and column of the chart, not only those in use.
- **Tone apart** takes tone marks off the vowels (á à â ǎ a̋ ȁ, and ā), and drops tone letters and tone numbers, so `á` and `à` count as the vowel `a` in the chart and every table below it. The tone melodies still read the marks. Leave it off where a macron means a long vowel.
- Click a sound to open the Concordance on every word that holds it.
- **Copy table** copies each sound with its count, the number of words it occurs in, an example word and its place on the chart.
- **Copy chart** copies the consonant chart (places across, manners down, voiceless before voiced in each cell) with the vowels and other sounds under it, for a paper or grammar, in the format chosen for **Copy example** in the Gloss panel: a table for Word and Google Docs, a LaTeX `tabular`, or plain text.

**Features** gives each sound on the chart its distinctive features, + or − for syllabic, consonantal, sonorant, continuant, delayed release, nasal, lateral, voice, labial, coronal, dorsal, high, low, back and round (after Hayes's *Introductory Phonology*, simplified; a blank where a feature does not apply). Click a feature's name for +, again for −, again to drop it: the sounds with every value you picked stay lit, and the line above names the class, such as `[+voice, −sonorant]: b d g z`. It is a quick way to see whether a set of sounds in a rule is a natural class. **Copy table** copies the whole table.

**Possible allophones** goes through every pair of similar sounds (one or two of those features apart, each used at least five times) and lists the pairs that never occur in the same environment (the same sound before and after, a word edge counting as one): the pairs a phonologist checks first when working out which sounds are phonemes. Each comes with the rule **Environments** would suggest, when there is one, and a button that opens the pair in Environments. In a small corpus many pairs keep apart by chance, so treat the list as places to look.

**Minimal pairs** lists every pair of sounds that two words tell apart, such as `p ~ b` for `pata` and `bata`, best attested first. Click a pair of sounds to see its words, and click a word to open its lines in the Concordance. Choose a sound to see only the pairs it is in. **Copy pairs** copies the words as a table. In a large corpus the 20,000 commonest word types are read.

**Compared with PHOIBLE** sets the sounds of the track beside [PHOIBLE](https://phoible.org), a collection of about 3,000 phoneme inventories. It needs the project's Glottocode (the Language row of the Texts panel's Import and export). Press **Compare with PHOIBLE**:

- When PHOIBLE has an inventory of the language (choose one when it has several), the panel lists the phonemes it gives that these transcriptions have not shown yet, and the sounds here that it does not list. The first may be sounds not yet recorded, written another way, or missing from this variety; the second may be allophones, loans, another analysis or slips.
- It also lists the sounds most of the language's relatives have (half or more of its Glottolog family, or of its macroarea when PHOIBLE has fewer than three of the family) that these transcriptions lack, and the sounds here that fewer than one relative in twenty has.

Sounds are matched as written, apart from tie bars, so write affricates with a tie bar (`t͡ʃ`) for them to count as one sound. Hover over a sound for its share among the relatives. Nothing is sent anywhere: PHOIBLE's inventories come with the app.

Click any of these sounds for a map of which of the compared languages have it in their PHOIBLE inventory: a dot per language at its Glottolog place, the project's language ringed, as the Typology panel's maps draw them. Click it again to close the map. **Save map…** saves it as an SVG file.

**Environments** compares where two sounds occur, to test whether they are separate phonemes or allophones of one. Choose the two sounds, or press **Environments** beside a pair of sounds under Minimal pairs. The line under them says what the counts suggest:

- If the two share an environment (the same sound before and after, such as `a_a`, with `#` for the edge of a word), they contrast there or vary freely, and minimal pairs tell which.
- If they never do, they may be in complementary distribution. When one side alone tells them apart, it says so, and names what the sounds on each side have in common, such as "«s» only before a, o, u (back vowels), «ʃ» only before i, e (front vowels)". These natural classes come from where each sound sits on the IPA chart (height, backness and rounding for vowels; voicing, place and manner for consonants), so check them against your own analysis.
- Then it offers **A rule to try**, as phonology problem sets are solved: the sound whose environment is narrower and forms a natural class is taken as the conditioned allophone, the other (found elsewhere) as the underlying sound, such as `/s/ → [ʃ] / _ front vowels`. It is a hypothesis from these words, to check against more data and against what speakers hear.

The **Before** and **After** tables count each sound by what follows and what precedes it. Under them, every whole environment is listed with an example word of each sound, which opens its lines in the Concordance. Rows where both sounds occur are shaded. Counts are of running words. **Copy table** copies the environments with their counts and examples.

**Word shapes** shows each word as consonants (C) and vowels (V), such as `CVCV` or `CVC`, with its share of the running words. **Syllables** cuts each word into syllables and counts their shapes (`CV`, `CVC`, `V`…) with an example word written with dots (`kun.ta`). Between two vowels, the next syllable starts with the longest run of consonants that starts a word somewhere in the data (so `aspa` is `a.spa` when words start with `sp`, and `kunta` is `kun.ta` when none starts with `nt`), and one consonant can always start a syllable. A line gives the template, such as `(C)(C)V(C)`, and how many syllables are open. Each vowel counts as a syllable, so a diphthong written as two vowel letters counts as two; check the cuts before you rely on them. The grammar sketch prints the same table. **Tone melodies** reads each word as the tones on its vowels: `á` H, `à` L, `ā` M, `â` HL, `ǎ` LH, `a̋` XH, `ȁ` XL, and – for a vowel with no mark (in many spellings the default tone). Tone letters and tone numbers written after a syllable (`ma˥˩`, `ma⁵¹`) are read as written. The table counts each melody (`H.L`, `L.H.H`) by number of syllables, with the word forms and running words that have it and an example (click it for its lines in the Concordance). A note gives how many words carry a mark, each tone's share, and the two-syllable melodies of level tones that never occur: the gaps a tone chapter discusses. **Copy table** copies the melodies. Without any tone marks, a line says so. **Where sounds occur** gives, for each sound, the share of its uses at the start of a word, inside one, at the end, and as a word on its own. A note names the consonants never found at the start or at the end of a word, the gaps a phonology chapter describes.

**Consonant clusters** says how many consonants a word starts with before its first vowel and ends with after its last, the syllable limits a grammar states, then lists every run of two or more consonants with how often it starts a word, stands inside one or ends one, and an example word. Click a cluster to see every word with it in the Concordance.

**Not on the IPA chart** lists capital letters and letters the IPA does not use. These are often typing slips, or a sign that the track is written in a spelling rather than IPA. A spelling with digraphs (`ng`, `ch`) shows its letters one by one.

### Show a spectrogram

Click **Spectrogram** in the Timeline toolbar to see the recording's spectrogram under the waveform: frequency from 0 at the bottom to 5000 Hz at the top, darker where louder, the way Praat draws it. Formants show as dark bands, fricatives as noise high up, and stop closures as gaps. wugs makes the spectrogram itself, so it works without Praat. Turn on **Formants** too, and the formant dots sit on the bands.

Like Praat's analysis, the spectrogram shows at most one minute of view at a time. Zoom in to see it. Click **Spectrogram** again to hide it.

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

### Import files from ELAN, FLEx, Praat and other tools

1. Click **Import a file…**.
2. Choose the file.

Or drop the file anywhere on the wugs window.

wugs reads these formats:

- ELAN `.eaf`
- FLEx `.flextext` and LIFT `.lift`
- Praat `.TextGrid`
- Toolbox `.db`, `.sfm`, and `.lex`, and `.txt` files in Toolbox format. A lexicon (`\lx` records) goes to the Lexicon. An interlinear text (`\ref` records with `\tx` lines, as Toolbox's Interlinearize writes them) becomes texts:
  - each `\id` starts a text, and each `\ref` is a sentence;
  - the words of `\tx`, and the morphs of `\mb` with their `\ge` glosses lined up under them, arrive in the Gloss panel already glossed (`ni-` is a prefix, `-ler` a suffix, `=ma` a clitic);
  - a root whose form and gloss match exactly one lexicon sense is linked to it, so import your Toolbox lexicon first;
  - `\ft` becomes the free translation, `\nt` a note on the sentence, and `\ELANBegin` / `\ELANEnd` its times;
  - other lines such as `\ps` are counted in the message. Importing the same file again adds nothing.
- CoNLL-U `.conllu` (Universal Dependencies treebanks, and the output of UDPipe, Stanza and spaCy): each `# newdoc` becomes a text, each sentence a segment and each word a word in the Gloss panel. Lemmas, tags, features, the dependency tree, multi-word tokens (`al` = `a` + `el`), empty nodes and every comment are kept with the sentence. Punctuation is kept but not counted as a word in Concordance and Frequencies. A file with a mistake is refused whole, and the message names each problem by line. Importing the same file again adds nothing.
- TEI `.xml` (TEI P5 and P4, as ParlaMint, national corpora and digital editions publish them): each `<TEI>` becomes a text, titled from its header. Each `<s>` becomes a segment, or each paragraph, speaker turn (`<u>`), verse line or block when it has no `<s>` inside. Each `<w>` and `<pc>` becomes a word with its `lemma`, `pos` and `msd`, so a TEI corpus answers CQP queries and exports as CoNLL-U. Notes, page furniture and figures are left out, and the message counts them. Importing the same file again adds nothing.
- Glossed examples in a `.txt`, as papers, grammars and handouts print them: copy the examples from a Word file or PDF into a plain text file, with a blank line between examples. Each example is a line of words cut into morphemes (`kitab-lar-ı`), a gloss line under it with as many words and pieces (`book-PL-ACC`), and a translation in quotes (`'Ali read the books.'`). An example number such as `(1)`, `12a.` or `b.` at the start is kept. A line above the morphemes with as many words is the sentence as written; without one, the words are the morphemes joined up. A line above that (a language name, a source) and anything after the closing quote (a page reference) become notes on the sentence. `=` marks a clitic, `~` reduplication and `<um>` an infix; the root is the piece glossed with a lower-case word. The examples become one text, glossed in the Gloss panel and with translations, and roots whose form and gloss match one lexicon sense are linked to it. A block whose pieces do not match is left out, and the message counts it. Importing the same file again adds nothing.
- A table of sentences, `.csv` or `.tsv` (or Excel's Unicode text), or a spreadsheet as it is (`.xlsx`, `.xls`, `.ods`: its first sheet with anything in it; dates come in as `2026-03-12`): a row per sentence. wugs takes the text from a column named sentence, utterance, transcription, text and the like (else the column with the longest cells) and the translation from a column named translation, free translation or English. A column named document, file or title makes one text per value. Columns named start and end that hold times (seconds, or `0:01:02.500`) time each row, so a spreadsheet wugs wrote comes back with its times. Every other column is kept with its sentence.
- Subtitles, SubRip `.srt` and WebVTT `.vtt`, as video editors, YouTube and Subtitle Edit write them: the file becomes a text with a timed segment per caption, in order, its lines joined. Styling such as `<i>` is dropped. A WebVTT speaker (`<v Ana>`) is kept with the sentence. Pick the video's audio as the text's recording on the Timeline to hear each caption. Importing the same file again adds nothing.
- Audacity labels (`.txt`, from Audacity's **File > Export > Labels**): a timed segment per label, in order. A point label (one with no length) comes in untimed. Import the same recording's WAV and pick it on the Timeline to hear them.
- Plain text `.txt`: any other text file becomes a text named after the file, and the message says how it was cut. A file whose lines mostly end in a full stop, with no blank lines, comes in a segment per line. Otherwise blank lines separate paragraphs, a line ending in a full stop ends its sentence, and inside a line a sentence ends after `.`, `!`, `?` or the full stop of another script when the next word starts with a capital, a digit or a quote; "e.g. the" stays together, but "Dr. Smith" is cut. Thai and Lao paragraphs come in whole, and an orthography written all in lower case is cut only at line ends and blank lines. The file must be UTF-8 or UTF-16; a damaged one is refused. Importing the same file again adds nothing.

A message reports what wugs imported, and the new text is selected. The import report also counts anything the format holds that wugs does not keep. For FLEx, that includes notes, pronunciations, relations, and etymologies.

An imported ELAN or Praat text is not linked to its recording. Import the audio with **Import audio…** on the Timeline and pick it there.

### Export a text or the lexicon

To export a text, select it, choose ELAN (.eaf), FLEx (.flextext), Praat (.TextGrid), CoNLL-U (.conllu), TEI (.xml), Toolbox interlinear text, or plain text, and click **Export…**. ELAN gets the glossing too: under each track whose sentences are broken into words, three tracks of ELAN's usual interlinear kind follow it, named after it: **words** (the words of each sentence), **morphs** (each word's morphemes, marked `ni-`, `-ler`, `=ma` as in a Leipzig example) and **glosses** (each morpheme's gloss), so the text opens in ELAN glossed. When that file comes back into wugs, those three tracks are recognised as the glossing already here and are not read in again. **Toolbox interlinear text** writes a `\ref` per sentence with its words, morphs and glosses lined up in columns, its free translation and notes, ready for Toolbox or a script that reads Toolbox files. For corpus tools, **CWB vertical** writes a `.vert` file that CQPweb, NoSketch Engine and Sketch Engine import: one word a line with its lemma, UPOS, XPOS and features, `<s>` around each sentence, and `<g/>` where no space follows. **AntConc** writes a `.txt` with a sentence a line and each word as `word_TAG`; turn on tags in AntConc's Tag Settings with `_` as the separator. TEI writes a P5 document with a `<s>` per segment and a `<w>` or `<pc>` per word carrying its lemma, part of speech and features, and each translation as a `<note type="translation">`. Plain text writes one track a segment per line, in time order, which AntConc and most corpus tools read. **Spreadsheet** writes a `.csv` with a row per sentence: the sentence, its translation, the words cut into morphemes and their glosses (when any word is glossed), its start and end in seconds (when it is timed), and every column a table import brought in with it, so a table you imported goes back out with your edits. Excel, LibreOffice and Google Sheets open it, and wugs reads it back.

For a video of the recording, **Subtitles for a video** (`.srt`) and **Subtitles for the web** (`.vtt`) write each timed sentence as a caption with its translation on a second line, so viewers read the language and its meaning together. Video editors (Premiere, DaVinci Resolve, CapCut, Shotcut), YouTube and VLC read SubRip; web pages and most players read WebVTT. Sentences with no time are left out, and the message counts them. A `.vtt` file wugs wrote comes back with each caption's second line as its translation. **Audacity labels** writes each timed sentence as a label (`start`, `end`, text, times in seconds); in Audacity, choose **File > Import > Labels** with the recording open to see the segments as a label track.

CoNLL-U gives back an imported treebank exactly as it came in, with your edits: retyped words, and a `# text` line that matches the transcription. A word you add has no tags yet; the words around it keep theirs, and the tree still points at the same words. A text made in wugs exports too. Each sentence gets `# sent_id`, `# text` and its translations (`# text_en` when the translation track has a language), and each glossed word carries its analysis in MISC the way UD treebanks do: `Gloss=` for one morpheme, `MSeg=` and `MGloss=` for several. A root linked to a lexicon entry also gives LEMMA, and the entry's part of speech goes in XPOS (and UPOS when it is a common name such as noun or verb).

To export every glossed sentence of a text as numbered examples for a grammar, a paper or a handout, select the text, choose a format beside **Glossed examples for**, and click **Export…**. Each example is the same as **Copy example** gives in the Gloss panel, with the same number:

- **LaTeX (ExPex)** and **LaTeX (gb4e)** write a `.tex` file that compiles on its own with XeLaTeX or LuaLaTeX. Those read IPA and every script. Change the font named near the top to one you have, such as Charis SIL.
- **Word / Google Docs** writes an `.html` file. Open it in Word, or upload it to Google Docs, and save it as a document there.
- **Plain text** writes aligned columns.

Sentences not glossed yet are left out, and the message says how many. After the examples comes the list of abbreviations their glosses use, with the meanings from the Interlinear panel's **Abbreviations**. An abbreviation with no meaning yet shows "?".

To give a text back to the people who told it, or to use it in a literacy class, select the text and click **Save…** beside **Reader to print**. wugs writes one `.html` file with the text's title, the language's name, who told it, where and when (from the text's **Details**), then each sentence in large type with its translation. Choose **translation beside** for two columns, or **translation under** for a narrow page or a phone. There are no glosses and no linguistic terms. Open the file in a browser or Word to print it or save it as a PDF. A sentence with no translation yet prints alone, and the message says how many.

To export the lexicon, choose LIFT, Toolbox or **Spreadsheet** beside **Lexicon as** and click **Export…**. FLEx and WeSay read LIFT. A spreadsheet is a `.csv` file with a row per sense: lemma, homograph, category, citation, sense number, gloss, a column for each other gloss language, definition, usage and semantic domains. Excel, LibreOffice and Google Sheets open it.

To bring in a dictionary you keep in a spreadsheet, choose it as it is (`.xlsx`, `.xls` or `.ods`), or saved as `.csv` or `.tsv`, with **Import a lexicon table…**. Each row is a sense. wugs finds the columns by their names:

- the word: **lemma**, **headword**, **word**, **lexeme** or **form** (else the first column);
- its gloss: **gloss**, **english** or **meaning**;
- glosses in other languages: **gloss (fr)**, **gloss_es** and the like;
- **category** or **part of speech**, **citation**, **homograph**, **definition**, **usage** or **notes**.

Rows with the same word become one entry with a sense per row. A word already in the lexicon gets the new senses, and a sense whose gloss it already has is skipped, so importing the same table twice adds nothing. The message after the import names the columns it read and the ones it left out. When several entries share a word, add a **homograph** column so wugs knows which one a row belongs to.

wugs names the file after the text's title and never replaces a file that exists. It adds " (2)" to the name instead. You cannot export into the project folder itself.

### Say which language the project documents

At the top of **Import and export**, type the language's name and, if you know them, its Glottocode (from [glottolog.org](https://glottolog.org), like `stan1293`) and ISO 639-3 code (three letters, like `tur`). Each field saves when you press Enter or leave it. You do not need to look the codes up: as you type a name (or a code) in the first two fields, wugs lists the languages and dialects in Glottolog that fit, with their family and region, and clicking one fills in both codes. If you had already named the language yourself, such as by the name its speakers use, and you search from the Glottocode field, your name stays. The list comes from Glottolog 5.4, kept inside the app, so it works offline and sends nothing. Archive packages and CLDF datasets carry them, so the data can be found and joined with other datasets. Everyone on the project sees the same language.

### See how far the project has come

The **Progress** panel counts what a grant report, an archive deposit or a supervisor asks for. In Tongues, press **Ctrl+K** and choose **Show Progress**.

It lists how much is recorded, how many sentences are transcribed and how much speech they cover, how many words are glossed and how many sentences are translated, and the size of the lexicon. A table gives the same figures text by text, with the totals under it. Click a text to open it. **Copy table** copies the table, with lengths in minutes, to paste into a spreadsheet or a report.

A sentence is a segment on a transcription track. A word counts as glossed once it is broken into morphs. A recording that is not on this computer is not counted, and the panel says how many there are.

It also says how long the sentences are in words, and how many morphs a glossed word has on average (Greenberg's index of synthesis: near 1 for an isolating language, 2–3 for an agglutinating one), project-wide and text by text under **Morphs per word**. The grammar sketch's morphology chapter starts with the same figure.

**The corpus by detail** breaks the texts down by one of their details (genre, speaker, date, place… as given in the Texts panel's Details or by **Import details…**): for each value, how many texts, sentences and words it has, its share of the words and how many words are glossed, with texts that do not give the detail last. Pick the detail above the table; **Copy table** copies it for the corpus description in a paper or a deposit form.

Under the table, **Vocabulary growth** draws how many different words have come up against the running words, reading the texts in the order they were made (a dashed line where each text starts). A curve still climbing steeply means each new recording still brings many new words; a flattening one means most words of these kinds of text are in, and it may be time to record other genres. The sentence under it compares how many new words per 100 the first and the last tenth brought.

### See the affixes in their slots

The **Affixes** panel reads every glossed word in the project and lists its affixes and clitics the way a grammar's morphology chapter does. In Tongues, press **Ctrl+K** and choose **Show Affixes**.

At the top is a position-class chart: a column for each slot, counted from the root (−1 right before it, +1 right after it), with each affix in the slot it holds most often, its gloss and how often it is used. Click an affix to see every word it is in, in the Concordance. Under the chart, a table gives each affix's gloss, slot (and the other slots it turns up in), uses, how many different roots it is found on, its productivity **P** (Baayen's measure: the share of its uses in word forms met only once, so an affix still used to make new words scores higher than one found only in a few set words; hover for the counts, and read it only for affixes used often), the parts of speech of those roots (from their lexicon entries), and an example word. Click the example to open its sentence.

**Order** lists every two affixes found together in one word and how often each comes first: the evidence a slot chart rests on. Two affixes in one slot should never occur in the same word, and two in different slots should always come in the slots' order, so rows that break either rule are marked, with a note saying which. A pair met in both orders may be a glossing slip or a real freedom of order; open the words to see. **Copy pairs** copies the table.

**Forms of one gloss** lists every gloss written more than one way on the same side of the root, such as `-si` and `-su` both glossed PL: candidates for allomorphs. For each form it shows the sounds right next to it in the words it is in, and the stem's nearest vowel (the last vowel before a suffix, the first after a prefix), each named as a natural class where one fits. When the forms never share a neighbouring sound, or never share a nearest vowel, the panel says that this may be what conditions them (assimilation, a vowel- or consonant-final stem, vowel harmony), and marks that column; when neither keeps them apart, they may be free variants or glossing slips. Click a form to see its words in the Concordance. **Copy forms** copies the sets as a table.

**Guessed from the words** needs nothing glossed: it reads the transcriptions and looks for stems that take the same set of endings, as `walk` and `talk` both take nothing, `-s`, `-ed` and `-ing` (Goldsmith's *signatures*, from his program Linguistica). Choose **Endings** or **Beginnings**. Each row is one set, with how many stems take it and the stems most used first; click a stem to see every word starting with it (or ending with it, for beginnings) in the Concordance. A set needs three stems and stems need three letters, and each word is read one way only. Use it early on to see where to look for suffixes and prefixes: some sets are chance, and nothing is written into the project. **Copy table** copies the sets.

An affix is told apart by its spelling, type and gloss, so `-ler` PL and `-ler` 3PL are two rows. A word with no morph marked as a root is left out, and the panel says how many there are. **Copy chart** and **Copy table** copy them to paste into a document or a spreadsheet, and **Grammar sketch…** prints the chart in its morphology chapter.

### Test sound changes and phonological rules

The **Sound changes** panel runs ordered rewrite rules over your words, to test a reconstruction (did *p really become b between vowels?) or the phonological rules of a grammar (underlying forms to surface forms). In Tongues, press **Ctrl+K** and choose **Show Sound changes**.

Write the rules in the box on the left, in the notation of Zompist's Sound Change Applier:

- `V = a e i o u` makes a category: a capital letter and its sounds.
- `p t k > b d g / V_V` changes each sound into the one in the same place, between vowels. `s/z/V_V` is the same rule written with slashes.
- In the place after `/`, `_` is the sound itself, `#` the start or end of the word, `( )` something optional (`_(C)i`), and several places can be given with commas.
- `∅` (or nothing) on the right deletes (`V > ∅ / _#`); on the left it inserts (`∅ > ə / C_C`).
- After `//`, where the rule does not apply (`a > e / _i // k_`).
- `;` starts a comment, and so does `*` at the start of a line.

Rules run in order, each over the words as the rule before left them, and a sound with an accent or other mark is not taken for the plain letter. The words are the lexicon's headwords, or any words you type (**Words typed here**). The table lists each word, what it becomes and the steps, rule by rule (only the words that change, unless you untick that). A line that does not read is named in red and skipped. **Keep with the project** saves the rules in the project so they sync; **Copy table** copies every word with its output and steps. Nothing in the lexicon is changed.

To test the rules against the forms you know, type each word with its attested form after `>` (or `→`, or a tab): `kapa > kab`. The table then has an **Expected** column with ✓ or ✗, the rows the rules get wrong are shaded and listed even when the rules leave the word alone, and the line above says how many come out as expected. Write `∅` for a word that should disappear. **Copy table** adds the expected form and whether it matched.

### Compare your word list with related languages

The **Compare word lists** panel sets the project's answers to a concept list (a Swadesh list, say, from **Elicit**) beside the words other languages have for the same concepts, to look for cognates and sound correspondences. In Tongues, press **Ctrl+K** and choose **Show Compare word lists**.

Pick the concept list at the top, then **Load a word list…** and choose a table of another language's words: a CSV, tab-separated or spreadsheet file with a concept column (Concept, Gloss, Meaning, English or Concepticon_Gloss) and a form column (Form, IPA, Value, Word, Tokens or Segments), such as an EDICTOR file, a LingPy wordlist or a CLDF `forms.csv`. A file with a Doculect or Language column brings each language as its own column; otherwise the file's name is the language. For a CLDF dataset (a Lexibank download, say), choose its `forms.csv`: the `parameters.csv` and `languages.csv` beside it turn its concept and language ids into Concepticon glosses and language names. Load as many files as you like; they stay until you press **Clear** or open another project, and nothing is written to the project.

A concept is matched by its gloss or its Concepticon gloss, case and spacing ignored. Each of their forms shows how alike it is to your word, by sounds (100% the same, 0% nothing in common); forms two sounds in three alike or more are in bold and underlined. Alike is not the same as cognate (loans and chance look alike too, and real cognates may not), so judge them yourself: click one of their forms to mark it cognate with your word (✓), click again for not cognate (crossed out), and once more to take the judgement back. Judgements are kept with the project, so they sync and stay when you load the same list again. Above the table, each language's line says how many of the judged concepts are cognate (the share lexicostatistics counts). Above those, before any judging, each language gets ASJP's distances, closest first: **LDN**, the share of sounds in which the two words for the same concept differ on average, and **LDND**, LDN divided by the same measure between words for different concepts, which takes out the likeness two languages' sound systems give any pair of words. LDND well below 100% means the words for the same things are alike beyond chance; near 100% or above, no closer than chance. They need ten concepts with words on both sides. **Only answered concepts** hides the concepts you have no word for yet. **Copy for EDICTOR** copies every form, yours under the project's language and theirs, as an EDICTOR/LingPy wordlist with a COGID column (your word and every form judged cognate with it share a cognate set), to paste into a `.tsv` file and open at [edictor.org](https://edictor.org) for alignments, or to read with LingPy.

Under the table, **Sound correspondences** aligns your words with one language's, sound by sound, and counts which sounds stand across from which: if your *t* is their *ʔ* in *eye*, *louse* and *skin*, that is a regular correspondence and good evidence the words are related, while a pair met once may be chance. Choose the language and how alike a pair of words must be to be aligned (lower takes in more real cognates, and more chance look-alikes). Each concept gives its closest pair. A sound with nothing across from it shows as **–**. Correspondences of different sounds met twice or more are in bold; **Only where the sounds differ** hides the sounds that stay the same. **Copy table** copies the list with the concepts each was met in.

### Count word orders in the parsed sentences

The **Word order** panel counts, in every sentence with a dependency parse (from CoNLL-U, TEI, or tagging and parsing in the Sentence panel), the orders the World Atlas of Language Structures describes. Open it with **Ctrl+K**, **Show Word order**. It counts:

- subject, object and verb (WALS 81A);
- subject and verb (82A);
- object and verb (83A);
- adposition and noun (85A);
- genitive (86A), adjective (87A), demonstrative (88A), numeral (89A) and relative clause (90A) with the noun;
- negative particle and verb (143A).

Each feature shows how often each order occurs, as a bar. Click an order to open an example sentence in the Sentence panel. Under the bars is the order WALS would call dominant: one at least twice as common as any other, else "No dominant order" (or "Mixed"). Nothing is suggested from fewer than 5. As in WALS, subjects and objects count only when they are nouns, since pronouns often take other places.

**Use … as the answer** writes the suggestion as the project's WALS answer, with the counts as its note, and the Typology panel shows it there. A tagger's parse can be wrong, so open the examples first. **Copy table** copies every feature with its counts.

### See what typology says about the language

The **Typology** panel shows what [Grambank](https://grambank.clld.org) or the [World Atlas of Language Structures](https://wals.info) (WALS) says about the project's language. Choose **Grambank** (195 questions such as "Are there definite or specific articles?") or **WALS** (192 features such as "Order of Subject, Object and Verb") at the top; the choice is remembered. Each feature shows the dataset's answer for the language. Beside each answer it shows how the other coded languages of the same family answer, as a share of each answer and a bar. When the family has fewer than three coded languages, it compares with the languages of the same region instead. Open the panel from Settings (Closed panels) or with **Find an action**. It needs the project's Glottocode (see above).

Many languages are in neither dataset. For these, every feature is listed as **Not coded**, with how related languages answer it, so the list works as a checklist of what to find out. A dialect the dataset has not coded shows the answers for the language it belongs to.

- **Still open** lists the questions without an answer for this language.
- **Unlike its relatives** lists the answers that differ from what most related languages say, which are often worth a second look or a paragraph in the grammar.
- Type in **Find a feature** to filter by words in the question or by its ID.
- Type another language's name or Glottocode at the top and press Enter to see its answers. **The project's language** goes back.
- **Copy table** copies the questions and answers for a spreadsheet.

For the project's own language, each feature also has a **This project** row: choose the answer your data gives (or **Not known** when you asked but could not tell) and type a note, such as an example and the page of your notebook or grammar it comes from. A choice saves at once; a note saves when you press Enter or leave the field. The answers are kept with the project and sync to its members, so a team can fill in the questionnaire together. The line under the language says how many you have answered.

- A feature you answered no longer counts as **Still open**.
- **Ours differs** lists the features where your answer differs from the dataset's. These are worth checking, and if your data is right, worth sending to the dataset's editors.
- **Copy table** adds your answers and notes as two more columns.
- **Copy answers as CLDF** copies your answers as a CLDF `values.csv` table (ID, Language_ID, Parameter_ID, Value, Code_ID, Comment), the shape Grambank and WALS publish theirs in. Paste it into a spreadsheet or a CLDF dataset.

**Map** beside a feature draws where the languages it is compared with answer it: a dot per language at its Glottolog coordinates, coloured by its answer (the legend counts each), with the language itself ringed in its own answer, or the project's. A family that spans the Pacific is drawn centred on it. Hover over a dot for the language and its answer. **Save map…** saves the map as an SVG file for a paper or slides. The map shows areal patterns a share cannot: a feature common in one corner of a family and absent in another.

Grambank and WALS answers are kept apart: switching the dataset shows that dataset's answers.

The data comes from Grambank 1.0 (cite Skirgård et al. 2023) and WALS Online (cite Dryer & Haspelmath 2013), both CC BY 4.0 and kept inside the app. The sources and comments behind each value are on the language's page in the dataset, named under the list.

### Describe each text

Select a text in the **Texts** panel. Under the list, **Details** shows what is recorded about it: who wrote or spoke it, when, where, what kind of text it is. To add a detail, type a name (such as `genre`, `year` or `speaker`) and a value, then press Enter. Names you used on other texts are offered as you type. Change a value and press Enter or leave the field to save it. Press × to clear it.

To describe many texts at once, make a table with a row for each text: one column that names the text (its title, or the name of the file it came from), and a column for each detail. Keep it as a spreadsheet (`.xlsx`, `.xls`, `.ods`) or save it as `.csv` or `.tsv`, then press **Import details…** under Import and export. wugs reports the rows that named no text. Importing the same table again changes only what is different.

Details also come in with files: a TEI file's header (author, date, genre, licence, speakers, place and more), and the `# meta::` lines of a CoNLL-U file. They go back out the same way in TEI and CoNLL-U, and as attributes of `<text>` in a CWB vertical file.

Use details to study part of a corpus. In **Frequencies**, **Collocates** and **Keywords**, the list that picks where to look also offers the texts with one detail, such as **Genre: news (12 texts)**. Keywords then compares those texts with all the others. A row you open shows its lines in the Concordance from the same texts. In a CQP query, see [Search with CQP queries](#search-with-cqp-queries).

### Publish the dictionary and examples as CLDF

Click **Publish as CLDF…** and choose a folder. wugs writes a folder `wugs-cldf-<date>` holding a CLDF Dictionary dataset: the CSV tables and metadata that pycldf, Zenodo, Dictionaria and other CLLD sites read.

- `entries.csv`: one row per lexicon entry, with its part of speech, citation form and homograph number
- `senses.csv`: one row per sense, with its glosses in each language, definition, usage, semantic domains, the Concepticon concept you elicited it for, and the examples that use it
- `examples.csv`: every glossed sentence in Leipzig form (words, morpheme glosses, free translation), with its text and time span
- `languages.csv`: the project's language and the languages of its translations
- `media.csv` and an `audio` folder, when **With sound clips** is ticked (it is unless you untick it): for each glossed sentence with times, a WAV clip of it cut from its text's recording, at the recording's own quality. A CLLD site plays it beside the example. A sentence gets no clip when its text has no recording picked on the Timeline, when the recording's audio is not on this computer, or when the recording is not open; the message says how many.

Sentences from texts that are not open (restricted or community-only) are left out unless you tick **Include texts that are not open**. Tick it only if the speakers agreed to publication. Before you publish the dataset, add a licence and check it with `cldf validate Dictionary-metadata.json` (from `pip install pycldf`).

### Print the dictionary

Press **Dictionary to print…** beside the lexicon export. wugs writes one HTML file. Entries are in the order of your alphabet (the writing system's sort order, if the project has one), under letter headings. Each entry shows its part of speech and numbered senses, each sense with its glosses, definition, usage and semantic domains. Each sense also gets its example sentences from the lexicon, or, when it has none, its first use in your texts, with its translation. When senses are in semantic domains, **Words by topic** follows the entries: each domain in outline order (1.1 Sky before 8.1.1 Cardinal numbers) with its headwords and glosses, linked to their entries, a thematic word list for a community or a classroom. An index of glosses closes the file. Open it in a browser and print it to PDF, or open it in Word or Google Docs to edit it. Examples from texts that are not open are left out unless you tick **Include texts that are not open**.

### Flashcards for learners

Press **Flashcards…** beside **Dictionary to print…** to turn the lexicon into flashcards for people learning the language. wugs writes a folder `wugs-flashcards-<date>` with:

- `anki.txt` for Anki: a note per entry, so each word is asked both ways (word to meaning, meaning to word). The front shows the headword and plays it when the entry has a pronunciation heard in a recording. The back shows the meanings, the part of speech and the first example sentence with its translation.
- `quizlet.txt` for Quizlet and other flashcard sites: a word and its meaning on each line, separated by a tab.
- `sounds/`, the sound of each word that has one, and a README that says how to import.

In Anki, copy the files in `sounds/` into Anki's media folder first (in Anki, **Tools > Check Media > View Files**), then choose **File > Import** and pick `anki.txt`. Each card carries its entry's id, so when you import a newer `anki.txt` later, Anki updates the cards you already have and keeps your progress. Entries with no gloss yet are left out, and the message says how many. Sounds from recordings that are not open are left out unless you tick **Include texts that are not open**.

### Start a grammar sketch

Press **Grammar sketch…** beside **Dictionary to print…**. wugs writes one HTML file with a grammar's usual chapters, filled with what the project already knows, for you to open in Word or LibreOffice and go on writing:

- **Introduction**: the language's name and codes, a map of where Glottolog places it with its nearest languages (when the project has a Glottocode), its family and region, those neighbours by name with their families and distances, and how many texts, words and entries the sketch draws on.
- **Phonology**: the consonants in a chart by place and manner (with the Sounds panel's possible allophones and their rules to try under it) and the vowels, read from your IPA track (or your fullest track) as the Sounds panel reads them, with a table of how often each sound occurs and an example word, then the commonest word shapes (CV, CVCV…) and the consonants never found at the start or end of a word, and, when the words carry tone marks, a **Tone** section with the Sounds panel's tone melodies of one to three syllables and the two-syllable melodies never met. These are the sounds as written, not yet phonemes: say which contrast.
- **Orthography**: your orthography profile's letters beside their IPA, when the Orthography panel keeps one.
- **Morphology**: the lexicon's prefixes, suffixes, infixes and clitics (entries written like `ni-`, `-ok`, `<um>` or `=ma`) with their glosses, the Affixes panel's position-class chart, and an **Allomorphy** section with each gloss written more than one way, its forms' neighbouring sounds and nearest vowels, and what may condition them, then **Endings the words suggest**: the Affixes panel's guesses from the transcriptions alone (stems that share a set of endings), so a project with nothing glossed yet has a start too.
- **Syntax**: when sentences have a dependency parse, a table of the word orders the **Word order** panel counts (object and verb, adpositions, adjective and noun…) with the order each suggests.
- **Typological profile**: your own answers in the Typology panel, for Grambank or WALS, whichever has more.
- **Texts**: each text with its sentences, words and glossed words.

Bracketed passages in brown mark the places only you can write (word order, allophones, syllables and the like). The file is a starting point; wugs does not keep it in step with the project, so make a new one when the data has grown.

### Publish a website for the community

Press **Publish as a website…** to make a folder of web pages from your texts and dictionary, for the people whose language it is. It needs no server and nothing to install: open `index.html` in any browser, copy the folder to a USB stick or a phone, or put it on any web host.

- The first page has a search box and lists the texts, with a link to the dictionary. The search looks through headwords, glosses, sentences and translations as the reader types, and ignores capitals and accents, so a reader without the language's keyboard still finds words. Each result links to its dictionary entry or its sentence.
- When the project has a Glottocode (the Language row above), the first page also shows a small map of where Glottolog places the language, with its nearest languages as grey dots, and names four of them.
- Each text shows its sentences one by one: the sentence, each glossed word's morphs and glosses under it, and the translation.
- With **With sound clips** ticked, a ▶ beside each timed sentence plays it, cut from the text's recording.
- The dictionary is the same page **Dictionary to print…** makes.
- Put on a web host (any address that starts with `https://`), the site works as an app: on a phone, the browser offers **Add to Home Screen** or **Install**, and the site opens like any app, under the language's name. The first visit keeps every page, the search and the pictures on the phone, and each sound once it has played, so the site still works with no connection. A newer export replaces what was kept the next time a reader opens it online. Opened from a file or a USB stick, the site works as before, without the offline part. The app's icon is the wug; to use your own, replace `icon-192.png` and `icon-512.png` in the folder with square PNG pictures of those sizes.

Texts that are not open are left out unless you tick **Include texts that are not open**, and notes on sentences are never published. Check that the speakers agreed before you share the site. Each export goes into a new folder named `wugs-site-` and the date; an earlier one is never replaced.

### Load the corpus into CQPweb or Sketch Engine

Press **Corpus for CQPweb…** to export every text as one corpus. wugs makes a folder with three files:

- A vertical file (`.vert`): each word with its lemma, part of speech and features, sentences with their translations, and each text with its details.
- `encode.sh`: the commands that load the corpus into the Corpus Workbench, so that CQPweb and `cqp` can search it.
- `README.md`: how to load the corpus into CQPweb, NoSketch Engine and Sketch Engine.

Texts that are not open are left out unless you tick **Include texts that are not open**.

### Seal a version for the archive

1. Click **Seal a version for the archive…**.
2. Choose a folder outside the project folder.

wugs writes a BagIt package named `wugs-sealed-<date>`. It holds ELAN, FLEx, LIFT and CLDF copies, OLAC and IMDI metadata, every recording, the full history, and checksums for every file. ELAR, PARADISEC, and AILLA accept BagIt deposits. The IMDI metadata gives each text as a session with its date, place, genre and description (from the text's details, else its recording's) and, as its actors, the people whose consent records cover the text or its recording, with their role and details from the People panel.

If any of the project is restricted, the message says so. The package then holds restricted data, so treat the whole package as restricted.

Sealing copies every recording. Other actions wait until it finishes.

Archives also ask for a description of each item in their deposit forms. Click **Catalogue sheet…** beside the seal to save a spreadsheet with a row per text: its title, date, place, speakers, genre, description and keywords (from the text's **Details**, else from its recording's details in the Media panel), the language with its ISO 639-3 code and Glottocode, the linked recording and its length in seconds, its sharing level, and how many of its sentences are transcribed and translated and how many words are glossed. Any other details follow in their own columns. Fill the archive's forms from it, or import it where the archive's tool takes a spreadsheet. The message says which texts have no recording linked.

## Search the project

Type in the **Search** panel. Choose **All**, **Texts**, or **Lexicon** to narrow the results. Results are grouped by text. Click a result, or select it and press **Enter**, to jump to that segment.

Search ignores accents in both directions, so `e` finds `é`.

### Find and replace

To change a spelling everywhere, for example after you settle on an orthography, click **Replace…** beside the scope buttons. The search box is now what to find, and a second box is what to write instead.

1. Type what to find. It is matched exactly as you type it: no pattern syntax, and accents count, so `e` does not find `é`.
2. Type the replacement. Leave it empty to delete what you found.
3. Choose **Transcriptions**, **Translations**, **Headwords** or **Glosses**, and tick **Whole words**, **Match case** or **Only the text on screen** if you need them. Without **Match case**, capitals do not matter.
4. Every sentence that would change is listed with the change marked. Untick any you want to leave alone. Click a sentence to open it.
5. Click **Replace in N sentences**. The message that follows has an **Undo** button.

Each change is written as if you had typed it, so it syncs like any edit. A sentence someone changed after the preview is left alone, and the message says how many. A sentence marked **glossed** already has its words broken out: after the replace, open it in Gloss and use **Retokenize** to bring the words in line. A preview lists at most 2000 sentences; replace those, and the preview shows the rest.

**Headwords** changes the lexicon instead: each entry's lemma, citation form and other forms, so a new spelling reaches the dictionary too. Run it once with **Transcriptions** and once with **Headwords** to change both. Each headword is written as if you had retyped it, and **Undo** puts them back.

**Glosses** renames a gloss across every glossed word, for example `PST` to `PAST` when you change your abbreviations. Tick **Whole words** and **Match case** to change only the abbreviation itself: `SG` as a whole word does not touch `1SG`, which is one word. Each morph's gloss is written as if you had typed it in the Gloss panel, and **Undo** puts them back. Glosses of lexicon senses are not changed; edit those in the Lexicon.


## See every occurrence in context

The **Concordance** panel lists every place a word is used, one line per occurrence, with the word in the middle and what was said before and after it. It is a tab beside Lexicon. If your layout predates it, press **Ctrl+K** and choose **Show Concordance**.

1. Choose what to look for: **Words** as transcribed, **Morphs**, **Glosses** (such as PL or 1SG), or words of **Translations**.
2. Type it. Capital letters do not matter, but diacritics do: ŋa never finds na. Morph markers do not matter either: `-ra` and `ra` find the same morph. Use `*` for any letters (`ni*` finds every word starting with ni) and `?` for one letter.
3. Sort the lines **By the word before** or **By the word after** to see patterns, as in a concordancer such as AntConc. **By the second word before** and **By the second word after** sort by the word one further out (AntConc's 2L and 2R), and **By the match, then after** groups the different words a pattern such as `kan*` matched. The word the lines are sorted by is underlined in each line.

Click a line to open its segment on the Timeline and in Transcribe and Gloss. Click ▶ to play it. **Copy rows** copies every line as a table you can paste into a spreadsheet, with each line's start and end in seconds, the word's morphs and glosses when lines have them, and a column for each detail of its text (genre, speaker, date…), ready for a variation study in R or a spreadsheet; pasted into Word, LibreOffice or Google Docs it comes in as a concordance display, the left context right-aligned, the match in bold and the text named after each line, ready for a paper or handout. A morph or gloss line also shows the whole word's analysis under it.

**With translations** shows each line's translation under it in italics, so lines in a language you are still learning can be read at a glance; a line found in a translation shows the sentence it translates instead. The translation is the one the Gloss panel shows: a paired translation track's, else the timed translation overlapping the sentence most. While it is on, **Copy rows** copies the translations too: a translation column in the table, and each translation in italics under its line when pasted into a word processor. The choice is remembered on this computer.

When a search finds too many lines to read, choose **50**, **100**, **250** or **500 at random** beside the sort (instead of **Every line**) to look at a sample, as CQPweb's "thin" does. The sample keeps the lines in their order and spreads over the whole result. The same search always gives the same sample, so a count you report from it can be checked. Copy rows and Copy as examples copy the sample. Only the first 2000 lines found are sampled, and the count line says so.

**Copy as examples** copies the sentences the lines come from, each once and in the order shown, as numbered glossed examples with their translations, ready to paste into a paper. It uses the format last chosen for **Copy example** in the Gloss panel (Word or Google Docs, LaTeX gb4e or ExPex, or plain text). A line from a translation brings the sentence it translates. At most 50 sentences are copied at once.

**Save clips…** saves the sentences the lines come from as sound files: every token of a word or a morph, ready to open in Praat or to play in an experiment. Choose a folder outside the project. wugs makes a folder named after your search (`kani clips`) with a WAV per sentence, each cut from its text's own recording at the recording's quality and named after its transcription, and `clips.csv` listing each file with its start, end, transcription, translation, text and the word searched for. A sentence holding two matches is saved once. Lines with no times, and lines whose text has no recording picked on this computer, are left out, and the message says how many.

**Clusters** (AntConc's Clusters tool) counts the word sequences that hold what you searched for: choose how many words (2 to 5) and whether the item comes first, last or anywhere in them. `kani` with 2 words, item first, lists `kani ŋai`, `kani haru` and so on, commonest first, with how many lines and texts each is in. Click a cluster to see its lines. Clusters are counted over the lines found (at most 2000); **Copy rows** copies them as a table.

### See where it occurs across your texts

Click **Plot** beside the search box to see, text by text, where the word, morph or gloss occurs, as AntConc's Plot does. Each text gets a bar as long as the text, with a mark at every occurrence, its count and its rate per 1000 words.

The line above the bars says how evenly it is spread, as Gries's DP: 0 means it occurs in step with each text's size, and 1 means it all sits in one text. A word that seems frequent but sits in one text tells you about that text, not about the language.

- Order the texts as they are, by most matches, or by highest rate.
- Click a text to see its lines in the Concordance.
- **Copy rows** copies the plot as a table: each text, its words, its matches, the rate and the word positions.
- When texts have details (genre, speaker, place…), the menu beside **Plot** can sum them **By genre** or by any other detail: a bar per value with its matches, its rate per 1000 words and how many texts have it, texts without the detail last. It shows how the item varies across the corpus's parts, such as a word common in narratives and rare in conversation. **Copy rows** then copies that table.

The plot works for words, morphs, glosses and translation words, not CQP queries.

### Search with CQP queries

Choose **CQP query** to search the way CQPweb, Sketch Engine and NoSketch Engine do. Each token goes in square brackets, and a query can be several tokens in a row:

| Query | Finds |
| --- | --- |
| `"dog"` | the word dog (the same as `[word="dog"]`) |
| `[lemma="go"] [upos="ADP"]` | any form of go followed by a preposition |
| `[word="ni.*" %c]` | words starting with ni, any case |
| `[upos="DET"] [upos="ADJ"]? [upos="NOUN"]` | a determiner and a noun, with or without an adjective between |
| `[lemma="give"] []{0,3} [upos="NOUN"]` | give, then a noun up to three words later |
| `[upos="ADJ" & !lemma="big"]` | adjectives other than big |
| `[feats=".*Number=Plur.*"]` | plural words |
| `[gloss=".*PL.*"]` | words whose morpheme glosses include PL |

The attributes are `word`, `lemma`, `upos` (also `pos`), `xpos`, `feats` and `gloss`. A value is a regular expression that must match the whole value. Case matters unless you add `%c`, and `%d` ignores diacritics. Join conditions with `&` (and), `|` (or) and `!` (not), and repeat a token with `?`, `*`, `+` or `{1,3}`. `[]` is any token.

To search only some texts, end the query with `::` and a condition on their details, written as CQPweb writes it. Join several conditions with `&`:

| Query | Finds |
| --- | --- |
| `[upos="NOUN"] :: match.text_genre="news"` | nouns in texts whose genre is news |
| `"dog" :: match.text_year="19.*" & match.text_speaker!="B"` | dog in texts from the 1900s not spoken by B |

A text that lacks the detail reads as empty, so `!=` keeps it.

The lines of a CQP query in a tagged corpus can show each word's part of speech beside it: press **Tags** above the lines, and `a el mar` reads `a_ADP el_DET mar_NOUN`, as AntConc shows a tagged corpus. **Copy rows** then copies the tags too. The choice is remembered on this computer.

Lemmas and tags come from CoNLL-U files. For words you glossed in wugs, the lemma is the root's lexicon entry, and its part of speech is the tag. Punctuation counts as a token in a query, so `[upos!="PUNCT"]` skips it. A match never runs from one sentence into the next. If a query has a mistake, the panel says what is wrong and where.

## Correct a sentence's tags and tree

The **Sentence** panel shows the selected sentence's dependency tree: each word with its part of speech, and an arc from each word's head to it, labelled with the relation. It is a tab beside Gloss in Scrolls; in Tongues, press **Ctrl+K** and choose **Show Sentence**.

Under the tree, a table lists every word with its lemma, UPOS, XPOS, features, head and relation. Change a cell and the tree redraws:

- **UPOS** is one of the Universal Dependencies tags.
- **Head** is the word it depends on, by number, or 0 for the root.
- **Relation** suggests UD's relations; type a subtype after a colon, such as `obl:tmod`.
- **Features** are written as UD writes them: `Number=Plur|Case=Nom`.

Click a word in the tree to find its row. If the tree has a problem (no root, two roots, words that are each other's heads, or a head that is no longer in the sentence), a line under the tree says so. You can still save it while you work.

Sentences imported from CoNLL-U or TEI arrive with their tags and tree. A sentence typed in wugs gets them the first time you correct a cell. Every correction comes back out in a CoNLL-U or TEI export.

### Tag and parse a text automatically

A tagger can fill in lemmas, parts of speech, features and a tree for every sentence, ready for you to check. wugs works with three taggers you may know: UDPipe, made at Charles University with models for over a hundred languages, spaCy, and Stanza, made at Stanford with models for about seventy languages trained on Universal Dependencies. Select a sentence of the text, then press **Tag and parse this text** at the top of the **Sentence** panel. To tag a whole corpus, press **Every text** beside it: wugs tags one text after another with the same tagger, each written as soon as it is done, and **Stop** halts it after the text it is on. A text that cannot be tagged (one that is not open, for the web service) is named at the end and the rest go on; if two texts in a row fail the same way, the tagger itself is not working, so wugs stops and says why.

A tagger is usually right, but not always, so its tags stay marked until a person has looked. A sentence tagged by a machine shows a yellow line in the Sentence panel naming the tagger. Correct anything wrong in the table, which counts as checking it, or press **Mark checked** when it reads right; **Checked, next** does that and moves on to the next sentence still to check. The line under it counts how many of the text's tagged sentences are left, with **Next to check**, and **Mark all checked** (press it twice) for a text you trust. Marking a sentence checked changes nothing in its tags.

Choose the tagger under **Tagger…**. **Automatic** uses UDPipe on this computer when it is set up, else spaCy, else Stanza. It never uses the web service unless you choose it.

- **UDPipe on this computer.** Nothing leaves your computer. Install UDPipe 1 from [ufal.mff.cuni.cz/udpipe/1](https://ufal.mff.cuni.cz/udpipe/1) and download a model for your language from the UDPipe models page. Then press **Choose UDPipe…** (if wugs did not find it) and **Choose a model…**.
- **spaCy on this computer.** Nothing leaves your computer. spaCy runs in Python: install it with `python -m pip install spacy`, then a pipeline for your language, such as `python -m spacy download en_core_web_sm` (spacy.io/models lists them). Type the pipeline's name and press **Use**. wugs loads it once to check it and says what it found. If spaCy is in another Python, such as a virtual environment, press **Choose Python for spaCy…** and pick that Python. Each line of your text stays one sentence.
- **Stanza on this computer.** Nothing leaves your computer. Stanza runs in Python too: install it with `python -m pip install stanza`, then download your language's models once, such as `python -c "import stanza; stanza.download('de')"`. Type the language's code (`de`, `grc`, `tr`…) and press **Use**. wugs loads the models once to check them; it never downloads anything itself. Words Stanza splits, such as Spanish `del` = `de` + `el`, are kept as one written word over two syntactic words. Each line of your text stays one sentence.
- **UDPipe 2 (LINDAT web service).** Nothing to install, and its models are better. Your text goes to ÚFAL in Prague, and their terms let them keep what is sent, so each project must allow it first. Texts that are not open are never sent. The service picks the model from the project's language (set in the Texts panel), or type a language or model name.

Only sentences with no tags yet are sent. Sentences you corrected or imported with tags keep theirs. For words you already split in the Gloss panel, the tags are added to your words and your glosses stay. A sentence whose words you split differently from the tagger is left alone, and wugs tells you how many. Check the result: a tagger's guess is a start, not an answer.

### Sketch a word's grammatical company

Once sentences are parsed, the **Word sketch** panel shows a lemma's company by grammatical relation, as Sketch Engine's word sketches do: the objects and subjects of a verb, the adjectives and determiners of a noun, the verbs a noun is the object of. It is a tab beside Search in Scrolls; in Tongues press **Ctrl+K** and choose **Show Word sketch**.

Type a lemma and press **Sketch**. Each relation is a box with the words found in it, how often (**n**) and their logDice, Sketch Engine's association score (14 when the two always come together, lower the more each also turns up apart), strongest first. Click a word to see the lines with both in the Concordance. Only parsed sentences count, punctuation is left out, and **Copy table** copies every relation and word.

To compare two lemmas, as Sketch Engine's sketch difference does, type the second in **Compare with…** and press **Sketch** again. Each relation then lists the collocates of either lemma with both scores, those typical of the first lemma first (shaded blue), those used alike in the middle (within one logDice), and those typical of the second last (shaded sand). `eat` against `drink` puts *bread* and *rice* at the top of the objects and *water* and *tea* at the bottom. Clear the field to sketch one lemma again.

### Find words used alike

The **Similar words** panel lists the words used in the same company as a word, as Sketch Engine's Thesaurus does, but from the plain text, so it needs no tagger or parser: type `dog` and words such as `cat` come first because both are found before `sleeps` and after `the`. The words found are candidates for synonyms, antonyms and words of one class (days, colours, verbs of motion) to check in the Concordance. Open it from Settings (Closed panels) or with **Find an action**.

Type a word and press **Enter**. Each word is described by the words within the **window** either side of it in the same segment (1 or 2 finds words of one grammatical class, 5 words of one topic), weighted by how much more often than chance they occur together (positive pointwise mutual information), and two words are as similar as those descriptions (their cosine, 1 when the company is the same). **Similarity** is that score, **Shared contexts** the words around both that count most. Only words met at least the chosen number of times are compared. Choose **Words** or **Translations**, and where to look as in Collocates. Click a row to read that word in the Concordance; **Copy table** copies the list. On a corpus of a few thousand words the list is only a hint: the method needs many contexts.

## Count words, morphs and glosses

The **Frequencies** panel lists the words of the project with how often each occurs, most frequent first. It is a tab beside Search in Scrolls; in Tongues, press **Ctrl+K** and choose **Show Frequencies**.

1. Choose what to count: **Words**, **Morphs**, **Glosses**, words of **Translations**, **Lemmas** or part-of-speech **Tags**.
2. Count in the **Whole project** or pick one text.
3. Sort by count, alphabetically, or by how many texts a word is in, and type in **Filter the list** to find one.

**DP** (when more than one text is counted) says how evenly each item is spread over the texts: 0 means it turns up in step with each text's size, 1 that it is all in one text (Gries's deviation of proportions, normalised). Hover over a number to read it in words. **Most evenly spread first** sorts by it, so the words used everywhere, the core vocabulary, come before ones that a single story or speaker uses a lot. Copy table includes the column.

**By ending (reverse list)** sorts words from their last letter backwards, as a reverse dictionary does, and lines them up on the right. Words that share a suffix sit together (every word ending in `-lar`, then `-ler`), so you can spot inflectional endings, rhymes, and spellings of one ending that differ. A letter keeps its accents and other marks.

**Per 1000** is how often it occurs in every thousand. Capital letters count together (Kesi and kesi are one word, shown as the spelling used most); diacritics do not (ŋa and na are two). A morph or gloss counts once per word that has it, so a word with PL twice counts once. Click a row to see its occurrences in the Concordance, looked up exactly (a `?` in a word is a letter there, not a wildcard) and in the chosen text: the number of lines matches the count. The list counts again when the project syncs or texts change; after glossing or typing, click **Recount**. Up to 5,000 items are listed and the first 500 drawn; filter to find the rest. The **#** column is each item's frequency rank, whatever the order shown. **Copy table** copies the list to paste into a spreadsheet.

For **Words** and **Translations**, the line above the list also gives the vocabulary's richness: the type/token ratio (different words over running words), Guiraud's index (different words over the square root of running words, which changes less with the length of the text, so texts of different lengths compare better), and how many words are met only once (hapax legomena).

**Zipf plot**, under that line, opens a chart of each item's count against its rank on log scales, with the straight line Zipf's law predicts. The panel gives the line's exponent: close to 1 in most texts of most languages (the second commonest word half as frequent as the first, the third a third), steeper when a few items take most of the uses, flatter when uses spread over many. Hover over a dot to see its item. With more than 5,000 different items, only the commonest 5,000 are plotted.

### Find the words your dictionary still lacks

A list of **Words** has an **In lexicon** column. **Yes** means a lexicon entry, or one of its other forms (an allomorph or a paradigm form), is spelled that way. **Glossed only** means the word is broken into morphs in a text but has no entry spelled that way, which is usual for inflected words. **No** means neither. The line above the list says how many words the lexicon lacks and how often they are used.

Tick **Only words not in the lexicon** to list just those, most frequent first. Click **Add** beside a word to make it an entry, then give it a gloss in the Lexicon panel. For glossed words, the Checks panel's **Not in the lexicon** adds their roots instead. **Copy table** includes the column.

A lemma list counts every form of a word together (go, goes, went), and a tag list how many nouns, verbs and so on a text has. Lemmas and tags come from CoNLL-U and TEI files and from **Tag and parse this text** in the Sentence panel; for words you glossed, the lemma is the root's lexicon entry and the tag its category. The line above the list says how many words have none yet. Click a lemma or tag to see its lines as a CQP query in the Concordance, such as `[lemma="go"]`.

### Find the words that set a text apart

The **Keywords** panel (a tab beside Search in Scrolls; in Tongues, open it with **Find an action**) compares one text, or the texts with one detail, with every other text in the project: the words used there more often than chance would have it (**Used more**), and those used less. Each row gives the counts on both sides, the log-likelihood (with • when it is significant after allowing for the number of words tested), the log ratio, and per 1000 words; hover over a row for %DIFF and Sketch Engine's simple maths. Choose **Words** or **Translations**, a minimum count, and the order. **2 words in a row** (up to 5) compares sequences of words inside a segment instead, the key clusters of a text, such as a formula a genre repeats. Click a row to read it in the Concordance; **Copy table** copies the list.

Under the list, **Texts most like this one** ranks the other texts by how alike their habits are in the project's 200 most frequent words, the measure stylometry uses to tell authors, speakers, genres and dialects apart (Cosine Delta). Each text gets a number from −1 to 1 with a bar: near 1 it leans on the same common words as the one picked, near 0 it is unrelated, below 0 it does the opposite. It needs three texts or more, and texts of a few hundred words give unsteady numbers, so the note says how many are that short. **Copy table** beside it copies every text against every other, ready for a cluster analysis in R or a spreadsheet.

## Hear any segment from anywhere

Every segment has a ▶ beside it: the lines in Transcribe, the top of the Gloss panel, each line of the Concordance, each segment found by Search, and the words a lexicon entry is used in. In the Lexicon, click **used in 3 words** (or however many) under a sense to list those words, then ▶ to hear each one.

▶ plays from the text's own recording. A text learns its recording the first time you pick a recording for it in the Timeline's list (or import or drop one while the text is open), and from then on the Timeline opens the text with it and ▶ works from every panel, even while another text is on the Timeline. A text whose recording was never picked plays only while it is the text on screen and the recording loaded is not another text's; elsewhere ▶ says to pick its recording first. The tutorial project comes with its recording picked.

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

## Ask for help

The wug answers questions about wugs from this guide. It needs an internet connection.

1. Click **Help** at the top right, or press Ctrl+K (⌘K on a Mac) and choose **Ask for help**. The wug jumps into the **Help** panel. With no project open, Help opens in a card instead.
2. Type your question, such as "How do I split a segment?", and press Enter or click **Ask**.
3. The wug answers, and names the parts of this guide the answer came from. Ask a follow-up in the same box. Help remembers the conversation until wugs closes, even if you close the panel.

Your question, your last few questions and answers in Help, and the wugs version are sent, never anything from your project. Help keeps nothing unless you tick **Share my questions with the people who make wugs**; then each question, the part of this guide it matched and the wugs version are kept, so this guide can cover what people ask most. Still having trouble? **Give wugs feedback** under the conversation opens the Feedback form, and the people who make wugs fix it as soon as they can. Each answer names the parts of this guide it came from.

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
- The Elicit panel has no paradigms, no session planning, and no lists of your own. An answer takes its sound only from a segment of a recording on the Timeline; it cannot record one itself.
- Devices appear as "this device" and "another device" in the Conflicts panel.

## How wugs keeps your work

wugs never overwrites your work. Every edit you make, by any person on any device, is written as a new line in a journal in the project folder, the op log. wugs adds to the journal and never changes or removes a line. A deleted segment is marked deleted, and a restore brings it back.

What you see on screen is built from the journal. wugs keeps a cache in the project's `cache` folder so it opens fast, but the cache is disposable. If it is lost or damaged, wugs rebuilds it from the journal. This is why a crash or an error message cannot lose saved work, and why the sealed package can include the project's full history.

The journal also makes sync safe. Each device writes only its own journal file and reads the others. When two people change the same thing, both versions stay in the journal, and the Conflicts panel asks a person to choose. A choice adds a new line to the journal and removes nothing, so an earlier version can always be recovered.
