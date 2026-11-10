# Licensing

Two tiers, on purpose.

## The code: MPL-2.0

Everything in this repository is licensed under the **Mozilla Public License 2.0** (`LICENSE`, SPDX `MPL-2.0`) unless a file says otherwise.

MPL-2.0 is file-level copyleft. In practice:

- You may use wugs in commercial and proprietary products, and you may combine it with code under other licenses.
- If you modify a wugs file and distribute it, those modifications must be published under MPL-2.0.
- Your own files stay under your own terms. Writing a kit, a plugin, or an integration does not make your code MPL.

Why this and not something else is recorded in `docs/DECISIONS.md` under "License". The short version: improvements to the engine stay available to the communities whose language data it holds, without putting any friction on the third-party kits and institutional integrations the project depends on.

## The archive format: Apache-2.0 or MIT, at your option

The **wugs op-log format specification** and any future reference reader for it are dual-licensed **Apache-2.0 OR MIT** (`LICENSES/Apache-2.0.txt`, `LICENSES/MIT.txt`).

Covered by this carve-out:

- `docs/DATA-MODEL.md` sections 1, 2, 3, 9 and 10 (identifiers and clocks, the op envelope and verbs, the entity catalogue, the blob store, schema evolution)
- the JSON schemas and format spec written into every sealed archive package
- a future minimal reference reader crate, to be published under these terms

The reason is a promise, not a preference. A sealed wugs archive may be the only surviving record of a language, and somebody must be able to write a reader for it in 2056 with no license question to answer. Permissive terms on the format are the license half of "data formats stay open forever."

The engine's own implementation of that format stays MPL-2.0. Only the specification and a reference reader are permissive.

## Bundled third-party data

Some kits ship reference lists made by others, kept as separate files under their own licenses and never mixed into code:

- `kits/tongues/concepts/*.jsonl`: concept lists from Concepticon (Max Planck Institute for Evolutionary Anthropology), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Source and changes: `kits/tongues/concepts/README.md`.
- `kits/tongues/semdom/semdom-v4-en.jsonl`: SIL Semantic Domains v4, © SIL Global, [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Source and changes: `kits/tongues/semdom/README.md`. The desktop app embeds it and shows the attribution wherever it offers the list.

## Contributing

See `CONTRIBUTING.md`. Contributions are accepted under a Developer Certificate of Origin, not a Contributor License Agreement.
