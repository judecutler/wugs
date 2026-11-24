// The sounds found in a project's transcriptions set beside PHOIBLE: the
// language's own inventories there (which phonemes the data has not shown
// yet, which sounds it has that the inventory does not list) and its
// relatives' inventories (sounds common among them but missing here, and
// sounds here that are rare among them). Pure, npm-tested; the data comes
// from src-tauri/src/phoible.rs.

export type PhoibleInventory = { id: string; source: string; name: string; phonemes: string[] };
export type PhoibleShare = { phoneme: string; languages: number };
export type PhoibleCompared = { name: string; own: PhoibleInventory[]; comparedWith: string; relatives: number; shares: PhoibleShare[]; edition: string };

/** How a sound is compared: tie bars dropped, the retraction mark of t̠ʃ d̠ʒ
 * dropped, ASCII g and apostrophe read as IPA ɡ and ʼ. */
export function phonemeKey(p: string): string {
  return p
    .normalize("NFD")
    .replace(/[͜͡]/gu, "")
    .replace(/̠(?=[ʃʒ])/gu, "")
    .replace(/g/gu, "ɡ")
    .replace(/'/gu, "ʼ")
    .normalize("NFC");
}

export type InventoryCheck = { inBoth: string[]; notFound: string[]; notListed: string[] };

/** The project's sounds against one inventory: phonemes it lists that the
 * transcriptions have not shown (`notFound`), and sounds the transcriptions
 * have that it does not list (`notListed`, commonest first). */
export function checkInventory(ours: readonly { sound: string; count: number }[], theirs: readonly string[]): InventoryCheck {
  const ourKeys = new Set(ours.map((s) => phonemeKey(s.sound)));
  const theirKeys = new Set(theirs.map(phonemeKey));
  return {
    inBoth: theirs.filter((p) => ourKeys.has(phonemeKey(p))),
    notFound: theirs.filter((p) => !ourKeys.has(phonemeKey(p))),
    notListed: [...ours].sort((a, b) => b.count - a.count).filter((s) => !theirKeys.has(phonemeKey(s.sound))).map((s) => s.sound),
  };
}

/** A share at or above this counts as common among the relatives; below `RARE` as rare. */
export const COMMON = 0.5;
export const RARE = 0.05;

export type RelativesCheck = { missing: PhoibleShare[]; rare: { sound: string; languages: number }[] };

/** Sounds common among the relatives that the transcriptions lack, and
 * sounds in the transcriptions that few or none of the relatives have. */
export function againstRelatives(ours: readonly { sound: string; count: number }[], c: Pick<PhoibleCompared, "shares" | "relatives">): RelativesCheck {
  if (c.relatives === 0) return { missing: [], rare: [] };
  const ourKeys = new Set(ours.map((s) => phonemeKey(s.sound)));
  const byKey = new Map<string, number>();
  for (const s of c.shares) byKey.set(phonemeKey(s.phoneme), Math.max(byKey.get(phonemeKey(s.phoneme)) ?? 0, s.languages));
  return {
    missing: c.shares.filter((s) => s.languages / c.relatives >= COMMON && !ourKeys.has(phonemeKey(s.phoneme))),
    rare: [...ours]
      .sort((a, b) => b.count - a.count)
      .map((s) => ({ sound: s.sound, languages: byKey.get(phonemeKey(s.sound)) ?? 0 }))
      .filter((s) => s.languages / c.relatives < RARE),
  };
}

/** A share in words: "41 of 52 (79%)". */
export const shareOf = (n: number, of: number): string => `${n} of ${of} (${Math.round((100 * n) / of)}%)`;

/** The line naming what PHOIBLE has, under the section's heading. */
export function phoibleLine(c: PhoibleCompared): string {
  const own =
    c.own.length === 0
      ? `PHOIBLE has no inventory of ${c.name}.`
      : `PHOIBLE has ${c.own.length === 1 ? "1 inventory" : `${c.own.length} inventories`} of ${c.name} (${[...new Set(c.own.map((i) => i.source.toUpperCase()))].join(", ")}).`;
  const rel = c.relatives === 0 ? "" : ` Its relatives are ${c.relatives} ${c.comparedWith} with an inventory there.`;
  return `${own}${rel} A sound is matched as written, apart from tie bars; write affricates with a tie bar (t͡ʃ) so they count as one sound.`;
}
