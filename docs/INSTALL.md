# Install a build

wugs publishes installers for Windows, macOS and Linux from CI, so you do not need Rust or a C++ toolchain to try it. The builds are **unsigned** for now, so each OS warns you once. The steps below get past the warning.

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

## Cut a release

1. Bump `version` in `apps/desktop/src-tauri/tauri.conf.json` (and `Cargo.toml`, `package.json` to match).
2. `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The **Release** workflow builds all three installers and attaches them to a **draft** release for that tag.
4. Open the draft on the Releases page, check the files, and click **Publish release**.

## Signing (not set up yet)

Unsigned builds work but show the warnings above. Removing them needs credentials that do not exist yet:

- **Windows.** An Authenticode code-signing certificate, or Azure Trusted Signing. SmartScreen reputation still builds up over time.
- **macOS.** An Apple Developer ID Application certificate plus notarization (Apple ID or API key). `tauri-action` reads these from repository secrets such as `APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY` and `APPLE_ID`.

Add them as repository secrets when they exist and extend `release.yml`. Nothing in the workflow requires them today.

## Not done yet

- **Auto-update.** `tauri-plugin-updater` needs a signing key pair and a place to host update manifests. Both are open decisions, so each release is installed by hand.
- **MSI.** Only the NSIS installer is built on Windows. NSIS is smaller and installs per user. Add `"msi"` to `bundle.targets` if an organization needs one.
