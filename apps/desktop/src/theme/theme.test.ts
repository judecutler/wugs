import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { PATTERNS, savedTheme, THEMES } from "./theme.ts";
import { existsSync } from "node:fs";

const store = (v: string | null) => ({ getItem: () => v });

test("an unknown or missing saved theme falls back to Cream", () => {
  assert.equal(savedTheme(store(null)), "cream");
  assert.equal(savedTheme(store("neon")), "cream");
  assert.equal(savedTheme(store("night")), "night");
});

test("every theme but Cream has a block in themes.css, and nothing else does", () => {
  const css = readFileSync(new URL("./themes.css", import.meta.url), "utf8");
  const blocks = [...css.matchAll(/data-theme="([a-z]+)"/g)].map((m) => m[1]);
  assert.deepEqual(blocks.sort(), THEMES.map((t) => t.id).filter((id) => id !== "cream").sort());
});

// Every theme keeps text readable: ink, muted, accent, warn and error on every
// surface, and on-accent on accent, at WCAG AA (4.5:1).
function vars(block: string): Record<string, string> {
  return Object.fromEntries([...block.matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6})\b/gi)].map((m) => [m[1]!, m[2]!.toLowerCase()]));
}
function luminance(hex: string): number {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0]! + 0.7152 * c[1]! + 0.0722 * c[2]!;
}
const ratio = (a: string, b: string) => {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x! + 0.05) / (y! + 0.05);
};

test("every theme passes 4.5:1 for text on every surface", () => {
  const base = vars(readFileSync(new URL("./tokens.css", import.meta.url), "utf8"));
  const css = readFileSync(new URL("./themes.css", import.meta.url), "utf8");
  const themes: [string, Record<string, string>][] = [["cream", base]];
  const failures: string[] = [];
  const tested = new Set(["bg", "panel", "surface-raised", "chrome", "accent-soft", "line", "ink", "muted", "accent", "warn", "error", "on-accent", "accent-bright", "on-bright"]);
  for (const m of css.matchAll(/data-theme="([a-z]+)"\]\s*\{([^}]*)\}/g)) {
    themes.push([m[1]!, { ...base, ...vars(m[2]!) }]);
    // A tested token written any other way than #rrggbb would be skipped here: refuse it.
    for (const d of m[2]!.matchAll(/--([a-z-]+):\s*([^;]+);/g)) if (tested.has(d[1]!) && !/^#[0-9a-f]{6}$/i.test(d[2]!.trim())) failures.push(`${m[1]}: --${d[1]} must be #rrggbb to be checked`);
  }
  for (const [name, t] of themes) {
    for (const fg of ["ink", "muted", "accent", "warn", "error"])
      for (const bg of ["bg", "panel", "surface-raised", "chrome", "accent-soft", "line"]) {
        if (fg !== "ink" && fg !== "muted" && bg === "line") continue;
        const r = ratio(t[fg]!, t[bg]!);
        if (r < 4.5) failures.push(`${name}: --${fg} on --${bg} is ${r.toFixed(2)}`);
      }
    for (const [fg, bg] of [["on-accent", "accent"], ["on-bright", "accent-bright"]] as const) {
      const r = ratio(t[fg]!, t[bg]!);
      if (r < 4.5) failures.push(`${name}: --${fg} on --${bg} is ${r.toFixed(2)}`);
    }
  }
  assert.deepEqual(failures, []);
});

test("every pattern family has its CSS rule and its three tile files", () => {
  const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
  for (const p of PATTERNS) {
    if (p.id === "none") continue;
    const rule = css.match(new RegExp(`:root\\[data-pattern="${p.id}"\\]\\s*\\{([^}]*)\\}`));
    assert.ok(rule, `no CSS rule for ${p.id}`);
    const vars = ["--pattern-bg", "--pattern-panel", "--pattern-band"];
    p.tiles.forEach((tile, i) => {
      assert.ok(rule![1]!.includes(`${vars[i]}: url("/patterns/${tile}.svg")`), `${p.id}: ${vars[i]} should be ${tile}`);
      assert.ok(existsSync(new URL(`../../public/patterns/${tile}.svg`, import.meta.url)), `missing ${tile}.svg`);
    });
  }
});
