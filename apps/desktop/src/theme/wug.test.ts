import assert from "node:assert/strict";
import { test } from "node:test";
import drawing from "./wug-parts.json" with { type: "json" };
import { ACCESSORIES, kitWug, wugLayers, wugSvg } from "./wug.ts";

const ids = (look: Parameters<typeof wugLayers>[0]) => wugLayers(look).map((l) => l.id);

test("every accessory names parts the drawing has", () => {
  const have = new Set(drawing.parts.map((p) => p.id));
  for (const [id, a] of Object.entries(ACCESSORIES)) for (const p of a.parts) assert.ok(have.has(p), `${id}: ${p}`);
});

test("a plain wug is its body and eye", () => {
  assert.deepEqual(ids({ body: "#123456", accessories: [] }), ["body-shape", "body-line", "eye-shape", "eye-line"]);
});

test("the body takes the look's colour; lines stay black", () => {
  const layers = wugLayers({ body: "#123456", accessories: ["hat"] });
  assert.equal(layers.find((l) => l.id === "body-shape")!.fill, "#123456");
  assert.equal(layers.find((l) => l.id === "hat-line")!.fill, "#000000");
});

test("the recorder is its own line and shape over the paper's white fill", () => {
  const layers = wugLayers({ body: "#000", accessories: ["recorder"] });
  const held = layers.filter((l) => !l.id.startsWith("body") && !l.id.startsWith("eye"));
  assert.deepEqual(held.map((l) => l.id), ["paper-shape", "recorder-shape", "recorder-line"]);
  assert.equal(held[0]!.fill, "#ffffff");
});

test("parts stack in the drawing's order, whatever order accessories are listed", () => {
  const a = ids({ body: "#000", accessories: ["hat", "recorder"] });
  const b = ids({ body: "#000", accessories: ["recorder", "hat"] });
  assert.deepEqual(a, b);
  assert.ok(a.indexOf("recorder-line") < a.indexOf("hat-shape"));
});

test("one accessory per slot: the later one wins", () => {
  const got = ids({ body: "#000", accessories: ["hat", "crown"] });
  assert.ok(got.includes("crown-shape") && !got.includes("hat-shape"));
});

test("accessory colours override shapes only", () => {
  const layers = wugLayers({ body: "#000", accessories: ["hat"], colours: { "hat-shape": "#ff00ff", "hat-line": "#ff00ff" } });
  assert.equal(layers.find((l) => l.id === "hat-shape")!.fill, "#ff00ff");
  assert.equal(layers.find((l) => l.id === "hat-line")!.fill, "#000000");
});

test("see-through parts keep the drawing's 35% opacity", () => {
  const layers = wugLayers({ body: "#000", accessories: ["monocle", "spacesuit"] });
  assert.equal(layers.find((l) => l.id === "monocle-shape")!.opacity, 0.35);
  assert.equal(layers.find((l) => l.id === "spacesuit-mask-shape")!.opacity, 0.35);
});

test("built-in kits have their own wug; any other kit gets the brand wug", () => {
  assert.deepEqual(kitWug("tongues").accessories, ["hat", "recorder"]);
  assert.equal(kitWug("someones-kit").body, kitWug(undefined).body);
});

test("a kit id that names an Object property still gets the brand wug", () => {
  assert.equal(kitWug("constructor").body, kitWug(undefined).body);
  assert.equal(kitWug("toString").body, kitWug(undefined).body);
});

test("an accessory this build does not know is left off", () => {
  assert.deepEqual(ids({ body: "#000", accessories: ["jetpack" as never] }), ["body-shape", "body-line", "eye-shape", "eye-line"]);
});

test("svg markup escapes the label and colours", () => {
  const svg = wugSvg({ body: '"><x', accessories: [] }, 'a "b"');
  assert.ok(!svg.includes('"><x'));
  assert.ok(svg.includes('aria-label="a &quot;b&quot;"'));
});
