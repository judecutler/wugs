# Suite Context: The Big Picture

*A living doc for the overall vision and structure. Each kit's details belong in its own PRD (e.g. `product/PRD-TONGUES.md` for Tongues). Architecture decisions: `product/DECISIONS.md`. Code: github.com/judecutler/wugs.*

## Vision
**wugs** is one platform for every linguist. Users open a project, pick a **kit** (a preset environment of tools) and add or remove tools as they go. Every tool works on the same shared data, so a recording, its transcription, its glosses and the dictionary entries it produces stay one connected body of data.

*The name: a wug is the imaginary bird from Jean Berko Gleason's 1958 wug test ("This is a wug. Now there are two of them. There are two ___.").*

**Model:** open-source and local-first. Everything is open source and no feature is paywalled. Revenue comes only from hosted services that cost money to run (hosted AI compute and training, hosted sync, backup and storage, institutional hosting and support). Their prices may include a margin that funds development and growth. Every service can also be self-hosted for free. Data formats stay open forever. See `product/DECISIONS.md`, "Open source with paid hosted services".

## Terminology
- **Platform:** the shared core: data model, sync, storage, plugin API, Unicode/IPA, import/export, ethics and access control.
- **Tool:** one capability (waveform canvas, lexicon, tokenizer…). Each tool is built **once** and reused in any kit.
- **Kit:** a preset of tools plus layout, templates and defaults for one kind of project. A kit is a recipe, not a container.
- **Layer:** a capability available in every kit regardless of preset (analysis, datasets, publishing).

## Core design rule: tools don't own data, the platform does
| Shared object | Examples |
| --- | --- |
| **Source** | audio, video, manuscript image, scanned page, document |
| **Text** | ordered segments of a source with annotation tiers |
| **Lexical entry** | lemma → senses → forms, linked from every text where it occurs |
| **Writing system** | orthography, sign inventory, keyboard, sort order, normalization |
| **Person** | speaker / consultant / scribe, with consent and access levels |
| **Relation** | cognate sets, correspondences, dialect links, translations |
| **Language** | Glottolog-keyed metadata, so a project's data can join cross-linguistic datasets |

## The three core kits
| Kit | Project type | Core workflow | Also serves |
| --- | --- | --- | --- |
| **Tongues** — Fieldwork | Language documentation | Record → slice → transcribe → gloss → build lexicon | Sign language documentation, conversation analysis, revitalization, phonetics data collection |
| **Scrolls** — Corpus | Corpus linguistics | Ingest → tokenize and tag → IGT → treebank → query | Sociolinguistic corpora, child language (CHAT), corpus lexicography, parallel and translation texts |
| **Glyphs** — Historical | Historical, epigraphic and comparative work | Catalog and image sources → sign/n-gram analysis → test hypotheses → cognates and reconstruction | Philology, **decipherment of unknown scripts and undeciphered manuscripts**, sound change, comparative reconstruction |

**Where decipherment lives:** Glyphs. Working out what an undeciphered script says is its central workflow (facsimile viewer, sign inventory and distribution analysis, n-gram and positional statistics, hypothesis tracking, cognate alignment against candidate relatives). This is distinct from the shared **writing systems** tool, which defines how a script is *encoded* (orthography, keyboards, sort order) and which Tongues needs just as much for designing orthographies for unwritten languages.

## Layers (available in every kit)
- **Analysis:** statistics, charts, maps and a notebook (Python/R) that can read any shared object. Every project type uses statistics, so statistics is not a kit of its own.
- **Datasets:** connectors to Glottolog, WALS, Grambank and other CLDF datasets, available in every kit, not only to typologists.
- **Publishing:** dictionaries, IGT exports, archive deposit (ELAR, PARADISEC, AILLA), community apps.

**Typology** remains a real project type: the data comes from many languages in outside datasets, not from your own recordings or texts. It ships as an **official starter kit** made with the custom-kit system (Analysis + Datasets + Maps). It needs no special engine code, and building it proves the custom-kit system works.

## Shared tools (built once, used in many kits)
| Tool | Tongues | Scrolls | Glyphs |
| --- | :-: | :-: | :-: |
| Lexicon | ● | ● | ● |
| Interlinear glossing (IGT) | ● | ● | ● |
| Media / annotation timeline | ● | ○ (spoken corpora) | |
| Image / facsimile viewer | | ○ | ● |
| Search and concordance | ● | ● | ● |
| Writing systems (orthography, signs, keyboards) | ● | ○ | ● |

Tools specific to one kit: waveform canvas and elicitation (Tongues) · tokenizer, tagger and treebank (Scrolls) · sign analysis, decipherment tools and cognate aligner (Glyphs).

## Offline vs. online
Every feature is labelled **Offline** or **Online**.
- **Offline (the core):** capture, annotation, glossing, lexicon, search, local and peer-to-peer sync, basic statistics and charts. Must run smoothly on a low-end laptop. Keep it lean.
- **Online:** AI copilot, cloud compute, dataset connectors, hosted sync, browsing the kit library, and the extra "fun" features. **Cloud AI is off by default and opt-in per project: no language data reaches a cloud model without an explicit project-level decision.**
- **Nuance:** some add-ons need a download but not a connection, e.g. a local phone recognizer for the field. Treat these as optional offline extensions. They are installed while online and work offline afterward, but are never bundled into the core.

## Custom kits
Linguists can build, save and share their own kits.
- **Kit = a declarative manifest** (`kit.toml`): tools and versions, layout, templates (tier schemas, gloss abbreviations, field sets), default settings, optional scripts.
- **Two equal ways to edit it:**
  - **Kit Builder UI:** pick tools, arrange the workspace, set templates. No code needed.
  - **"View code":** opens the same manifest and scripts directly for advanced users. Both ways edit the same file, so switching between them loses nothing.
- **Sharing:** a kit is a GitHub repo. The in-app **Kit Library** (online) is an index of those repos, with search, ratings and an "official" badge. Installed kits work offline.
- **Safety:** a manifest with no scripts is safe to install. Scripts and plugins run sandboxed (WASM), with a permission prompt listing what they can access (files, network, AI). Protecting community data is non-negotiable.

## Target users
Field linguists · community language workers · corpus and computational linguists · philologists and historical linguists · typologists · students and teachers.

## Success signals (early)
- Real FLEx and ELAN projects imported and kept in use
- One archive deposit made end-to-end from the suite
- Community members contributing, not just researchers
- Custom kits and plugins published by people outside the team

## Out of scope (for now)
Psycholinguistic and neurolinguistic experiment running, clinical and forensic linguistics. Users can still bring results into the Analysis layer.

## Open decisions
- Legal or organizational form behind the paid services (for-profit company, public-benefit corporation, nonprofit or fiscal sponsor)
- Build order. Current: platform core + Tongues (offline core first), then Scrolls, then Glyphs; Typology starter kit once custom kits work
