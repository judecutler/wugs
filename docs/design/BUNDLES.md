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
[kit]                      # unchanged: identity, tools, layout, wug, templates
id = "org.wugs.scrolls"
name = "Scrolls"
subtitle = "Corpus"
version = "0.1.0"

[bundle]                   # new: what makes it shareable
licence = "MPL-2.0"        # the kit's own files; each required package has its own
authors = ["…"]
homepage = "https://…"
summary = "Concordance, frequency lists, keywords and a tagger for any corpus."

[[requires]]               # one per package the kit relies on
type = "model"             # model | dataset | integration | plugin (after item 36)
id = "udpipe-ud-2.15-english-ewt"
capability = "morphosyntax"    # models: the job it does here (features, not models)
version = "1.0.0"
sha256 = "…"               # of the package manifest, so the index cannot swap it
optional = true            # offered, not forced: the person can pick another provider
why = "Tags and lemmatizes English texts"

[[requires]]
type = "dataset"
id = "ud-tagset"
version = "2.15"
sha256 = "…"

[examples]
project = "examples/sample/"   # a small synthetic or openly licensed project, opened from the welcome card
```

Rules:

- **Requirements pin a version and the hash of the package's manifest.** The package's own manifest pins its files' hashes (as today). A bundle therefore always installs exactly what its author tested, and the index cannot change it underneath.
- **Model requirements are recommendations.** They name a capability as well as a model, so the capability layer can still let the person choose another provider or pin one per project. A bundle never makes a feature depend on one model (CLAUDE.md, "features, not models").
- **Projects never depend on bundles.** A project's data stays in its op log. A project may record which bundle and version it was made with, and may pin capability providers (both are settings, as capability pins are today). Removing a bundle never changes a project. It only means that its panels are no longer offered.

## 3. Two ways to ship one

1. **By reference** (online): the kit package is listed in the index like any package. Installing it fetches the kit, then each required package from where its author hosts it. Each file is checked against the index's hashes. This is the marketplace path (row 37).
2. **Self-contained file** (offline): `name.wugskit`, a zip holding the kit package with `bundle.toml`, and optionally the files of its required packages under `packages/<id>/`. This suits fieldwork: a USB stick, a shared drive, an email to the next student. Installing checks every embedded file against the hashes in the manifests, exactly as a download would. A file that does not match is refused, so a self-contained bundle is never less safe than the index.

The Kits panel's **Install a kit file** grows to accept `.wugskit` as well as `.toml`. **Share** on a kit writes either form: "Save as a file" (choose whether to include model files, since they can be large) or "Publish" (the `PACKAGES.md` §5 flow).

## 4. Installing, in the person's words

Before anything is written, the install screen lists what will be added:

- the kit, with its wug and panels;
- each required package with its size, licence and where it comes from;
- the network addresses its integrations will contact, each marked Online;
- the example project.

Optional packages have checkboxes. Licences that restrict use, such as CC BY-NC-SA on the UD models for UDPipe, are named in words: "free for non-commercial research". The person decides with that in view. One button installs all of it; Cancel leaves nothing behind. Large downloads ask first (`PACKAGES.md` §7).

Uninstalling a bundle removes the kit and offers to remove the packages that no other installed kit requires. Models still selected for a capability are kept unless the person says otherwise.

## 5. Integrations as data, not code

An integration is a manifest that describes a connection: an archive deposit profile, a REST service, an external program. It is not a script. Examples:

- **A web service** (the UDPipe REST service at LINDAT): base URL, the request it takes, the response format, the capability it provides, its licence and rate rules, labelled Online.
- **An external program** (Praat today; UDPipe 1, CWB later): how to find it, the version it needs, the arguments for each operation, the output format. The shell already does this by hand for Praat (`praat.rs`). The manifest form makes the next ones data instead of Rust.
- **An archive** (ELAR, PARADISEC, AILLA): which seal profile, metadata standard and access-level mapping it expects.

The shell ships the adapters (one per *shape*: REST-JSON, external program with files in and out, deposit profile); integrations fill them in. An integration that needs more than an adapter can express waits for sandboxed plugins (item 36), so the marketplace never ships unsandboxed code.

## 6. Signing

Index-listed packages are trusted through the index's hashes and review (`PACKAGES.md` §4). A self-contained file found on a USB stick has no such chain. A bundle may therefore carry a signature (minisign, the scheme the updater already uses) by its publisher's key. The publisher's key is recorded in the index once they are verified. The install screen then says one of:

- "signed by «publisher», who is listed in the wugs index";
- "signed by a key wugs does not know";
- "not signed".

None of these blocks the install. A linguist sharing a kit with a classmate should not need a key. The screen informs, as consent records do.

## 7. Scrolls is the first bundle

Scrolls (row 41, `docs/design/SCROLLS.md`) ships built in. It is still written as a bundle: its `kit.toml` with `[bundle]` and `[[requires]]` for the morphosyntax providers and the UD tagset dataset, and a synthetic example corpus. The format is therefore exercised by a real kit before the marketplace exists. Tongues gets the same treatment: its concept lists and SIL domains become dataset requirements, and the sample project becomes `[examples]`.

## 8. Order of work

1. `[bundle]`, `[[requires]]` and `[examples]` in `wugs-kit` (parse and check, every problem listed with line and column, as today). The built-in kits are rewritten to use them.
2. `.wugskit` read and write: zip, hash checks, the install screen, uninstall.
3. Integration manifests and the REST-JSON and external-program adapters (UDPipe REST first, for Scrolls).
4. The index (row 37) lists kit packages with their requirements; "Publish" (row 54).
5. Signatures on files.
