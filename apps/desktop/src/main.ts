import { dropTarget } from "./drop/target";
import { kitWug, setCurrentWug } from "./theme/wug";
import { applyEmphasisPattern, applyPattern, applyTheme, chooseEmphasisPattern, choosePattern, chooseTheme, emphasisPatternOn, PATTERNS, savedPattern, savedTheme, THEMES } from "./theme/theme";

applyTheme(savedTheme(localStorage));
applyPattern(savedPattern(localStorage));
applyEmphasisPattern(emphasisPatternOn(localStorage));
import { open as pickDirectory } from "@tauri-apps/plugin-dialog";
import { captureReset, createProject, errorText, kitsList, onAssistDownload, openProject } from "./api";
import { reloadModels } from "./panels/assist";
import { closeBlocker, refuseProjectChange } from "./capture/session.ts";
import { closedPanels, layoutKit, mountDock, showPanel } from "./dock";
import { setTabMode, tabMode } from "./dock-reveal";
import type { DockviewApi } from "./dockview";
import { h, wugArt } from "./dom";
import { menuButton } from "./menu";
import { wireVisits, type Visits } from "./wug-peek";
import { setVisitsOn, visitsOn } from "./wug-visit";
import { autoOpen, lastProject } from "./last-project";
import { folderName, welcomeView } from "./welcome";
import { onDevicesChange, relocationBanner, wireDeviceEvents } from "./panels/devices";
import { PANELS, type PanelId } from "./panels/registry";
import { attachDock } from "./kits/workspace";
import { autoChecks, checkForUpdates, setAutoChecks, setBeforeInstall, updateSlot } from "./updates/notice";
import { activeKit, assistModelList, attempt, enterProject, getState, getToast, researcher, setAssistModels, setKits, setResearcher, showToast, subscribe, subscribeToast } from "./store";

const top = h("header", { className: "top" });
/** A redraw held back while a menu was open (renderTop never rebuilds under one). */
let topPending = false;
top.addEventListener("menu-closed", (e) => {
  if (!topPending) return;
  const { label, focus } = (e as CustomEvent<{ label: string; focus: boolean }>).detail;
  queueMicrotask(() => {
    renderTop();
    // The rebuilt bar has new buttons: give focus back to the menu it came from.
    if (focus) [...top.querySelectorAll<HTMLButtonElement>(".menu-button")].find((b) => b.textContent === label)?.focus();
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
  const focusedMenu = top.contains(document.activeElement) && document.activeElement?.classList.contains("menu-button") ? document.activeElement.textContent : null;
  queueMicrotask(() => {
    if (focusedMenu !== null && !top.contains(document.activeElement)) [...top.querySelectorAll<HTMLButtonElement>(".menu-button")].find((b) => b.textContent === focusedMenu)?.focus();
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
  const tabsAlways = tabMode(localStorage) === "always";
  const showOrFocus = (id: PanelId) => {
    if (dock !== null) (dock.getPanel(id) ?? showPanel(dock, id))?.api.setActive();
  };
  const project = menuButton("Project", () => [
    { label: "Open another project…", run: () => void pickProject("open") },
    { label: "Create a project…", run: () => void pickProject("create") },
  ]);
  // One menu for the workspace: bring back a closed panel, how tabs show, reset.
  const workspace = menuButton(
    "Workspace",
    () => {
      const closed = dock === null || kit === null ? [] : closedPanels(dock, kit);
      return [
        ...(closed.length === 0 ? [] : [{ heading: "Show a closed panel" }, ...closed.map((id) => ({ label: PANELS[id].title, run: () => showOrFocus(id) }))]),
        ...(closed.length === 0 ? [] : ["separator" as const]),
        { heading: "Theme" },
        ...THEMES.map((t) => ({
          label: t.label,
          note: t.note,
          radio: true,
          checked: savedTheme(localStorage) === t.id,
          run: () => chooseTheme(localStorage, t.id),
        })),
        "separator" as const,
        { heading: "Patterns" },
        ...PATTERNS.map((p) => ({
          label: p.label,
          note: p.note,
          radio: true,
          checked: savedPattern(localStorage) === p.id,
          run: () => choosePattern(localStorage, p.id),
        })),
        {
          label: "Pattern on emphasised sections",
          note: "Adds the family's third pattern to the tier gutter, the IPA palette and Import and export",
          checked: emphasisPatternOn(localStorage),
          disabled: savedPattern(localStorage) === "none",
          run: () => {
            chooseEmphasisPattern(localStorage, !emphasisPatternOn(localStorage));
            renderTop();
          },
        },
        "separator" as const,
        {
          label: "Let the wug visit",
          checked: visitsOn(localStorage),
          note: "Now and then it peeks over the panel you are in. Touch it and it hides.",
          run: () => {
            const on = !visitsOn(localStorage);
            setVisitsOn(localStorage, on);
            if (on) window.setTimeout(() => visits?.visitNow(), 400);
            else visits?.stop();
            renderTop();
          },
        },
        {
          label: "Always show panel tabs",
          checked: tabsAlways,
          note: "Otherwise they rise when you reach for a panel's top edge",
          run: () => {
            setTabMode(localStorage, tabsAlways ? "reach" : "always");
            host.classList.toggle("tabs-always", !tabsAlways);
            renderTop();
          },
        },
        {
          label: `Reset to how ${kit?.name ?? "the kit"} starts`,
          run: () => {
            if (dock !== null && kit !== null) layoutKit(dock, kit);
            renderTop();
          },
        },
      ];
    },
    { className: "panel-menu", title: "Arrange the workspace" },
  );
  // Online. Per machine; checking never downloads anything.
  const updates = menuButton("Updates", () => [
    { label: "Check for updates now", note: "Online: asks GitHub Releases", run: () => void checkForUpdates(true) },
    {
      label: "Check when wugs starts",
      checked: autoChecks(),
      note: "Online, a few kB; nothing downloads until you say so",
      run: () => {
        setAutoChecks(!autoChecks());
        renderTop();
      },
    },
  ]);
  project.classList.add("project-menu");
  setCurrentWug(kitWug(kit?.id));
  const conflicts = s.kind === "open" ? s.info.conflicts : 0;
  top.replaceChildren(
    ...[
    h("span", { className: "brand-mark" }, (() => {
      const art = wugArt("brand-wug", `${kit?.name ?? "wugs"} wug`);
      // Out on a visit: a redraw must not put a second wug in the corner.
      if (visits?.away()) art.classList.add("wug-away");
      return art;
    })()),
    h("strong", { className: "brand", textContent: kit?.name ?? "wugs", title: kit === null ? "wugs" : `wugs: ${kit.name}${kit.subtitle === "" ? "" : `, ${kit.subtitle.toLowerCase()}`}` }),
    h("span", { className: "subtitle", textContent: kit?.subtitle ?? "" }),
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
    h("span", { className: "project" }, project, workspace, updates),
    ].filter((c) => c !== null),
  );
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
        busy: () => closeBlocker() !== null || top.querySelector(".menu-list") !== null || getState().kind !== "open",
      });
      renderTop();
      await wireDeviceEvents();
      await wireAssist();
      // A reload leaves the old page's captures open on the Rust side; this page owns none.
      await captureReset().catch(() => 0);
      await reopenLast();
      // After the window is up and the project is back; never awaited by anything.
      void checkForUpdates(false);
    } finally {
      booting = false;
      renderWelcome();
    }
  })();
}
