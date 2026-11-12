import { dropTarget } from "./drop/target";
import { kitWug, setCurrentWug } from "./theme/wug";
import { getVersion } from "@tauri-apps/api/app";
import { applyEmphasisPattern, applyPattern, applyTheme, choosePattern, chooseTheme, emphasisPatternOn, PATTERNS, savedPattern, savedTheme, THEMES } from "./theme/theme";

applyTheme(savedTheme(localStorage));
applyPattern(savedPattern(localStorage));
applyEmphasisPattern(emphasisPatternOn(localStorage));
// Shown in Settings; read once, never awaited by anything.
let appVersion = "";
/** The wug was just let back in from Settings: it visits when the sheet closes. */
let visitAfterSettings = false;
void getVersion()
  .then((v) => (appVersion = v))
  .catch(() => {});
import { open as pickDirectory } from "@tauri-apps/plugin-dialog";
import { captureReset, createProject, errorText, feedbackRetry, kitsList, onAssistDownload, openProject, openSample } from "./api";
import { startTour, tourDone } from "./tour/tour";
import { reloadModels } from "./panels/assist";
import { closeBlocker, refuseProjectChange } from "./capture/session.ts";
import { closedPanels, layoutKit, mountDock, offers, showPanel } from "./dock";
import { setTabMode, tabMode } from "./dock-reveal";
import type { DockviewApi } from "./dockview";
import { h, wugArt } from "./dom";
import { menuButton } from "./menu";
import { wireVisits, type Visits } from "./wug-peek";
import { setVisitsOn, visitsOn } from "./wug-visit";
import { autoOpen, lastProject } from "./last-project";
import { folderName, welcomeView } from "./welcome";
import { onDevicesChange, relocationBanner, wireDeviceEvents } from "./panels/devices";
import { isPanelId, PANELS, type PanelId } from "./panels/registry";
import { SHOW_PANEL } from "./panels/panel";
import { attachDock, useKit } from "./kits/workspace";
import { openFeedback } from "./feedback/form";
import { openSettings } from "./settings/panel";
import { provide } from "./palette/registry";
import { openPalette, openShortcuts, wirePalette } from "./palette/ui";
import type { Command } from "./palette/logic";
import { autoChecks, checkForUpdates, setAutoChecks, setBeforeInstall, updateSlot } from "./updates/notice";
import { activeKit, assistModelList, attempt, dropWantedMedia, enterProject, getKits, getState, selectSegment, selectText, getToast, researcher, setAssistModels, setKits, setResearcher, showToast, subscribe, subscribeToast } from "./store";

const top = h("header", { className: "top" });
/** A redraw held back while a menu was open (renderTop never rebuilds under one). */
let topPending = false;
top.addEventListener("menu-closed", (e) => {
  if (!topPending) return;
  const { key, focus } = (e as CustomEvent<{ key: string; focus: boolean }>).detail;
  queueMicrotask(() => {
    renderTop();
    // The rebuilt bar has new buttons: give focus back to the menu it came from.
    if (focus) menuNamed(key)?.focus();
  });
});
const bannerSlot = h("div", { className: "relocation-slot" });
const toastSlot = h("div", { className: "toast-slot" });
const host = h("div", { className: "dock" });
const welcomeSlot = h("div", { className: "welcome-slot" });
/** Until the last project has had its chance to reopen, show neither the welcome nor an empty dock. */
let booting = true;
/** "Join from another device" shows the workspace with no project, on the Devices panel. */
let joining = false;
let dock: DockviewApi | null = null;
let visits: Visits | null = null;
/** The id the open project signs edits with: fixed when it opened. */
let signedAs = "";
let signedFor = "";
/** The chip's id is being changed. */
let editingId = false;

/** The top bar's menu button with this key (`menuButton`'s `key`). */
const menuNamed = (key: string): HTMLButtonElement | undefined => [...top.querySelectorAll<HTMLButtonElement>(".menu-button")].find((b) => b.dataset.menu === key);

function renderTop() {
  // Never rebuild the bar under an open menu; it redraws when the menu closes.
  if (top.querySelector(".menu-list") !== null) {
    topPending = true;
    return;
  }
  topPending = false;
  // Typing in the bar's id field must not be redrawn under the person.
  if (top.contains(document.activeElement) && document.activeElement instanceof HTMLInputElement) return;
  // A redraw replaces every button: keyboard focus goes back to the same menu.
  const active = document.activeElement;
  const focusedMenu = top.contains(active) && active instanceof HTMLElement && active.classList.contains("menu-button") ? (active.dataset.menu ?? null) : null;
  queueMicrotask(() => {
    if (focusedMenu !== null && !top.contains(document.activeElement)) menuNamed(focusedMenu)?.focus();
  });
  const s = getState();
  const kit = activeKit();
  if (s.kind === "open" && s.info.projectId !== signedFor) {
    signedFor = s.info.projectId;
    signedAs = researcher();
    editingId = false;
  }
  const who = h("input", {
    className: "researcher",
    placeholder: "your id",
    title: "Who is at the keyboard: projects opened from now on sign every edit with this id",
    value: researcher(),
    size: 8,
  });
  who.oninput = () => setResearcher(who.value);
  const changeId = (value: string) => {
    const next = value.trim();
    if (next !== "" && next !== signedAs && s.kind === "open") {
      setResearcher(next);
      const path = s.info.path;
      showToast({
        tone: "info",
        lines: [`Projects you open from now on sign edits «${next}». This one keeps signing «${signedAs}» until it is opened again.`],
        action: { label: "Reopen it now", run: () => void reopenAs(path) },
      });
    }
    renderTop();
  };
  const idControl = editingId
    ? (() => {
        const field = h("input", { className: "researcher", value: signedAs, size: 8, title: "A new researcher id, then Enter" });
        // Enter takes the new id; Escape or leaving the field keeps the old one.
        const close = (commit: boolean) => {
          if (!editingId) return;
          const value = field.value;
          editingId = false;
          field.blur();
          if (commit) changeId(value);
          else renderTop();
        };
        field.onkeydown = (e) => {
          if (e.key === "Enter") close(true);
          if (e.key === "Escape") close(false);
        };
        field.onblur = () => close(false);
        queueMicrotask(() => field.select());
        return field;
      })()
    : h("button", {
        className: "signed-as",
        textContent: signedAs,
        title: `Edits in this project are signed «${signedAs}». Click to use another id.`,
        onclick: () => {
          editingId = true;
          renderTop();
        },
      });
  const project = menuButton("Project", () => [
    { label: "Open another project…", run: () => void pickProject("open") },
    { label: "Create a project…", run: () => void pickProject("create") },
    { label: "Open the sample project", run: () => void enterSample() },
  ]);
  // Settings (owner, 2026-11-12): one panel for how the workspace looks and
  // behaves and for updates, with themes and patterns shown, not named.
  const settings = h("button", {
    className: "menu-button settings-button",
    textContent: "Settings",
    title: "Colours, patterns, the workspace and updates",
    onclick: openSettingsSheet,
  });
  project.classList.add("project-menu");
  setCurrentWug(kitWug(kit));
  const conflicts = s.kind === "open" ? s.info.conflicts : 0;
  top.replaceChildren(
    ...[
    h("span", { className: "brand-mark" }, (() => {
      const art = wugArt("brand-wug", `${kit?.name ?? "wugs"} wug`);
      // Out on a visit: a redraw must not put a second wug in the corner.
      if (visits?.away()) art.classList.add("wug-away");
      return art;
    })()),
    kitSwitcher(),
    s.kind === "open"
      ? h(
          "span",
          { className: "project-chip" },
          h("span", { className: "project-name", textContent: folderName(s.info.path), title: `${s.info.path}\nProject ${s.info.projectId}` }),
          h("span", { className: "project-as", textContent: "as" }),
          idControl,
          conflicts > 0
            ? h("button", {
                className: "badge badge-hot",
                textContent: conflicts === 1 ? "1 conflict" : `${conflicts} conflicts`,
                title: "Edits from different devices that need a decision",
                onclick: () => {
                  if (dock !== null) (dock.getPanel("conflicts") ?? showPanel(dock, "conflicts"))?.api.setActive();
                },
              })
            : null,
        )
      : null,
    // Joining from another device: the id field and a way back to the welcome.
    s.kind !== "open" && joining
      ? h(
          "span",
          { className: "project-chip" },
          h("span", { className: "project-as", textContent: "Joining as" }),
          who,
          h("button", {
            className: "signed-as",
            textContent: "Back",
            title: "Back to the welcome screen",
            onclick: () => {
              joining = false;
              renderWelcome();
              renderTop();
            },
          }),
        )
      : null,
    h(
      "span",
      { className: "project" },
      h(
        "button",
        { className: "menu-button palette-button", title: "Find any action or panel by name", onclick: openPalette },
        "Find an action",
        h("kbd", { textContent: /Mac|iPhone|iPad/u.test(navigator.platform || navigator.userAgent) ? "⌘K" : "Ctrl+K" }),
      ),
      project,
      settings,
      h("button", {
        className: "menu-button feedback-button",
        textContent: "Feedback",
        title: "Tell the people who make wugs what went wrong or what would help (Online)",
        onclick: openFeedbackForm,
      }),
    ),
    ].filter((c) => c !== null),
  );
}

/** Bring a panel to the front, opening it if it is closed. */
function showOrFocus(id: PanelId): void {
  if (dock !== null) (dock.getPanel(id) ?? showPanel(dock, id))?.api.setActive();
}

/** Settings: from the top bar's button and the command palette. */
function openSettingsSheet(): void {
  const kit = activeKit();
  openSettings({
    projectOpen: () => getState().kind === "open" && dock !== null && kit !== null,
    closedPanels: () => (dock === null || kit === null ? [] : closedPanels(dock, kit).map((id) => ({ id, title: PANELS[id].title }))),
    showPanel: (id) => showOrFocus(id as PanelId),
    kitName: kit?.name ?? "the kit",
    resetLayout: () => {
      if (dock !== null && kit !== null) layoutKit(dock, kit);
    },
    tabsAlways: () => tabMode(localStorage) === "always",
    setTabsAlways: (on) => {
      setTabMode(localStorage, on ? "always" : "reach");
      host.classList.toggle("tabs-always", on);
    },
    visitsOn: () => visitsOn(localStorage),
    setVisitsOn: (on) => {
      setVisitsOn(localStorage, on);
      // Turned on, the wug comes to say hello once the sheet is closed.
      visitAfterSettings = on;
      if (!on) visits?.stop();
    },
    checkForUpdates: () => checkForUpdates(true),
    autoChecks,
    setAutoChecks,
    version: () => appVersion,
    changed: () => renderTop(),
    closed: () => {
      if (visitAfterSettings) window.setTimeout(() => visits?.visitNow(), 400);
      visitAfterSettings = false;
    },
  });
}

/** Feedback: from the top bar's button and the command palette. */
function openFeedbackForm(): void {
  const s = getState();
  const kit = activeKit();
  openFeedback({
    kit: kit === null ? null : `${kit.id} ${kit.version}`,
    theme: savedTheme(localStorage),
    pattern: savedPattern(localStorage),
    projectOpen: s.kind === "open",
    window: `${window.innerWidth}×${window.innerHeight} at ${window.devicePixelRatio}x`,
    webview: navigator.userAgent,
  });
}

/** The kit's name in the top bar is the kit switcher (roadmap 73): one click
 * between Tongues, Scrolls and any installed kit, over the same project. */
function kitSwitcher(): HTMLElement {
  const kit = activeKit();
  const all = getKits()?.kits ?? [];
  const label = kit?.name ?? "wugs";
  const menu = menuButton(
    label,
    () => [
      { heading: "Switch kit" },
      ...all.map((k) => ({
        label: k.kit.subtitle === "" ? k.kit.name : `${k.kit.name} · ${k.kit.subtitle}`,
        radio: true,
        checked: k.kit.id === kit?.id,
        run: () => {
          if (k.kit.id !== kit?.id) void switchKit(k.kit.id);
        },
      })),
      "separator",
      { label: "Manage kits…", run: () => showOrFocus("kits") },
    ],
    { className: "kit-switcher", key: "kit", title: `${label}: switch kit (same project, other tools)` },
  );
  return h("span", { className: "brand-kit" }, menu, h("span", { className: "subtitle", textContent: kit?.subtitle ?? "" }));
}

async function switchKit(id: string) {
  // Another kit may not offer the Media panel, which holds a take's Stop and Save.
  if (closeBlocker() !== null) {
    showToast({ tone: "error", lines: ["A recording is in progress. Stop and save it before switching kits."] });
    return;
  }
  await attempt(async () => {
    await useKit(id, false);
    const now = activeKit();
    if (now === null) return;
    const same = getState().kind === "open" ? " Your project and its data are the same; only the tools changed." : "";
    showToast({ tone: "info", lines: [`Now in ${now.name}.${same}`] });
  });
}

/** The shell's own commands for the palette: panels, look, project, help. */
function shellCommands(): Command[] {
  const kit = activeKit();
  const opened = getState().kind === "open";
  const noProject = () => (opened ? null : "Open a project first");
  const panels: Command[] =
    kit === null
      ? []
      : (Object.keys(PANELS) as PanelId[])
          .filter((id) => offers(kit, id))
          .map((id) => ({ id: `panel:${id}`, title: `Show ${PANELS[id].title}`, group: "Panels", words: ["open", "go to", "panel", "tab"], disabled: noProject, run: () => showOrFocus(id) }));
  return [
    ...panels,
    ...(getKits()?.kits ?? [])
      .filter((k) => k.kit.id !== kit?.id)
      .map((k) => ({ id: `kit:${k.kit.id}`, title: `Switch to ${k.kit.name}`, group: "Kits", words: ["kit", k.kit.subtitle, "workspace"], run: () => void switchKit(k.kit.id) })),
    { id: "app:settings", title: "Open Settings", group: "wugs", words: ["preferences", "options", "theme", "pattern"], run: openSettingsSheet },
    { id: "app:shortcuts", title: "Show keyboard shortcuts", group: "Help", keys: "?", words: ["keys", "hotkeys"], run: openShortcuts },
    { id: "app:feedback", title: "Send feedback", group: "Help", words: ["bug", "problem", "report", "contact"], run: openFeedbackForm },
    { id: "app:reset-layout", title: "Reset layout", group: "Workspace", words: ["arrange", "panels", "default"], disabled: noProject, run: () => {
      if (dock !== null && kit !== null) layoutKit(dock, kit);
    } },
    { id: "app:tabs-always", title: tabMode(localStorage) === "always" ? "Hide panel tabs until reached for" : "Always show panel tabs", group: "Workspace", run: () => {
      const on = tabMode(localStorage) !== "always";
      setTabMode(localStorage, on ? "always" : "reach");
      host.classList.toggle("tabs-always", on);
    } },
    ...THEMES.map((t) => ({ id: `theme:${t.id}`, title: `Colours: ${t.label}`, group: "Look", words: ["theme", t.note], run: () => chooseTheme(localStorage, t.id) })),
    ...PATTERNS.map((p) => ({ id: `pattern:${p.id}`, title: `Pattern: ${p.label}`, group: "Look", words: ["background", p.note], run: () => choosePattern(localStorage, p.id) })),
    { id: "project:open", title: "Open another project…", group: "Project", words: ["folder"], run: () => void pickProject("open") },
    { id: "project:create", title: "Create a project…", group: "Project", words: ["new"], run: () => void pickProject("create") },
    { id: "project:sample", title: "Open the sample project", group: "Project", words: ["example", "demo", "try", "practice"], run: () => void enterSample() },
    { id: "app:tour", title: "Take the tour", group: "Help", words: ["introduction", "guide", "start", "learn"], disabled: () => noProject() ?? (kit !== null && !kit.tools.includes("timeline") ? `The tour shows Tongues; ${kit.name} has no Timeline` : null), run: () => startTour((id) => showOrFocus(id as PanelId)) },
    { id: "app:updates", title: "Check for updates", group: "wugs", words: ["version", "upgrade"], run: () => void checkForUpdates(true) },
  ];
}

function renderWelcome() {
  const showWelcome = !joining && getState().kind !== "open";
  if (getState().kind === "open") joining = false;
  // Hidden, never removed from layout: dockview sizes its panels from the host, and a host of zero size loses their proportions.
  host.classList.toggle("dock-veiled", booting || showWelcome);
  document.body.classList.toggle("welcome-on", showWelcome);
  if (!showWelcome) {
    welcomeSlot.replaceChildren();
    return;
  }
  const last = lastProject(localStorage);
  if (booting) {
    // Opening the last project can take a while (a cache rebuild): say so.
    welcomeSlot.replaceChildren(
      last === "" || researcher() === "" ? "" : h("div", { className: "welcome-loading" }, wugArt("welcome-wug", ""), h("p", { textContent: `Reopening ${folderName(last)}…` })),
    );
    return;
  }
  // Typing in the id field must not be redrawn under the person.
  if (welcomeSlot.contains(document.activeElement) && welcomeSlot.querySelector(".welcome-card") !== null) return;
  welcomeSlot.replaceChildren(
    welcomeView(activeKit(), {
      researcher: researcher(),
      setResearcher,
      create: () => void pickProject("create"),
      open: () => void pickProject("open"),
      sample: () => void enterSample(),
      reopen:
        last === ""
          ? null
          : {
              label: `Reopen ${folderName(last)}`,
              run: () => {
                if (refuseProjectChange()) return;
                if (researcher() === "") return showToast({ tone: "error", lines: ["Enter your researcher id first."] });
                void attempt(async () => enterProject(await openProject({ path: last, researcher: researcher() })));
              },
            },
      join: () => {
        if (researcher() === "") {
          showToast({ tone: "error", lines: ["Enter your researcher id first: the project you join signs your edits with it."] });
          document.querySelector<HTMLInputElement>(".welcome-researcher")?.focus();
          return;
        }
        joining = true;
        renderTop();
        renderWelcome();
        if (dock !== null) (dock.getPanel("devices") ?? showPanel(dock, "devices"))?.api.setActive();
      },
    }),
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

/** Open the sample project, show its text, and give the tour the first time. */
async function enterSample() {
  if (refuseProjectChange()) return;
  await attempt(async () => {
    await enterProject(await openSample({ researcher: researcher() }), false);
    const s = getState();
    const first = s.kind === "open" ? s.texts[0] : undefined;
    if (first !== undefined) await selectText(first.id);
    // The first utterance selected, so Transcribe and Gloss show it at once.
    const now = getState();
    if (now.kind === "open" && now.text !== null) {
      const text = now.text;
      const seg = text.segments.find((x) => text.tiers.some((t) => t.id === x.tier && t.utterance === true) && x.content !== "");
      if (seg !== undefined) selectSegment({ textId: text.id, tierId: seg.tier, segmentId: seg.id });
    }
    if (!tourDone(localStorage)) window.setTimeout(() => startTour((id) => showOrFocus(id as PanelId)), 600);
  });
}

async function reopenAs(path: string) {
  if (refuseProjectChange()) return;
  // The same project reopens with the same id: read who it signs as afresh,
  // but only once it really reopened (a failed open keeps the old id shown).
  await attempt(async () => {
    const info = await openProject({ path, researcher: researcher() });
    signedFor = "";
    await enterProject(info);
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
  // The welcome covers the dock inside one stage, below the update notice and banner.
  app.replaceChildren(top, updateSlot(), bannerSlot, h("div", { className: "stage" }, host, welcomeSlot), toastSlot, dropTarget());
  host.classList.add("dock-veiled");
  subscribe(renderTop);
  subscribe(renderBanner);
  // A panel asking for another (Frequencies opens the Concordance).
  window.addEventListener(SHOW_PANEL, (e) => {
    const id = (e as CustomEvent<string>).detail;
    const kit = activeKit();
    if (!isPanelId(id)) return;
    const title = PANELS[id].title;
    if (kit !== null && !offers(kit, id)) {
      // A recording asked for on a Timeline this kit lacks must not wait for a later one.
      if (id === "timeline") dropWantedMedia();
      showToast({ tone: "info", lines: [`${kit.name} has no ${title} panel. Add it in the Kits panel, or switch kits.`] });
      return;
    }
    showOrFocus(id);
  });
  // Ctrl+K finds any action; ? lists the keys (roadmap 70).
  provide("shell", shellCommands);
  wirePalette();
  subscribe(renderWelcome);
  renderWelcome();
  onDevicesChange(renderBanner);
  subscribeToast(renderToast);
  renderTop();
  void (async () => {
    try {
      // The bundled kit is compiled in and an unreadable kits folder is
      // reported as a broken entry, so there is always an active kit.
      setKits(await kitsList());
      const mounted = await mountDock(host, activeKit()!);
      attachDock(mounted);
      dock = mounted.api;
      setBeforeInstall(mounted.flush);
      dock.onDidLayoutChange(renderTop);
      visits = wireVisits({
        logo: () => top.querySelector(".brand-wug"),
        avoid: () => [...top.querySelectorAll(".menu-wrap, .project-chip, .brand-mark, .brand")],
        onLayout: (run) => mounted.api.onDidLayoutChange(run),
        busy: () => closeBlocker() !== null || top.querySelector(".menu-list") !== null || document.querySelector(".settings-overlay") !== null || getState().kind !== "open",
      });
      renderTop();
      await wireDeviceEvents();
      await wireAssist();
      // A reload leaves the old page's captures open on the Rust side; this page owns none.
      await captureReset().catch(() => 0);
      await reopenLast();
      // After the window is up and the project is back; never awaited by anything.
      void checkForUpdates(false);
      // Feedback written offline goes once wugs can reach the internet.
      void sendWaitingFeedback();
      window.addEventListener("online", () => void sendWaitingFeedback());
    } finally {
      booting = false;
      renderWelcome();
    }
  })();
}

/** Send feedback that waited for a connection, quietly. */
function sendWaitingFeedback() {
  return feedbackRetry().catch(() => null);
}
