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
3. **A concordance with queries people already know.** The Tongues concordance (row 71) grows CQP-style queries: `[lemma="go"] [upos="ADP"]`, `[word="ni.*"]`, distances, and metadata filters (`:: match.text_genre="news"`). Sort by left or right context, as now. Rather than writing a CQP engine of our own (the rebuilding the owner asked to avoid), the first version translates the common subset into queries on the typed tables. A project can also be exported to CWB and searched with the real `cqp` where it is installed.
4. **Counts that stay correct.** Frequency lists (word, lemma, tag, n-grams), collocations (MI, log-Dice, t-score) and keyness against a reference corpus (log-likelihood, %DIFF, log ratio), where the reference corpus is another project or a frequency-list dataset package. All are computed from the live projection, so they change when a tag is corrected.
5. **Correct where you look.** A CoNLL-U view of one sentence (a table and a dependency arc diagram), editable in place. A correction made in the concordance (re-tag this hit, or all hits like it) is one intent, undoable.
6. **Hand off to anything.** Export to CoNLL-U, TEI, CSV, CWB vertical format (what CQPweb and Sketch Engine import), and plain text for AntConc (one file per document, optionally with `word_TAG` tags, which AntConc reads). Sealing for an archive works as in Tongues.

## 2. Data model

A corpus document is a `text`, and its sentences are `segment`s on a sentence tier (untimed for written text). Syntactic words are the existing `token` entities. Nothing new is added at the core. What matters is how much gets written, because a 10-million-token corpus is the sizing target.

**Scale first.** Writing each annotation as its own field would cost about a dozen ops per token: lemma, UPOS, XPOS, features, head and relation, plus a machine record for each. That is over 100 million ops for 10 million tokens. The repo's own gate is 500k ops replayed in 45 s with a 643 MB projection, so replay would take hours, the projection would reach ~150 GB, and sync would move tens of GB. So:

- **One `analysis` field per sentence** (on the segment) holds the sentence's annotation as one value. It is a JSON array of rows in CoNLL-U column order, keyed by token id: lemma, UPOS, XPOS, FEATS, HEAD, DEPREL, DEPS, MISC. A re-tag, a re-parse or a person's fix of one sentence is therefore one op.
  - A tree is never half one device's parse and half another's. Concurrent edits to one sentence's analysis are a conflict on one field, shown in the Conflicts panel like any other.
- **One machine record per sentence write** (`x.tag:<op id>`: provider, model, confidence per row), not one per field.
- Tokens keep only `form` (and `kind`, below). A sentence's tokens are written once at import, in one txn per document.
- `wugs-bench` gets a corpus gate before import is built: 10M tokens imported and tagged, replay time, projection size and log size, each with a limit.

**CoNLL-U, all of it.**
- **Word lines:** a segment lists its syntactic words only, so nothing is counted twice.
- **Multi-word tokens** (`1-2`) are kept as a range on the segment (`mwt`: surface form plus first and last word). HEAD and IDs always point at syntactic words, as in CoNLL-U.
- **Empty nodes** (`8.1`) are rows in `analysis` with a decimal id and no token.
- **Sentence comments** (`# sent_id`, `# text`, any other) are kept in `comments` on the segment.

Import followed by export therefore gives back the same file, a fidelity test like the ELAN and FLEx ones.

**Trees stay well formed.** A validator (warn-only, like every validator) flags:
- a sentence whose heads form a cycle;
- more than one root;
- a head pointing at a token that is no longer in the sentence.

A retokenize goes through the existing diff-based retokenize, so token ids survive and heads keep pointing at the same words.

**Checked, not just unedited.** Speech assist leaves a machine transcription marked until a person changes it. Taggers are right about 97% of the time, so most rows will never be changed. Scrolls therefore adds a **Mark checked** intent (one sentence, or every hit in a concordance) that clears the mark without changing the analysis.

**Metadata** for a document is fields on its `text` (`meta:<key>`), so a filter is a field query.

**Typed tables come first.** The EAV projection is not made for counting 10M tokens. A typed `tokens` table maintained by the projector serves the concordance, frequency and collocation queries: text, segment, position, form key, plus lemma and UPOS keys unpacked from `analysis`. This is the "real tool query" CLAUDE.md names as the trigger for DATA-MODEL §8's typed tables, and it is built with import (step 2 below), before any counting.

## 3. Taggers: existing tools behind one capability

The `morphosyntax` capability takes a sentence of tokens and returns its `analysis`. **It never retokenizes:** providers get the sentence pretokenized (UDPipe, Stanza and spaCy all accept that), so the corpus's tokenization, and every id that hangs off it, is the person's. Multi-word tokens a tagger would split are kept as the `mwt` ranges import found. The providers are listed below. Each is reached through a **shell-known adapter** (`BUNDLES.md` §5).

| Provider | How it runs | Models and licence | Notes |
| --- | --- | --- | --- |
| Built-in tokenizer | in the app | (no model) | Always ready: Unicode word and sentence boundaries plus the Gloss panel's edge-punctuation rule. It does no tagging. |
| UDPipe 2 through the LINDAT REST service | Online, HTTPS | UD 2.15: 147 treebanks, 78 languages; CC BY-NC-SA 4.0, non-commercial | No install. Sends text to LINDAT, so it is **off until the project opts in** (below). The service's own terms page names the models' licence and nothing else, so no rate rules are assumed. |
| UDPipe 1 program | external program, like Praat | UD 2.5 models (fewer languages, older treebanks); CC BY-NC-SA 4.0 | Offline. ÚFAL publishes UDPipe 1 binaries for Linux, Windows and macOS. The person picks the program, or wugs downloads ÚFAL's release pinned by hash. |
| Stanza (Python) | external program, if installed | Apache-2.0 code; each model's licence follows its treebank | For people who already have Python. Never required. |
| spaCy (Python) | external program, if installed | per model | Its English models use ClearNLP dependency labels, not UD, so the adapter maps them to UD and says so. |
| ONNX taggers | `tract` in the app, like the phone models | per model | Later. They need a model family we can export and test the way wav2vec2 and ZIPA were. |

- **Scores are per provider and model.** "Automatic" compares the published score of each model on the same treebank's test set: UDPipe 1's UD 2.5 numbers against UDPipe 1, UDPipe 2's UD 2.15 numbers against UDPipe 2. It never compares across UD versions as if they were one table. Pins per project work as today.
- **Text going online is the project's decision.** The online provider is off until the project opts in once ("Send this project's texts to LINDAT for tagging"). A text marked restricted is named in a notice before it is sent. The notice informs and never blocks (DECISIONS 2026-11-11).
- **Non-commercial models stay off the paid hosted services.** Hosting is commercial use. The paid hosted tagging that the business model allows (CLAUDE.md, "Open source with paid hosted services") would therefore use only models whose licences permit it.

## 4. Switching kits (row 73)

A kit switcher in the top bar: the kit's wug and name become a button, with a menu listing the installed kits and each one's subtitle.

- Switching keeps the project open and changes only which panels are offered and how they are arranged. Each kit keeps its own arrangement (`layouts/<kit id>.json`, as now).
- The active kit stays per machine, as today.
- A project can name the kit it was made with, so opening it offers "This project was made in Scrolls. Switch?" once.
- The command palette lists "Switch to Scrolls".

Panels that both kits use (Search, Concordance, Texts, Conflicts, Devices, Kits) are shared. Scrolls adds Corpus (documents and metadata), Frequencies, Collocations, Keywords and Sentence (the CoNLL-U view and editor).

## 5. Order of work

1. **Kit switcher (row 73)** and a built-in Scrolls kit offering Texts, Concordance, Search, Gloss, Lexicon and Transcribe, so Scrolls exists and can be switched to.
2. *First form built (2026-11-12, roadmap 41): plain `.txt` in a txn per document, segments untokenized, no typed table yet; the Concordance and Frequencies split words on read, which serves fieldwork-scale corpora. The typed table and the `wugs-bench` corpus gate come before CSV, CoNLL-U and TEI, the formats that bring corpora large enough to need them.* *Built 2026-11-12 (`feat/corpus-index`): `corpus_segments` and `corpus_words` (form and folded key per word, lemma and UPOS columns to come with the `analysis` field), rebuilt per dirty segment before each read; the corpus gate at 10M tokens: index 51 s, whole-corpus frequency list 7.5 s, 62 MB peak, +0.5 GB on a 1.1 GB projection. Replay of 10M tokens is 73 s, 2 s over what it was without the index and over the 60 s gate, so replay cost per sentence is the next scale problem.* **Corpus import with the typed `tokens` table:** plain text and CSV with sentence and token splitting, and CoNLL-U with every column, multi-word ranges, empty nodes and comments kept (round-trip fidelity test). One txn per document; the `wugs-bench` corpus gate (§2) set up before import ships.
3. **Frequency list panel** (word, lemma, tag; per document, or the whole corpus, filtered by metadata), with copy and export, on the typed table.
4. **`morphosyntax` capability:** the built-in tokenizer, then UDPipe 1 (offline), then the opt-in UDPipe 2 REST provider; a sentence's analysis in one field; machine marks; Mark checked; the tree validator.
5. **CQP-subset queries** in the concordance; export to CWB for real `cqp`.
6. **Collocations and keywords;** *(collocations and keywords started early, 2026-11-12. Keywords compare one text with the project's other texts until reference corpora come as dataset packages, with 4-term log-likelihood, log ratio and simple maths. Collocations: over words and translation words on the EAV readers like the frequency list: log-likelihood, MI, logDice and t-score, windows within a segment, overlapping windows counted once; lemma and tag collocations wait for step 4)* the Sentence (CoNLL-U) editor; exports to CoNLL-U, TEI, CWB vertical and AntConc text.

## 6. Gathering a corpus from the web (row 75)

Owner, 2026-11-12: people building corpora of low-resource languages (Yakut, say) spend much of their time collecting text from the web. In wugs they should set it up once and leave it to run.

**Who it is for.** A linguist with a language code, a few sites they know publish in the language (a newspaper, a ministry, a blog platform, Wikipedia) and no time to write a crawler. What they get back is ordinary texts in the project, each with where it came from, ready for the Concordance, Frequencies and the taggers.

**Start from what already exists.** For many languages a cleaned web corpus has already been built, and importing it takes minutes instead of weeks. The first screen asks for the language and lists what is ready for it, each with size, source and licence:
- the language's Wikipedia (a dump, CC BY-SA);
- the Leipzig Corpora Collection (news, web and Wikipedia sentences; Yakut is there);
- the Common Crawl corpora filtered by language ID: GlotCC (built for minority languages), MADLAD-400 (ODC-By, 419 languages) and OSCAR;
- Common Voice's sentence collection (CC0).

These come in as dataset packages (`docs/design/PACKAGES.md`), so the list is an index entry per source, not code.

**Then crawl what is not there.** A crawl job has:
- *seeds*: site addresses, sitemaps or RSS feeds, and optionally seed words (BootCaT's method: combinations of typical words sent to a search engine; a search API is a provider the person connects, never built in);
- *rules*: stay on the seed sites or go one link out, at most N pages, at most one request per site every few seconds;
- *a target*: the language (ISO 639-3 plus script) and how sure the language ID must be.

The job runs in the background and survives a restart. It resumes from its frontier, and the status card shows pages fetched, pages kept, words kept, and how much was dropped and why (wrong language, duplicate, too short, robots.txt).

**Each step is a capability with providers**, so nothing hard-codes one tool:
- `web-text-extraction`: the page's main text without menus and boilerplate. A built-in Rust readability-style extractor ships; trafilatura (Apache-2.0, the best-scoring open extractor in published benchmarks) is the provider when Python is installed.
- `language-id`: per page and per paragraph, so a Russian paragraph inside a Yakut page is dropped. The provider is GlotLID (fastText, Apache-2.0, over 2000 labels with script, low-resource languages its focus), checked against the expected script; another model can replace it.
- Deduplication is built in: exact by hash, near-duplicates by MinHash across the whole corpus, and repeated boilerplate lines across a site.

**Each kept page becomes a text**, written by `import_written_text` in one txn: segments are its paragraphs, then sentences as plain text import cuts them. The document metadata is the URL, site, title, date published if found, date fetched, the language-ID score, the extractor and the licence if the page declares one (a Creative Commons link or meta tag), else "unknown". The HTML is not kept; the URL and date are enough to fetch it again.

**Being a good citizen and honest about rights:**
- robots.txt is always obeyed.
- The crawler sends a User-Agent that names wugs and links to a page explaining it.
- It keeps per-site delays.
- Texts gathered from the web get the project's restricted sharing level by default when their licence is unknown, so a seal or export says so (sharing levels inform, never block; DECISIONS).
- The corpus stays on the person's computer unless they share the project.

**Order:**
1. Ready-made sources as dataset packages, with Wikipedia and Leipzig first.
2. A site crawl from seeds, with the built-in extractor, GlotLID and deduplication.
3. Seed words through a connected search provider.
4. A scheduled refresh that fetches only new pages from feeds and sitemaps.

