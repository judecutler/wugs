# Concept lists (data, not code)

Word lists for elicitation, taken from **Concepticon** (<https://concepticon.clld.org>), the
reference catalogue that links published concept lists to shared concept sets.

| File | List | Concepts |
| --- | --- | --- |
| `Swadesh-1955-100.jsonl` | Swadesh 100 (Swadesh 1955) | 100 |
| `Swadesh-1952-200.jsonl` | Swadesh 200 (Swadesh 1952) | 200 |
| `Tadmor-2009-100.jsonl` | Leipzig-Jakarta list (Tadmor, Haspelmath & Taylor 2010) | 100 |

- **Source:** `concepticon/concepticon-data`, `concepticondata/conceptlists/<list>.tsv`, commit
  `7b5af5fcb18113ecc3b60129bc68d08594f2596e` (2026-11-06).
- **License:** [Creative Commons Attribution 4.0](https://creativecommons.org/licenses/by/4.0/)
  (CC BY 4.0), published by the Max Planck Institute for Evolutionary Anthropology, Leipzig.
  Cite Concepticon and the original list (the citation is the first line of each file).
  These files keep that license; they are not covered by the wugs code license (MPL-2.0).
- **Regenerate:** `python3 build.py <concepticon-data checkout> .`
- **Changes from the source:** each TSV converted to JSON lines, with a header line (`list`,
  `name`, `citation`) and one line per concept: `id` (the Concepticon row id, permanent),
  `number`, `gloss` (the list's own English wording), `concepticon` and `concepticonGloss`
  (the Concepticon concept set). Other columns (ranks, scores) were dropped.

The desktop app embeds these files. "Load" in the Elicit panel copies a list into the project's
config as `concept-list` and `concept` entities whose ids derive from the row ids, so loading a
list on two devices gives one list. The Concepticon ids make answers joinable with CLDF
wordlists and other datasets later (the Datasets layer, roadmap item 39).
