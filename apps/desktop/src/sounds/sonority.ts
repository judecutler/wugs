// Sonority sequencing: within a syllable, sonority rises towards the vowel
// and falls after it (the Sonority Sequencing Principle; Clements 1990).
// A word-initial cluster is an onset and a word-final one a coda, so each can
// be checked; clusters inside a word straddle a syllable boundary and are
// not judged. Sonority from the chart's manner: plosives lowest, then
// affricates, fricatives, nasals, liquids and glides. Pure and npm-tested.
import { placeOf, sounds } from "./ipa.ts";
import type { Cluster } from "./shapes.ts";

/** A consonant's sonority, or null for one the chart does not place by manner. */
export function sonority(sound: string): number | null {
  const p = placeOf(sound);
  if (p.kind === "affricate") return 2;
  if (p.kind === "vowel") return 7;
  if (p.kind !== "consonant") return null;
  switch (p.manner) {
    case "plosive":
      return 1;
    case "fricative":
    case "lateral-fricative":
      return 3;
    case "nasal":
      return 4;
    case "trill":
    case "tap":
    case "lateral-approximant":
      return 5;
    case "approximant":
      // Glides (j, w, ɰ, ɥ) above the central approximants that act as liquids (ɹ, ɻ).
      return /[jwɰɥ]/u.test(sound.normalize("NFD")[0] ?? "") ? 6 : 5;
  }
}

/** How sonority runs through a cluster: up, down, level, or both ways. */
export type Profile = "rises" | "falls" | "level" | "mixed";

export function profileOf(cluster: string): Profile | null {
  const son = sounds(cluster).map(sonority);
  if (son.length < 2 || son.some((x) => x === null)) return null;
  let up = false;
  let down = false;
  let flat = false;
  for (let i = 1; i < son.length; i++) {
    const d = son[i]! - son[i - 1]!;
    if (d > 0) up = true;
    else if (d < 0) down = true;
    else flat = true;
  }
  if (up && down) return "mixed";
  if (flat) return up || down ? "mixed" : "level";
  return up ? "rises" : "falls";
}

/** Whether a cluster keeps to sonority sequencing where it stands: an onset rises, a coda falls. */
export const keeps = (profile: Profile, where: "onset" | "coda"): boolean => profile === (where === "onset" ? "rises" : "falls");

/** An s before a plosive (s, ʃ and the like), the usual exception at a word's edge. */
export function sPlusStop(cluster: string): boolean {
  const s = sounds(cluster);
  if (s.length < 2) return false;
  const first = placeOf(s[0]!);
  const second = placeOf(s[1]!);
  return first.kind === "consonant" && first.manner === "fricative" && /^[sʃʂz]/u.test(s[0]!) && second.kind === "consonant" && second.manner === "plosive";
}

export type Break = { cluster: string; where: "onset" | "coda"; profile: Profile; uses: number; example: string; sStop: boolean };

/** Word-initial clusters that do not rise and word-final ones that do not fall, commonest first. */
export function sonorityBreaks(clusters: readonly Cluster[]): Break[] {
  const out: Break[] = [];
  for (const c of clusters) {
    const p = profileOf(c.cluster);
    if (p === null) continue;
    if (c.initial > 0 && !keeps(p, "onset")) out.push({ cluster: c.cluster, where: "onset", profile: p, uses: c.initial, example: c.example, sStop: sPlusStop(c.cluster) });
    if (c.final > 0 && !keeps(p, "coda")) out.push({ cluster: c.cluster, where: "coda", profile: p, uses: c.final, example: c.example, sStop: false });
  }
  return out.sort((a, b) => b.uses - a.uses || a.cluster.localeCompare(b.cluster));
}

/** An arrow for a profile, for the clusters table. */
export const PROFILE_MARK: Record<Profile, string> = { rises: "↗", falls: "↘", level: "→", mixed: "↗↘" };

/** The line under the clusters table, in words. */
export function sonorityNote(clusters: readonly Cluster[], breaks: readonly Break[]): string {
  const judged = clusters.filter((c) => (c.initial > 0 || c.final > 0) && profileOf(c.cluster) !== null).length;
  if (judged === 0) return "";
  if (breaks.length === 0) return `Every word-initial cluster rises in sonority towards the vowel and every word-final one falls, as the Sonority Sequencing Principle expects.`;
  const say = (b: Break) => `${b.cluster} (${b.where === "onset" ? "starting" : "ending"} words, ${b.profile === "level" ? "level" : b.profile === "mixed" ? "up and down" : b.profile})`;
  const sStops = [...new Set(breaks.filter((b) => b.sStop).map((b) => b.cluster))];
  const head = `${breaks.length} ${breaks.length === 1 ? "cluster breaks" : "clusters break"} sonority sequencing at a word's edge: ${breaks.slice(0, 6).map(say).join(", ")}${breaks.length > 6 ? ` and ${breaks.length - 6} more` : ""}.`;
  if (sStops.length === breaks.length) return `${head} All are an s before a plosive, the commonest exception across languages.`;
  const exception = sStops.length === 0 ? "" : ` ${sStops.join(", ")} ${sStops.length === 1 ? "is" : "are"} an s before a plosive, the commonest exception across languages.`;
  return `${head}${exception} The others often hide a syllable of their own (a syllabic consonant, a dropped vowel) or come in loanwords.`;
}
