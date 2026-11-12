import { brandLogo } from "./theme/brand";
import { dropTarget } from "./drop/target";
import { open as pickDirectory } from "@tauri-apps/plugin-dialog";
import { captureReset, createProject, errorText, kitsList, onAssistDownload, openProject } from "./api";
import { reloadModels } from "./panels/assist";
import { refuseProjectChange } from "./capture/session.ts";
import { closedPanels, layoutKit, mountDock, showPanel } from "./dock";
import type { DockviewApi } from "./dockview";
import { h } from "./dom";
import { autoOpen } from "./last-project";
import { onDevicesChange, relocationBanner, wireDeviceEvents } from "./panels/devices";
import { isPanelId, PANELS } from "./panels/registry";
import { attachDock } from "./kits/workspace";
import { autoChecks, checkForUpdates, setAutoChecks, setBeforeInstall, updateSlot } from "./updates/notice";
import { activeKit, assistModelList, attempt, enterProject, getState, getToast, researcher, setAssistModels, setKits, setResearcher, showToast, subscribe, subscribeToast } from "./store";

const top = h("header", { className: "top" });
const bannerSlot = h("div", { className: "relocation-slot" });
const toastSlot = h("div", { className: "toast-slot" });
const host = h("div", { className: "dock" });
let dock: DockviewApi | null = null;

function renderTop() {
  const s = getState();
  const kit = activeKit();
  const who = h("input", {
    className: "researcher",
    placeholder: "researcher id",
    title: "Who is at the keyboard; every edit is attributed to this id",
    value: researcher(),
  });
  who.oninput = () => setResearcher(who.value);
  const panels = h(
    "select",
    {
      className: "panel-menu",
      title: "Bring back a closed panel",
      onchange: () => {
        if (dock !== null && isPanelId(panels.value)) showPanel(dock, panels.value);
        renderTop();
      },
    },
    h("option", { value: "", textContent: "Show panel…" }),
    ...(dock === null || kit === null ? [] : closedPanels(dock, kit)).map((id) => h("option", { value: id, textContent: PANELS[id].title })),
  );
  panels.disabled = panels.options.length === 1;
  // Online. Per machine; checking never downloads anything.
  const updates = h(
    "select",
    {
      className: "updates-menu",
      title: "Updates (Online: asks GitHub Releases)",
      onchange: () => {
        if (updates.value === "check") void checkForUpdates(true);
        if (updates.value === "auto") setAutoChecks(!autoChecks());
        renderTop();
      },
    },
    h("option", { value: "", textContent: "Updates…" }),
    h("option", { value: "check", textContent: "Check for updates now (Online)" }),
    h("option", { value: "auto", textContent: autoChecks() ? "Stop checking at start (Online)" : "Check at start (Online)" }),
  );
  const logo = brandLogo();
  top.replaceChildren(
    ...(logo === null ? [] : [logo]),
    h("strong", { className: "brand", textContent: kit?.name ?? "wugs", title: kit === null ? "wugs" : `wugs: ${kit.name}${kit.subtitle === "" ? "" : `, ${kit.subtitle.toLowerCase()}`}` }),
    h("span", { className: "subtitle", textContent: kit?.subtitle ?? "" }),
    who,
    h("button", { textContent: "Open project", onclick: () => void pickProject("open") }),
    h("button", { textContent: "Create project", onclick: () => void pickProject("create") }),
    h(
      "span",
      { className: "project" },
      s.kind === "open" ? h("code", { textContent: s.info.projectId, title: s.info.path }) : null,
      s.kind === "open"
        ? h("span", { className: s.info.conflicts > 0 ? "badge badge-hot" : "badge", textContent: `${s.info.conflicts} conflicts` })
        : null,
      panels,
      updates,
      h("button", {
        textContent: "Reset layout",
        title: `Put every panel back where ${kit?.name ?? "the kit"} starts`,
        onclick: () => {
          if (dock !== null && kit !== null) layoutKit(dock, kit);
          renderTop();
        },
      }),
    ),
  );
}

function renderBanner() {
  const banner = relocationBanner();
  bannerSlot.replaceChildren(...(banner === null ? [] : [banner]));
}

function renderToast() {
  const toast = getToast();
  toastSlot.replaceChildren(
    toast === null
      ? ""
      : h(
          "div",
          { className: `toast toast-${toast.tone}`, role: "status" },
          h("div", {}, ...toast.lines.map((line) => h("div", { textContent: line }))),
          ...(toast.action === undefined
            ? []
            : [
                h("button", {
                  className: "dismiss",
                  textContent: toast.action.label,
                  onclick: () => {
                    showToast(null);
                    toast.action?.run();
                  },
                }),
              ]),
          h("button", { className: "dismiss", textContent: "Dismiss", onclick: () => showToast(null) }),
        ),
  );
}

/** Speech-assist models: listed at start, kept current while they download. */
async function wireAssist() {
  await attempt(reloadModels);
  await onAssistDownload((e) => {
    if (e.finished === null) {
      setAssistModels(assistModelList().map((m) => (m.id === e.id ? { ...m, download: { done: e.done, total: e.total, error: null } } : m)));
      return;
    }
    if ("Err" in e.finished && e.finished.Err !== "cancelled") showToast({ tone: "error", lines: [`The model download failed: ${e.finished.Err}`] });
    if ("Ok" in e.finished) showToast({ tone: "info", lines: ["Model downloaded. Empty segments can now get suggestions."] });
    void attempt(reloadModels);
  });
}

async function pickProject(mode: "open" | "create") {
  if (refuseProjectChange()) return;
  const who = researcher();
  if (who === "") {
    showToast({ tone: "error", lines: ["Enter your researcher id first."] });
    return;
  }
  const path = await pickDirectory({
    directory: true,
    title: mode === "open" ? "Open a wugs project" : "Choose an empty folder for the new project",
  });
  if (typeof path !== "string") return;
  await attempt(async () => {
    await enterProject(await (mode === "open" ? openProject : createProject)({ path, researcher: who }));
  });
}

/** Never a dialog: a failure leaves the no-project screen and a toast. */
async function reopenLast() {
  const result = await autoOpen(
    localStorage,
    researcher(),
    () => getState().kind === "open",
    async (path, who) => enterProject(await openProject({ path, researcher: who })),
    errorText,
  );
  if (result.kind === "failed") showToast({ tone: "error", lines: [`Could not reopen ${result.path}: ${result.error}`] });
}

const app = document.querySelector<HTMLDivElement>("#app");
if (app !== null) {
  app.replaceChildren(top, updateSlot(), bannerSlot, host, toastSlot, dropTarget());
  subscribe(renderTop);
  subscribe(renderBanner);
  onDevicesChange(renderBanner);
  subscribeToast(renderToast);
  renderTop();
  void (async () => {
    // The bundled kit is compiled in and an unreadable kits folder is
    // reported as a broken entry, so there is always an active kit.
    setKits(await kitsList());
    const mounted = await mountDock(host, activeKit()!);
    attachDock(mounted);
    dock = mounted.api;
    setBeforeInstall(mounted.flush);
    dock.onDidLayoutChange(renderTop);
    renderTop();
    await wireDeviceEvents();
    await wireAssist();
    // A reload leaves the old page's captures open on the Rust side; this page owns none.
    await captureReset().catch(() => 0);
    await reopenLast();
    // After the window is up and the project is back; never awaited by anything.
    void checkForUpdates(false);
  })();
}
