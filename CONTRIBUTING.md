# Contributing to wugs

wugs is infrastructure for people documenting languages, often languages with few remaining speakers. Correctness and data safety come before features.

## Before you write code

Read `docs/DATA-MODEL.md`. It is the specification this engine implements, and most review comments trace back to it. `CLAUDE.md` lists the hard rules that no change may violate, including the ones that keep the op log the single source of truth and keep restricted recordings from crossing a sync boundary.

## Sign your commits (DCO, not a CLA)

wugs accepts contributions under the [Developer Certificate of Origin 1.1](https://developercertificate.org/). You certify that you wrote the contribution, or have the right to submit it under this project's license, by adding a sign-off line to each commit:

```
Signed-off-by: Your Name <your.email@example.com>
```

`git commit -s` adds it for you.

There is no Contributor License Agreement. You keep the copyright in your contribution, and nobody can relicense it without you. That is deliberate, so the project cannot quietly stop being open.

## Build and test

```
cargo build --workspace
cargo test --workspace
cargo run -p wugs-bench --release -- 500000   # op-log replay gate
```

Requires Rust and a system `libsqlite3`. **The core crates take no external dependencies.** That is a design choice about archival longevity and supply-chain surface, not an accident of the build environment. If you believe a dependency is genuinely necessary, raise it in an issue before writing the patch, and expect it to land at the edges rather than in the core.

## What a reviewable change looks like

- Tests assert behaviour a user of the code can observe, not internal structure.
- A bug fix comes with a test that fails without it.
- Comments explain a non-obvious *why*. Code that needs a comment to say *what* it does usually wants rewriting instead.
- New entities, fields, or op verbs update `docs/DATA-MODEL.md` in the same change. The spec and the code move together.
- Anything touching merge, sync, or the op log explains in the PR description what happens when two devices do it concurrently.

## Things that will be sent back

- Writing to the SQLite projection outside `wugs-projection` or `wugs-core`. The projection is disposable and rebuilt from the log; direct writes break that guarantee.
- Rewriting or deleting log history. Tombstones only.
- A validator that blocks local work. Validators warn locally and hold only on incoming sync.
- Kit-specific behaviour added to a core crate.

## Reporting a data-loss or privacy bug

Open an issue marked clearly, or contact the maintainer directly if it involves a path by which restricted material could leave a device. These take priority over everything else.
