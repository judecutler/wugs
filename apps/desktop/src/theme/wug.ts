// Wugs drawn from the owner's layered parts (design/wugs/, traced by
// tools/brand/trace_wugs.py into wug-parts.json). A look is a body colour
// plus accessories; each accessory is a set of parts, and parts are always
// stacked in the drawing's own order, so any combination lines up.
import drawing from "./wug-parts.json" with { type: "json" };

type Part = { id: string; kind: "shape" | "line"; fill: string; opacity: number; d: string };
const PARTS: readonly Part[] = drawing.parts as Part[];

/** One accessory per slot: a second hat replaces the first. */
export type Slot = "head" | "suit" | "face" | "held" | "badge";

export const ACCESSORIES = {
  hat: { slot: "head", label: "Explorer hat", parts: ["hat-shape", "hat-line"] },
  crown: { slot: "head", label: "Crown", parts: ["crown-shape", "crown-line"] },
  antennae: { slot: "head", label: "Antennae", parts: ["antenna-left-shape", "antenna-left-line", "antenna-right-shape", "antenna-right-line"] },
  spacesuit: { slot: "suit", label: "Space helmet", parts: ["spacesuit-base-shape", "spacesuit-mask-shape", "spacesuit-line"] },
  monocle: { slot: "face", label: "Monocle", parts: ["monocle-shape", "monocle-line"] },
  glasses: { slot: "face", label: "Glasses", parts: ["glasses-shape", "glasses-line"] },
  // The recorder reuses the paper's white fill under its own body.
  recorder: { slot: "held", label: "Recorder", parts: ["paper-shape", "recorder-shape", "recorder-line"] },
  scroll: { slot: "held", label: "Notes", parts: ["paper-shape", "paper-line", "paper-text-line"] },
  tablet: { slot: "held", label: "Glyph tablet", parts: ["paper-shape", "paper-line", "paper-glyph-line"] },
  "number-one": { slot: "badge", label: "Number one", parts: ["first-line"] },
} as const satisfies Record<string, { slot: Slot; label: string; parts: readonly string[] }>;

export type AccessoryId = keyof typeof ACCESSORIES;

export type WugLook = {
  /** The body's fill. */
  body: string;
  accessories: readonly AccessoryId[];
  /** Fill overrides for an accessory's shapes, by part id (`hat-shape`). */
  colours?: Readonly<Record<string, string>>;
};

/** The brand wug, and one per built-in kit, as in the brand sheet. */
export const BRAND_BLUE = "#00c1e3";
export const KIT_WUGS: Readonly<Record<string, WugLook>> = {
  wugs: { body: BRAND_BLUE, accessories: [] },
  tongues: { body: "#f08060", accessories: ["hat", "recorder"] },
  scrolls: { body: "#e8b030", accessories: ["monocle", "scroll"] },
  glyphs: { body: "#70b850", accessories: ["glasses", "tablet"] },
};

/** A kit's wug: its own look when built in, else the brand wug. */
export const kitWug = (kitId: string | undefined): WugLook => KIT_WUGS[kitId ?? ""] ?? KIT_WUGS.wugs!;

export type Layer = { id: string; d: string; fill: string; opacity: number };

/** The layers to draw, bottom to top. */
export function wugLayers(look: WugLook): Layer[] {
  const bySlot = new Map<Slot, AccessoryId>();
  for (const a of look.accessories) bySlot.set(ACCESSORIES[a].slot, a);
  const wanted = new Set<string>(["body-shape", "body-line", "eye-shape", "eye-line"]);
  for (const a of bySlot.values()) for (const p of ACCESSORIES[a].parts) wanted.add(p);
  return PARTS.filter((p) => wanted.has(p.id)).map((p) => ({
    id: p.id,
    d: p.d,
    fill: p.id === "body-shape" ? look.body : p.kind === "shape" ? (look.colours?.[p.id] ?? p.fill) : p.fill,
    opacity: p.opacity,
  }));
}

const attr = (s: string) => s.replace(/[&"<]/g, (c) => ({ "&": "&amp;", '"': "&quot;", "<": "&lt;" })[c]!);

/** The wug as SVG markup, square, scaled by its container. */
export function wugSvg(look: WugLook, label = "wug"): string {
  const paths = wugLayers(look)
    .map((l) => `<path fill="${attr(l.fill)}"${l.opacity === 1 ? "" : ` fill-opacity="${l.opacity}"`} d="${l.d}"/>`)
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${drawing.size} ${drawing.size}" role="img" aria-label="${attr(label)}">${paths}</svg>`;
}
