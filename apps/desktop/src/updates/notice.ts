// The update notice under the header and the Updates menu's actions. Owns
// the update state for the window; what it says and when it moves are in
// logic.ts. Nothing downloads until "Install and restart" is clicked.

import { errorText, onUpdateProgress, updateCheck, updateDownload, updateInstall } from "../api";
import { closeBlocker } from "../capture/session.ts";
import { h } from "../dom";
import { showToast } from "../store";
import { checksEnabled, fraction, idle, manualResult, noticeText, setChecksEnabled, step, type UpdateEvent, type UpdateState } from "./logic.ts";

let state: UpdateState = idle;
let beforeInstall: () => Promise<void> = async () => {};
const slot = h("div", { className: "update-slot" });

export const updateSlot = (): HTMLElement => slot;
export const autoChecks = (): boolean => checksEnabled(localStorage);
export const setAutoChecks = (on: boolean) => setChecksEnabled(localStorage, on);

/** Runs right before the app restarts (the layout's pending save). */
export const setBeforeInstall = (f: () => Promise<void>) => (beforeInstall = f);

function dispatch(e: UpdateEvent) {
  const next = step(state, e);
  if (next === state) return;
  state = next;
  render();
}

/** One check. Automatic checks stay silent on failure (field links drop). */
export async function checkForUpdates(manual: boolean) {
  if (!manual && !autoChecks()) return;
  dispatch({ type: "check", manual });
  if (state.kind !== "checking") return;
  try {
    const result = await updateCheck();
    dispatch({ type: "checked", update: result.available });
    const message = manualResult(manual, result.available === null ? { kind: "newest", version: result.currentVersion } : { kind: "found" });
    if (message !== null) showToast({ tone: "info", lines: [message] });
  } catch (e) {
    dispatch({ type: "check-failed" });
    const message = manualResult(manual, { kind: "failed", error: errorText(e) });
    if (message !== null) showToast({ tone: "error", lines: [message] });
  }
}

/** "Install and restart" on the offer: download (with progress), then install. */
async function download() {
  // Asked before the download, so a recording never waits on a restart,
  // and again before installing, and by the Rust side.
  const blocker = closeBlocker();
  if (blocker !== null) {
    dispatch({ type: "failed", message: blocker });
    return;
  }
  dispatch({ type: "download" });
  const unlisten = await onUpdateProgress((p) => dispatch({ type: "progress", done: p.done, total: p.total }));
  try {
    await updateDownload();
  } catch (e) {
    dispatch({ type: "failed", message: `The download failed: ${errorText(e)}` });
    return;
  } finally {
    unlisten();
  }
  dispatch({ type: "downloaded", message: null });
  await install();
}

async function install() {
  const blocker = closeBlocker();
  if (blocker !== null) {
    // A take started during the download: the bytes wait on the Rust side.
    dispatch({ type: "failed", message: `${blocker}, then click Install and restart.` });
    return;
  }
  dispatch({ type: "install" });
  try {
    await beforeInstall();
    await updateInstall();
  } catch (e) {
    dispatch({ type: "failed", message: errorText(e) });
  }
}

function render() {
  const text = noticeText(state);
  if (text === null) {
    slot.replaceChildren();
    return;
  }
  const online = h("span", { className: "online-chip", textContent: "Online", title: "Uses the internet: GitHub Releases" });
  const line = h("span", { textContent: text });
  const children: (HTMLElement | null)[] = [online, line];
  if (state.kind === "available") {
    const s = state;
    children.push(
      h("button", { textContent: "Install and restart", onclick: () => void download() }),
      h("button", { textContent: "Later", onclick: () => dispatch({ type: "later" }) }),
      s.update.notes === null ? null : h("button", { textContent: s.showNotes ? "Hide what's new" : "What's new", onclick: () => dispatch({ type: "notes" }) }),
    );
    if (s.message !== null) children.push(h("span", { className: "update-message", textContent: s.message }));
    if (s.showNotes && s.update.notes !== null) children.push(h("pre", { className: "update-notes", textContent: s.update.notes }));
  }
  if (state.kind === "ready") {
    children.push(h("button", { textContent: "Install and restart", onclick: () => void install() }));
    if (state.message !== null) children.push(h("span", { className: "update-message", textContent: state.message }));
  }
  if (state.kind === "downloading") {
    const f = fraction(state);
    const bar = h("progress", { max: 1 });
    if (f !== null) bar.value = f;
    children.push(bar);
  }
  slot.replaceChildren(h("div", { className: "update-notice", role: "status" }, ...children));
}
