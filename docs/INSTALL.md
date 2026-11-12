# Install a build

wugs publishes installers for Windows, macOS and Linux from CI, so you do not need Rust or a C++ toolchain to try it. The installers are **not code-signed** for now, so each OS warns you once. The steps below get past the warning. After the first install, wugs updates itself (see [Updates](#updates)).

## Where to download

- **A tagged release.** Go to the repository's **Releases** page and download the file for your OS from the latest release.
- **Any commit or branch.** Open the **Actions** tab, choose the **Release** workflow, click **Run workflow**, pick the branch, and wait for it to finish (roughly 15 to 30 minutes). Open the finished run and download `wugs-Windows`, `wugs-macOS` or `wugs-Linux` from **Artifacts**. Each is a zip that contains the installers (and, on macOS, an unsigned `.app.tar.gz` update payload you can ignore).

| OS | File | Notes |
| --- | --- | --- |
| Windows | `wugs_<version>_x64-setup.exe` | NSIS installer, per-user, no admin rights needed |
| macOS | `wugs_<version>_universal.dmg` | One build for Intel and Apple Silicon |
| Linux | `wugs_<version>_amd64.AppImage` or `.deb` | AppImage runs anywhere (`chmod +x`, then run); `.deb` is for Debian and Ubuntu |

## First run on Windows

1. Unzip the artifact (Releases downloads are not zipped) and double-click `wugs_..._x64-setup.exe`.
2. Windows SmartScreen shows **Windows protected your PC**. Click **More info**, then **Run anyway**.
3. Finish the installer. wugs appears in the Start menu.

Windows needs the WebView2 runtime. Windows 10 and 11 already include it, and the installer fetches it if it is missing.

## First run on macOS

1. Open the `.dmg` and drag wugs to Applications.
2. The first launch says Apple could not verify wugs. Choose **Done**, open **System Settings, Privacy & Security**, scroll to the message about wugs and click **Open Anyway** (on macOS 14 and older, right-clicking the app and choosing **Open** also works).

The app is ad-hoc signed (`signingIdentity: "-"` in `tauri.conf.json`). Without any signature, an Apple silicon Mac calls a downloaded app damaged and offers no way to open it.

## First run on Linux

AppImage: `chmod +x wugs_*.AppImage && ./wugs_*.AppImage`. Debian or Ubuntu: `sudo apt install ./wugs_*.deb`.

## Updates

wugs can update itself from the repository's GitHub Releases. This is an **Online** feature.

- **Checking.** Shortly after wugs starts, it asks GitHub once whether a newer release exists (a few kilobytes). If you are offline, nothing happens. To stop this, open **Settings** in the top bar and, under **Updates**, turn off **Check when wugs starts**. The setting is per computer. **Check for updates now** works either way.
- **Asking.** When a newer version exists, a line under the header says so, for example "wugs 0.2.0 is available (42 MB)". Choose **Install and restart**, **Later** or **What's new**. Nothing downloads until you choose Install and restart, so a slow or paid connection is never used without your say.
- **Installing.** wugs downloads the update with a progress bar, checks its signature against the key built into the app, installs it and restarts. If a recording is in progress, wugs refuses to restart until you stop it; a finished download waits.
- On Windows wugs finishes what it was saving, closes the project and hands over to the installer, which opens the new version. On Linux, an AppImage replaces itself; a `.deb` install downloads the new `.deb` and asks for your password (`pkexec dpkg -i`). A `.deb` install is never given the AppImage.

0.1.0 is the first published release; builds made before it (0.0.1 and earlier, never published) update by hand once.

## Cut a release

### Once, before the first tag

- **Environment.** In **Settings → Environments**, the `release` environment must exist with the secrets `SIGNING_KEY` and `SIGNING_KEY_PASSWORD`, **Required reviewers** set to you, and under **Deployment branches and tags** a rule that allows only tags matching `v*`.
- **Tag ruleset.** In **Settings → Rules → Rulesets**, add a tag ruleset that targets `v*` and restricts creation, update and deletion, with only you (the repository admin role) on the bypass list. Without it, anyone with write access could push a `v*` tag; the environment's approval still stands in front of the key, but the ruleset keeps stray tags from starting release runs at all.

### Each release

Never make the release on GitHub's Releases page. Pushing the tag is all it takes: the run below builds the installers and makes a **draft** with them attached, and you publish that. A release made by hand is published before any installer exists, the download page then has nothing to offer, and the run never changes a published release, so it stays empty. If that happens, delete the release (its tag stays) and re-run the failed `publish` job, or let a run still in progress reach it. A run can be re-run for 30 days; after that, release a new version.

1. Bump `version` in `apps/desktop/src-tauri/tauri.conf.json` (and `Cargo.toml`, `package.json` to match). The workflow refuses a tag that does not match.
2. Make an annotated tag whose message is the release notes. The app shows them under **What's new**: `git tag -a vX.Y.Z -m "wugs X.Y.Z" -m "What changed, in a few lines."`, then `git push origin vX.Y.Z`.
3. In GitHub, open **Actions**, then the **Release** run for the tag. The three `build` jobs run first. Then the `sign` job waits for approval of the `release` environment: click **Review deployments**, select `release` and **Approve and deploy**.
4. The run then does the rest in three separate jobs, so the signing key never meets the build and the write token never meets the key:
   - `build` (three OSes) builds the installers and the updater payloads **unsigned**, with read-only access, no secrets and no caches, and uploads them as workflow artifacts. The payloads are the files the updater installs: the Windows `-setup.exe`, a `.app.tar.gz` of the macOS app, the Linux AppImage and `.deb`.
   - `sign` holds the key. It installs only the pinned Tauri CLI from `.github/signer` (with install scripts off), signs each payload with `tauri signer sign --app-version X.Y.Z` and uploads the `.sig` files. It has read-only access.
   - `publish` has write access and no key. It writes `latest.json` with `.github/scripts/updater_manifest.py` (which refuses a missing or unsigned payload, a signature by another key or for another version, and a tag that does not match `tauri.conf.json`) and attaches everything to a **draft** release. A re-run updates the draft; a published release is never changed. It refuses any `.sig` or `latest.json` that came from a build, so only the sign job's signatures reach the release.

   What the split protects is the key: no build script can read it, so nobody can sign an update later without you. It cannot tell whether a compromised build dependency changed the installer itself before it was signed; your approval covers the run, not the files' contents. Keep dependencies pinned (lockfiles) and review dependency updates as code.
5. Open the draft on the Releases page, check the files (below), and click **Publish release**. Installed copies see the update only after it is published: drafts and pre-releases are never offered.

Manual runs and pull requests run only `build`, never use the `release` environment or its secrets, and produce unsigned artifacts. A local `npm run tauri build` in `apps/desktop` needs no key either. To sign a file by hand: `npx tauri signer sign --app-version X.Y.Z <file>` with the key in `TAURI_SIGNING_PRIVATE_KEY`.

### First signed release: what to check

Nothing in this pipeline has run yet. On the first tag, check:

- The draft has the installers (`*-setup.exe`, `*.dmg`, `*.AppImage`, `*.deb`), `wugs_X.Y.Z_universal.app.tar.gz`, a `.sig` for each of `*-setup.exe`, `*.app.tar.gz`, `*.AppImage` and `*.deb`, and `latest.json`.
- `latest.json` has `version` without the `v`, the tag's message as `notes`, and `platforms` with `windows-x86_64`, `darwin-aarch64`, `darwin-x86_64`, `linux-x86_64` (the AppImage) and `linux-x86_64-deb`, each URL under `releases/download/vX.Y.Z/`.
- After publishing, `https://github.com/judecutler/wugs/releases/latest/download/latest.json` downloads that file.
- On one machine with the previous release installed: the notice appears, What's new shows the notes, Install and restart downloads with progress and comes back as the new version. Start a recording first once, to see the refusal. On macOS check that the app opens after the update (the payload is our own tar of the universal `.app`, not one `tauri build` made); on Linux try both an AppImage and a `.deb` install.

## Signing installers (not set up yet)

The updater's signature protects updates. The installers themselves are not code-signed, so first installs show the warnings above. Removing them needs credentials that do not exist yet:

- **Windows.** An Authenticode code-signing certificate, or Azure Trusted Signing. SmartScreen reputation still builds up over time.
- **macOS.** An Apple Developer ID Application certificate plus notarization (Apple ID or API key). `tauri-action` reads these from repository secrets such as `APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY` and `APPLE_ID`.

Add them as repository secrets when they exist and extend `release.yml`. Nothing in the workflow requires them today.

## Not done yet

- **MSI.** Only the NSIS installer is built on Windows. NSIS is smaller and installs per user. Add `"msi"` to `bundle.targets` if an organization needs one.
