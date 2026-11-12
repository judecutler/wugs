// The user guide as searchable sections, and the search the help bot uses to
// pick what the model reads. Pure functions: no network, no Worker APIs.

/** Splits the guide's Markdown at `##` and `###` headings. Each section keeps
 * its heading path ("Segment the recording › Split, merge, and nudge"), so a
 * subsection's words are found together with what it is part of. */
export function sections(markdown) {
  const out = [];
  let h2 = "";
  let current = null;
  for (const line of markdown.split(/\r?\n/u)) {
    const m = /^(#{2,3})\s+(.+?)\s*$/u.exec(line);
    if (m) {
      if (current !== null) out.push(current);
      const title = m[2];
      if (m[1] === "##") h2 = title;
      current = { title: m[1] === "##" ? title : `${h2} › ${title}`, anchor: anchor(title), body: "" };
    } else if (current !== null) {
      current.body += `${line}\n`;
    }
  }
  if (current !== null) out.push(current);
  return out.map((s) => ({ ...s, body: s.body.trim() })).filter((s) => s.body !== "");
}

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to hyphens. */
export function anchor(title) {
  return title
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /gu, "-");
}

// Words too common in questions and the guide to tell sections apart.
const STOP = new Set(
  "a an and are as at be but by can do does for from how i if in into is it its me my of on or so than that the then there this to was what when where which who why will with you your wugs".split(" "),
);

/** Lower-cased words, stemmed so "segments", "segmenting" and "segment" meet. */
export function words(text) {
  return (text.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [])
    .filter((w) => !STOP.has(w))
    .map(stem);
}

/** A light English stemmer, enough to meet plurals and verb forms in the
 * guide: `files`→`file`, `glosses`→`gloss`, `boxes`→`box`, `segmenting`→`segment`. */
export function stem(w) {
  if (w.length <= 3) return w;
  if (/(?:ss|us|is)$/u.test(w)) return w;
  if (/(?:ches|shes|sses|xes|zes)$/u.test(w)) return w.slice(0, -2);
  if (/ies$/u.test(w) && w.length > 4) return `${w.slice(0, -3)}y`;
  if (/ing$/u.test(w) && w.length > 5) w = w.slice(0, -3);
  else if (/ed$/u.test(w) && w.length > 4) w = w.slice(0, -2);
  else if (/s$/u.test(w)) w = w.slice(0, -1);
  return tidy(w);
}

/** The tail the rules leave: `deleting`→`delet` and `delete` meet at
 * `delet`, `running`→`runn` becomes `run`. */
function tidy(w) {
  if (w.length >= 4 && w.endsWith("e")) w = w.slice(0, -1);
  if (/([bdfgklmnprt])\1$/u.test(w)) w = w.slice(0, -1);
  return w;
}

/** The sections that best answer `question`, best first, by BM25 over each
 * section's title (counted three times) and body, at most `limit` sections and
 * `maxWords` words in all. Sections sharing no word with the question are left
 * out, so an off-topic question reaches the model with nothing to quote. */
export function search(all, question, { limit = 4, maxWords = 1800 } = {}) {
  const docs = all.map((s) => {
    const title = words(s.title);
    const terms = [...title, ...title, ...title, ...words(s.body)];
    const tf = new Map();
    for (const t of terms) tf.set(t, (tf.get(t) ?? 0) + 1);
    return { s, tf, len: terms.length };
  });
  const avg = docs.reduce((n, d) => n + d.len, 0) / Math.max(docs.length, 1);
  const df = new Map();
  for (const d of docs) for (const t of d.tf.keys()) df.set(t, (df.get(t) ?? 0) + 1);
  const query = [...new Set(words(question))];
  const k1 = 1.2;
  const b = 0.75;
  const scored = docs
    .map((d) => {
      let score = 0;
      for (const t of query) {
        const f = d.tf.get(t) ?? 0;
        if (f === 0) continue;
        const n = df.get(t) ?? 0;
        const idf = Math.log(1 + (docs.length - n + 0.5) / (n + 0.5));
        score += idf * ((f * (k1 + 1)) / (f + k1 * (1 - b + (b * d.len) / avg)));
      }
      return { s: d.s, score };
    })
    .filter((x) => x.score > 0)
    .sort((a, b2) => b2.score - a.score);
  const picked = [];
  let used = 0;
  for (const { s } of scored) {
    if (picked.length === limit) break;
    const n = s.body.split(/\s+/u).length;
    if (picked.length > 0 && used + n > maxWords) continue;
    picked.push(s);
    used += n;
  }
  return picked;
}
