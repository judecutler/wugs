# Bundles: one shareable unit for a kit and everything it needs

*Status: proposed design, roadmap row 74. Extends `PACKAGES.md` (capabilities, packages, the index). Owner, 2026-11-12: "the marketplace is coming next, start designing how we want these kits, integrations, etc. to bundle together so they're easily shareable."*

## 1. The problem

A kit today is one `kit.toml`: which panels open and how they are laid out. A useful kit also needs:

- models for its capabilities (a phone recognizer, a tagger);
- datasets (concept lists, a tagset, a lexicon to start from);
- integrations (an archive's deposit format, a web service);
- templates (tier schemas, gloss conventions);
- an example project people can try.

These live in different places. Sharing "my kit" by sending a `kit.toml` leaves the receiver to find the rest by hand. That is the same problem an untechnical GSI would hit, and the reason the owner asked for this.

## 2. What a bundle is

**A bundle is a kit package that names everything it relies on.** It is not a new kind of container beside packages. It is the `kit` package type from `PACKAGES.md` §4, grown a `[requires]` section and an optional `examples/` folder. One manifest therefore stays the single source of truth for a kit, whether it comes from a file, the index or a link.

```toml
[package]                  # the same table every package has (PACKAGES.md §4)
type = "kit"
id = "org.wugs.scrolls"
version = "0.1.0"
license = "MPL-2.0"        # the kit's own files; each required package has its own
source = "https://github.com/…"

[kit]                      # unchanged: name, tools, layout, wug, templates
name = "Scrolls"
subtitle = "Corpus"

[bundle]
authors = ["…"]
summary = "Concordance, frequency lists, keywords and a tagger for any corpus."

[[requires]]               # one per package the kit relies on
type = "model"             # model | dataset | integration | plugin (after item 36)
id = "udpipe1-ud-2.5-english-ewt"
version = "1.0.0"
files_sha256 = "…"         # the package's file list as the index records it (below)
capability = "morphosyntax"    # models: the job it does here (features, not models)
recommended = true         # offered and ticked; the person can pick another provider
why = "Tags and lemmatizes English texts offline"

[[requires]]
type = "dataset"
id = "ud-tagset"
version = "2.5"
files_sha256 = "…"

[examples]
project = "examples/sample/"   # a small synthetic or openly licensed project, opened from the welcome card
```

Rules:

- **Requirements pin a version and `files_sha256`.** `files_sha256` is the SHA-256 of the package's file list in canonical form: one line per file (`<sha256> <size> <path>`, sorted by path, LF, UTF-8). This is exactly what the index records per version, so a client can check a requirement against the index, or against the files on disk, with no other format. When the person installs what the bundle recommends, it installs what its author tested. When they choose another provider for a capability, that is their choice, made on the install screen.
- **Model requirements are recommendations.** They name a capability as well as a model, so the capability layer can still let the person choose another provider or pin one per project. A bundle never makes a feature depend on one model (CLAUDE.md, "features, not models").
- **Versions sit side by side.** Packages install to `<app data>/packages/<type>/<id>/<version>/`, so two kits that need different versions of one model can both be installed (the current `models/<id>/` moves there with the index, row 37).
- **The package format grows to match.** `wugs-package` reads only `type = "model"` today. It gains `kit`, `dataset` and `integration`, each with its own checks, and the kit's `[package]` table replaces the identity keys now in `[kit]` (a kit file without `[package]` is read as today, so existing user kits keep working).
- **Projects never depend on bundles.** A project's data stays in its op log. A project may record which bundle and version it was made with, and may pin capability providers (both are settings, as capability pins are today). Removing a bundle never changes a project. It only means that its panels are no longer offered.

## 3. Two ways to ship one

1. **By reference** (online): the kit package is listed in the index like any package. Installing it fetches the kit, then each required package from where its author hosts it. Each file is checked against the index's hashes. This is the marketplace path (row 37).
2. **Self-contained file** (offline): `name.wugskit`, a zip holding the kit package with its manifest, and optionally the files of its required packages under `packages/<type>/<id>/<version>/`. This suits fieldwork: a USB stick, a shared drive, an email to the next student.
   - **What the hashes prove.** Hashes inside the file were written by whoever made it, so they catch damage, not tampering. A required package that is also in the index is checked against the index's hashes when the machine is online, and the install screen says which packages were checked that way and which were only checked against the file itself. A signature (§6) is the offline answer to "who made this".
   - **Opened like untrusted input.** A `.wugskit` is extracted to a staging folder, never in place. Every entry name follows `wugs-package`'s file-name rules: relative, no `..`, no absolute or drive paths, no Windows-reserved names, no symlinks, and no two entries that differ only in case. Sizes are capped (the whole file, each entry, and the ratio of unpacked to packed size, against zip bombs). Anything that breaks a rule refuses the whole file and names the entry and the rule.

The Kits panel's **Install a kit file** grows to accept `.wugskit` as well as `.toml`. **Share** on a kit writes either form: "Save as a file" (choose whether to include model files, since they can be large) or "Publish" (the `PACKAGES.md` §5 flow).

## 4. Installing, in the person's words

Before anything is written, the install screen lists what will be added:

- the kit, with its wug and panels;
- each required package with its size, licence and where it comes from, and whether its hashes were checked against the index;
- the network addresses its integrations will contact, each marked Online;
- the example project.

Optional packages have checkboxes. Licences that restrict use, such as CC BY-NC-SA on the UD models for UDPipe, are named in words: "free for non-commercial research". The person decides with that in view. One button installs all of it; Cancel leaves nothing behind. Large downloads ask first (`PACKAGES.md` §7).

Uninstalling a bundle removes the kit and offers to remove the packages that no other installed kit requires. Models still selected for a capability are kept unless the person says otherwise.

## 5. Integrations: settings for adapters the shell already has

An integration is a manifest of *settings* for an adapter compiled into the shell. It never carries a script. A bundle can never embed or name an arbitrary executable. The adapter shapes:

- **REST-JSON service** (the UDPipe 2 service at LINDAT): base URL, request and response mapping, the capability it provides, its licence, labelled Online. A service only ever receives what its capability sends (for tagging: sentences). The project opts in before any of its text goes to a service (SCROLLS §3).
- **Known program** (Praat today; UDPipe 1, CWB `cqp` next): each is its own adapter in the shell, with the program's arguments fixed in code (as `praat.rs` does now). The manifest may only say which known program and which version it needs. The program comes from one of two places:
  - the person picks it on their computer;
  - wugs downloads the project's official release, pinned by hash in the integration's own index entry.
  Any other program waits for sandboxed plugins (item 36).
- **Archive deposit profile** (ELAR, PARADISEC, AILLA): which seal profile, metadata standard and access-level mapping it expects. Pure data.

So the marketplace never ships unsandboxed code: what runs is either code in the shell, reviewed with it, or an official program the person chose or that was pinned by hash.

## 6. Signing

Index-listed packages are trusted through the index's hashes and review (`PACKAGES.md` §4). A self-contained file found on a USB stick has no such chain. A bundle may therefore carry a signature (minisign, the scheme the updater already uses) by its publisher's key. The publisher's key is recorded in the index once they are verified. The install screen says one of:

- "signed by «publisher», who is listed in the wugs index";
- "signed by a key wugs does not know";
- "not signed";
- **"the signature does not match the file"**: the file was changed after it was signed. That one **refuses** the install, because nothing honest produces it.

The first three inform and never block: a linguist sharing a kit with a classmate should not need a key, so the screen informs, as consent records do.

## 7. Scrolls is the first bundle

Scrolls (row 41, `docs/design/SCROLLS.md`) ships built in. It is still written as a bundle: its `kit.toml` with `[bundle]` and `[[requires]]` for the morphosyntax providers and the UD tagset dataset, and a synthetic example corpus. The format is therefore exercised by a real kit before the marketplace exists. Tongues gets the same treatment: its concept lists and SIL domains become dataset requirements, and the sample project becomes `[examples]`.

## 8. Order of work

1. `[package]`, `[bundle]`, `[[requires]]` and `[examples]` in `wugs-kit` and `wugs-package` (parse and check, every problem listed with line and column, as today); the canonical file-list hash; side-by-side package folders. The built-in kits are rewritten to use them.
2. `.wugskit` read and write: staged extraction under the rules in §3, hash checks, the install screen, uninstall.
3. Integration manifests and the adapters: UDPipe 1 as a known program first (offline), then the REST-JSON adapter with the project opt-in.
4. The index (row 37) lists kit packages with their requirements; "Publish" (row 54).
5. Signatures on files.
