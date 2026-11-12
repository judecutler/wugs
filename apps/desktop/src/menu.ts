// A small drop-down menu: a button that opens a list of actions under it.
// Native <select> menus are drawn by the operating system and look different
// on every platform; this one follows the app's own style. Keyboard: Enter,
// Space or ArrowDown opens, arrows move, Enter picks, Escape or Tab closes.
import { h } from "./dom";

export type MenuItem =
  /** `checked` makes it a checkbox item; with `radio`, one of a set of choices. */
  | { label: string; run: () => void; note?: string; checked?: boolean; radio?: boolean; disabled?: boolean }
  | { heading: string }
  | "separator";

/** `key` names the menu for focus after a redraw; it defaults to the label,
 * so give one when the label can change (the kit switcher). */
export function menuButton(label: string, items: () => MenuItem[], opts: { title?: string; className?: string; key?: string } = {}): HTMLElement {
  const button = h("button", {
    className: `menu-button ${opts.className ?? ""}`.trim(),
    textContent: label,
    title: opts.title ?? "",
  });
  const key = opts.key ?? label;
  button.dataset.menu = key;
  button.setAttribute("aria-haspopup", "menu");
  button.setAttribute("aria-expanded", "false");
  const wrap = h("span", { className: "menu-wrap" }, button);
  let list: HTMLElement | null = null;

  const close = (focusButton: boolean) => {
    if (list === null) return;
    list.remove();
    list = null;
    button.setAttribute("aria-expanded", "false");
    document.removeEventListener("pointerdown", onOutside, true);
    if (focusButton) button.focus();
    wrap.dispatchEvent(new CustomEvent("menu-closed", { bubbles: true, detail: { key, focus: focusButton } }));
  };
  const onOutside = (e: Event) => {
    if (!wrap.contains(e.target as Node)) close(false);
  };
  const entries = () => [...(list?.querySelectorAll<HTMLButtonElement>("button.menu-item:not(:disabled)") ?? [])];
  const open = () => {
    if (list !== null) return;
    list = h("div", { className: "menu-list" });
    list.setAttribute("role", "menu");
    for (const item of items()) {
      if (item === "separator") {
        const sep = h("div", { className: "menu-sep" });
        sep.setAttribute("role", "separator");
        list.append(sep);
      } else if ("heading" in item) {
        const heading = h("div", { className: "menu-heading", textContent: item.heading });
        heading.setAttribute("role", "presentation");
        list.append(heading);
      }
      else {
        const b = h(
          "button",
          {
            className: `menu-item${item.checked ? " checked" : ""}`,
            disabled: item.disabled ?? false,
            onclick: () => {
              close(true);
              item.run();
            },
          },
          h("span", { textContent: item.label }),
          item.note === undefined ? null : h("span", { className: "menu-note", textContent: item.note }),
        );
        b.setAttribute("role", item.checked === undefined ? "menuitem" : item.radio ? "menuitemradio" : "menuitemcheckbox");
        if (item.checked !== undefined) b.setAttribute("aria-checked", String(item.checked));
        list.append(b);
      }
    }
    list.onkeydown = (e) => {
      const all = entries();
      const at = all.indexOf(document.activeElement as HTMLButtonElement);
      if (e.key === "ArrowDown") all[(at + 1) % all.length]?.focus();
      else if (e.key === "ArrowUp") all[(at - 1 + all.length) % all.length]?.focus();
      else if (e.key === "Home") all[0]?.focus();
      else if (e.key === "End") all.at(-1)?.focus();
      else if (e.key === "Escape") close(true);
      else if (e.key === "Tab") {
        // Back on the button first, so Tab carries on from there rather than from a removed item.
        close(true);
        return;
      } else return;
      e.preventDefault();
    };
    wrap.append(list);
    button.setAttribute("aria-expanded", "true");
    document.addEventListener("pointerdown", onOutside, true);
    entries()[0]?.focus();
  };
  button.onclick = () => (list === null ? open() : close(false));
  button.onkeydown = (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      open();
    }
  };
  return wrap;
}
