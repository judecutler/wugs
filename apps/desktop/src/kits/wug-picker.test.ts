import assert from "node:assert/strict";
import { test } from "node:test";
import { ACCESSORIES, wugLayers } from "../theme/wug.ts";
import { accessoryColour, paint, recolourable, SLOTS, wear, wornIn } from "./wug-picker.ts";

const coral = { body: "#f08060", accessories: ["hat", "recorder"] };
const fill = (look: Parameters<typeof wugLayers>[0], part: string) => wugLayers(look).find((l) => l.id === part)?.fill;

test("every accessory is offered in exactly one slot", () => {
  const offered = SLOTS.flatMap((s) => s.accessories);
  assert.deepEqual([...offered].sort(), Object.keys(ACCESSORIES).sort());
  assert.deepEqual(SLOTS.find((s) => s.slot === "head")!.accessories, ["hat", "crown", "antennae"]);
});

test("only an accessory's own solid shapes take a colour", () => {
  assert.deepEqual(recolourable("hat"), ["hat-shape"]);
  assert.deepEqual(recolourable("antennae"), ["antenna-left-shape", "antenna-right-shape"]);
  assert.deepEqual(recolourable("recorder"), ["recorder-shape"], "the paper under it stays white");
  assert.deepEqual(recolourable("spacesuit"), ["spacesuit-base-shape"], "the visor stays glass");
  for (const a of ["monocle", "glasses", "scroll", "tablet", "number-one"] as const) assert.deepEqual(recolourable(a), [], a);
});

test("what a slot wears is what gets drawn", () => {
  assert.equal(wornIn(coral, "head"), "hat");
  assert.equal(wornIn(coral, "face"), null);
  assert.equal(wornIn({ body: "#000000", accessories: ["hat", "crown"] }, "head"), "crown");
});

test("wearing replaces the slot's accessory and keeps the others", () => {
  const crowned = wear(coral, "head", "crown");
  assert.deepEqual(crowned.accessories, ["recorder", "crown"]);
  assert.deepEqual(wear(coral, "held", null).accessories, ["hat"]);
  assert.equal(crowned.body, coral.body);
});

test("taking an accessory off drops its colours but keeps parts this build does not know", () => {
  const look = { body: "#000000", accessories: ["hat", "jetpack"], colours: { "hat-shape": "#111111", "jetpack-shape": "#222222" } };
  const bare = wear(look, "head", null);
  assert.deepEqual(bare.accessories, ["jetpack"]);
  assert.deepEqual(bare.colours, { "jetpack-shape": "#222222" });
  assert.equal(wear({ body: "#000000", accessories: ["hat"], colours: { "hat-shape": "#111111" } }, "head", null).colours, undefined);
});

test("a hand-written colour for a part still drawn survives another slot's change", () => {
  const look = { body: "#000000", accessories: ["scroll"], colours: { "paper-shape": "#eeddcc" } };
  assert.deepEqual(wear(look, "head", "hat").colours, { "paper-shape": "#eeddcc" });
  assert.deepEqual(wear(look, "held", "tablet").colours, { "paper-shape": "#eeddcc" }, "the tablet draws the same paper");
  assert.equal(wear(look, "held", null).colours, undefined);
});

test("painting colours every shape of the accessory and shows in the drawing", () => {
  const look = paint({ body: "#000000", accessories: ["antennae"] }, "antennae", "#00ff00");
  assert.equal(fill(look, "antenna-left-shape"), "#00ff00");
  assert.equal(fill(look, "antenna-right-shape"), "#00ff00");
  assert.equal(accessoryColour(look, "antennae"), "#00ff00");
});

test("an accessory's colour is the drawing's own until painted", () => {
  assert.equal(accessoryColour(coral, "hat"), "#a1691c");
  assert.equal(accessoryColour(coral, "monocle"), null);
});
