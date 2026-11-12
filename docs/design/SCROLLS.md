# Scrolls: the corpus kit

*Status: proposed design, roadmap row 41 (and 73 for switching kits). Owner, 2026-11-12: Scrolls comes before Glyphs. Focus on the tools that cut corpus work time. Make it a whole kit, easy to switch to and from Tongues. Corpus tooling is a crowded space, which is why the ACL demo paper leaves it out, so "use that": connect to the tools and formats corpus linguists already use instead of rebuilding them.*

## 1. Who it is for, and what it saves them

The user is a linguist with texts rather than recordings: a thesis corpus of interviews already transcribed, a set of novels, newspaper text, a historical collection, a treebank to correct. Their day today looks like this:

1. clean files by hand;
2. run a tagger in a terminal (UDPipe, Stanza, spaCy);
3. load the result into a concordancer (AntConc, CQPweb, Sketch Engine);
4. copy numbers into a spreadsheet;
5. fix tagging errors in yet another tool;
6. repeat each step after every correction.

Each hand-off is a format conversion and a chance to lose work.

Scrolls keeps all of these steps in one project, over the same op log as Tongues. Correcting a tag updates every concordance and count at once, and a fieldwork corpus from Tongues is a Scrolls corpus with no export.

**The time savers, in order:**

1. **Drop files in, get a searchable corpus.** Plain text, CSV/TSV, CoNLL-U, TEI and the Tongues texts of the same project are read in one step. Sentence and token splitting run on import. Metadata per document (author, date, genre, speaker) comes from a CSV beside the files or from TEI headers.
2. **Tag and lemmatize without a terminal.** A `morphosyntax` capability. Its providers are the existing taggers (§3), picked per language like any capability. Results are stored as ordinary ops, marked as machine output until a person checks them, exactly as speech assist marks transcriptions.
3. **A concordance with queries people already know.** The Tongues concordance (row 71) grows CQP-style queries: `[lemma="go"] [pos="ADP"]`, `[word="ni.*"]`, distances, and metadata filters (`::match.text_genre="news"`). Sort by left or right context, as now.
4. **Counts that stay correct.** Frequency lists (word, lemma, tag, n-grams), collocations (MI, log-Dice, t-score) and keyness against a reference corpus (log-likelihood, %DIFF). All are computed from the live projection, so they change when a tag is corrected.
5. **Correct where you look.** A CoNLL-U view of one sentence (a table and a dependency arc diagram), editable in place. A correction made in the concordance (re-tag this hit, or all hits like it) is one intent, undoable.
6. **Hand off to anything.** Export to CoNLL-U, TEI, CSV, CWB vertical format (what CQPweb and Sketch Engine import), and AntConc's plain-text and tagged formats. Sealing for an archive works as in Tongues.

## 2. Data model

Nothing new at the core. A corpus document is a `text`. Its sentences are `segment`s on a sentence tier, untimed for written text. Tokens are the existing `token` entities, with new fields:

- `lemma`
- `upos`, `xpos`
- `feats` (the UD features string)
- `head` (a ref to a token)
- `deprel`
- `misc`

A field written by a machine carries an `x.tag:<op id>` record (provider, confidence), as `x.asr:` does for speech assist. Multi-word tokens (CoNLL-U `1-2` ranges) are a `token` whose members are its syntactic words, using the membership lists the glossing model already has (`GLOSSING-MODEL.md`).

Document metadata is fields on the `text` (`meta:<key>`), so a filter is a field query. A corpus can be large: 10 million tokens is a normal thesis corpus. The EAV projection is not made for counting at that size, so this is where DATA-MODEL §8's typed tables earn their place. A `tokens` table (text, segment, position, form key, lemma key, upos, …) is maintained by the projector, and the concordance, frequency and collocation queries run on it. This is the "real tool query" CLAUDE.md names as the trigger for typed tables.

## 3. Taggers: existing tools behind one capability

The `morphosyntax` capability takes sentences of tokens and returns lemma, UPOS, XPOS, features, head and relation, each optional. Providers, all through the bundle's integration manifests (`BUNDLES.md` §5):

| Provider | How it runs | Licence of the models | Notes |
| --- | --- | --- | --- |
| Built-in tokenizer | in the app | (no model) | Always ready: Unicode word and sentence boundaries plus the Gloss panel's edge-punctuation rule. It does no tagging. |
| UDPipe REST service (LINDAT) | Online, HTTPS | CC BY-NC-SA (UD 2.15: 147 treebanks, 78 languages) | No install. Free for non-commercial use, which the install screen says in words. Our hosted services never use it commercially. |
| UDPipe 1 program | external program, like Praat | CC BY-NC-SA | Offline. A single binary per OS, small models. Found like Praat or downloaded on request. |
| Stanza (Python) | external program, if installed | per model (Apache-2.0 code; models on Hugging Face) | For people who already have Python. Never required. |
| spaCy (Python) | external program, if installed | per model | Same as Stanza. |
| ONNX taggers | `tract` in the app, like the phone models | per model | Later. They need a model family we can export and test the way wav2vec2 and ZIPA were. |

"Automatic" picks the best published score for the language, as for phone transcription. The UD treebank scores published by UDPipe and Stanza are the evaluation. Pins per project work as today.

## 4. Switching kits (row 73)

A kit switcher in the top bar: the kit's wug and name become a button, with a menu listing the installed kits and each one's subtitle.

- Switching keeps the project open and changes only which panels are offered and how they are arranged. Each kit keeps its own arrangement (`layouts/<kit id>.json`, as now).
- The active kit stays per machine, as today.
- A project can name the kit it was made with, so opening it offers "This project was made in Scrolls. Switch?" once.
- The command palette lists "Switch to Scrolls".

Panels that both kits use (Search, Concordance, Texts, Conflicts, Devices, Kits) are shared. Scrolls adds Corpus (documents and metadata), Frequencies, Collocations, Keywords and Sentence (the CoNLL-U view and editor).

## 5. Order of work

1. **Kit switcher (row 73)** and a minimal Scrolls `kit.toml` (bundle form, `BUNDLES.md`) offering Texts, Search, Concordance and a first Corpus panel. Scrolls then exists and can be switched to.
2. **Corpus import:** plain text and CSV with sentence and token splitting, and CoNLL-U with every column kept. Each is one txn per document, reported in words.
3. **Frequency list panel** (word, lemma, tag; per document, or the whole corpus, filtered by metadata), with copy and export.
4. **`morphosyntax` capability** with the built-in tokenizer and the UDPipe REST provider; machine marks; then the UDPipe 1 program provider.
5. **CQP-style queries** in the concordance, and typed `tokens` tables in the projection.
6. **Collocations and keywords;** the Sentence (CoNLL-U) editor; exports to CoNLL-U, TEI, CWB vertical and AntConc.
