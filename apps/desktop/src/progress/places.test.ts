import assert from "node:assert/strict";
import { test } from "node:test";
import { parseCoordinates, placeOf, placesNote, textPlaces } from "./places.ts";

const near = (a: number, b: number) => Math.abs(a - b) < 1e-4;

test("coordinates read in the usual writings", () => {
  assert.deepEqual(parseCoordinates("-8.65, 115.22"), { lat: -8.65, lon: 115.22 });
  assert.deepEqual(parseCoordinates("−8.65 115.22"), { lat: -8.65, lon: 115.22 });
  assert.deepEqual(parseCoordinates("8.65 S, 115.22 E"), { lat: -8.65, lon: 115.22 });
  assert.deepEqual(parseCoordinates("115.22E 8.65S"), { lat: -8.65, lon: 115.22 }, "longitude first, by its hemisphere");
  const dms = parseCoordinates(`8°39'0"S 115°13'12"E`)!;
  assert.ok(near(dms.lat, -8.65) && near(dms.lon, 115.22), JSON.stringify(dms));
  assert.equal(parseCoordinates("the village by the river"), null);
  assert.equal(parseCoordinates("95, 10"), null, "no latitude past the pole");
  assert.equal(parseCoordinates("12"), null);
});

test("a text's place from its details", () => {
  assert.deepEqual(placeOf([["place", "Ubud"], ["gps", "-8.5, 115.26"]]), { lat: -8.5, lon: 115.26 });
  assert.deepEqual(placeOf([["latitude", "8.5S"], ["longitude", "115.26"]]), { lat: -8.5, lon: 115.26 });
  assert.equal(placeOf([["place", "Ubud"]]), null);
});

test("texts placed, and those with a place in words only", () => {
  const found = textPlaces([
    { title: "A", meta: [["coordinates", "-8.5, 115.26"]] },
    { title: "B", meta: [["coordinates", "-8.5, 115.26"]] },
    { title: "C", meta: [["place", "Ubud"]] },
    { title: "D", meta: [] },
  ]);
  assert.equal(found.places.length, 2);
  assert.equal(found.wordsOnly, 1);
  assert.match(placesNote(found, 4), /^2 texts of 4 placed, at 1 site\. 1 text names a place in words only/u);
});
