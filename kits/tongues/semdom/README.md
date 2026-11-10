# SIL Semantic Domains (data, not code)

`semdom-v4-en.jsonl` is the **SIL Semantic Domains** list, version 4, in English:
1,792 domains for collecting and organizing words by meaning, each with its outline
number, permanent GUID, name, description and elicitation questions.

- **Source:** the `sillsdev/semdom` repository (`data/v4/domains/`), commit
  `e66b00aa7710e7a328f8fba84865f58b0e151427` (2026-10-13). Website: <https://semdom.org>.
- **Copyright:** © SIL Global.
- **License:** [Creative Commons Attribution-ShareAlike 4.0](https://creativecommons.org/licenses/by-sa/4.0/)
  (CC BY-SA 4.0). This file keeps that license; it is not covered by the wugs code
  license (MPL-2.0) and is bundled alongside the app as a separate work.
- **Changes from the source:** converted from one YAML file per domain to one JSON object
  per line, in outline order; whitespace inside text collapsed to single spaces. No wording
  was changed. Fields: `code`, `guid`, `name`, `description`, `questions` (`[question, words]` pairs).

The desktop app embeds this file and loads it into a project only when someone asks
("Load SIL Semantic Domains" in the Lexicon panel). Loaded domains carry
`origin = "semdom:v4:<GUID>"`, and their entity ids are derived from the GUID, so loading on
two devices gives one tree. The elicitation questions are kept for the elicitation tracker
(roadmap item 23); they are not copied into projects yet.

To regenerate: check out `sillsdev/semdom` and run
`python3 build.py <checkout> semdom-v4-en.jsonl` (needs PyYAML). The script checks that every
parent comes before its children and that GUIDs are unique.
