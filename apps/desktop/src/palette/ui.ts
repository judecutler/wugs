// The command palette and the shortcuts sheet (roadmap 70). One modal at a
// time, centred near the top like VS Code's: type, arrow keys, Enter.
import { h } from "../dom";
import { keyLabel, moveIndex, opensPalette, opensShortcuts, rank, SHORTCUTS, type Command } from "./logic";
import { allCommands } from "./registry";

const MAC = /Mac|iPhone|iPad/u.test(navigator.platform || navigator.userAgent);
let open: { close: () => void } | null = null;

function modal(className: string, label: string, content: (close: () => void) => HTMLElement[], onKey?: (e: KeyboardEvent) => void): void {
  open?.close();
  const back = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const box = h("div", { className: `palette ${className}`, role: "dialog", ariaModal: "true", ariaLabel: label });
  const overlay = h("div", { className: "palette-overlay" }, box);
  const close = () => {
    if (open?.close !== close) return;
    open = null;
    window.removeEventListener("keydown", keys, true);
    overlay.remove();
    if (back?.isConnected) back.focus();
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopImmediatePropagation();
      close();
      return;
    }
    if (e.key === "Tab") {
      // The keyboard stays inside: the palette's own list is driven by arrows.
      const stops = [...box.querySelectorAll<HTMLElement>("input, button")].filter((x) => x.tabIndex >= 0);
      const first = stops[0];
      const last = stops[stops.length - 1];
      if (first !== undefined && last !== undefined && (e.shiftKey ? document.activeElement === first : document.activeElement === last)) {
        e.preventDefault();
        (e.shiftKey ? last : first).focus();
      }
    }
    onKey?.(e);
  };
  overlay.addEventListener("mousedown", (e) => {
    if (e.target === overlay) close();
  });
  box.append(...content(close));
  window.addEventListener("keydown", keys, true);
  document.body.append(overlay);
  open = { close };
  box.querySelector<HTMLElement>("input, button")?.focus();
}

/** Find any action or panel by name. */
export function openPalette(): void {
  let commands = allCommands();
  let shown: Command[] = commands;
  let at = 0;
  const input = h("input", { className: "palette-input", type: "text", placeholder: "Type what you want to do: “split”, “night”, “lexicon”…", spellcheck: false, autocomplete: "off" });
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-expanded", "true");
  input.setAttribute("aria-controls", "palette-list");
  const list = h("ul", { className: "palette-list", id: "palette-list", role: "listbox" });
  let closeIt: () => void = () => {};
  const runAt = (i: number) => {
    const c = shown[i];
    if (c === undefined) return;
    const why = c.disabled?.() ?? null;
    if (why !== null) return;
    closeIt();
    c.run();
  };
  const draw = () => {
    shown = rank(input.value, commands);
    at = Math.min(at, Math.max(0, shown.length - 1));
    list.replaceChildren(
      ...(shown.length === 0
        ? [h("li", { className: "palette-empty", textContent: "Nothing matches. Try another word, or press ? for every shortcut." })]
        : shown.map((c, i) => {
            const why = c.disabled?.() ?? null;
            const row = h(
              "li",
              { className: `palette-row${i === at ? " is-at" : ""}${why !== null ? " is-off" : ""}`, role: "option", id: `palette-opt-${i}`, title: why ?? "" },
              h("span", { className: "palette-title", textContent: c.title }),
              h("span", { className: "palette-group", textContent: why ?? c.group }),
              c.keys === undefined ? null : h("kbd", { className: "palette-keys", textContent: keyLabel(c.keys, MAC) }),
            );
            row.setAttribute("aria-selected", String(i === at));
            if (why !== null) row.setAttribute("aria-disabled", "true");
            row.onmousemove = () => {
              if (at !== i) {
                at = i;
                draw();
              }
            };
            row.onclick = () => runAt(i);
            return row;
          })),
    );
    input.setAttribute("aria-activedescendant", shown.length === 0 ? "" : `palette-opt-${at}`);
    list.querySelector(".is-at")?.scrollIntoView({ block: "nearest" });
  };
  input.oninput = () => {
    at = 0;
    draw();
  };
  modal(
    "palette-commands",
    "Find an action",
    (close) => {
      closeIt = close;
      commands = allCommands();
      draw();
      return [input, list, h("p", { className: "palette-foot", textContent: "↑ ↓ to move · Enter to run · Esc to close · ? for every shortcut" })];
    },
    (e) => {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        at = moveIndex(at, e.key === "ArrowDown" ? 1 : -1, shown.length);
        draw();
      } else if (e.key === "Enter") {
        e.preventDefault();
        runAt(at);
      }
    },
  );
}

/** Every key the app answers to, by where it works. */
export function openShortcuts(): void {
  modal("palette-shortcuts", "Keyboard shortcuts", (close) => [
    h("header", { className: "palette-head" }, h("h2", { textContent: "Keyboard shortcuts" }), h("button", { textContent: "Close", onclick: close })),
    h(
      "div",
      { className: "shortcut-groups" },
      ...SHORTCUTS.map((g) =>
        h(
          "section",
          { className: "shortcut-group" },
          h("h3", { textContent: g.where }),
          h("dl", {}, ...g.keys.flatMap(([k, what]) => [h("dt", {}, h("kbd", { textContent: keyLabel(k, MAC) })), h("dd", { textContent: what })])),
        ),
      ),
    ),
  ]);
}

const typing = (t: EventTarget | null): boolean =>
  t instanceof HTMLElement && (t.isContentEditable || t instanceof HTMLTextAreaElement || (t instanceof HTMLInputElement && !["checkbox", "radio", "button", "range"].includes(t.type)) || t instanceof HTMLSelectElement);

/** Ctrl+K and ? from anywhere in the window. */
export function wirePalette(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.defaultPrevented || e.isComposing) return;
    if (opensPalette(e)) {
      e.preventDefault();
      if (open !== null && document.querySelector(".palette-commands") !== null) open.close();
      else openPalette();
    } else if (open === null && opensShortcuts(e, typing(e.target))) {
      e.preventDefault();
      openShortcuts();
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
