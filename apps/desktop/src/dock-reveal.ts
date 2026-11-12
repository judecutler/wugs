// Binder-divider tabs: each panel group's tab strip rests as a slim lip and
// rises when the pointer comes near the group's top edge, when a tab has
// keyboard focus, or while a panel is being dragged, so the panels get the
// height and the tabs are still there the moment someone reaches for them.
// The rule for "near" is pure (`revealIndex`); `wireReveal` applies it.

export type Box = { left: number; right: number; top: number; bottom: number };

/** How far above a group's top edge, and how far into it, the pointer reveals
 * its tabs: the gap above and the 9 px lip, never the card's content, so a
 * click aimed at a panel's first row is never taken by a tab. */
export const REACH_ABOVE = 10;
export const REACH_BELOW = 9;
/** Risen tabs push the group's content down (owner, 2026-11-12), so once up
 * they stay up while the pointer is anywhere in that group: lowering them
 * there would slide the content under the pointer, and a click aimed at the
 * first row would land on the row below. They lower once the pointer leaves
 * the group, where nothing under it moves. */
/** How long tabs stay up after the pointer leaves, so a slow hand never chases them. */
export const LINGER_MS = 350;
/** How long the pointer must rest in reach before tabs rise, so a hand passing
 * through the top edge on its way into a panel never raises them. */
export const DWELL_MS = 140;

/** The group whose tab strip the pointer is reaching for, or null. `shown`
 * is the group whose tabs are up now: its reach extends over the risen strip. */
export function revealIndex(x: number, y: number, boxes: readonly Box[], shown: number | null = null): number | null {
  let best: number | null = null;
  let bestDist = Infinity;
  boxes.forEach((b, i) => {
    if (x < b.left || x > b.right) return;
    const d = y - b.top;
    if (d < -REACH_ABOVE || (i === shown ? y > b.bottom : d > REACH_BELOW)) return;
    // The shown group's whole height counts, but a nearer top edge still wins.
    const dist = i === shown && d > REACH_BELOW ? REACH_BELOW + 1 : Math.abs(d);
    if (dist < bestDist) {
      bestDist = dist;
      best = i;
    }
  });
  return best;
}

export type TabMode = "reach" | "always";
const MODE_KEY = "wugs.dock.tabs";

export function tabMode(store: Pick<Storage, "getItem">): TabMode {
  return store.getItem(MODE_KEY) === "always" ? "always" : "reach";
}

export function setTabMode(store: Pick<Storage, "setItem">, mode: TabMode) {
  store.setItem(MODE_KEY, mode);
}

/** Raise a group's tabs while the pointer reaches for them. Returns an unwire. */
export function wireReveal(host: HTMLElement): () => void {
  let shown: Element | null = null;
  // Two timers, never mixed up: one raises a group after a rest in reach,
  // the other lowers the shown group after the pointer has gone.
  let pending: Element | null = null;
  let rise: number | undefined;
  let fall: number | undefined;
  const cancelRise = () => {
    window.clearTimeout(rise);
    pending = null;
  };
  const show = (group: Element | null) => {
    if (group !== null) window.clearTimeout(fall);
    if (group === shown) {
      cancelRise();
      return;
    }
    if (group === null) {
      cancelRise();
      window.clearTimeout(fall);
      fall = window.setTimeout(() => {
        shown?.classList.remove("dv-reveal");
        shown = null;
      }, LINGER_MS);
      return;
    }
    if (shown !== null) {
      // Moving from one risen strip to another: no wait.
      cancelRise();
      shown.classList.remove("dv-reveal");
      group.classList.add("dv-reveal");
      shown = group;
      return;
    }
    if (pending === group) return;
    cancelRise();
    pending = group;
    rise = window.setTimeout(() => {
      pending = null;
      group.classList.add("dv-reveal");
      shown = group;
    }, DWELL_MS);
  };
  const onMove = (e: PointerEvent) => {
    const groups = [...host.querySelectorAll(".dv-groupview")];
    const now = shown === null ? -1 : groups.indexOf(shown);
    const i = revealIndex(
      e.clientX,
      e.clientY,
      groups.map((g) => {
        const r = g.getBoundingClientRect();
        return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
      }),
      now < 0 ? null : now,
    );
    show(i === null ? null : groups[i]!);
  };
  const onLeave = () => show(null);
  // A tab drag (dockview's own HTML5 drag) shows every strip, so any group can take the drop.
  const onDragStart = (e: DragEvent) => {
    const from = e.target as Element | null;
    // Only a tab drags (owner, 2026-11-12): the strip's empty stretch is not a
    // handle for the whole group. It stays hit-testable, because it is also
    // where a tab dropped "at the end of this group" lands.
    if (from?.closest?.(".dv-void-container") != null) {
      e.preventDefault();
      return;
    }
    if (from?.closest?.(".dv-tab, .dv-tabs-and-actions-container") != null) host.classList.add("dv-dragging-tab");
  };
  const onDragEnd = () => host.classList.remove("dv-dragging-tab");
  host.addEventListener("pointermove", onMove);
  host.addEventListener("pointerleave", onLeave);
  document.addEventListener("dragstart", onDragStart, true);
  document.addEventListener("dragend", onDragEnd, true);
  document.addEventListener("drop", onDragEnd, true);
  return () => {
    host.removeEventListener("pointermove", onMove);
    host.removeEventListener("pointerleave", onLeave);
    document.removeEventListener("dragstart", onDragStart, true);
    document.removeEventListener("dragend", onDragEnd, true);
    document.removeEventListener("drop", onDragEnd, true);
  };
}

export type Span = { left: number; right: number };

/** The groups at the workspace's left and right edges, which carry the side
 * panels' pattern. A group that spans the whole width (one column) is not a
 * side, so a single-column workspace has none. */
export function sideIndexes(groups: readonly Span[], slack = 2): number[] {
  if (groups.length === 0) return [];
  const left = Math.min(...groups.map((g) => g.left));
  const right = Math.max(...groups.map((g) => g.right));
  return groups.flatMap((g, i) => {
    const atLeft = g.left - left <= slack;
    const atRight = right - g.right <= slack;
    return atLeft !== atRight ? [i] : [];
  });
}

/** Mark the side groups (`dv-side`) whenever the layout or the window changes. */
export function wireSides(host: HTMLElement, onLayout: (run: () => void) => void) {
  const mark = () => {
    const groups = [...host.querySelectorAll(".dv-groupview")].filter((g) => g.closest(".dv-floating, .dv-resize-container") === null);
    const sides = new Set(sideIndexes(groups.map((g) => g.getBoundingClientRect())));
    groups.forEach((g, i) => g.classList.toggle("dv-side", sides.has(i)));
  };
  // Many layout events can arrive in one frame (a sash drag): mark once per frame.
  let queued = false;
  const later = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      mark();
    });
  };
  onLayout(later);
  new ResizeObserver(later).observe(host);
  later();
}
