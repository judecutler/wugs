import assert from "node:assert/strict";
import { test } from "node:test";
import { readerDetails, readerHtml } from "./booklet.ts";

const base = {
  title: "The fox & the well",
  language: "Kanu",
  details: [["Speakers", "Ana"]] as [string, string][],
  sentences: [
    { number: 1, text: "kesi <mopa>", translation: "The fox drinks." },
    { number: 2, text: "tela senu", translation: null },
  ],
  date: "2026-11-23",
};

test("side by side when there are translations, escaped", () => {
  const html = readerHtml({ ...base, layout: "beside" });
  assert.match(html, /<title>The fox &amp; the well<\/title>/);
  assert.match(html, /<td class="tx">kesi &lt;mopa&gt;<\/td><td class="tr">The fox drinks\.<\/td>/);
  assert.match(html, /<td class="tx">tela senu<\/td><td class="tr"><\/td>/, "an untranslated sentence keeps its row");
  assert.match(html, /<p class="details">Speakers: Ana<\/p>/);
});

test("one under the other, and a text with no translations at all", () => {
  const under = readerHtml({ ...base, layout: "under" });
  assert.match(under, /<p class="tx">kesi &lt;mopa&gt;<\/p><p class="tr">The fox drinks\.<\/p>/);
  assert.doesNotMatch(under, /<table>/);
  const bare = readerHtml({ ...base, layout: "beside", sentences: [{ number: 1, text: "a", translation: null }] });
  assert.doesNotMatch(bare, /<table>/, "no empty column when nothing is translated");
});

test("the details a reader prints", () => {
  assert.deepEqual(
    readerDetails([
      ["genre", "story"],
      ["speakers", "Ana; Ben"],
      ["licence", "CC BY"],
      ["date", " "],
    ]),
    [
      ["Speakers", "Ana; Ben"],
      ["Genre", "story"],
    ],
  );
});

test("a conversation names each speaker", () => {
  const html = readerHtml({ title: "Lunch", language: "English", details: [], layout: "under", date: "2026-11-24", sentences: [{ number: 1, text: "look", translation: null, speaker: "MOT" }, { number: 2, text: "ball <", translation: null, speaker: "CHI" }] });
  assert.match(html, /<b class="who">MOT:<\/b> look/u);
  assert.match(html, /<b class="who">CHI:<\/b> ball &lt;/u);
});
