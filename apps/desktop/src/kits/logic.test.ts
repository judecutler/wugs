import assert from "node:assert/strict";
import { test } from "node:test";
import { addsFor, describeLayout, kitNotes, placementsFrom, withoutPanel, type Kit, type Placement, type SerializedLayout } from "./logic.ts";

const tongues: Placement[] = [
  { panel: "timeline" },
  { panel: "media", place: "left", of: "timeline", width: 260 },
  { panel: "texts", place: "tab", of: "media" },
  { panel: "lexicon", place: "right", of: "timeline", width: 320 },
  { panel: "transcribe", place: "below", of: "timeline", height: 260 },
];

test("placements become addPanel calls", () => {
  assert.deepEqual(addsFor(tongues, () => true), [
    { id: "timeline", inactive: false },
    { id: "media", position: { referencePanel: "timeline", direction: "left" }, initialWidth: 260, inactive: false },
    { id: "texts", position: { referencePanel: "media", direction: "within" }, inactive: true },
    { id: "lexicon", position: { referencePanel: "timeline", direction: "right" }, initialWidth: 320, inactive: false },
    { id: "transcribe", position: { referencePanel: "timeline", direction: "below" }, initialHeight: 260, inactive: false },
  ]);
});

test("a left-out panel does not drop the ones placed against it", () => {
  const adds = addsFor(tongues, (p) => p !== "media");
  assert.deepEqual(
    adds.map((a) => [a.id, a.position?.referencePanel, a.position?.direction]),
    [
      ["timeline", undefined, undefined],
      ["texts", "timeline", "within"],
      ["lexicon", "timeline", "right"],
      ["transcribe", "timeline", "below"],
    ],
  );
  const noFirst = addsFor(tongues, (p) => p !== "timeline");
  assert.equal(noFirst[0].id, "media");
  assert.equal(noFirst[0].position, undefined);
  assert.ok(noFirst.slice(1).every((a) => a.position !== undefined));
});

test("a size only applies along its split", () => {
  const [, a] = addsFor([{ panel: "a" }, { panel: "b", place: "below", of: "a", width: 300 }], () => true);
  assert.equal(a.initialWidth, undefined);
});

const leaf = (views: string[], size?: number, activeView?: string) => ({ type: "leaf" as const, data: { views, activeView }, size });

test("a live arrangement turns back into placements, level by level", () => {
  // [media|texts] | [timeline over (transcribe | gloss)] | [lexicon|search]
  const live: SerializedLayout = {
    grid: {
      orientation: "HORIZONTAL",
      root: {
        type: "branch",
        data: [
          leaf(["media", "texts"], 260, "texts"),
          { type: "branch", size: 900, data: [leaf(["timeline"], 500), { type: "branch", size: 260, data: [leaf(["transcribe"], 450), leaf(["gloss"], 450)] }] },
          leaf(["lexicon", "search"], 320),
        ],
      },
    },
  };
  const { layout, active } = placementsFrom(live);
  assert.deepEqual(layout, [
    { panel: "media" },
    { panel: "timeline", place: "right", of: "media", width: 900 },
    { panel: "lexicon", place: "right", of: "timeline", width: 320 },
    { panel: "texts", place: "tab", of: "media" },
    { panel: "transcribe", place: "below", of: "timeline", height: 260 },
    { panel: "search", place: "tab", of: "lexicon" },
    { panel: "gloss", place: "right", of: "transcribe", width: 450 },
  ]);
  assert.deepEqual(active, ["texts"]);
  // Every placement names an anchor placed before it, so it replays.
  const seen = new Set<string>();
  for (const p of layout) {
    if (p.of !== undefined) assert.ok(seen.has(p.of), `${p.panel} placed against ${p.of} before it exists`);
    seen.add(p.panel);
  }
});

test("an empty dock has no placements", () => {
  assert.deepEqual(placementsFrom({ grid: { orientation: "HORIZONTAL", root: { type: "branch", data: [] } } }), { layout: [], active: [] });
});

test("sizes are clamped to what a kit file accepts", () => {
  const { layout } = placementsFrom({
    grid: { orientation: "VERTICAL", root: { type: "branch", data: [leaf(["a"], 10), leaf(["b"], 9999.4)] } },
  });
  assert.deepEqual(layout[1], { panel: "b", place: "below", of: "a", height: 4000 });
});

const full: Placement[] = [
  ...tongues,
  { panel: "gloss", place: "right", of: "transcribe" },
  { panel: "conflicts", place: "right", of: "gloss", width: 320 },
];

test("removing a panel hands its spot to the first panel beside it", () => {
  // Gloss and Conflicts stay on the bottom row: Gloss takes Transcribe's spot.
  assert.deepEqual(withoutPanel(full, "transcribe"), [
    { panel: "timeline" },
    { panel: "media", place: "left", of: "timeline", width: 260 },
    { panel: "texts", place: "tab", of: "media" },
    { panel: "lexicon", place: "right", of: "timeline", width: 320 },
    { panel: "gloss", place: "below", of: "timeline", height: 260 },
    { panel: "conflicts", place: "right", of: "gloss", width: 320 },
  ]);
  // Only a tab was placed against Media, so it keeps the left column.
  assert.deepEqual(withoutPanel(tongues, "media").slice(0, 2), [
    { panel: "timeline" },
    { panel: "texts", place: "left", of: "timeline", width: 260 },
  ]);
  // The first panel: the first one beside it fills the workspace instead.
  assert.deepEqual(withoutPanel(tongues, "timeline"), [
    { panel: "media" },
    { panel: "texts", place: "tab", of: "media" },
    { panel: "lexicon", place: "right", of: "media", width: 320 },
    { panel: "transcribe", place: "below", of: "media", height: 260 },
  ]);
  assert.equal(withoutPanel(tongues, "nowhere"), tongues);
  assert.deepEqual(withoutPanel([{ panel: "a" }], "a"), []);
  assert.deepEqual(withoutPanel(full, "conflicts"), full.slice(0, -1));
  // A tab's group survives it: what was placed against the tab stays beside that group.
  assert.deepEqual(
    withoutPanel(
      [{ panel: "timeline" }, { panel: "lexicon", place: "right", of: "timeline" }, { panel: "elicit", place: "tab", of: "lexicon" }, { panel: "gloss", place: "below", of: "elicit" }],
      "elicit",
    ),
    [{ panel: "timeline" }, { panel: "lexicon", place: "right", of: "timeline" }, { panel: "gloss", place: "below", of: "lexicon" }],
  );
});

test("every panel can be removed and the rest still replays", () => {
  for (const p of full) {
    const rest = withoutPanel(full, p.panel);
    assert.equal(rest.length, full.length - 1);
    const seen = new Set<string>();
    rest.forEach((q, i) => {
      if (i === 0) assert.equal(q.of, undefined, `${p.panel}: first placement anchored`);
      else assert.ok(q.of !== undefined && seen.has(q.of), `${p.panel}: ${q.panel} placed against ${q.of} before it exists`);
      seen.add(q.panel);
    });
  }
});

test("layouts read as sentences", () => {
  const title = (p: string) => p[0].toUpperCase() + p.slice(1);
  assert.deepEqual(describeLayout(tongues.slice(0, 3), title), [
    "Timeline fills the workspace",
    "Media to the left of Timeline, 260 px",
    "Texts as a tab with Media",
  ]);
});

test("notes name what this version cannot give", () => {
  const kit: Kit = {
    id: "k",
    name: "K",
    subtitle: "",
    version: "",
    description: "",
    tools: ["lexicon", "treebank"],
    layout: [{ panel: "lexicon" }, { panel: "search", place: "tab", of: "lexicon" }, { panel: "conflicts", place: "tab", of: "lexicon" }, { panel: "map", place: "tab", of: "lexicon" }],
    active: [],
  };
  const known = (p: string) => ["lexicon", "search", "conflicts"].includes(p);
  const platform = (p: string) => p === "conflicts";
  assert.deepEqual(kitNotes(kit, known, platform), [
    "Names tools this version does not have: treebank.",
    "Places panels the kit does not turn on, so they stay closed: search.",
    "Places panels this version does not have: map.",
  ]);
});
