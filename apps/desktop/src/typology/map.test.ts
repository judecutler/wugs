import assert from "node:assert/strict";
import { test } from "node:test";
import { colourOf, COLOURS, legend, mapNote, mapView, UNKNOWN, type FeatureMap } from "./map.ts";

const codes = [
  { value: "0", label: "absent" },
  { value: "1", label: "present" },
];
const pt = (name: string, lat: number, lon: number, value: string | null, me = false) => ({ glottocode: name.toLowerCase(), name, lat, lon, value, me });

test("colours follow the codes, grey for not known", () => {
  assert.equal(colourOf("0", codes), COLOURS[0]);
  assert.equal(colourOf("1", codes), COLOURS[1]);
  assert.equal(colourOf("?", codes), UNKNOWN);
  assert.equal(colourOf(null, codes), UNKNOWN);
  assert.equal(colourOf("9", codes), UNKNOWN);
});

test("the view frames the points, across the Pacific when that is tighter", () => {
  const africa = mapView([pt("A", 10, 10, "0"), pt("B", -5, 30, "1")]);
  assert.equal(africa.pacific, false);
  const b = africa.box;
  for (const p of africa.placed) assert.ok(p.x > b.x && p.x < b.x + b.w && p.y > b.y && p.y < b.y + b.h);
  assert.ok(b.w >= 24 && b.h >= b.w / 2 - 1e-9);
  // Madagascar to Rapa Nui: the short way round is east across the Pacific.
  const austronesian = mapView([pt("Malagasy", -19, 47, "1"), pt("Tagalog", 14, 121, "0"), pt("Rapanui", -27, -109, "1")]);
  assert.equal(austronesian.pacific, true);
  const rapanui = austronesian.placed.find((p) => p.name === "Rapanui")!;
  assert.equal(rapanui.x, -109 + 540);
  assert.ok(austronesian.box.w < 360);
  assert.deepEqual(mapView([]).box, { x: 0, y: 0, w: 360, h: 180 });
});

test("legend and note in words", () => {
  const m: FeatureMap = {
    feature: "GB020",
    question: "Are there definite articles?",
    codes,
    comparedWith: "Austronesian languages",
    points: [pt("A", 0, 100, "1"), pt("B", 0, 101, "1"), pt("C", 0, 102, "?"), pt("Me", 0, 103, "0", true)],
    unplaced: 2,
    land: "M0 0Z",
    edition: "Grambank 1.0",
  };
  assert.deepEqual(legend(m).map((r) => [r.value, r.count, r.mine]), [["0", 0, true], ["1", 2, false], ["?", 1, false]]);
  assert.equal(mapNote(m), "3 Austronesian languages with an answer (2 more that Glottolog does not place). Me is ringed. Land from Natural Earth; places from Glottolog; answers from Grambank 1.0.");
  assert.match(mapNote({ ...m, points: m.points.filter((p) => !p.me) }), /gives the language no place/u);
});
