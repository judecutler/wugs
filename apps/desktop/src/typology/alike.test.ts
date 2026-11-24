import { test } from "node:test";
import assert from "node:assert/strict";
import { alikeLine, alikeTsv, percentOf } from "./alike.ts";

const l = (name: string, family: string, compared: number, same: number) => ({ glottocode: name.toLowerCase().slice(0, 4) + "1234", name, family, macroarea: "Eurasia", compared, same });

test("the list's line and table", () => {
  const r = { answered: 40, least: 20, languages: [l("Dutch", "Indo-European", 38, 36), l("Frisian", "Indo-European", 30, 27), l("Basque", "isolate", 40, 28)] };
  assert.match(alikeLine(r, "Grambank", "Kanu"), /^Compared on the 40 features answered \(each language on those it is coded for, 20 at least\): Dutch agrees most, on 36 of 38\. The closest ten come from one family, Indo-European\./u);
  assert.equal(percentOf(r.languages[0]!), "95%");
  assert.equal(alikeTsv(r).split("\n")[3], "basq1234\tBasque\tisolate\tEurasia\t40\t28\t0.700");
  assert.match(alikeLine({ answered: 3, least: 5, languages: [] }, "WALS", "Kanu"), /^Answer at least five WALS features/u);
  assert.match(alikeLine({ answered: 8, least: 5, languages: [] }, "WALS", "Kanu"), /^No language in WALS is coded on 5 or more/u);
});
