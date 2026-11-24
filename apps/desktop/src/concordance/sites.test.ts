import { test } from "node:test";
import assert from "node:assert/strict";
import { bySite, dotShare, hasSites, siteTitle, sitesLine, sitesTsv } from "./sites.ts";
import { plotDetails, type TextPlot } from "./plot.ts";

const text = (title: string, words: number, hits: number, coordinates?: string): TextPlot => ({ textId: title, title, words, hits, positions: [], ...(coordinates === undefined ? {} : { meta: [["coordinates", coordinates]] }) });

test("texts recorded at one place are summed into one site", () => {
  const texts = [text("A", 1000, 4, "-8.65, 115.22"), text("B", 1000, 0, "8°39′S 115°13′12″E"), text("C", 500, 5, "-8.10, 115.09"), text("D", 200, 1)];
  const found = bySite(texts);
  assert.equal(found.unplaced, 1);
  assert.equal(found.sites.length, 2);
  const denpasar = found.sites.find((s) => s.titles.includes("A"))!;
  assert.deepEqual([denpasar.titles, denpasar.words, denpasar.hits], [["A", "B"], 2000, 4]);
  assert.ok(hasSites(texts));
  assert.ok(!hasSites([text("D", 10, 1)]));
});

test("dots are sized by area and titled with their rate", () => {
  const found = bySite([text("A", 1000, 2, "1, 1"), text("B", 1000, 8, "2, 2"), text("C", 1000, 0, "3, 3")]);
  const [a, b, c] = found.sites;
  assert.equal(dotShare(b!, found.sites), 1);
  assert.equal(dotShare(a!, found.sites), 0.5);
  assert.equal(dotShare(c!, found.sites), 0);
  assert.equal(siteTitle(a!), "A: 2 matches, 2.0 per 1000 words");
  assert.equal(siteTitle(c!), "C: 0 matches, 0 per 1000 words");
  assert.equal(sitesLine(found), "Found at 2 sites of 3: each dot's size is its matches per 1000 words, an empty ring none.");
  assert.equal(sitesLine({ sites: found.sites, unplaced: 1 }).endsWith("1 text has no coordinates detail and is not on the map."), true);
  assert.equal(sitesTsv([a!]), "latitude\tlongitude\ttexts\twords\tmatches\tper 1000\n1.00000\t1.00000\tA\t1000\t2\t2.00\n");
});

test("coordinates are offered as a map, not as a detail to sum by", () => {
  assert.deepEqual(plotDetails([{ ...text("A", 10, 1, "1, 1"), meta: [["coordinates", "1, 1"], ["genre", "story"], ["latitude", "1"]] }]), ["genre"]);
});
