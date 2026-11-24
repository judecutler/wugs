import assert from "node:assert/strict";
import { test } from "node:test";
import { checklistSheet } from "./checklist.ts";

test("a checklist lists each entry with a box and room for a correction", () => {
  const html = checklistSheet(
    [
      { lemma: "kuri", homograph: null, posOfSpeech: "noun", glosses: ["dog", " "] },
      { lemma: "tela", homograph: 2, posOfSpeech: "", glosses: ["house", "home"] },
      { lemma: "a<b>", homograph: null, posOfSpeech: "", glosses: [] },
    ],
    "Kanu",
    "Every entry",
  );
  assert.match(html, /<title>Kanu lexicon: entries to check<\/title>/);
  assert.match(html, /Every entry: 3 entries/);
  assert.match(html, /<td class="n">1<\/td><td class="hw">kuri<\/td><td class="pos">noun<\/td><td>dog<\/td><td class="ok">☐<\/td>/);
  assert.match(html, /tela<sub>2<\/sub>.*<td>house; home<\/td>/);
  assert.match(html, /a&lt;b&gt;/);
  assert.match(checklistSheet([], "", "Needs a gloss"), /<h1>Lexicon: entries to check<\/h1>/);
});
