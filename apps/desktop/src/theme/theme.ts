// The workspace theme: a named set of token overrides (src/theme/themes.css),
// chosen per machine. Canvas views listen for THEME_EVENT and read their
// colours again, since they cannot follow CSS on their own.

export const THEMES = [
  { id: "cream", label: "Cream", note: "The wugs default", swatches: ["#fff8de", "#00c1e3", "#1b1a17"] },
  { id: "seafoam", label: "Seafoam", note: "Cool and calm", swatches: ["#e6f6f3", "#00c1e3", "#006d7e"] },
  { id: "clay", label: "Clay", note: "Warm, after the Tongues wug", swatches: ["#fbeee4", "#f08060", "#983823"] },
  { id: "night", label: "Night", note: "Dark, for late transcription", swatches: ["#0f2128", "#5ed6ee", "#f3ead2"] },
] as const;

export type ThemeId = (typeof THEMES)[number]["id"];

/** Pattern families: one tile behind everything, a related one across the
 * side panels, a third on emphasised bands (toolbars, gutters, palettes).
 * Tiles are in public/patterns/, drawn by tools/brand/patterns.py: original
 * geometric tiles in the manner of each tradition. */
export const PATTERNS = [
  { id: "persian", label: "Persian", note: "Girih stars, an eslimi vine, small khatam stars", tiles: ["girih", "eslimi", "khatam"] },
  { id: "garden", label: "Persian garden", note: "An eslimi vine, palmettes, khatam stars", tiles: ["eslimi", "palmette", "khatam"] },
  { id: "japanese", label: "Japanese", note: "Asanoha, kikko, shippo", tiles: ["asanoha", "kikko", "shippo"] },
  { id: "sea", label: "Japanese sea", note: "Seigaiha waves, shippo, kikko", tiles: ["seigaiha", "shippo", "kikko"] },
  { id: "rock", label: "Rock art", note: "Spirals and animals, pecked dots, zigzags", tiles: ["petroglyph", "pecked", "zigzag"] },
  { id: "none", label: "No pattern", note: "Plain", tiles: [] },
] as const;
export type PatternId = (typeof PATTERNS)[number]["id"];
const PATTERN_KEY = "wugs.pattern";

export const isPattern = (v: string | null): v is PatternId => PATTERNS.some((p) => p.id === v);

export function savedPattern(store: Pick<Storage, "getItem">): PatternId {
  const v = store.getItem(PATTERN_KEY);
  return isPattern(v) ? v : "persian";
}

export function applyPattern(id: PatternId, root: HTMLElement = document.documentElement) {
  root.dataset.pattern = id;
}

const EMPHASIS_KEY = "wugs.pattern.emphasis";
/** The third tile on emphasised sections: off unless asked for (owner, 2026-11-11). */
export const emphasisPatternOn = (store: Pick<Storage, "getItem">) => store.getItem(EMPHASIS_KEY) === "on";
export function applyEmphasisPattern(on: boolean, root: HTMLElement = document.documentElement) {
  root.dataset.emphasisPattern = on ? "on" : "off";
}
export function chooseEmphasisPattern(store: Pick<Storage, "setItem">, on: boolean) {
  store.setItem(EMPHASIS_KEY, on ? "on" : "off");
  applyEmphasisPattern(on);
}

export function choosePattern(store: Pick<Storage, "setItem">, id: PatternId) {
  store.setItem(PATTERN_KEY, id);
  applyPattern(id);
}
export const THEME_EVENT = "wugs-theme";
const KEY = "wugs.theme";

export const isTheme = (v: string | null): v is ThemeId => THEMES.some((t) => t.id === v);

export function savedTheme(store: Pick<Storage, "getItem">): ThemeId {
  const v = store.getItem(KEY);
  return isTheme(v) ? v : "cream";
}

/** Put a theme on the page (Cream is the stylesheet's own, so no attribute). */
export function applyTheme(id: ThemeId, root: HTMLElement = document.documentElement) {
  if (id === "cream") delete root.dataset.theme;
  else root.dataset.theme = id;
  window.dispatchEvent(new Event(THEME_EVENT));
}

export function chooseTheme(store: Pick<Storage, "setItem">, id: ThemeId) {
  store.setItem(KEY, id);
  applyTheme(id);
}
