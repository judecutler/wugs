// The bridge between the Kits panel and the dock: main.ts attaches the dock
// once it is mounted, and the panel switches kits and reads the live
// arrangement through here.
import { kitSetActive, kitsList } from "../api";
import { offers, showPanel, type Dock } from "../dock";
import { activeKit, setKits } from "../store";
import { placementsFrom, type Placement, type SerializedLayout } from "./logic";

let dock: Dock | null = null;

export function attachDock(d: Dock) {
  dock = d;
}

/** Fetch the installed kits again. Panels the active kit no longer offers
 * (a tool just turned off) close. */
export async function reloadKits() {
  setKits(await kitsList());
  const kit = activeKit();
  if (dock === null || kit === null) return;
  for (const panel of [...dock.api.panels]) {
    if (!offers(kit, panel.id)) dock.api.removePanel(panel);
  }
}

/** Make `id` the kit in use and show its workspace. The Kits panel, where
 * the switch was made, stays open in the new workspace. */
export async function useKit(id: string) {
  await kitSetActive({ id });
  setKits(await kitsList());
  const kit = activeKit();
  if (dock === null || kit === null) return;
  await dock.switchKit(kit);
  if (dock.api.getPanel("kits") === undefined) showPanel(dock.api, "kits");
  dock.api.getPanel("kits")?.api.setActive();
}

/** The workspace as it is arranged right now, as kit placements. */
export function currentArrangement(): { layout: Placement[]; active: string[] } | null {
  return dock === null ? null : placementsFrom(dock.api.toJSON() as unknown as SerializedLayout);
}
