// The Kit Builder's wug picker as data: the slots and what each can hold,
// which of an accessory's shapes take a colour, and edits to a look. Pure,
// so it runs under node --test.
import drawing from "../theme/wug-parts.json" with { type: "json" };
import { ACCESSORIES, isAccessory, wugLayers, type AccessoryId, type Slot, type WugLook } from "../theme/wug.ts";

const SLOT_WORDS: readonly [Slot, string][] = [
  ["head", "Head"],
  ["face", "Face"],
  ["held", "Holding"],
  ["suit", "Suit"],
  ["badge", "Badge"],
];

const IDS = Object.keys(ACCESSORIES) as AccessoryId[];

/** The picker's rows: each slot in words, with the accessories it can hold. */
export const SLOTS: readonly { slot: Slot; label: string; accessories: readonly AccessoryId[] }[] = SLOT_WORDS.map(([slot, label]) => ({
  slot,
  label,
  accessories: IDS.filter((a) => ACCESSORIES[a].slot === slot),
}));

const PART_IDS = new Set(drawing.parts.map((p) => p.id));
const OPAQUE_FILL = new Map(drawing.parts.filter((p) => p.kind === "shape" && p.opacity === 1).map((p) => [p.id, p.fill]));
const owners = (part: string) => IDS.filter((a) => (ACCESSORIES[a].parts as readonly string[]).includes(part)).length;

/** The shapes that read as the accessory's own colour: opaque, and its own.
 * Lenses and the helmet's visor are see-through glass, and the white paper
 * is shared by everything held, so those keep the drawing's fill. */
export const recolourable = (a: AccessoryId): readonly string[] => ACCESSORIES[a].parts.filter((p) => OPAQUE_FILL.has(p) && owners(p) === 1);

/** What `look` wears in `slot`: the last one listed there, as it is drawn. */
export function wornIn(look: WugLook, slot: Slot): AccessoryId | null {
  let worn: AccessoryId | null = null;
  for (const a of look.accessories) if (isAccessory(a) && ACCESSORIES[a].slot === slot) worn = a;
  return worn;
}

/** `look` wearing `a` in `slot`, or nothing there for null. Colours of
 * shapes no longer drawn are dropped so the file holds only what shows;
 * ids and parts this build does not know (a newer kit) are kept. */
export function wear(look: WugLook, slot: Slot, a: AccessoryId | null): WugLook {
  const accessories = look.accessories.filter((x) => !(isAccessory(x) && ACCESSORIES[x].slot === slot));
  if (a !== null) accessories.push(a);
  const drawn = new Set(wugLayers({ body: look.body, accessories }).map((l) => l.id));
  const colours = Object.fromEntries(Object.entries(look.colours ?? {}).filter(([part]) => drawn.has(part) || !PART_IDS.has(part)));
  return { body: look.body, accessories, ...(Object.keys(colours).length === 0 ? {} : { colours }) };
}

/** The colour `a` shows in `look`, or null when it has none to change. */
export function accessoryColour(look: WugLook, a: AccessoryId): string | null {
  const first = recolourable(a)[0];
  return first === undefined ? null : (look.colours?.[first] ?? OPAQUE_FILL.get(first)!);
}

/** `look` with every recolourable shape of `a` filled with `colour`. */
export function paint(look: WugLook, a: AccessoryId, colour: string): WugLook {
  const colours = { ...look.colours };
  for (const part of recolourable(a)) colours[part] = colour;
  return { ...look, colours };
}
