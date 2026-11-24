import assert from "node:assert/strict";
import { test } from "node:test";
import { asTsv, kwicHtml, clip, countLine, hasTags, isRtl, withTags, keyword, left, right, sortHits, type Hit, speakerOf } from "./logic.ts";

const hit = (words: string[], at: number, title = "T"): Hit => ({ textId: title, textTitle: title, segmentId: words.join("_"), tierId: "u", startMs: 0, endMs: 1, words, at, breakdown: null, glosses: null });

test("keyword in the middle with its context either side", () => {
  const h = hit(["ni", "kani", "ŋai"], 1);
  assert.equal(left(h), "ni");
  assert.equal(keyword(h), "kani");
  assert.equal(right(h), "ŋai");
});

test("sorting by the word before compares the nearest word first", () => {
  const hits = [hit(["b", "z", "KEY"], 2), hit(["c", "a", "KEY"], 2), hit(["KEY"], 0)];
  assert.deepEqual(sortHits(hits, "left").map((h) => h.words.join(" ")), ["KEY", "c a KEY", "b z KEY"]);
  assert.deepEqual(sortHits(hits, "text"), hits);
});

test("sorting by the word after ignores case but not diacritics", () => {
  const hits = [hit(["KEY", "Zeta"], 0), hit(["KEY", "abd"], 0), hit(["KEY", "Abc"], 0)];
  assert.deepEqual(sortHits(hits, "right").map((h) => right(h)), ["Abc", "abd", "Zeta"]);
  // na and ŋa are not ties: their order is stable and not by input order alone.
  const a = sortHits([hit(["KEY", "ŋa"], 0), hit(["KEY", "na"], 0)], "right").map((h) => right(h));
  const b = sortHits([hit(["KEY", "na"], 0), hit(["KEY", "ŋa"], 0)], "right").map((h) => right(h));
  assert.deepEqual(a, b);
});

test("right-to-left scripts are recognised", () => {
  assert.ok(isRtl("كتاب"));
  assert.ok(isRtl("ספר"));
  assert.ok(!isRtl("ŋai"));
});

test("long context is cut at a word, with … on the far side", () => {
  assert.equal(clip("one two three four", 9, "left"), "… four");
  assert.equal(clip("one two three four", 9, "right"), "one two …");
  assert.equal(clip("short", 9, "left"), "short");
});

test("the count line says what is shown", () => {
  assert.equal(countLine(0, 0, 0), "Nothing found.");
  assert.equal(countLine(1, 1, 1), "1 occurrence in 1 text.");
  assert.equal(countLine(2000, 2500, 4), "2500 found in 4 texts; the first 2000 are shown.");
});

test("tab-separated rows keep tabs out of the cells", () => {
  assert.equal(asTsv([hit(["a\tb", "K", "c"], 1, "Title")]), "left\tkeyword\tright\ttext\na b\tK\tc\tTitle");
  // With the texts' details: times, the analysis when any line has one, and a column per detail.
  const one = { ...hit(["a", "K", "c"], 1, "Title"), textId: "t1", startMs: 1500, endMs: 2250, breakdown: "k-a", glosses: "go-PST" };
  const two = { ...hit(["x", "K"], 1, "Other"), textId: "t2", startMs: null, endMs: null };
  const details = new Map([["t1", new Map([["genre", "story"], ["speaker", "A\tB"]])], ["t2", new Map([["genre", "talk"]])]]);
  assert.equal(
    asTsv([one, two], details),
    "left\tkeyword\tright\ttext\tstart_s\tend_s\tbreakdown\tglosses\tgenre\tspeaker\na\tK\tc\tTitle\t1.500\t2.250\tk-a\tgo-PST\tstory\tA B\nx\tK\t\tOther\t\t\t\t\ttalk\t",
  );
  assert.equal(asTsv([two], new Map()), "left\tkeyword\tright\ttext\tstart_s\tend_s\nx\tK\t\tOther\t\t");
});

test("a match of several words is the keyword whole, and the right context starts after it", () => {
  const h: Hit = { ...hit(["went", "to", "the", "park", "."], 1), len: 3 };
  assert.equal(keyword(h), "to the park");
  assert.equal(left(h), "went");
  assert.equal(right(h), ".");
  const other: Hit = { ...hit(["x", "to", "a", "b", "Alpha"], 1), len: 2 };
  // Sorting by the word after reads past the whole match.
  assert.deepEqual(sortHits([h, other], "right").map((x) => right(x)), [". ", "b Alpha"].map((s) => s.trim()));
});

test("the CQP chip is the Concordance's own; Frequencies keeps the four kinds", async () => {
  const { KINDS, PANEL_KINDS } = await import("./logic.ts");
  assert.equal(KINDS.length, 4);
  assert.equal(PANEL_KINDS.at(-1)?.id, "query");
});

test("a tagged line reads word_TAG", () => {
  const h = { textId: "t", textTitle: "T", segmentId: "s", tierId: "u", startMs: null, endMs: null, words: ["a", "el", "mar", "."], at: 0, len: 3, breakdown: null, glosses: null, tags: ["ADP", "DET", "NOUN", ""] };
  assert.equal(hasTags([h]), true);
  assert.equal(hasTags([{ ...h, tags: undefined }]), false);
  assert.equal(hasTags([{ ...h, tags: ["", "", "", ""] }]), false);
  const t = withTags(h);
  assert.equal(keyword(t), "a_ADP el_DET mar_NOUN");
  assert.equal(right(t), ".");
  assert.equal(withTags({ ...h, tags: undefined }).words[0], "a");
});

test("second-word and match sorts, as AntConc's sort levels", () => {
  const hits = [hit(["x", "b", "KEY", "q", "z"], 2), hit(["y", "a", "KEY", "p", "a"], 2), hit(["w", "c", "KEY"], 2)];
  // 2L first: w, x, y; ties broken by 1L.
  assert.deepEqual(sortHits(hits, "left2").map((h) => h.words[0]), ["w", "x", "y"]);
  // 2R first: the line with no second word after sorts first, then a, z.
  assert.deepEqual(sortHits(hits, "right2").map((h) => h.words[0]), ["w", "y", "x"]);
  // The match itself, then the word after: different matches of a pattern sort together.
  const glob = [hit(["kanu", "z"], 0), hit(["kani", "b"], 0), hit(["kani", "a"], 0)];
  assert.deepEqual(sortHits(glob, "keyword").map((h) => h.words.join(" ")), ["kani a", "kani b", "kanu z"]);
  // A phrase match counts its whole length before the word after.
  const phrase = { ...hit(["a", "b", "c", "d"], 1), len: 2 };
  assert.deepEqual(sortHits([phrase], "right2"), [phrase]);
});

test("the sorted word is marked in its context", async () => {
  const { markWord, sortedWord } = await import("./logic.ts");
  assert.deepEqual(markWord("a b c", "left", -1), ["a b ", "c", ""]);
  assert.deepEqual(markWord("a b c", "left", -2), ["a ", "b", " c"]);
  assert.deepEqual(markWord("x y", "right", 1), ["", "x", " y"]);
  assert.deepEqual(markWord("x y", "right", 2), ["x ", "y", ""]);
  assert.deepEqual(markWord("x", "right", 2), ["x", "", ""], "too short");
  assert.deepEqual(markWord("", "left", -1), ["", "", ""]);
  assert.deepEqual(markWord("…far b", "left", -2), ["…far b", "", ""], "a clipped word is not marked");
  assert.equal(sortedWord("left2"), -2);
  assert.equal(sortedWord("right"), 1);
  assert.equal(sortedWord("keyword"), null);
  assert.equal(sortedWord("text"), null);
});

test("lines copied as a KWIC table for a word processor", async () => {
  const { kwicHtml } = await import("./logic.ts");
  const html = kwicHtml([hit(["a", "<b>", "KEY", "c"], 2, "Fox & well")]);
  assert.match(html, /^<table /u);
  assert.match(html, /text-align:right">a &lt;b&gt;<\/td>/u, "left context right-aligned and escaped");
  assert.match(html, /<b>KEY<\/b>/u);
  assert.match(html, /Fox &amp; well/u);
  const long = kwicHtml([hit([..."abcdefghijklmnopqrstuvwxyz".split("").map((c) => c.repeat(4)), "KEY"], 26)], 20);
  assert.match(long, />…/u, "the left context is clipped on its far side");
});

test("with translations shown, copied lines carry them", () => {
  const one = hit(["kuri", "mopa"], 0, "Fish");
  const under = (h: Hit) => (h === one ? "the dog sleeps" : null);
  const tsv = asTsv([one], new Map(), under).split("\n");
  assert.equal(tsv[0], "left\tkeyword\tright\ttext\tstart_s\tend_s\ttranslation");
  assert.ok(tsv[1].endsWith("\tthe dog sleeps"));
  assert.ok(!asTsv([one], new Map(), () => null).includes("translation"), "no column when none is shown");
  assert.match(kwicHtml([one], 40, under), /<i>|font-style:italic[^>]*>‘the dog sleeps’/);
  assert.doesNotMatch(kwicHtml([one]), /italic/);
});

test("a conversation's lines name their speaker", () => {
  const hit = (tierId: string): Hit => ({ textId: "t", textTitle: "Lunch", segmentId: tierId, tierId, startMs: 0, endMs: 1000, words: ["big", "doggie"], at: 1, breakdown: null, glosses: null });
  const names = new Map([["m", "MOT"], ["c", "CHI"]]);
  assert.equal(speakerOf([hit("m"), hit("m")], names), null, "one track: nothing to tell apart");
  const who = speakerOf([hit("m"), hit("c")], names)!;
  assert.equal(who(hit("c")), "CHI");
  const tsv = asTsv([hit("m"), hit("c")], new Map(), undefined, who).split("\n");
  assert.equal(tsv[0], "left\tkeyword\tright\ttext\tspeaker\tstart_s\tend_s");
  assert.equal(tsv[2], "big\tdoggie\t\tLunch\tCHI\t0.000\t1.000");
});
