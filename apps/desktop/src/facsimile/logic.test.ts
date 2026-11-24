import { test } from "node:test";
import assert from "node:assert/strict";
import { areasOn, facsimileLine, nextToMark, pageOf, pointIn, rectFrom, zoomBy, type FacsimileView } from "./logic.ts";

test("a drag becomes a rectangle on the page, and a click does not", () => {
  assert.deepEqual(rectFrom({ x: 0.6, y: 0.4 }, { x: 0.2, y: 0.1 }, "p1"), { source: "p1", x: 0.2, y: 0.1, w: 0.4, h: 0.3 });
  assert.deepEqual(rectFrom({ x: -0.2, y: 0.5 }, { x: 0.3, y: 1.4 }, "p1"), { source: "p1", x: 0, y: 0.5, w: 0.3, h: 0.5 }, "clamped to the page");
  assert.equal(rectFrom({ x: 0.5, y: 0.5 }, { x: 0.501, y: 0.6 }, "p1"), null);
  assert.deepEqual(pointIn(150, 60, { left: 100, top: 20, width: 200, height: 400 }), { x: 0.25, y: 0.1 });
});

const view: FacsimileView = {
  pages: [
    { source: "p1", label: "a.png", path: "/x/a" },
    { source: "p2", label: "b.png", path: null },
  ],
  marked: [
    { segment: "s1", tier: "t", content: "a-na", area: { source: "p1", x: 0, y: 0, w: 0.5, h: 0.1 } },
    { segment: "s3", tier: "t", content: "ti", area: { source: "p2", x: 0, y: 0, w: 0.5, h: 0.1 } },
  ],
};

test("areas belong to their pages", () => {
  assert.deepEqual(areasOn(view, "p2").map((m) => m.segment), ["s3"]);
  assert.equal(pageOf(view, "s3"), 1);
  assert.equal(pageOf(view, "s2"), -1);
});

test("the next line to mark skips marked ones and other tracks", () => {
  const segs = [
    { id: "s1", tier: "t" },
    { id: "x", tier: "translation" },
    { id: "s3", tier: "t" },
    { id: "s4", tier: "t" },
  ];
  assert.equal(nextToMark(segs, view, "s1"), "s4");
  assert.equal(nextToMark(segs, view, "s4"), null);
});

test("the line above the page and the zoom steps", () => {
  assert.equal(facsimileLine(view, 4), "2 pages · 2 of 4 segments marked on them.");
  assert.match(facsimileLine({ pages: [], marked: [] }, 0), /^No page images yet/);
  assert.equal(zoomBy(1, 1), 1.5);
  assert.equal(zoomBy(1, -1), 0.75);
  assert.equal(zoomBy(4, 1), 4);
});
