// Allomorphs: affixes that share a gloss and a side of the root but are
// written differently (-ler and -lar PL, a- and an- 3SG), with what each
// attaches to: the sound right next to it and the stem's nearest vowel. When
// those sets never overlap between the forms, that is what conditions them
// (vowel harmony, assimilation, a vowel- or consonant-final stem), and the
// panel says so, naming each side as a natural class where it can. Pure; no DOM.
import { naturalClass } from "../sounds/classes.ts";
import { sounds } from "../sounds/ipa.ts";
import { classOf } from "../sounds/shapes.ts";
import type { Affix } from "./logic.ts";

export type Counted = { sound: string; count: number };
export type Allomorph = { affix: Affix; next: Counted[]; vowel: Counted[] };
export type Tell = "next" | "vowel";
export type AllomorphSet = { gloss: string; kind: string; side: "before" | "after"; uses: number; members: Allomorph[]; tells: Tell[] };

const sideOf = (a: Affix): "before" | "after" => (a.slot < 0 ? "before" : "after");

function counted(map: Map<string, number>): Counted[] {
  return [...map.entries()].map(([sound, count]) => ({ sound, count })).sort((x, y) => y.count - x.count || x.sound.localeCompare(y.sound));
}

/** The sound next to the affix and the nearest vowel, over its stems. */
export function neighbours(affix: Affix): { next: Counted[]; vowel: Counted[] } {
  const next = new Map<string, number>();
  const vowel = new Map<string, number>();
  const after = sideOf(affix) === "after";
  for (const [stem, n] of affix.stems ?? []) {
    const s = sounds(stem.replace(/\s+/gu, ""));
    if (s.length === 0) continue;
    const near = after ? s[s.length - 1]! : s[0]!;
    next.set(near, (next.get(near) ?? 0) + n);
    const ordered = after ? [...s].reverse() : s;
    const v = ordered.find((x) => classOf(x) === "V");
    if (v !== undefined) vowel.set(v, (vowel.get(v) ?? 0) + n);
  }
  return { next: counted(next), vowel: counted(vowel) };
}

const disjoint = (lists: readonly Counted[][]): boolean => {
  if (lists.some((l) => l.length === 0)) return false;
  const seen = new Set<string>();
  for (const l of lists) {
    for (const c of l) if (seen.has(c.sound)) return false;
    for (const c of l) seen.add(c.sound);
  }
  return true;
};

/** Every gloss written two or more ways on one side of the root, the most
 * used first; each form with its neighbours, and which neighbour (if either)
 * keeps the forms apart. Affixes without a gloss are left out. */
export function allomorphSets(affixes: readonly Affix[]): AllomorphSet[] {
  const by = new Map<string, Affix[]>();
  for (const a of affixes) {
    const gloss = a.gloss.trim();
    if (gloss === "" || a.kind === "reduplication") continue;
    const key = `${a.kind}\u0001${sideOf(a)}\u0001${gloss}`;
    by.set(key, [...(by.get(key) ?? []), a]);
  }
  const sets: AllomorphSet[] = [];
  for (const list of by.values()) {
    if (list.length < 2) continue;
    const members = [...list].sort((x, y) => y.uses - x.uses || x.surface.localeCompare(y.surface)).map((affix) => ({ affix, ...neighbours(affix) }));
    const tells: Tell[] = [];
    if (disjoint(members.map((m) => m.next))) tells.push("next");
    if (disjoint(members.map((m) => m.vowel))) tells.push("vowel");
    const first = list[0]!;
    sets.push({ gloss: first.gloss.trim(), kind: first.kind, side: sideOf(first), uses: list.reduce((n, a) => n + a.uses, 0), members, tells });
  }
  return sets.sort((x, y) => y.uses - x.uses || x.gloss.localeCompare(y.gloss));
}

/** The sounds, commonest first (at most `max`), and what they share when a natural class names them. */
export function soundsLine(list: readonly Counted[], max = 8): string {
  if (list.length === 0) return "–";
  const shown = list.slice(0, max).map((c) => c.sound).join(", ");
  const more = list.length > max ? ` and ${list.length - max} more` : "";
  const cls = naturalClass(list.map((c) => c.sound));
  return `${shown}${more}${cls === null ? "" : ` (${cls})`}`;
}

/** What keeps the forms apart, in words. */
export function setNote(s: AllomorphSet): string {
  const where = s.side === "after" ? "stem's last vowel" : "stem's first vowel";
  const next = s.side === "after" ? "sound before it" : "sound after it";
  const same = (a: readonly Counted[], b: readonly Counted[]) => a.length === b.length && a.every((c, i) => c.sound === b[i]!.sound);
  if (s.tells.length === 2 && s.members.every((m) => same(m.next, m.vowel))) return `The ${where}, right ${s.side === "after" ? "before" : "after"} it, keeps the forms apart: perhaps vowel harmony, or a rule for vowel-${s.side === "after" ? "final" : "initial"} stems.`;
  if (s.tells.length === 2) return `The ${next} and the ${where} each keep the forms apart: either may condition them.`;
  if (s.tells[0] === "vowel") return `The ${where} keeps the forms apart: perhaps vowel harmony.`;
  if (s.tells[0] === "next") return `The ${next} keeps the forms apart: perhaps assimilation, or a vowel- or consonant-final stem.`;
  if (s.members.some((m) => m.next.length === 0)) return "Some forms have no stems recorded here; read the texts again to see what they attach to.";
  return "Neither the neighbouring sound nor the nearest vowel keeps the forms apart: they may be free variants, slips, or conditioned by something else (the root, the speaker).";
}

/** Every set as a tab-separated table. */
export function allomorphsTsv(sets: readonly AllomorphSet[], mark: (a: Affix) => string): string {
  const rows = ["gloss\tform\tuses\troots\tnext to\tnearest vowel"];
  for (const s of sets) for (const m of s.members) rows.push([s.gloss, mark(m.affix), String(m.affix.uses), String(m.affix.roots), soundsLine(m.next, 99), soundsLine(m.vowel, 99)].join("\t"));
  return rows.join("\n") + "\n";
}
