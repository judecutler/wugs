// The kit's wug now and then slips out of its corner, peeks over the top edge
// of the panel being worked in, watches the pointer for a few seconds and
// slips back. Touch it and it ducks away and stays away for a long while.
// Rare and quiet by rule: never while typing, dragging or recording, never
// when the person asked for reduced motion, and off with one menu item.
// The rules are pure (`visitDue`, `peekSpot`, `gaze`); `wireVisits` acts.

/** Least time between visits. */
export const VISIT_EVERY_MS = 6 * 60_000;
/** No visit in the first minutes after the app opens. */
export const FIRST_VISIT_AFTER_MS = 2 * 60_000;
/** A scared wug stays away this long. */
export const SCARED_FOR_MS = 30 * 60_000;
/** Typing this recently means the person is busy. */
export const QUIET_AFTER_KEY_MS = 4_000;
/** The person counts as present if the pointer moved this recently. */
export const PRESENT_WITHIN_MS = 45_000;
/** How long a visit lasts once the wug is up. */
export const PEEK_MS = 5_500;

export type VisitState = {
  enabled: boolean;
  reducedMotion: boolean;
  startedAt: number;
  lastVisit: number;
  scaredUntil: number;
  lastKey: number;
  lastPointer: number;
  /** Recording, playing, a mouse button held, a drag, a menu open, the window in the background. */
  busy: boolean;
};

export function visitDue(now: number, s: VisitState): boolean {
  if (!s.enabled || s.reducedMotion || s.busy) return false;
  if (now - s.startedAt < FIRST_VISIT_AFTER_MS) return false;
  if (now < s.scaredUntil) return false;
  if (now - s.lastVisit < VISIT_EVERY_MS) return false;
  if (now - s.lastKey < QUIET_AFTER_KEY_MS) return false;
  return now - s.lastPointer < PRESENT_WITHIN_MS;
}

export type Rect = { left: number; top: number; right: number; bottom: number };

/** Where the wug peeks: over the top edge of the card, as near its right
 * end as it can be without covering anything in `avoid` (the top bar's
 * buttons and menus), away from the left-aligned controls. Null when the
 * card is too small or no spot along its top edge is clear. */
export function peekSpot(card: Rect, size: number, avoid: readonly Rect[] = []): { x: number; y: number } | null {
  if (card.right - card.left < size * 4 || card.bottom - card.top < size * 2) return null;
  if (card.top < size * 0.6) return null;
  const top = card.top - size;
  const clear = (x: number) => avoid.every((r) => r.right <= x || r.left >= x + size || r.bottom <= top || r.top >= card.top);
  for (let x = card.right - size - 28; x >= card.left + 28; x -= size / 2) if (clear(x)) return { x, y: card.top };
  return null;
}

/** Where the eye looks: toward the pointer, at most `reach` from its rest. */
export function gaze(eye: { x: number; y: number }, pointer: { x: number; y: number }, reach: number): { dx: number; dy: number } {
  const dx = pointer.x - eye.x;
  const dy = pointer.y - eye.y;
  const d = Math.hypot(dx, dy);
  if (d < 1) return { dx: 0, dy: 0 };
  const k = Math.min(1, d / 200) * reach;
  return { dx: (dx / d) * k, dy: (dy / d) * k };
}

const KEY = "wugs.wugVisits";
export const visitsOn = (store: Pick<Storage, "getItem">) => store.getItem(KEY) !== "off";
export const setVisitsOn = (store: Pick<Storage, "setItem">, on: boolean) => store.setItem(KEY, on ? "on" : "off");
