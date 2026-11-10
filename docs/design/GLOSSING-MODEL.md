# Glossing data model: comparison and decision

*2026-11-09. Status: Decided (reversible; owner may override). Feeds roadmap item 21 and DATA-MODEL §3. Mirrored from the claude.ai Project doc `product/design/GLOSSING-MODEL.md`.*

## The question
Item 21 (interlinear glossing linked to the lexicon) needs three model choices settled before UI is built on them:
1. **Where a word's analysis lives**: on each occurrence in a text, or on a shared per-spelling object that occurrences point to.
2. **How a tier says what it is for** (transcription, translation, …), since `TierDef.kind` is ELAN's structural stereotype, not a role.
3. **How glosses handle several analysis languages** (English plus a national language is the field norm).

## How the established tools do it
| Tool / format | Where analyses live | How reuse/speed works | Tier/line roles | Multi-language glosses |
| --- | --- | --- | --- | --- |
| **FLEx** (FieldWorks) | Shared per-wordform inventory: a wordform has analyses (morph bundles linking allomorph + sense + category) and word glosses; each text occurrence points to one | Suggests "based on a previous user analysis for an identically spelled word"; parser guesses; user/parser approval status | Fixed set of interlinear lines (word, morphemes, lex entries, lex gloss, word gloss, word cat, free/literal translation, note) | Yes, per writing system on every gloss |
| **Toolbox** | Per occurrence in the text record (`\tx \mb \ge \ps \ft`) | Interlinearize (Alt-I) parses words against the lexicon (`\lx`/`\ge`); "later Toolbox will remember the morpheme and parse it correctly" | Fixed MDF/interlinear markers | Separate markers per language (`\ge`, `\gn`, `\gr`) |
| **ELAN** | Per occurrence, as annotations on dependent tiers (word tier = symbolic subdivision of utterance; morph tier = subdivision of word; gloss/translation = symbolic association) | None built in (FLEx round-trip, or external tools) | Free tier names; a **linguistic type** holds the stereotype, optional controlled vocabulary and optional ISO data category reference | One tier per language |
| **Xigt** | Per occurrence; flat tiers of items linked by IDs and **alignment expressions** (substring spans, floating alignment for portmanteau morphemes) | n/a (storage format) | Open `tier type` strings (words, morphemes, glosses, …) | One gloss tier per language |
| **Ligt** (RDF) | Per occurrence; WordTier / MorphTier of Items linked with `next` | n/a | Tier subclasses | Language-tagged literals |
| **CLDF ExampleTable** | Per example; aligned lists `Analyzed_Word`, `Gloss` (Leipzig) | n/a | Fixed columns | One gloss line (others as extra columns) |

Sources: FLEx suggestion behaviour and approval status (flex-list threads), the one-choice-per-wordform limit (flex-list, phonetic line thread), Toolbox marker set and Alt-I parsing (Erlewine field-methods handout; Griffiths, *Introduction to Toolbox*), ELAN stereotypes, linguistic types, controlled vocabularies and ISO data categories (ELAN manual §2, §5.3.2), Xigt alignment expressions (xigt/xigt README; Goodman et al. 2015, *LRE*), Ligt model (Chiarcos & Ionov, LDK 2019; Ionov, LDK 2025), CLDF/pyigt (cldf/pyigt docs).

## Decision 1: analyses live on each occurrence; reuse is derived, never shared
**Storage stays as DATA-MODEL already has it**: Segment → Token (form) → Morph (surface, type, form ref, sense ref, gloss). This is what Toolbox, ELAN, Xigt, Ligt, CLDF and FLEx's own `.flextext` export all carry, so import/export and sealed archives stay self-contained per text.

**FLEx-level speed comes from a derived suggestion index**, not a shared mutable object: when the user glosses a word, the projection is queried for every prior analysis of the same form (NFC, case-folded) across the project, ranked by how often it was used, plus Toolbox-style lexicon lookup of each morph surface against lexeme lemmas and allomorph Forms. Accepting a suggestion writes ordinary per-occurrence ops under an `apply-analysis` intent (so provenance shows it came from a suggestion). An explicit **"apply to the N other unanalysed occurrences"** batch action gives FLEx's bulk approval without hidden coupling.

Why not FLEx's shared wordform analyses:
- **Sync safety.** wugs merges per field across devices. A shared analysis edited on one laptop would silently re-gloss every occurrence on every synced machine; a per-occurrence edit changes exactly what the linguist touched, and conflicts surface on the word that conflicted.
- **Context.** Users report FLEx's "one choice per wordform" pain when sandhi or tone changes a form in context. Per-occurrence analyses carry context for free.
- **Archive and interchange.** Every format above is per-occurrence; a shared inventory must be flattened on export anyway.
- **Everything is human-approved by construction.** Suggestions are never stored until accepted, so no "guess vs approved" status field is needed. Machine output (the recognizer, item 27) is marked by its intent/provenance, as the UI design already specifies.

Cost accepted: changing your mind about a word's analysis everywhere is an explicit batch action rather than one edit. That is the same trade every per-occurrence tool makes, and it is the safer default for synced data.

## Decision 2: `TierDef.role`, an open string with a recommended vocabulary
ELAN keeps structure (stereotype) and meaning (free tier name, optional data-category link) separate; FLEx hard-codes a line set; Xigt uses open type strings. wugs follows ELAN + Xigt: `kind` stays the ELAN stereotype; new optional **`role str?`** says what the tier is for. Recommended values, aligned with FLEx line names so `.flextext` round-trips map cleanly: `transcription`, `translation`, `literal-translation`, `note`. Kits may add their own (a phonetics kit might add `phonetic`). Core never interprets the vocabulary.

Also new: optional **`language ref?`** on TierDef, so a project can hold translation tiers in English and in a national language side by side (ELAN's one-tier-per-language pattern).

## Decision 3: per-language glosses as separate fields
`Morph.gloss` and `Sense.gloss` stay the **primary** analysis language. Additional languages are stored as sibling fields **`gloss:<bcp47>`** (e.g. `gloss:es`), one field per language. Per-field last-writer-wins then merges each language independently, which matches FLEx/LIFT's per-writing-system glosses without a JSON blob that two devices would overwrite wholesale.

## Smaller rules recorded
- Leipzig conventions: glosses are free strings; `.` joins several meanings of one morph (portmanteau), all-caps tokens are grammatical categories (rendered in small caps). A project-level abbreviation list (CLDF-style `gloss_abbrs`) is a later item.
- Zero morphs use surface `∅`; validator #1 (surfaces concatenate to the token form) ignores them.
- Non-concatenative morphology: morph types include infix (`<…>`), reduplication (`~`), clitic (`=`). Process glossing (GGG-style apophony) is out of scope for v1.
- Retokenizing a changed transcription keeps the analyses of unchanged words (sequence match by folded form), so a typo fix or a sentence-initial capital never discards glossing work.

## Merge semantics
*Added 2026-11-09 after a two-device review. The first version replaced a token's morphs (and a segment's tokens) wholesale on every re-analysis, by tombstoning the old entities and creating copies. A gloss made concurrently on another device then landed on a tombstoned morph and disappeared from both screens. Two concurrent analyses of one word interleaved their morphs, and two "Break into words" doubled every token.*

- **Ordered membership is one atomic field.** `Segment.tokens` and `Token.morphs` are JSON arrays of ids, written with `set`, `base` and `heads` like any field. Two devices that write different lists without seeing each other's produce the projection's ordinary conflict row (the "choose version" prompt, roadmap item 24), and per-field LWW picks the same winner on every device. A segment or token shows exactly the live entities its winning list names, in list order. Data without a list (imports, older projects) shows its live children by `pos`. The rule lives in one place, the projection's `members` view, and every reader of tokens or morphs goes through it: suggestions, validators #1 and #2, the Gloss view and FlexText export.
- **Member fields still merge per field.** A morph's `gloss`, `gloss:<lang>`, `sense` and `surface` are its own fields, so a gloss on one device and a re-split of another word on a second device both survive.
- **Intents diff, they do not replace.** `retokenize`, `tokenize` and `apply-analysis` match the new sequence to the current one (tokens by longest common subsequence of folded forms; morphs by folded surface, then one for one in equal-length gaps). A matched entity keeps its id, its `form` ref and its `x.*` fields (FlexText identity, PRD gate 3), and only fields that actually changed are `set`. New entities are created. The list is written once, with base = the list the device saw.
- **Hidden, never deleted.** An entity a winning list leaves out is not tombstoned. It stays live in the log and the projection and comes back if a list names it again (`restore-member`). Tombstoning was rejected because a concurrent list that still names the entity would then show a tombstone, and because a tombstone cannot tell "removed on purpose" from "lost the race".
- **The visibility row.** `Projection::hidden_edits(segment)` lists the non-empty edits on hidden tokens and morphs: every `set` or `text-edit`, plus a `gloss`, `gloss:*` or `sense` that came inside the member's `create` (an accepted analysis carries its glosses there, so a whole analysis that lost the race is reported too). An edit is left out only when the device that made it later removed the member itself. The remover is found from the projection's history of list writes: walk back from the current list through each write's `base` to the first write whose base list still named the member. A member no list on that chain ever named (it was only in a losing list) has no remover, so its edits are always reported. The Gloss panel marks each row with a restore control. It errs towards showing: if the removing device had already seen the other device's gloss, the row still appears, because the person who made the gloss was not the one who dropped it.
- **Why this rule for the visibility row.** Two simpler rules were rejected after a probe. "Not made by the device of the current list" loses a row as soon as the glossing device writes any list, such as restoring one sibling. "The first list after the edit in HLC order" charges the wrong device when the removing list carries an earlier HLC than a gloss made without seeing it. "Every edit on a hidden member, with a local dismiss" floods: every re-split of a word you glossed by hand would report your own gloss. The base chain names the list that actually removed the member, needs one small table (list writes only), and is a function of the ops held, so replay and both devices agree.
- **Choosing the other analysis.** When a word's `morphs` (or a segment's `tokens`) has an open conflict, the Gloss panel shows the losing list inline: "Another device analysed this word as «köpek-ler 'dog-PL'»" with "Use that instead" (for words: "Use the other word breakdown"). The action is `choose_member_list`, which writes the losing list naming both lists in its `heads`, so every device settles the conflict, and the analysis set aside stays reported as hidden edits. This is the glossing slice of the conflict UI; the general prompt is roadmap item 24.
- **Why not per-morph generations** (a counter on each morph, bumped by each re-analysis, with the newest generation shown): that makes membership two separate LWW decisions, one per morph, which can disagree under interleaved clocks. Device A's morph 1 can win while device B's morph 2 wins, showing a breakdown neither device wrote. One list field is one LWW decision with one conflict row.
- **Concurrent list writes produce the conflict prompt, whoever made them.** Two devices that write different membership lists for the same word or segment without seeing each other's get one LWW winner and the ordinary conflict row (DATA-MODEL §5.3), and the Gloss panel offers the losing list. Detection follows write heads, not actors, so one linguist on two laptops is prompted just as two linguists are. Hidden edits do not depend on it: they are reported per device (above).

## Not chosen, and why
- **Xigt-style character-span alignment** for morphs: more expressive (substring spans, floating alignment) but brittle under transcription edits merged across devices. Ordered morph entities with surfaces are simpler to merge, and Xigt export can derive spans.
- **Shared wordform inventory** (FLEx): see Decision 1.
- **One JSON field holding all languages' glosses**: two devices editing different languages would overwrite each other.
