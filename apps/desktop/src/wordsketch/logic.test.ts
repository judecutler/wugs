import assert from "node:assert/strict";
import { test } from "node:test";
import { diffLine, diffTsv, groupTitle, leaning, pairQuery, sketchDiff, sketchLine, sketchTsv, type WordSketch } from "./logic.ts";

const sketch: WordSketch = {
  lemma: "eat",
  count: 2,
  parsed: 3,
  groups: [
    { relation: "obj", direction: "dependent", total: 2, items: [{ lemma: "bone", count: 1, logDice: 13.41, after: 1 }] },
    { relation: "nsubj", direction: "dependent", total: 2, items: [{ lemma: "cat", count: 3, logDice: 12.5, after: 0 }] },
  ],
};

test("relations in words", () => {
  assert.equal(groupTitle(sketch.groups[0]!, "eat"), "Objects of «eat»");
  assert.equal(groupTitle({ relation: "nmod:poss", direction: "dependent", total: 1, items: [] }, "dog"), "Noun modifiers of «dog» (nmod:poss)");
  assert.equal(groupTitle({ relation: "obj", direction: "head", total: 1, items: [] }, "bone"), "Words «bone» is among the objects of");
  assert.equal(groupTitle({ relation: "conj", direction: "head", total: 1, items: [] }, "salt"), "Coordinated with «salt»");
  assert.equal(groupTitle({ relation: "xyz", direction: "dependent", total: 1, items: [] }, "a"), "xyz of «a»");
});

test("the line, the query and the copy", () => {
  assert.match(sketchLine({ lemma: "", count: 0, parsed: 0, groups: [] }, "x"), /^No sentence is parsed yet/);
  assert.equal(sketchLine({ lemma: "", count: 0, parsed: 12, groups: [] }, " giraffe "), "«giraffe» is not a lemma in the 12 parsed sentences.");
  assert.match(sketchLine(sketch, "eat"), /^«eat»: 2 uses in 3 parsed sentences, 2 relations\./);
  assert.equal(pairQuery("eat", sketch.groups[0]!.items[0]!), '[lemma="eat" %c] []{0,10} [lemma="bone" %c]');
  assert.equal(pairQuery("eat", sketch.groups[1]!.items[0]!), '[lemma="cat" %c] []{0,10} [lemma="eat" %c]');
  assert.equal(pairQuery("a.b", { lemma: "x", count: 1, logDice: 1, after: 1 }), '[lemma="a\\.b" %c] []{0,10} [lemma="x" %c]');
  assert.equal(sketchTsv(sketch), "relation\tdirection\tlemma\tcount\tlogDice\nobj\tdependent\tbone\t1\t13.41\nnsubj\tdependent\tcat\t3\t12.50");
});

test("a sketch difference sets two lemmas' collocates side by side", () => {
  const item = (lemma: string, count: number, logDice: number) => ({ lemma, count, logDice, after: 0 });
  const eat: WordSketch = { lemma: "eat", count: 20, parsed: 100, groups: [{ relation: "obj", direction: "dependent", total: 12, items: [item("bread", 6, 11), item("fish", 4, 9.5), item("soup", 2, 8)] }] };
  const drink: WordSketch = { lemma: "drink", count: 15, parsed: 100, groups: [{ relation: "obj", direction: "dependent", total: 9, items: [item("water", 5, 11.5), item("soup", 2, 8.5), item("fish", 1, 4)] }, { relation: "advmod", direction: "dependent", total: 2, items: [item("slowly", 2, 9)] }] };
  const d = sketchDiff(eat, drink);
  assert.deepEqual(d.map((g) => [g.relation, g.total]), [["obj", 21], ["advmod", 2]]);
  assert.deepEqual(d[0]!.items.map((i) => [i.lemma, leaning(i)]), [["bread", "a"], ["fish", "a"], ["soup", "both"], ["water", "b"]]);
  assert.equal(diffLine(eat, drink, d), "«eat» (20) against «drink» (15): 5 collocates in 2 relations, 1 used alike. Those typical of «eat» first, of «drink» last.");
  assert.equal(diffLine({ ...eat, lemma: "" }, drink, d), "One lemma is not in the parsed sentences.");
  assert.equal(diffTsv(eat, drink, d).split("\n")[1], "obj\tdependent\tbread\t6\t11.00\t0\t\t11.00");
});

test("valency frames in words, the line over them and the table", async () => {
  const { frameWords, framesLine, framesTsv } = await import("./logic.ts");
  assert.equal(frameWords("nsubj obj obl"), "subject + object + oblique");
  assert.equal(frameWords("nsubj:pass"), "passive subject");
  assert.equal(frameWords(""), "no core arguments");
  const frames = [
    { frame: "nsubj obj", count: 3, textId: "t", segmentId: "s1", tierId: "u", content: "Dogs eat\tbones." },
    { frame: "nsubj", count: 1, textId: "t", segmentId: "s2", tierId: "u", content: "Dogs eat." },
  ];
  assert.match(framesLine("eat", frames), /^«eat» as a verb: 4 uses in 2 frames, most often subject \+ object \(75%\)\./);
  assert.equal(framesLine("eat", []), "");
  assert.equal(framesTsv("eat", frames).split("\n")[1], "eat\tnsubj obj\tsubject + object\t3\tDogs eat bones.");
});
