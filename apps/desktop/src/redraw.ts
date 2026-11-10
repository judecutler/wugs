// When a panel redraws. Pure, so it is unit-tested without a DOM
// (`npm test`).

export type Redraw = "draw" | "same" | "defer";

/** `last` is what was drawn (null before the first draw). A redraw while
 * the person is typing in the panel would throw the typing away, so it
 * waits ("defer") until focus leaves; unchanged content never redraws. */
export function decideRedraw<T>(last: { value: T } | null, next: T, editing: boolean, equal: (a: T, b: T) => boolean = Object.is): Redraw {
  if (last !== null && equal(last.value, next)) return "same";
  if (last !== null && editing) return "defer";
  return "draw";
}

/** Equal by content. A refetch builds new objects for the same data, so
 * identity alone would redraw on every sync that touched nothing here. */
export function sameContent(a: unknown, b: unknown): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b);
}
