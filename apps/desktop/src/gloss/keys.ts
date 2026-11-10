// Keyboard decisions for the Gloss panel, apart from the DOM. The IME gate is
// the Transcribe panel's own: nothing is decided while an input method owns the key.
import { imeBusy, type KeyLike } from "../transcribe/logic.ts";

export type GlossAction = "pick" | "create-lexeme" | "commit-next" | "commit-prev" | "next" | "prev" | "menu-down" | "menu-up" | "dismiss" | "revert" | null;
export type GlossContext = {
  /** The lexicon autocomplete is showing. */
  menuOpen: boolean;
  /** The highlighted option, or -1 when none is. */
  highlighted: number;
  /** "Add «surface» to the lexicon" is showing under the cell. */
  offer: boolean;
};

/** A key in a gloss cell. Enter saves and moves on, unless it picks a match or confirms the lexicon offer.
 * Tab and Shift+Tab move across cells and words, saving a changed cell on the way. */
export function glossKey(ev: KeyLike, composing: boolean, ctx: GlossContext): GlossAction {
  if (imeBusy(ev, composing) || ev.altKey || ev.ctrlKey || ev.metaKey) return null;
  switch (ev.key) {
    case "Enter":
      if (ev.shiftKey) return "commit-prev";
      if (ctx.offer) return "create-lexeme";
      if (ctx.menuOpen && ctx.highlighted >= 0) return "pick";
      return "commit-next";
    case "Tab":
      return ev.shiftKey ? "prev" : "next";
    case "ArrowDown":
      return ctx.menuOpen ? "menu-down" : null;
    case "ArrowUp":
      return ctx.menuOpen ? "menu-up" : null;
    case "Escape":
      return ctx.offer || ctx.menuOpen ? "dismiss" : "revert";
    default:
      return null;
  }
}

export type BreakdownAction = "accept-suggestion" | "apply-next" | "apply-prev" | "next" | "prev" | "revert" | null;
export type BreakdownContext = {
  /** The field is empty. */
  empty: boolean;
  /** The word has a ghost suggestion showing. */
  hasGhost: boolean;
  /** The typed text would change the word's analysis. */
  changed: boolean;
};

/** A key in a word's breakdown field. Enter on an empty field accepts the ghost suggestion: one keypress. */
export function breakdownKey(ev: KeyLike, composing: boolean, ctx: BreakdownContext): BreakdownAction {
  if (imeBusy(ev, composing) || ev.altKey || ev.ctrlKey || ev.metaKey) return null;
  switch (ev.key) {
    case "Enter":
      if (ev.shiftKey) return ctx.changed ? "apply-prev" : "prev";
      if (ctx.empty && ctx.hasGhost) return "accept-suggestion";
      return ctx.changed ? "apply-next" : "next";
    case "Tab":
      return ev.shiftKey ? "prev" : "next";
    case "Escape":
      return "revert";
    default:
      return null;
  }
}

/** Cell ids in reading order: `b:<token>` for a word's breakdown, `g:<morph>` and `l:<morph>` for its gloss cells. */
export const breakdownCell = (token: string): string => `b:${token}`;
export const glossCell = (morph: string): string => `g:${morph}`;
export const langCell = (morph: string): string => `l:${morph}`;

/** The cell a step away, or null past either end (focus then leaves the panel). */
export function nextCell(order: readonly string[], current: string, step: 1 | -1): string | null {
  const i = order.indexOf(current);
  if (i === -1) return null;
  return order[i + step] ?? null;
}

/** The next word's breakdown cell after `current`: where Enter goes once a word is settled. */
export function nextWordCell(order: readonly string[], current: string, step: 1 | -1): string | null {
  const i = order.indexOf(current);
  if (i === -1) return null;
  for (let j = i + step; j >= 0 && j < order.length; j += step) if (order[j]!.startsWith("b:")) return order[j]!;
  return null;
}
