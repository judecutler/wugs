# Capabilities and packages: features powered by swappable models, and the wugs marketplace

*Status: decided direction (owner, 2026-11-10); built in stages, see the roadmap (items 53, 51, 37, 54, 36). Mirror of the claude.ai Project doc `product/design/PACKAGES.md`.*

Two ideas, one shape. **Features are stable; what powers them is replaceable.** A linguist wants phones transcribed, pitch drawn or a list of concepts to elicit; which model, program or dataset does it is a choice that changes as the field moves, so wugs names the job, not the tool. **Everything replaceable is a package**, and packages live where their authors already publish (GitHub, Hugging Face), with wugs keeping only a small index.

## 1. Capabilities: features, not models

A **capability** is a job the app knows how to ask for, with a fixed request and answer:

| Capability | Request → answer | Powered today by | Next |
| --- | --- | --- | --- |
| `phone-transcription` | audio span → phones with times and confidence | wav2vec2 espeak CTC (ONNX, tract) | ZIPA (item 51), later whatever leads |
| `acoustic-analysis` | audio span → pitch, intensity, formant tracks | the user's Praat (`praat --run`) | other analysers (e.g. a bundled pitch tracker for machines without Praat) |
| later: `segmentation`, `forced-alignment`, `gloss-suggestion`, `translation`, `orthographic-transcription` | … | — | — |

The feature code (Speech assist's Suggest, the Timeline overlays) calls the capability and never names a model. Machine provenance already records the model per write (`x.asr:<op id>` carries the model id), so changing what powers a capability never makes old output ambiguous.

**A model is a package** with a manifest saying which capability it serves and how to run it:

```toml
[package]
id = "zipa-small"                        # short and path-safe: the install folder, and the model name machine writes record
source = "hf:someone/zipa-small-onnx"   # where it lives (hf: or github:)
type = "model"
version = "1.0.0"
name = "ZIPA small"
license = "CC-BY-4.0"

[model]
capability = "phone-transcription"
runtime = "onnx-ctc-fbank"              # which adapter in wugs can run it
languages = ["*"]                        # or BCP 47 tags

[runtime]                                # the adapter's own settings, read only by that adapter
onnx = "model.int8.onnx"
tokens = "tokens.txt"

[[files]]
path = "model.int8.onnx"
url = "https://huggingface.co/someone/zipa-small-onnx/resolve/<commit>/model.int8.onnx"
sha256 = "…"
bytes = 64000000

[evaluation]                             # what "best" is decided on (section 3)
benchmark = "wugs-phone-bench@1"
per = { "*" = 0.21, "tr" = 0.18 }
```

The format is parsed and checked by the `wugs-package` crate (unknown keys, non-HTTPS URLs, unsafe ids and file names and malformed digests are refused, every problem listed at once). The bundled catalogue is `catalog/*.toml` in the repository until the index exists (item 37). As built in item 53, the shipped wav2vec2 model is `catalog/wav2vec2-espeak.toml`.

A **runtime adapter** is code in wugs that runs a family of models (`onnx-ctc-raw` for wav2vec2-style models, `onnx-ctc-fbank` for Zipformer/ZIPA-style models, `praat-script` for Praat, later `http` for a model served by the user's own machine or a paid hosted service). New models of a known family need no wugs release, only a manifest; a genuinely new architecture needs one adapter, written once. That is the line between "data" (installable any time) and "code" (ships with wugs, or later as a sandboxed plugin).

## 2. Choosing and updating

Each capability has a setting in the app: **Automatic (recommended)** or a model the person picks. Automatic means: the installed model with the best published evaluation for the project's language (falling back to the overall score), among those whose runtime this wugs build has. A project can pin a model so a long annotation campaign is not switched halfway.

As built (item 53): this computer's choice lives in `<app data>/capabilities.json`; a project's pin is a `Setting` in the project config scope (`capability:<id>`, DATA-MODEL §3), so it syncs to every member and two people pinning different models at once get the ordinary conflict card. The pin wins over the computer's choice. A picked or pinned model that is not downloaded here is reported by name ("This project uses X … not downloaded"), never silently swapped for another. Until the evaluation harness scores the catalogue, Automatic ranks unscored models after scored ones instead of skipping them, so the one model that exists today is still chosen; the project language is not modelled yet, so Automatic uses the overall score. Praat is the `acoustic-analysis` provider: not a package but the person's own program, found the way the handoff finds it.

Updates, adjusted for field conditions:

- **The catalogue updates by itself** whenever wugs is online (a few kB of index, section 4). When a better model appears for a capability someone uses, the app says so once: "ZIPA small transcribes Turkish about 15% better: 64 MB, download?"
- **Weights download only with consent** by default, because a 600 MB surprise on a satellite link or a metered phone hotspot in the field is a real harm. A setting allows "download better models automatically on unmetered connections". Old models stay until the person removes them; nothing in progress switches mid-batch.

## 3. Evaluation is part of the design

"Best" needs a number, and the demo-track paper (item 52) needs the same numbers, so they are one piece of work: a benchmark harness (phone error rate per language on public held-out data, plus speed and memory on a modest laptop) that runs over any `phone-transcription` package and writes the `[evaluation]` table. Scores in the index come from the harness run by the index's CI, not from authors' claims. Packages without scores are listed and, once the harness scores the catalogue, never chosen by Automatic while a scored one is ready (until then they rank after scored ones, section 2).

**Human in the loop.** Every time a linguist corrects a machine suggestion, the log already holds both sides: the machine write with its model and confidence (`x.asr:<op id>`) and the person's edit that replaced it. That makes corrections a by-product of ordinary work, not extra labour. With the project's consent, wugs can export those pairs (per language, per model) as a benchmark set or as fine-tuning data, so the models that power a capability get better for the languages people actually document. Nothing leaves the machine unless the project opts in, as with cloud AI. Before corrections ever train a shared model (item 58), they pass through filters per capability: a correction counts only if it fits what that model is being taught (a segmentation model learning word edges should not learn from someone who keeps long pauses inside their segments), and contributions are weighed by whether they improve held-out scores, never by volume. What was used, and why, stays visible to the person. As built (item 57): Speech assist's "Share corrections" writes the checked pairs to a folder the person picks (a Kaldi-style set `wugs-asr-eval` scores directly, with a datasheet); each clip carries what the project's consent records and access levels say, clips they speak against start left out, and the person decides (consent records inform, never enforce: DECISIONS 2026-11-11).

## 4. The marketplace: packages hosted by their authors, indexed by wugs

**Package types:** `kit` (today's `kit.toml`), `model`, `dataset` (reference lists such as Concepticon lists or semantic domains, lexicons, word lists, benchmark sets), `integration` (connectors to other tools and archives, as data-only manifests where possible) and `plugin` (code, only once sandboxed WASM exists, item 36).

**Where things live:** each package is a GitHub repository (kits, datasets, integrations) or a Hugging Face repository (models, large datasets), released at a tag or commit. Downloads come from there; wugs hosts no package files.

**The index** is one public git repository (`wugs-index`) holding a small JSON entry per package version: where it lives, the commit, every file's SHA-256 and size, type, license, and for models the evaluation. Its CI builds a single catalogue file served by GitHub Pages, which clients fetch with caching. This is how Homebrew, Flathub and the crates index work, and it is what makes "fetch from the author's GitHub" safe:

- **Integrity:** the client checks every file against the hash in the index, so a moved tag or a replaced file on the author's side fails closed instead of installing something nobody reviewed.
- **Curation and takedown:** listing is a pull request to the index (generated by wugs, section 5); CI checks the manifest, hashes, licence and size automatically, a person approves a publisher's first package, and later versions from the same verified publisher merge automatically. Delisting is a revert; since wugs hosts nothing, there is nothing else to take down.
- **Self-hostable:** the index format is open and the client accepts several index URLs, so a university or an archive can run a private index (for example for restricted community datasets that must never be public) beside the public one. This keeps the "every hosted service is self-hostable" rule.

**Ethics for datasets:** a dataset package must declare its licence and its consent basis (who agreed to what; CARE principles), and the index refuses packages that carry access-restricted material. Language data from communities is not a commodity; the default path for restricted material is a private index, never the public one.

## 5. Publishing from inside wugs

"Share" on a kit, dataset or model in wugs: sign in to GitHub (device-code flow, no password typed into wugs; Hugging Face token for models), wugs creates or updates the author's repository, tags a release, computes the hashes and opens the index pull request. Staged: first wugs writes a ready-to-publish folder with the manifest and hashes and shows the two steps to do by hand; then the GitHub flow; then Hugging Face.

## 6. The Packages panel

One place showing the person's setup as clusters by type (kits, models per capability, datasets, integrations, plugins), each card with version, size, licence, source and an update badge. A browsable marketplace beside it: drag a card into your setup (or click Add) to download, verify and enable it; drag it out to remove. Installing a kit pulls the packages it lists as dependencies. Installed packages work offline; browsing needs the network (labelled Online).

## 7. What was adjusted from the original ask, and why

- **"Automatically update" models:** the catalogue and the choice update automatically; large downloads ask first (an opt-in setting allows automatic downloads on unmetered connections). Field bandwidth makes silent gigabytes a real cost.
- **"Fetch from their GitHub instead of hosting":** yes, with a thin index in between. Pure GitHub search (by repo topic) cannot pin hashes, review first submissions or delist, so it would let a moved tag ship unreviewed code or data to linguists' machines. The index costs nothing to host (a git repo and GitHub Pages).
- **Code plugins in the marketplace:** data-only packages (kits, models of known families, datasets, integrations) come first; code plugins wait for the WASM sandbox and permission prompts (item 36), as the suite context already requires.
- **Publish from wugs:** staged, folder first, then GitHub, then Hugging Face, so sharing works before every sign-in flow is built.

## 8. Order of work

1. **Capabilities and model manifests (item 53):** the capability layer in the desktop app, the current wav2vec2 model expressed as a manifest, a per-capability model choice in the Speech assist panel, and Praat analysis registered as an `acoustic-analysis` provider. Bundled catalogue only.
2. **ZIPA (item 51):** the second `phone-transcription` model and the `onnx-ctc-fbank` adapter, proving the swap.
3. **Evaluation harness** (part of 52): scores for the catalogue and the paper.
4. **Index and Packages panel (item 37):** the remote catalogue, install/update/remove, drag to install, kits and datasets as packages.
5. **Publish from wugs (item 54).**
6. **Sandboxed code plugins (item 36)** join the marketplace.
