// Settings (owner, 2026-11-12): one panel that slides in from the right,
// replacing the Workspace and Updates menus. Themes and patterns are shown,
// not named: a first-time user cannot know what "Seafoam" or "Garden" means.
import { h } from "../dom";
import { chooseEmphasisPattern, choosePattern, chooseTheme, emphasisPatternOn, PATTERNS, savedPattern, savedTheme, THEMES } from "../theme/theme";

export type SettingsActions = {
  /** A project is open: the workspace rows act on its panels. */
  projectOpen(): boolean;
  /** Closed panels this kit offers, to bring back. */
  closedPanels(): { id: string; title: string }[];
  showPanel(id: string): void;
  kitName: string;
  resetLayout(): void;
  tabsAlways(): boolean;
  setTabsAlways(on: boolean): void;
  visitsOn(): boolean;
  setVisitsOn(on: boolean): void;
  /** True when a newer version was found and its offer is showing. */
  checkForUpdates(): Promise<boolean>;
  autoChecks(): boolean;
  setAutoChecks(on: boolean): void;
  /** The running version, or "" until it is known. */
  version(): string;
  /** After any change, so the top bar and dock follow. */
  changed(): void;
  /** The sheet has closed. */
  closed(): void;
};

/** What each pattern tile is used for, in the order families list them. */
export const TILE_ROLES = ["Page", "Sides", "Strips"] as const;

let current: HTMLElement | null = null;

export function openSettings(a: SettingsActions): void {
  if (current !== null) {
    current.querySelector<HTMLElement>(".settings-close")?.focus();
    return;
  }
  const body = h("div", { className: "settings-body" });
  const close = () => {
    if (current !== overlay) return;
    current = null;
    overlay.classList.add("settings-leaving");
    window.removeEventListener("keydown", onKey, true);
    window.setTimeout(() => overlay.remove(), 160);
    // Every change re-renders the top bar, so find its Settings button again.
    document.querySelector<HTMLElement>(".settings-button")?.focus();
    a.closed();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopImmediatePropagation();
      close();
    } else if (e.key === "Tab") {
      // The sheet is modal: the keyboard stays inside it.
      const stops = [...sheet.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled)")].filter((el) => el.tabIndex >= 0 && el.offsetParent !== null);
      const first = stops[0];
      const last = stops[stops.length - 1];
      if (first === undefined || last === undefined) return;
      if (!sheet.contains(document.activeElement)) {
        e.preventDefault();
        first.focus();
      } else if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };
  // Redrawing replaces the controls, so the one in use is found again by its key.
  const redraw = () => {
    const key = (document.activeElement as HTMLElement | null)?.dataset?.key;
    body.replaceChildren(...sections(a, redraw, close));
    a.changed();
    if (key === undefined) return;
    // A chip that went away (its panel is back) hands focus to Close.
    (body.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`) ?? sheet.querySelector<HTMLElement>(".settings-close"))?.focus();
  };
  const sheet = h(
    "aside",
    { className: "settings-sheet", role: "dialog", ariaModal: "true", ariaLabel: "Settings" },
    h("header", { className: "settings-head" }, h("h2", { textContent: "Settings" }), h("button", { className: "settings-close", textContent: "Close", onclick: close })),
    body,
  );
  const overlay = h("div", { className: "settings-overlay" }, h("div", { className: "settings-scrim", onclick: close }), sheet);
  body.replaceChildren(...sections(a, redraw, close));
  window.addEventListener("keydown", onKey, true);
  document.body.append(overlay);
  current = overlay;
  sheet.querySelector<HTMLElement>(".settings-close")?.focus();
}

function sections(a: SettingsActions, redraw: () => void, close: () => void): HTMLElement[] {
  const theme = savedTheme(localStorage);
  const pattern = savedPattern(localStorage);
  const open = a.projectOpen();
  const closed = open ? a.closedPanels() : [];
  const version = a.version();
  return [
    section(
      "Colours",
      "How the whole workspace looks on this computer.",
      radioGroup(
        "Colours",
        "settings-cards",
        THEMES.map((t) =>
          card(`theme:${t.id}`, t.label, t.note, theme === t.id, themePreview(t.swatches), () => {
            chooseTheme(localStorage, t.id);
            redraw();
          }),
        ),
      ),
    ),
    section(
      "Patterns",
      "Each family has three: one for the page behind the panels, one across the side panels, and one you can add to emphasised strips.",
      radioGroup(
        "Patterns",
        "settings-cards settings-cards-wide",
        PATTERNS.map((p) =>
          card(`pattern:${p.id}`, p.label, p.note, pattern === p.id, patternPreview(p.tiles), () => {
            choosePattern(localStorage, p.id);
            redraw();
          }),
        ),
      ),
      toggle(
        "emphasis",
        "Pattern on emphasised strips",
        "The third pattern on the tier names beside the timeline, the IPA palette and Import and export.",
        emphasisPatternOn(localStorage),
        () => {
          chooseEmphasisPattern(localStorage, !emphasisPatternOn(localStorage));
          redraw();
        },
        pattern === "none",
      ),
    ),
    section(
      "Workspace",
      "",
      toggle("tabs", "Always show panel tabs", "Otherwise they rise when you reach for the top edge of a panel.", a.tabsAlways(), () => {
        a.setTabsAlways(!a.tabsAlways());
        redraw();
      }),
      toggle("visits", "Let the wug visit", "Now and then it peeks over the panel you are in. Touch it and it hides.", a.visitsOn(), () => {
        a.setVisitsOn(!a.visitsOn());
        redraw();
      }),
      closed.length === 0
        ? null
        : h(
            "div",
            { className: "settings-row" },
            h("span", { className: "settings-row-label", textContent: "Closed panels" }),
            h("div", { className: "settings-chips" }, ...closed.map((p) => keyed(`show:${p.id}`, h("button", { textContent: `Show ${p.title}`, onclick: () => (a.showPanel(p.id), redraw()) })))),
          ),
      !open
        ? null
        : h(
            "div",
            { className: "settings-row" },
            h("span", { className: "settings-row-label", textContent: `Arrange the panels as ${a.kitName} starts` }),
            keyed("reset", h("button", { textContent: "Reset layout", onclick: () => (a.resetLayout(), redraw()) })),
          ),
    ),
    section(
      "Updates",
      `${version === "" ? "" : `This is wugs ${version}. `}Checking asks GitHub for a newer version (Online, a few kB); nothing downloads until you say so.`,
      toggle("autocheck", "Check when wugs starts", "", a.autoChecks(), () => {
        a.setAutoChecks(!a.autoChecks());
        redraw();
      }),
      h("div", { className: "settings-row" }, checkNow(a, close)),
    ),
  ];
}

/** "Check for updates now": says it is checking, and when a newer version is
 * found closes the sheet, because its offer shows under the top bar. */
function checkNow(a: SettingsActions, close: () => void): HTMLButtonElement {
  const button = h("button", { textContent: "Check for updates now" });
  button.dataset.key = "check";
  button.onclick = async () => {
    button.disabled = true;
    button.textContent = "Checking…";
    const found = await a.checkForUpdates();
    if (found) close();
    else if (button.isConnected) {
      button.disabled = false;
      button.textContent = "Check for updates now";
      button.focus();
    }
  };
  return button;
}

/** A row of cards that behaves as one radio group: one Tab stop (the chosen
 * card), arrow keys move between cards and choose. */
function radioGroup(label: string, className: string, cards: HTMLButtonElement[]): HTMLElement {
  const chosen = cards.findIndex((c) => c.getAttribute("aria-checked") === "true");
  cards.forEach((c, i) => (c.tabIndex = i === (chosen < 0 ? 0 : chosen) ? 0 : -1));
  const group = h("div", { className, role: "radiogroup", ariaLabel: label }, ...cards);
  group.addEventListener("keydown", (e) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (step === 0) return;
    e.preventDefault();
    const at = cards.indexOf(document.activeElement as HTMLButtonElement);
    const next = cards[(Math.max(0, at) + step + cards.length) % cards.length];
    next?.focus();
    next?.click();
  });
  return group;
}

function section(title: string, lede: string, ...content: (HTMLElement | null)[]): HTMLElement {
  return h("section", { className: "settings-section" }, h("h3", { textContent: title }), lede === "" ? null : h("p", { className: "settings-lede", textContent: lede }), ...content);
}

function card(key: string, label: string, note: string, on: boolean, preview: HTMLElement, choose: () => void): HTMLButtonElement {
  const button = h(
    "button",
    { className: `settings-card${on ? " is-on" : ""}`, role: "radio", ariaChecked: String(on), onclick: choose, title: note },
    preview,
    h("span", { className: "settings-card-label", textContent: label }),
    h("span", { className: "settings-card-note", textContent: note }),
  );
  button.dataset.key = key;
  return button;
}

function toggle(key: string, label: string, note: string, on: boolean, flip: () => void, disabled = false): HTMLElement {
  const box = h("input", { type: "checkbox", checked: on, disabled, onchange: flip });
  box.dataset.key = key;
  return h("label", { className: `settings-toggle${disabled ? " is-disabled" : ""}` }, box, h("span", {}, h("span", { className: "settings-toggle-label", textContent: label }), note === "" ? null : h("span", { className: "settings-toggle-note", textContent: note })));
}

/** A miniature workspace in the theme's own colours: the page, a panel on
 * it with a heading line, text lines and an accent button. */
function themePreview([bg, accent, ink]: readonly string[]): HTMLElement {
  const preview = h("span", { className: "theme-preview", ariaHidden: "true" });
  preview.style.setProperty("--p-bg", bg ?? "#fff");
  preview.style.setProperty("--p-accent", accent ?? "#000");
  preview.style.setProperty("--p-ink", ink ?? "#000");
  // A dark theme's panels are a shade lighter than its page, not near-white.
  if (isDark(bg ?? "#fff")) preview.style.setProperty("--p-panel", `color-mix(in srgb, ${bg} 88%, white)`);
  preview.append(
    h("span", { className: "tp-panel" }, h("span", { className: "tp-line tp-head" }), h("span", { className: "tp-line" }), h("span", { className: "tp-line tp-short" }), h("span", { className: "tp-button" })),
    h("span", { className: "tp-panel tp-side" }, h("span", { className: "tp-line" }), h("span", { className: "tp-line tp-short" })),
  );
  return preview;
}

/** The family's tiles side by side, each as it is used, in the current ink. */
function patternPreview(tiles: readonly string[]): HTMLElement {
  if (tiles.length === 0) return h("span", { className: "pattern-preview pattern-plain", ariaHidden: "true" });
  return h(
    "span",
    { className: "pattern-preview", ariaHidden: "true" },
    ...tiles.map((tile, i) => {
      const swatch = h("span", { className: "pattern-tile", title: TILE_ROLES[i] ?? "" });
      swatch.style.setProperty("--tile", `url("/patterns/${tile}.svg")`);
      return h("span", { className: "pattern-cell" }, swatch, h("span", { className: "pattern-role", textContent: TILE_ROLES[i] ?? "" }));
    }),
  );
}

function keyed<T extends HTMLElement>(key: string, el: T): T {
  el.dataset.key = key;
  return el;
}

function isDark(hex: string): boolean {
  const n = Number.parseInt(hex.replace("#", "").slice(0, 6), 16);
  if (Number.isNaN(n)) return false;
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 96;
}
