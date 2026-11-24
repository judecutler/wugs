// Topics: sentences filed under a grammar topic while glossing, gathered for
// writing a grammar (src-tauri/src/topics.rs). Pure: no DOM.

export type TopicSentence = { textId: string; textTitle: string; segmentId: string; tierId: string; content: string; translation: string | null; start: number | null };
export type Topic = { name: string; sentences: TopicSentence[] };
export type Topics = { topics: Topic[] };

const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** The topics whose name holds the typed words, in order. */
export function matchTopics(topics: readonly Topic[], query: string): Topic[] {
  const words = fold(query).split(/\s+/u).filter(Boolean);
  return topics.filter((t) => words.every((w) => fold(t.name).includes(w)));
}

/** The line above the list. */
export function topicsLine(t: Topics): string {
  if (t.topics.length === 0) return "No sentence is filed under a topic yet. In the Gloss panel, type a topic under a sentence (such as negation or relative clauses) to start collecting examples for a grammar chapter.";
  const sentences = new Set(t.topics.flatMap((x) => x.sentences.map((s) => s.segmentId))).size;
  const texts = new Set(t.topics.flatMap((x) => x.sentences.map((s) => s.textId))).size;
  const n = (x: number, one: string, many: string) => `${x.toLocaleString("en")} ${x === 1 ? one : many}`;
  return `${n(t.topics.length, "topic", "topics")}, ${n(sentences, "sentence", "sentences")} from ${n(texts, "text", "texts")}.`;
}

/** Topics the Gloss panel offers while typing: known ones the sentence is not filed under, closest first. */
export function topicSuggestions(known: readonly string[], on: readonly string[], typed: string): string[] {
  const have = new Set(on.map(fold));
  const t = fold(typed.trim());
  const left = known.filter((k) => !have.has(fold(k)));
  if (t === "") return left;
  return left.filter((k) => fold(k).includes(t)).sort((a, b) => Number(!fold(a).startsWith(t)) - Number(!fold(b).startsWith(t)) || a.localeCompare(b));
}

/** One topic's sentences as a table: text, sentence, translation. */
export function topicTsv(topic: Topic): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ");
  return ["topic\ttext\tsentence\ttranslation", ...topic.sentences.map((s) => [clean(topic.name), clean(s.textTitle), clean(s.content), clean(s.translation ?? "")].join("\t"))].join("\n");
}
