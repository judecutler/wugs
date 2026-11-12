// Kit layouts as data. A kit lists placements (src-tauri/src/kits.rs,
// crates/wugs-kit); these functions turn them into dockview additions and
// turn a live dockview arrangement back into placements. Pure, so they run
// under node --test without a DOM.
import type { WugLook } from "../theme/wug.ts";

export type Place = "left" | "right" | "above" | "below" | "tab";

export type Placement = { panel: string; place?: Place; of?: string; width?: number; height?: number };

export type Kit = {
  id: string;
  name: string;
  subtitle: string;
  version: string;
  description: string;
  tools: string[];
  layout: Placement[];
  active: string[];
  /** The kit's `[wug]`, checked by wugs-kit (colours are `#rrggbb`). */
  wug?: WugLook;
  /** The kit as a shareable bundle (docs/design/BUNDLES.md); read only. */
  bundle?: BundleInfo;
};

export type Requirement = {
  kind: string;
  id: string;
  version: string;
  filesSha256: string;
  capability?: string;
  recommended: boolean;
  why: string;
};

export type BundleInfo = {
  license: string;
  source: string;
  authors: string[];
  summary: string;
  requires: Requirement[];
  examples?: string;
};

const KIND_WORDS: Record<string, string> = { model: "Model", dataset: "Dataset", integration: "Integration", plugin: "Plugin" };

/** A bundle in words, a line each, for the Kits panel. */
export function bundleLines(b: BundleInfo): string[] {
  const lines: string[] = [];
  if (b.summary !== "") lines.push(b.summary);
  const who = b.authors.length > 0 ? `By ${b.authors.join(", ")}` : "";
  const licence = b.license !== "" ? `licence ${b.license}` : "";
  if (who !== "" || licence !== "") lines.push([who, licence].filter((x) => x !== "").join("; ") + ".");
  if (b.source !== "") lines.push(`Published at ${b.source}`);
  if (b.requires.length === 0) lines.push("Relies on no other packages.");
  for (const r of b.requires) {
    const role = r.capability !== undefined ? ` for ${r.capability.replace(/-/g, " ")}` : "";
    const rec = r.recommended ? " (recommended; another provider can be chosen)" : "";
    lines.push(`${KIND_WORDS[r.kind] ?? r.kind} ${r.id} ${r.version}${role}${rec}${r.why !== "" ? `: ${r.why}` : ""}`);
  }
  if (b.examples !== undefined) lines.push(`Comes with an example project (${b.examples}).`);
  return lines;
}

export type Direction = "left" | "right" | "above" | "below" | "within";

/** One dockview addPanel call. */
export type Add = {
  id: string;
  position?: { referencePanel: string; direction: Direction };
  initialWidth?: number;
  initialHeight?: number;
  inactive: boolean;
};

const MIN_SIZE = 50;
const MAX_SIZE = 4000;

/** The addPanel calls that lay out `layout`, keeping only panels `offered`
 * accepts. A placement whose anchor was left out joins the first placed
 * panel as a tab, so one unknown panel never drops the ones after it. */
export function addsFor(layout: Placement[], offered: (panel: string) => boolean): Add[] {
  const out: Add[] = [];
  const placed = new Set<string>();
  for (const p of layout) {
    if (!offered(p.panel) || placed.has(p.panel)) continue;
    if (out.length === 0) {
      out.push({ id: p.panel, inactive: false });
    } else if (p.place !== undefined && p.of !== undefined && placed.has(p.of)) {
      const tab = p.place === "tab";
      const direction: Direction = p.place === "tab" ? "within" : p.place;
      out.push({
        id: p.panel,
        position: { referencePanel: p.of, direction },
        ...(p.width !== undefined && (p.place === "left" || p.place === "right") ? { initialWidth: p.width } : {}),
        ...(p.height !== undefined && (p.place === "above" || p.place === "below") ? { initialHeight: p.height } : {}),
        inactive: tab,
      });
    } else {
      out.push({ id: p.panel, position: { referencePanel: out[0]!.id, direction: "within" }, inactive: true });
    }
    placed.add(p.panel);
  }
  return out;
}

/** The parts of dockview's toJSON() this reads. */
export type GridNode = { type: "leaf"; data: { views: string[]; activeView?: string }; size?: number } | { type: "branch"; data: GridNode[]; size?: number };
export type SerializedLayout = { grid: { root: GridNode; orientation: "HORIZONTAL" | "VERTICAL" } };

function anchor(node: GridNode): string | null {
  if (node.type === "leaf") return node.data.views[0] ?? null;
  for (const child of node.data) {
    const a = anchor(child);
    if (a !== null) return a;
  }
  return null;
}

const clampSize = (n: number | undefined) => (n === undefined || !Number.isFinite(n) ? undefined : Math.min(MAX_SIZE, Math.max(MIN_SIZE, Math.round(n))));

/** Placements that rebuild a live arrangement. Siblings are placed level
 * by level, so each split lands in the branch it came from: every sibling
 * of one level is placed beside the previous one before any of them is
 * split the other way. Floating and popped-out groups are left out. */
export function placementsFrom(layout: SerializedLayout): { layout: Placement[]; active: string[] } {
  const first = anchor(layout.grid.root);
  if (first === null) return { layout: [], active: [] };
  const out: Placement[] = [{ panel: first }];
  const active: string[] = [];
  const queue: { node: GridNode; horizontal: boolean }[] = [{ node: layout.grid.root, horizontal: layout.grid.orientation === "HORIZONTAL" }];
  while (queue.length > 0) {
    const { node, horizontal } = queue.shift()!;
    if (node.type === "leaf") {
      const [head, ...rest] = node.data.views;
      for (const v of rest) out.push({ panel: v, place: "tab", of: head });
      if (node.data.activeView !== undefined && node.data.activeView !== head) active.push(node.data.activeView);
      continue;
    }
    const children = node.data.filter((c) => anchor(c) !== null);
    let prev = anchor(children[0] ?? node);
    for (const child of children.slice(1)) {
      const a = anchor(child)!;
      const size = clampSize(child.size);
      out.push({
        panel: a,
        place: horizontal ? "right" : "below",
        of: prev!,
        ...(size === undefined ? {} : horizontal ? { width: size } : { height: size }),
      });
      prev = a;
    }
    for (const child of children) queue.push({ node: child, horizontal: !horizontal });
  }
  return { layout: out, active };
}

/** The layout with one panel taken out. The panels placed against it keep
 * their places. A tab's group survives it, so they are placed against that
 * group instead. Otherwise the first one beside it (a tab only if there is
 * none) moves into its spot, taking its place, anchor, size and position in
 * the list, and the others are placed against that heir as they were. */
export function withoutPanel(layout: Placement[], panel: string): Placement[] {
  const at = layout.findIndex((p) => p.panel === panel);
  if (at < 0) return layout;
  const gone = layout[at]!;
  if (gone.place === "tab") return layout.filter((p) => p !== gone).map((p) => (p.of === panel ? { ...p, of: gone.of } : p));
  const dependents = layout.filter((p) => p.of === panel);
  const heir = dependents.find((p) => p.place !== "tab") ?? dependents[0];
  if (heir === undefined) return layout.filter((p) => p !== gone);
  const moved: Placement = { ...gone, panel: heir.panel };
  const out: Placement[] = [];
  for (const p of layout) {
    if (p === gone) out.push(moved);
    else if (p === heir) continue;
    else out.push(p.of === panel ? { ...p, of: heir.panel } : p);
  }
  return out;
}

const PLACE_WORDS: Record<Place, string> = {
  left: "to the left of",
  right: "to the right of",
  above: "above",
  below: "below",
  tab: "as a tab with",
};

/** One line per placement, in words, for the Kit Builder. */
export function describeLayout(layout: Placement[], title: (panel: string) => string): string[] {
  return layout.map((p, i) => {
    if (i === 0 || p.place === undefined || p.of === undefined) return `${title(p.panel)} fills the workspace`;
    const size = p.width ?? p.height;
    return `${title(p.panel)} ${PLACE_WORDS[p.place]} ${title(p.of)}${size === undefined ? "" : `, ${size} px`}`;
  });
}

/** What a kit asks for that this version cannot give, in words. */
export function kitNotes(kit: Kit, known: (panel: string) => boolean, platform: (panel: string) => boolean): string[] {
  const notes: string[] = [];
  const missing = kit.tools.filter((t) => !known(t));
  if (missing.length > 0) notes.push(`Names tools this version does not have: ${missing.join(", ")}.`);
  const unplaceable = kit.layout.map((p) => p.panel).filter((p) => known(p) && !platform(p) && !kit.tools.includes(p));
  if (unplaceable.length > 0) notes.push(`Places panels the kit does not turn on, so they stay closed: ${unplaceable.join(", ")}.`);
  const unknownPlaced = kit.layout.map((p) => p.panel).filter((p) => !known(p));
  if (unknownPlaced.length > 0) notes.push(`Places panels this version does not have: ${unknownPlaced.join(", ")}.`);
  return notes;
}
