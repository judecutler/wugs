// The first-run tour (roadmap 68): five stops over the sample project, each a
// highlighted panel and a card that says what it is for and one thing to try.
// Skippable at every step, replayable from the command palette.
import { h } from "../dom";

export type Stop = {
  /** The panel to bring forward first, when the stop is about one. */
  panel?: string;
  /** What to highlight. */
  target: string;
  title: string;
  body: string;
};

export const STOPS: Stop[] = [
  {
    panel: "timeline",
    target: ".panel-timeline",
    title: "Hear it and cut it",
    body: "The recording and its segments, like tracks in a video editor. Press Space to play. Drag across a track to make a segment, or click Auto-segment and wugs finds the speech for you.",
  },
  {
    panel: "transcribe",
    target: ".panel-transcribe",
    title: "Type what you hear",
    body: "One row per segment. Enter saves and plays the next one; Tab plays it again. The last segment of the sample is empty: try it, with the IPA palette below.",
  },
  {
    panel: "gloss",
    target: ".panel-gloss",
    title: "Gloss as you go",
    body: "Break a word into morphs (ni-kani) and gloss each one. wugs remembers every analysis and offers it the next time the word comes up. Copy example puts the sentence in your paper, already formatted.",
  },
  {
    panel: "lexicon",
    target: ".panel-lexicon",
    title: "The dictionary builds itself",
    body: "Every root you gloss can go straight into the lexicon, linked back to every place it is used. Add senses, domains and whole paradigms here.",
  },
  {
    target: ".palette-button",
    title: "Find anything",
    body: "Press Ctrl+K (⌘K on a Mac) and type what you want to do: “split”, “night”, “lexicon”. Press ? for every keyboard shortcut. When you are ready, open the Project menu and create a project of your own.",
  },
];

const DONE_KEY = "wugs.tour.done";

export function tourDone(store: Pick<Storage, "getItem">): boolean {
  try {
    return store.getItem(DONE_KEY) === "1";
  } catch {
    return true;
  }
}

function markDone(): void {
  try {
    localStorage.setItem(DONE_KEY, "1");
  } catch {
    // A tour that comes back once more is the worst that happens.
  }
}

let running: (() => void) | null = null;

/** Walk the stops. `show` brings a panel forward by id. */
export function startTour(show: (panel: string) => void): void {
  running?.();
  let at = 0;
  const ring = h("div", { className: "tour-ring" });
  const card = h("div", { className: "tour-card", role: "dialog", ariaLabel: "Tour" });
  const layer = h("div", { className: "tour-layer" }, ring, card);
  const back = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const end = () => {
    markDone();
    window.removeEventListener("keydown", keys, true);
    window.removeEventListener("resize", place);
    layer.remove();
    running = null;
    if (back?.isConnected) back.focus();
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopImmediatePropagation();
      end();
    } else if (e.key === "ArrowRight") go(at + 1);
    else if (e.key === "ArrowLeft") go(at - 1);
  };
  function place(): void {
    const stop = STOPS[at];
    if (stop === undefined) return;
    const el = document.querySelector<HTMLElement>(stop.target);
    const r = el?.getBoundingClientRect();
    const pad = 6;
    if (r === undefined || r.width === 0) {
      ring.hidden = true;
      card.style.left = `${Math.max(16, window.innerWidth / 2 - 180)}px`;
      card.style.top = `${window.innerHeight / 3}px`;
      return;
    }
    ring.hidden = false;
    Object.assign(ring.style, { left: `${r.left - pad}px`, top: `${r.top - pad}px`, width: `${r.width + 2 * pad}px`, height: `${r.height + 2 * pad}px` });
    // The card goes beside the highlight where there is room, else under it, else over it.
    const w = Math.min(360, window.innerWidth - 32);
    card.style.width = `${w}px`;
    const ch = card.offsetHeight || 200;
    let left = r.right + 16;
    let top = Math.max(16, r.top);
    if (left + w > window.innerWidth - 16) left = r.left - w - 16;
    if (left < 16) {
      left = Math.min(Math.max(16, r.left), window.innerWidth - w - 16);
      top = r.bottom + 16 + ch < window.innerHeight ? r.bottom + 16 : Math.max(16, r.top - ch - 16);
    }
    card.style.left = `${left}px`;
    card.style.top = `${Math.min(top, window.innerHeight - ch - 16)}px`;
  }
  function go(i: number): void {
    if (i < 0) return;
    if (i >= STOPS.length) return end();
    at = i;
    const stop = STOPS[i]!;
    if (stop.panel !== undefined) show(stop.panel);
    const next = h("button", { className: "primary", textContent: i === STOPS.length - 1 ? "Done" : "Next", onclick: () => go(at + 1) });
    card.replaceChildren(
      h("p", { className: "tour-step", textContent: `${i + 1} of ${STOPS.length}` }),
      h("h2", { textContent: stop.title }),
      h("p", { textContent: stop.body }),
      h(
        "div",
        { className: "tour-actions" },
        h("button", { className: "welcome-link", textContent: "Skip the tour", onclick: end }),
        i > 0 ? h("button", { textContent: "Back", onclick: () => go(at - 1) }) : null,
        next,
      ),
    );
    // The panel may have just come forward: place once it has laid out.
    requestAnimationFrame(() => {
      place();
      next.focus();
    });
  }
  document.body.append(layer);
  window.addEventListener("keydown", keys, true);
  window.addEventListener("resize", place);
  running = end;
  go(0);
}
