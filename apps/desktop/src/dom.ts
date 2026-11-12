import { currentWug, wugSvg } from "./theme/wug";

export type Child = Node | string | null | false;

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const el = Object.assign(document.createElement(tag), props);
  for (const c of children) if (c !== null && c !== false) el.append(c);
  return el;
}

/** The shell's wug (the active kit's) as an inline drawing. */
export function wugArt(className: string, label = "wug"): HTMLElement {
  const art = h("span", { className });
  art.innerHTML = wugSvg(currentWug(), label);
  return art;
}

/** A warm, quiet placeholder for panels with nothing to show yet. */
export function emptyState(title: string, body: string, caption?: string): HTMLElement {
  return h(
    "div",
    { className: "empty-state" },
    wugArt("empty-wug", ""),
    h("h2", { textContent: title }),
    h("p", { textContent: body }),
    caption === undefined ? null : h("p", { className: "wug-caption", textContent: caption }),
  );
}
