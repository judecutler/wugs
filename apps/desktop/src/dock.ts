// The dock: dockview hosts the panels. A kit's manifest says how the
// workspace starts; the person's rearrangement of it is saved as
// dockview's toJSON() in app data, one file per kit.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { loadLayout, saveLayout } from "./api";
import { createDockview, themeLight, type DockviewApi, type IContentRenderer } from "./dockview";
import { addsFor, type Kit } from "./kits/logic";
import { isPanelId, isPlatformPanel, PANELS, type PanelId } from "./panels/registry";
import { closeBlocker } from "./capture/session.ts";
import { attempt, showToast, subscribe } from "./store";

const SAVE_DELAY_MS = 400;

function renderer(id: PanelId): IContentRenderer {
  const view = PANELS[id].create();
  let unsubscribe = () => {};
  return {
    element: view.element,
    init() {
      unsubscribe = subscribe(() => view.render());
      view.render();
    },
    dispose() {
      unsubscribe();
      view.dispose?.();
    },
  };
}

/** Whether `kit` offers the panel: a tool it turns on, or a platform panel. */
export const offers = (kit: Kit, panel: string): panel is PanelId => isPanelId(panel) && (isPlatformPanel(panel) || kit.tools.includes(panel));

/** Lay the workspace out as the kit's manifest says. A kit that places
 * nothing this version has opens on the Kits panel, never an empty dock. */
export function layoutKit(api: DockviewApi, kit: Kit) {
  api.clear();
  const adds = addsFor(kit.layout, (p) => offers(kit, p));
  if (adds.length === 0) adds.push({ id: "kits", inactive: false });
  for (const a of adds) {
    const id = a.id as PanelId;
    api.addPanel({ id, component: id, title: PANELS[id].title, position: a.position, initialWidth: a.initialWidth, initialHeight: a.initialHeight, inactive: a.inactive });
  }
  for (const id of kit.active) api.getPanel(id)?.api.setActive();
}

export type Dock = {
  api: DockviewApi;
  /** Save this kit's arrangement, then show `kit`'s: its saved one, or its manifest's. */
  switchKit(kit: Kit): Promise<void>;
  /** Write a pending arrangement change now (before an update restarts the app). */
  flush(): Promise<void>;
};

/** Restore the kit's saved arrangement (or its manifest's) and save every change after. */
export async function mountDock(host: HTMLElement, kit: Kit): Promise<Dock> {
  const api = createDockview(host, {
    theme: themeLight,
    createComponent: ({ name }) => {
      if (!isPanelId(name)) throw new Error(`no panel named ${name}`);
      return renderer(name);
    },
  });
  let kitId = kit.id;
  const restore = async (k: Kit) => {
    let saved: string | null = null;
    await attempt(async () => {
      saved = await loadLayout({ kit: k.id });
    });
    try {
      if (saved === null) layoutKit(api, k);
      else api.fromJSON(JSON.parse(saved));
    } catch {
      // A layout naming a panel this version lacks, or one dockview cannot read.
      layoutKit(api, k);
    }
    // A saved arrangement from before a tool was turned off still holds it.
    for (const panel of [...api.panels]) if (!offers(k, panel.id)) api.removePanel(panel);
    if (api.panels.length === 0) layoutKit(api, k);
  };
  await restore(kit);
  let timer: number | undefined;
  let pending = false;
  const flush = async () => {
    window.clearTimeout(timer);
    if (!pending) return;
    pending = false;
    const id = kitId;
    await attempt(() => saveLayout({ kit: id, layout: JSON.stringify(api.toJSON()) }));
  };
  api.onDidLayoutChange(() => {
    pending = true;
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void flush(), SAVE_DELAY_MS);
  });
  // A change made just before closing must not wait out the debounce. The
  // close handler holds the window open until the save lands; the backend
  // still writes it by temp file and rename. beforeunload covers a reload.
  // A live or unsaved take keeps the window open; its audio is on disk
  // either way, this only stops a close from leaving it unnamed.
  await getCurrentWindow().onCloseRequested(async (event) => {
    const blocker = closeBlocker();
    if (blocker !== null) {
      event.preventDefault();
      if (api.getPanel("media") === undefined) showPanel(api, "media");
      showToast({ tone: "warn", lines: [blocker] });
      return;
    }
    await flush();
  });
  window.addEventListener("beforeunload", (e) => {
    if (closeBlocker() !== null) e.preventDefault();
    void flush();
  });
  return {
    api,
    flush,
    async switchKit(next: Kit) {
      await flush();
      kitId = next.id;
      await restore(next);
    },
  };
}

/** Panels the kit offers that are closed, for the "show panel" menu. */
export function closedPanels(api: DockviewApi, kit: Kit): PanelId[] {
  return (Object.keys(PANELS) as PanelId[]).filter((id) => offers(kit, id) && api.getPanel(id) === undefined);
}

export function showPanel(api: DockviewApi, id: PanelId) {
  api.addPanel({ id, component: id, title: PANELS[id].title });
}
