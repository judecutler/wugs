// Settings (owner, 2026-11-12): one panel that slides in from the right,
// replacing the Workspace and Updates menus. Themes and patterns are shown,
// not named: a first-time user cannot know what "Seafoam" or "Garden" means.
import { h } from "../dom";
import { chooseEmphasisPattern, choosePattern, chooseTheme, emphasisPatternOn, PATTERNS, savedPattern, savedTheme, THEMES } from "../theme/theme";

export type SettingsActions = {
  /** Closed panels this kit offers, to bring back. */
  closedPanels(): { id: string; title: string }[];
  showPanel(id: string): void;
  kitName: string;
  resetLayout(): void;
  tabsAlways(): boolean;
  setTabsAlways(on: boolean): void;
  visitsOn(): boolean;
  setVisitsOn(on: boolean): void;
  checkForUpdates(): void;
  autoChecks(): boolean;
  setAutoChecks(on: boolean): void;
  version: string;
  /** After any change, so the top bar and dock follow. */
  changed(): void;
};

/** What each pattern tile is used for, in the order families list them. */
export const TILE_ROLES = ["Page", "Sides", "Strips"] as const;

let current: HTMLElement | null = null;

export function openSettings(a: SettingsActions): void {
  if (current !== null) {
    current.querySelector<HTMLElement>(".settings-close")?.focus();
    return;
  }
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const body = h("div", { className: "settings-body" });
  const close = () => {
    overlay.classList.add("settings-leaving");
    window.removeEventListener("keydown", onKey, true);
    window.setTimeout(() => overlay.remove(), 160);
    if (current === overlay) current = null;
    opener?.focus();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopImmediatePropagation();
      close();
    }
  };
  const redraw = () => {
    body.replaceChildren(...sections(a, redraw));
    a.changed();
  };
  const sheet = h(
    "aside",
    { className: "settings-sheet", role: "dialog", ariaModal: "true", ariaLabel: "Settings" },
    h("header", { className: "settings-head" }, h("h2", { textContent: "Settings" }), h("button", { className: "settings-close", textContent: "Close", onclick: close })),
    body,
  );
  const overlay = h("div", { className: "settings-overlay" }, h("div", { className: "settings-scrim", onclick: close }), sheet);
  body.replaceChildren(...sections(a, redraw));
  window.addEventListener("keydown", onKey, true);
  document.body.append(overlay);
  current = overlay;
  sheet.querySelector<HTMLElement>(".settings-close")?.focus();
}

function sections(a: SettingsActions, redraw: () => void): HTMLElement[] {
  const theme = savedTheme(localStorage);
  const pattern = savedPattern(localStorage);
  const closed = a.closedPanels();
  return [
    section(
      "Colours",
      "How the whole workspace looks on this computer.",
      h(
        "div",
        { className: "settings-cards", role: "radiogroup", ariaLabel: "Colours" },
        ...THEMES.map((t) =>
          card(t.label, t.note, theme === t.id, themePreview(t.swatches), () => {
            chooseTheme(localStorage, t.id);
            redraw();
          }),
        ),
      ),
    ),
    section(
      "Patterns",
      "Each family has three: one for the page behind the panels, one across the side panels, and one you can add to emphasised strips.",
      h(
        "div",
        { className: "settings-cards settings-cards-wide", role: "radiogroup", ariaLabel: "Patterns" },
        ...PATTERNS.map((p) =>
          card(p.label, p.note, pattern === p.id, patternPreview(p.tiles), () => {
            choosePattern(localStorage, p.id);
            redraw();
          }),
        ),
      ),
      toggle(
        "Pattern on emphasised strips",
        "The third pattern on the tier names beside the timeline, the vowel palette and Import and export.",
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
      toggle("Always show panel tabs", "Otherwise they rise when you reach for the top edge of a panel.", a.tabsAlways(), () => {
        a.setTabsAlways(!a.tabsAlways());
        redraw();
      }),
      toggle("Let the wug visit", "Now and then it peeks over the panel you are in. Touch it and it hides.", a.visitsOn(), () => {
        a.setVisitsOn(!a.visitsOn());
        redraw();
      }),
      closed.length === 0
        ? null
        : h(
            "div",
            { className: "settings-row" },
            h("span", { className: "settings-row-label", textContent: "Closed panels" }),
            h("div", { className: "settings-chips" }, ...closed.map((p) => h("button", { textContent: `Show ${p.title}`, onclick: () => (a.showPanel(p.id), redraw()) }))),
          ),
      h(
        "div",
        { className: "settings-row" },
        h("span", { className: "settings-row-label", textContent: `Arrange the panels as ${a.kitName} starts` }),
        h("button", { textContent: "Reset layout", onclick: () => (a.resetLayout(), redraw()) }),
      ),
    ),
    section(
      "Updates",
      `This is wugs ${a.version}. Checking asks GitHub for a newer version (Online, a few kB); nothing downloads until you say so.`,
      toggle("Check when wugs starts", "", a.autoChecks(), () => {
        a.setAutoChecks(!a.autoChecks());
        redraw();
      }),
      h("div", { className: "settings-row" }, h("button", { textContent: "Check for updates now", onclick: () => a.checkForUpdates() })),
    ),
  ];
}

function section(title: string, lede: string, ...content: (HTMLElement | null)[]): HTMLElement {
  return h("section", { className: "settings-section" }, h("h3", { textContent: title }), lede === "" ? null : h("p", { className: "settings-lede", textContent: lede }), ...content);
}

function card(label: string, note: string, on: boolean, preview: HTMLElement, choose: () => void): HTMLElement {
  return h(
    "button",
    { className: `settings-card${on ? " is-on" : ""}`, role: "radio", ariaChecked: String(on), onclick: choose, title: note },
    preview,
    h("span", { className: "settings-card-label", textContent: label }),
    h("span", { className: "settings-card-note", textContent: note }),
  );
}

function toggle(label: string, note: string, on: boolean, flip: () => void, disabled = false): HTMLElement {
  const box = h("input", { type: "checkbox", checked: on, disabled, onchange: flip });
  return h("label", { className: `settings-toggle${disabled ? " is-disabled" : ""}` }, box, h("span", {}, h("span", { className: "settings-toggle-label", textContent: label }), note === "" ? null : h("span", { className: "settings-toggle-note", textContent: note })));
}

/** A miniature workspace in the theme's own colours: the page, a panel on
 * it with a heading line, text lines and an accent button. */
function themePreview([bg, accent, ink]: readonly string[]): HTMLElement {
  const preview = h("span", { className: "theme-preview", ariaHidden: "true" });
  preview.style.setProperty("--p-bg", bg ?? "#fff");
  preview.style.setProperty("--p-accent", accent ?? "#000");
  preview.style.setProperty("--p-ink", ink ?? "#000");
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
