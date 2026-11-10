import { decideRedraw } from "../redraw";

/** One dockable panel instance. `render` runs on every state change, so it
 * returns early when the slice of state it shows has not changed. */
export type PanelView = { element: HTMLElement; render(): void; dispose?(): void };

export type PanelDef = { title: string; create(): PanelView };

export type SliceOptions<T> = {
  /** How two slices compare; identity by default. */
  equal?: (a: T, b: T) => boolean;
  /** Hold redraws while an input inside the panel has focus, and catch up
   * once focus leaves it. */
  holdWhileEditing?: boolean;
};

const RECHECK_MS = 300;

/** Re-run `draw` only when `slice()` returns something new. */
export function sliced<T>(element: HTMLElement, slice: () => T, draw: (value: T) => void, options: SliceOptions<T> = {}): PanelView {
  let last: { value: T } | null = null;
  let recheck: number | undefined;
  const editing = () => {
    const active = document.activeElement;
    // A disabled input is one being saved, not typed in.
    return options.holdWhileEditing === true && active instanceof HTMLInputElement && !active.disabled && element.contains(active);
  };
  const render = () => {
    const value = slice();
    switch (decideRedraw(last, value, editing(), options.equal)) {
      case "same":
        return;
      case "defer":
        // focusout covers the usual case; the timer covers an input that
        // was removed while focused, which fires no focusout in every engine.
        window.clearTimeout(recheck);
        recheck = window.setTimeout(render, RECHECK_MS);
        return;
      case "draw":
        last = { value };
        draw(value);
    }
  };
  if (options.holdWhileEditing === true) element.addEventListener("focusout", () => window.setTimeout(render));
  return { element, render, dispose: () => window.clearTimeout(recheck) };
}
