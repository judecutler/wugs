// Tagging and parsing (the `morphosyntax` capability): what a run did, in words.
// Pure, so node --test runs it.

export type Tagged = { annotated: number; tokenized: number; kept: number; differ: number; words: number; by: string };

const n = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;

/** The toast after a run. */
export function taggedLines(r: Tagged, title: string): string[] {
  if (r.annotated === 0 && r.differ === 0) {
    return r.kept > 0 ? [`Every sentence of «${title}» has tags already; nothing was sent.`] : [`«${title}» has no transcribed sentences to tag.`];
  }
  const lines = [`Tagged and parsed ${n(r.annotated, "sentence", "sentences")} of «${title}» (${n(r.words, "word", "words")}) with ${r.by}.`];
  if (r.tokenized > 0) lines.push(`${n(r.tokenized, "sentence was", "sentences were")} broken into words the tagger's way.`);
  if (r.kept > 0) lines.push(`${n(r.kept, "sentence keeps its", "sentences keep their")} own tags.`);
  if (r.differ > 0) lines.push(`${n(r.differ, "sentence was", "sentences were")} left alone: ${r.differ === 1 ? "its words are" : "their words are"} split differently from the tagger's. Retokenize in the Gloss panel to tag ${r.differ === 1 ? "it" : "them"}.`);
  lines.push("Check them in this panel: a tagger's guess is a start, not an answer.");
  return lines;
}

/** The note beside the web service's allow box. */
export const WEB_NOTE =
  "Sends this project's open texts to ÚFAL's UDPipe service in Prague. Its terms let ÚFAL keep what is sent, and its models are for non-commercial use. Texts that are not open are never sent.";

/** Which setup rows the Tagger… section shows: what the person chose (the
 * project's pin, else this computer's choice), and for Automatic what it
 * found, or both local taggers when it found neither. */
export function setupFor(v: { machine: string; pin: string | null; effective: string | null }): "udpipe" | "spacy" | "stanza" | "web" | "local" {
  const chosen = v.pin ?? v.machine;
  if (chosen === "udpipe-lindat") return "web";
  if (chosen === "spacy" || chosen === "stanza" || chosen === "udpipe") return chosen;
  const found = v.effective;
  return found === "spacy" || found === "stanza" || found === "udpipe" ? found : "local";
}

/** The line under Stanza's fields. */
export function stanzaNote(s: { pipeline: string | null; about: string | null } | null): string {
  if (s?.pipeline && s.about) return `Checked: ${s.about}.`;
  return "Stanza runs in your own Python, with models trained on Universal Dependencies for about 70 languages: install it with `python -m pip install stanza`, download a language's models once with `python -c \"import stanza; stanza.download('de')\"`, then type its code (de) and press Use.";
}

/** The line under spaCy's fields. */
export function spacyNote(s: { pipeline: string | null; about: string | null } | null): string {
  if (s?.pipeline && s.about) return `Checked: ${s.about}.`;
  return "spaCy runs in your own Python: install it with `python -m pip install spacy`, a pipeline for the language with `python -m spacy download en_core_web_sm` (or another from spacy.io/models), then type the pipeline's name and press Use.";
}
