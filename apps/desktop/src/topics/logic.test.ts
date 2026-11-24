import assert from "node:assert/strict";
import { test } from "node:test";
import { matchTopics, topicSuggestions, topicsLine, topicTsv, type Topic } from "./logic.ts";

const s = (textId: string, segmentId: string, content: string, translation: string | null = null) => ({ textId, textTitle: textId === "a" ? "Story" : "List", segmentId, tierId: "t", content, translation, start: null });
const topics: Topic[] = [
  { name: "Négation", sentences: [s("a", "1", "ka ni", "not him"), s("b", "2", "te")] },
  { name: "Relative clauses", sentences: [s("a", "1", "ka ni", "not him")] },
];

test("topics are found by any word of their name, accents aside", () => {
  assert.deepEqual(matchTopics(topics, "negation").map((t) => t.name), ["Négation"]);
  assert.deepEqual(matchTopics(topics, "rel claus").map((t) => t.name), ["Relative clauses"]);
  assert.equal(matchTopics(topics, "").length, 2);
});

test("the line counts sentences once", () => {
  assert.equal(topicsLine({ topics }), "2 topics, 2 sentences from 2 texts.");
  assert.match(topicsLine({ topics: [] }), /^No sentence is filed under a topic yet/u);
});

test("suggestions leave out the sentence's own topics, starts first", () => {
  const known = ["Negation", "Clausal negation", "Relative clauses"];
  assert.deepEqual(topicSuggestions(known, ["relative clauses"], ""), ["Negation", "Clausal negation"]);
  assert.deepEqual(topicSuggestions(known, [], "neg"), ["Negation", "Clausal negation"]);
});

test("a topic as a table", () => {
  assert.equal(topicTsv(topics[0]), "topic\ttext\tsentence\ttranslation\nNégation\tStory\tka ni\tnot him\nNégation\tList\tte\t");
});
