# Demo script: wugs in about ten minutes

A walk through the fieldwork kit (Tongues) for showing wugs to a linguist. It goes from a raw recording to a glossed text, a lexicon, and files that ELAN, FLEx and an archive accept. Every step runs offline on the laptop.

## Before the meeting

Do these the day before. Two of them download large files.

1. **Install the newest build** (`docs/INSTALL.md`). Open it once, so Windows SmartScreen has been dealt with already.
2. **Download a phone model.** Open Speech assist and click Download beside the wav2vec2 model (about 640 MB). Optionally download ZIPA small too, to show that the model is a choice.
3. **Have Praat installed** if you want to show the pitch and formant overlays and the Praat round trip. The Timeline toolbar shows "Praat…" until wugs finds it.
4. **Make a demo project** in a new folder (Create project). Do one full rehearsal in it, then make a second, empty project for the meeting itself.
5. **Pick one short recording**, 20 to 60 seconds, ideally with clear pauses: counting, a word list, or a few sentences. A phone voice memo (`.m4a`) works. Use only a recording you may show: your own voice or a consented one. Keep a second copy on the desktop as a backup.
6. **Reset layout** (top bar) so every panel is where this script expects it. The People panel is a tab beside Lexicon.

## The walk

### 1. A project and a text (30 s)
- Point at the top bar: the project lives in an ordinary folder. Every edit is an entry in an append-only journal (the op log), so nothing is ever overwritten and the history is the archive.
- In **Texts**, type a title and click **New text**.

### 2. Bring in the recording (30 s)
- On the **Timeline**, click **Import audio…** and pick the voice memo. M4A, MP3, FLAC and WAV all work; compressed files are decoded to WAV, the format archives ask for.
- Or record live: **Media**, then the record button. A crash or a power cut loses at most the last five seconds.
- The waveform appears. Scroll to zoom and drag to pan.

### 3. Segment it automatically (1 min)
- Choose **by utterance** (or **by word** for counting and word lists) and click **Auto-segment**. It finds where speech starts and ends, offline.
- Show that machine segments are ordinary segments. Drag an edge: it snaps to where speech begins (hold Alt to place it freely). Select a segment, press **S** to split at the playhead and **M** to merge with the next.
- Click **Undo**, then **Redo**. An undo never overwrites work done since.

### 4. Transcribe with a machine suggestion (1–2 min)
- In **Speech assist**, click **Fill empty segments**. The model writes IPA into each empty segment, on this laptop. Suggestions are tinted yellow, and red where the model is unsure.
- In **Transcribe**: press Enter to save and play the next segment, Tab to replay, and use the IPA palette. Correct one suggestion. Its tint goes away, because a person has now checked it.
- Talking point: wugs asks for a *job* (phone transcription), not a model. Which model, or Automatic, is a setting. It can be pinned per project so a long campaign is never switched halfway.

### 5. Gloss (2 min)
- Select a segment and open **Gloss**. Click **Break into words**, then type a breakdown such as `bahay-an` and glosses in Leipzig style (`house-LOC`). Grammatical glosses turn to small caps.
- Show **add to lexicon** on an unknown root.
- Gloss a second occurrence of the same word: wugs suggests the earlier analysis (press Enter to accept). Nothing is ever applied without a person accepting it.

### 6. Lexicon and elicitation (1 min)
- **Lexicon**: the entry you just added, with its senses and glosses. Click **Load SIL Semantic Domains** and give the entry a domain.
- **Elicit**: load the Swadesh 100 list and answer two concepts. Each answer becomes a lexicon entry linked to its Concepticon concept, and coverage counts up.

### 7. People and consent (30 s)
- **People**: add the speaker and record what they agreed to. Each term is Yes, No or Not asked, and the signed form can be attached.
- Talking point: wugs records and reports these, and never blocks work with them. Ethics stays with the researcher and the review board.

### 8. Out to other tools and the archive (1 min)
- In **Texts**, under **Import and export**: export the text as **ELAN (.eaf)**, then open it in ELAN if it is installed. Export the lexicon as **LIFT** (FLEx reads it).
- Click **Seal a version for the archive…**. It writes a BagIt package with ELAN, FLEx and LIFT copies, OLAC and IMDI metadata, the recording and the full history, all checksummed and ready for ELAR, PARADISEC or AILLA.
- **Import a file…** brings ELAN, FLEx, Praat and Toolbox files in, so nobody has to start over.

### 9. If there is time
- **Praat**: Pitch, Intensity and Formants toggles draw Praat's own analysis on the timeline. **Open in Praat** hands the selection to Praat, and **Bring back from Praat** brings the edits home.
- **Speech assist, Share corrections**: every checked suggestion becomes a scored test set (and later, training data) for the language.
- **Devices**: two laptops pair with an eight-digit code and sync with no server. Show this only with a second machine.

## If something goes wrong

- **A panel is missing:** use Show panel… in the top bar, or Reset layout.
- **The model is not downloaded:** skip step 4 and type the transcription. Everything else works without it.
- **Auto-segment makes one long segment:** the recording has little silence. Switch to **by word**, or draw segments by dragging on a tier lane.
- **Import audio refuses the file:** use the WAV backup.
- **The app shows an error toast:** it says what failed in words. Nothing already saved is lost, because every edit was written to the journal when it was made.
