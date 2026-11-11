# Install a build

wugs publishes installers for Windows, macOS and Linux from CI, so you do not need Rust or a C++ toolchain to try it. The installers are **not code-signed** for now, so each OS warns you once. The steps below get past the warning. After the first install, wugs updates itself (see [Updates](#updates)).

## Where to download

- **A tagged release.** Go to the repository's **Releases** page and download the file for your OS from the latest release.
- **Any commit or branch.** Open the **Actions** tab, choose the **Release** workflow, click **Run workflow**, pick the branch, and wait for it to finish (roughly 15 to 30 minutes). Open the finished run and download `wugs-installers-Windows`, `wugs-installers-macOS` or `wugs-installers-Linux` from **Artifacts**. Each is a zip that contains the installer.

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
2. The first launch says wugs cannot be opened because the developer cannot be verified. Right-click (or Control-click) the app, choose **Open**, then **Open** again. Alternatively open **System Settings, Privacy & Security**, scroll to the message about wugs and click **Open Anyway**.

## First run on Linux

AppImage: `chmod +x wugs_*.AppImage && ./wugs_*.AppImage`. Debian or Ubuntu: `sudo apt install ./wugs_*.deb`.

## Updates

wugs can update itself from the repository's GitHub Releases. This is an **Online** feature.

- **Checking.** Shortly after wugs starts, it asks GitHub once whether a newer release exists (a few kilobytes). If you are offline, nothing happens. To stop this, open the **Updates…** menu in the header and choose **Stop checking at start**. The setting is per computer. **Check for updates now** works either way.
- **Asking.** When a newer version exists, a line under the header says so, for example "wugs 0.2.0 is available (42 MB)". Choose **Install and restart**, **Later** or **What's new**. Nothing downloads until you choose Install and restart, so a slow or paid connection is never used without your say.
- **Installing.** wugs downloads the update with a progress bar, checks its signature against the key built into the app, installs it and restarts. If a recording is in progress, wugs refuses to restart until you stop it; a finished download waits.
- On Windows the installer closes wugs and opens the new version. On Linux, an AppImage replaces itself; a `.deb` install asks for your password (`pkexec`).

Installs older than the first release with the updater (0.0.1) must install the next release by hand once.

## Cut a release

1. Bump `version` in `apps/desktop/src-tauri/tauri.conf.json` (and `Cargo.toml`, `package.json` to match). The workflow refuses a tag that does not match.
2. Make an annotated tag whose message is the release notes. The app shows them under **What's new**: `git tag -a vX.Y.Z -m "wugs X.Y.Z" -m "What changed, in a few lines."`, then `git push origin vX.Y.Z`.
3. In GitHub, open **Actions**, then the **Release** run for the tag. It waits for approval of the `release` environment. Click **Review deployments**, select `release` and **Approve and deploy**.
4. The `release` jobs build the three installers, sign the updater files with the key in the environment's secrets (`SIGNING_KEY`, `SIGNING_KEY_PASSWORD`), and attach everything to a **draft** release. The `manifest` job then adds `latest.json`.
5. Open the draft on the Releases page, check the files (below), and click **Publish release**. Installed copies see the update only after it is published: drafts and pre-releases are never offered.

Manual runs and pull requests build unsigned installers without updater files (through `apps/desktop/src-tauri/tauri.unsigned.conf.json`) and never use the `release` environment or its secrets. A local `npm run tauri build` in `apps/desktop` needs the same override, `npm run tauri build -- --config "$PWD/src-tauri/tauri.unsigned.conf.json"`, unless the signing key is set in `TAURI_SIGNING_PRIVATE_KEY`.

### First signed release: what to check

Nothing in this pipeline has run yet. On the first tag, check:

- The draft has, besides the installers, `*-setup.exe.sig`, `wugs*.app.tar.gz` with its `.sig`, `*.AppImage.sig` (and ideally `*.deb.sig`; the manifest job warns if the `.deb` has none), and `latest.json`.
- `latest.json` has `version` without the `v`, the tag's message as `notes`, and `platforms` with `windows-x86_64`, `darwin-aarch64`, `darwin-x86_64` and `linux-x86_64`, each URL under `releases/download/vX.Y.Z/`. The job fails if any of the four is missing.
- After publishing, `https://github.com/judecutler/wugs/releases/latest/download/latest.json` downloads that file.
- On one machine with the previous release installed: the notice appears, What's new shows the notes, Install and restart downloads with progress and comes back as the new version. Start a recording first once, to see the refusal.

## Signing installers (not set up yet)

The updater's signature protects updates. The installers themselves are not code-signed, so first installs show the warnings above. Removing them needs credentials that do not exist yet:

- **Windows.** An Authenticode code-signing certificate, or Azure Trusted Signing. SmartScreen reputation still builds up over time.
- **macOS.** An Apple Developer ID Application certificate plus notarization (Apple ID or API key). `tauri-action` reads these from repository secrets such as `APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY` and `APPLE_ID`.

Add them as repository secrets when they exist and extend `release.yml`. Nothing in the workflow requires them today.

## Not done yet

- **MSI.** Only the NSIS installer is built on Windows. NSIS is smaller and installs per user. Add `"msi"` to `bundle.targets` if an organization needs one.
