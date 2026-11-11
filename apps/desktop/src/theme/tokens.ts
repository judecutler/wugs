/** A design token's current value (src/theme/tokens.css), for canvas code
 * that cannot use CSS; `fallback` when the stylesheet has not loaded. */
export function token(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value === "" ? fallback : value;
}
