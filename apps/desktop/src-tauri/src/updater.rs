//! In-app updates from GitHub Releases (DECISIONS 2026-11-10). The webview
//! gets no updater permission: it calls these four commands, which keep the
//! announced update and its downloaded bytes on this side. Nothing downloads
//! until the person asks, the plugin verifies the minisign signature against
//! the public key in tauri.conf.json, and installing refuses while a
//! recording is open. Before the process goes, the session thread finishes
//! its queued commands and closes the project.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::capture::Captures;
use crate::SessionThread;

/// Same words as the window-close guard in `src/capture/take.ts`.
pub const RECORDING_BLOCKS: &str = "A recording is in progress, stop it first";

/// The update the last check announced, and its bytes once downloaded.
#[derive(Default)]
pub struct Staged(Mutex<Option<(Update, Option<Vec<u8>>)>>);

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    pub current_version: String,
    /// The release notes the manifest carries (the release body at build time).
    pub notes: Option<String>,
    /// Download size, when the server answered a HEAD request with one.
    pub size_bytes: Option<u64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    /// The running version, so "you have the newest" can name it.
    pub current_version: String,
    pub available: Option<Available>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: u64,
    total: Option<u64>,
}

/// Report download progress about every 1% (or 1 MB when the size is
/// unknown), and always the last chunk, so a 100 MB download sends about a
/// hundred events instead of thousands.
pub fn should_report(reported: u64, done: u64, total: Option<u64>) -> bool {
    let step = total.map_or(1 << 20, |t| (t / 100).max(64 << 10));
    done >= reported.saturating_add(step) || total.is_some_and(|t| done >= t)
}

fn lock(staged: &Staged) -> Result<std::sync::MutexGuard<'_, Option<(Update, Option<Vec<u8>>)>>, String> {
    staged.0.lock().map_err(|_| "update state poisoned".to_string())
}

/// The asset's size from a HEAD request (GitHub redirects to its CDN, which
/// answers with Content-Length). Best effort: None on any failure.
fn remote_size(url: &str) -> Option<u64> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into();
    let response = agent.head(url).call().ok()?;
    response.headers().get("content-length")?.to_str().ok()?.parse().ok()
}

/// Let the session thread finish every command already queued, then close
/// the project (its SQLite cache and the per-project lock) so nothing is cut
/// off mid-write when the process exits for the update.
fn close_session(thread: &SessionThread) {
    thread
        .run(|s| {
            *s = None;
            Ok(())
        })
        .ok();
}

/// Ask the release endpoint once; `available` is set only when it is newer.
#[tauri::command]
pub async fn update_check(app: AppHandle, staged: State<'_, Staged>) -> Result<Checked, String> {
    let current_version = app.package_info().version.to_string();
    // On Windows `install` starts the installer and calls process::exit
    // itself, after this hook. It replaces the plugin's default hook, so it
    // runs that one's cleanup too.
    let thread = app.state::<SessionThread>().inner().clone();
    let handle = app.clone();
    let updater = app
        .updater_builder()
        .on_before_exit(move || {
            close_session(&thread);
            handle.cleanup_before_exit();
        })
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        *lock(&staged)? = None;
        return Ok(Checked { current_version, available: None });
    };
    let url = update.download_url.to_string();
    let size_bytes = tauri::async_runtime::spawn_blocking(move || remote_size(&url)).await.ok().flatten();
    let available = Available {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone().filter(|b| !b.trim().is_empty()),
        size_bytes,
    };
    *lock(&staged)? = Some((update, None));
    Ok(Checked { current_version, available: Some(available) })
}

/// Download and verify the announced update, with `update-progress` events.
/// The bytes stay here until `update_install`.
#[tauri::command]
pub async fn update_download(app: AppHandle, staged: State<'_, Staged>) -> Result<(), String> {
    let update = match lock(&staged)?.as_ref() {
        Some((update, _)) => update.clone(),
        None => return Err("no update has been found; check again".into()),
    };
    let mut done = 0u64;
    let mut reported = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                done += chunk as u64;
                if should_report(reported, done, total) {
                    reported = done;
                    app.emit("update-progress", Progress { done, total }).ok();
                }
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    let mut slot = lock(&staged)?;
    match slot.as_mut() {
        Some((current, data)) if current.version == update.version => *data = Some(bytes),
        _ => return Err("a newer check replaced this update; try again".into()),
    }
    Ok(())
}

/// Install the downloaded update and start the new version. Refused while a
/// recording is open, and no recording can start until the process is gone:
/// the capture table stays locked through install and restart. On Windows
/// the installer takes over and this process exits inside `install` (after
/// the hook set in `update_check`); elsewhere the app restarts here.
#[tauri::command]
pub fn update_install(app: AppHandle, staged: State<'_, Staged>, captures: State<'_, Captures>, thread: State<'_, SessionThread>) -> Result<(), String> {
    let Some(_no_new_takes) = captures.none_open()? else {
        return Err(RECORDING_BLOCKS.into());
    };
    {
        let slot = lock(&staged)?;
        let Some((update, Some(bytes))) = slot.as_ref() else {
            return Err("the update is not downloaded yet".into());
        };
        update.install(bytes).map_err(|e| e.to_string())?;
    }
    close_session(&thread);
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_reported_by_the_percent_and_always_at_the_end() {
        let total = Some(100 << 20);
        assert!(!should_report(0, 512 << 10, total));
        assert!(should_report(0, 1 << 20, total));
        assert!(should_report(99 << 20, 100 << 20, total), "the last chunk always reports");
        // Unknown size: every megabyte.
        assert!(!should_report(0, 1000, None));
        assert!(should_report(0, 1 << 20, None));
        // A tiny file still has a floor of 64 KB steps.
        assert!(!should_report(0, 1000, Some(1 << 20)));
        assert!(should_report(0, 1 << 20, Some(1 << 20)));
    }

    fn config(text: &str) -> serde_json::Value {
        serde_json::from_str(text).expect("valid JSON")
    }

    /// The shipped config checks GitHub's latest release and verifies with
    /// the owner's key. The build never signs: `tauri build` would need the
    /// key in the environment of every build script, so the release
    /// workflow's separate `sign` job signs the payloads (DECISIONS
    /// 2026-11-10).
    #[test]
    fn shipped_config_points_at_github_with_the_release_key() {
        let conf = config(include_str!("../tauri.conf.json"));
        let updater = &conf["plugins"]["updater"];
        assert_eq!(updater["endpoints"], serde_json::json!(["https://github.com/judecutler/wugs/releases/latest/download/latest.json"]));
        assert!(updater["pubkey"].as_str().is_some_and(|k| k.starts_with("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWdu")));
        assert_eq!(updater["requireSignedVersion"], true);
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], false);
    }
}
