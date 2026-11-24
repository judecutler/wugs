// Sound changes: ordered rewrite rules run over a list of words, in the
// notation historical linguists and phonology classes use (Zompist's SCA²):
//
//   V = a e i o u          a category (a capital letter and its members)
//   p t k > b d g / V_V    targets, replacements, environment
//   s/z/V_V                the same with slashes
//   V > ∅ / _#             deletion (∅, 0 or nothing); # is a word edge
//   ∅ > ə / C_C            insertion
//   e > i / _(C)i          ( ) an optional element
//   a > e / _i // k_       after a second //, where the rule does not apply
//   ; a comment            * at the start of a line or ; anywhere
//
// A target or list with spaces is several sounds (p t k), one without is one
// sequence (ts); a capital letter defined as a category stands for its sounds.
// Each rule reads the word as it was before the rule (simultaneous), left
// to right, never overlapping. Pure; npm-tested.

export type Element = { kind: "lit"; text: string } | { kind: "cat"; name: string; members: string[] } | { kind: "edge" } | { kind: "opt"; inner: Element[] };
export type Env = { before: Element[]; after: Element[] };
export type Rule = {
  line: number;
  text: string;
  /** Alternatives to rewrite; [""] for an insertion. */
  targets: string[];
  /** One per target (or one for all). */
  replacements: string[];
  envs: Env[];
  exceptions: Env[];
};
export type Problem = { line: number; message: string };
export type Rules = { categories: Map<string, string[]>; rules: Rule[]; problems: Problem[] };

const NULL = new Set(["∅", "0", "Ø", "ø"]);

/** A list of members or targets: space-separated when it has spaces, else letter by letter (combining marks kept with their letter). */
function members(s: string): string[] {
  const t = s.trim();
  if (t === "") return [];
  if (/\s/u.test(t)) return t.split(/\s+/u);
  return t.match(/\P{M}\p{M}*/gu) ?? [];
}

/** Read one side of a rule (targets or replacements): a category name, a list, or ∅. */
function side(s: string, cats: Map<string, string[]>): string[] | null {
  const t = s.trim();
  if (t === "" || NULL.has(t)) return [""];
  if (cats.has(t)) return cats.get(t)!;
  if (!/\s/u.test(t)) return [t];
  return t.split(/\s+/u).map((m) => (NULL.has(m) ? "" : m));
}

/** Elements of one half of an environment, left to right. */
function elements(s: string, cats: Map<string, string[]>): Element[] | string {
  const out: Element[] = [];
  const chars: string[] = s.replace(/\s+/gu, "").match(/\P{M}\p{M}*/gu) ?? [];
  for (let i = 0; i < chars.length; i++) {
    const c = chars[i]!;
    if (c === "#") out.push({ kind: "edge" });
    else if (c === "(") {
      const close = chars.indexOf(")", i + 1);
      if (close < 0) return "a ( has no )";
      const inner = elements(chars.slice(i + 1, close).join(""), cats);
      if (typeof inner === "string") return inner;
      out.push({ kind: "opt", inner });
      i = close;
    } else if (c === ")") return "a ) has no (";
    else if (cats.has(c)) out.push({ kind: "cat", name: c, members: cats.get(c)! });
    else {
      const last = out[out.length - 1];
      if (last?.kind === "lit") last.text += c;
      else out.push({ kind: "lit", text: c });
    }
  }
  return out;
}

function env(s: string, cats: Map<string, string[]>): Env | string {
  const t = s.trim();
  const parts = t.split("_");
  if (parts.length !== 2) return `the environment «${t}» needs one _ for where the sound is`;
  const before = elements(parts[0]!, cats);
  if (typeof before === "string") return before;
  const after = elements(parts[1]!, cats);
  if (typeof after === "string") return after;
  if (before.slice(1).some((e) => e.kind === "edge") || after.slice(0, -1).some((e) => e.kind === "edge")) return "# stands only at the outer end of an environment";
  return { before, after };
}

/** Read the rules; every line that does not read is named, and the rest still run. */
export function parseRules(source: string): Rules {
  const categories = new Map<string, string[]>();
  const rules: Rule[] = [];
  const problems: Problem[] = [];
  source.split(/\r?\n/u).forEach((raw, i) => {
    const line = i + 1;
    const text = raw.replace(/;.*$/u, "").trim();
    if (text === "" || text.startsWith("*")) return;
    const cat = /^(\p{Lu})\s*=\s*(.+)$/u.exec(text);
    if (cat !== null) {
      const list = members(cat[2]!.replace(/\p{Lu}/gu, (c) => (categories.has(c) ? ` ${categories.get(c)!.join(" ")} ` : c)));
      categories.set(cat[1]!, list);
      return;
    }
    let target: string, replacement: string, rest: string[];
    if (text.includes(">") || text.includes("→")) {
      const [lhs, rhs = ""] = text.split(/>|→/u, 2);
      const [repl = "", ...envs] = rhs.split("/");
      target = lhs!;
      replacement = repl;
      rest = envs;
    } else {
      const parts = text.split("/");
      if (parts.length < 2) {
        problems.push({ line, message: "is neither a category (V = a e i) nor a rule (a > e / _i)" });
        return;
      }
      [target, replacement] = [parts[0]!, parts[1]!];
      rest = parts.slice(2);
    }
    // Environments: what follows the replacement; a second / starts the exceptions ("a/e/_i//k_" or "a > e / _i // k_").
    const joined = rest.join("/");
    const [envText = "", exceptText] = joined.split("//");
    const targets = side(target, categories);
    const replacements = side(replacement, categories);
    if (targets === null) {
      problems.push({ line, message: "has no sound to change" });
      return;
    }
    if (replacements === null || (replacements.length !== 1 && replacements.length !== targets.length)) {
      problems.push({ line, message: `has ${targets.length} sounds to change but ${replacements?.length ?? 0} to change them into` });
      return;
    }
    const readEnvs = (s: string | undefined): Env[] | string => {
      if (s === undefined || s.trim() === "") return [];
      const out: Env[] = [];
      for (const piece of s.split(/[,|]/u)) {
        const e = env(piece, categories);
        if (typeof e === "string") return e;
        out.push(e);
      }
      return out;
    };
    const envs = readEnvs(envText);
    const exceptions = readEnvs(exceptText);
    if (typeof envs === "string" || typeof exceptions === "string") {
      problems.push({ line, message: typeof envs === "string" ? envs : (exceptions as string) });
      return;
    }
    if (targets.length === 1 && targets[0] === "" && envs.length === 0) {
      problems.push({ line, message: "inserts a sound everywhere: give it an environment (∅ > ə / C_C)" });
      return;
    }
    rules.push({ line, text, targets, replacements, envs: envs.length === 0 ? [{ before: [], after: [] }] : envs, exceptions });
  });
  return { categories, rules, problems };
}

/** Ends of every way `els` matches forward from `pos` (backtracking through categories and options). */
function forward(word: string, pos: number, els: Element[]): number[] {
  if (els.length === 0) return [pos];
  const [e, ...rest] = els as [Element, ...Element[]];
  const out: number[] = [];
  if (e.kind === "edge") return pos === word.length ? forward(word, pos, rest) : [];
  if (e.kind === "lit") return word.startsWith(e.text, pos) ? forward(word, pos + e.text.length, rest) : [];
  if (e.kind === "opt") {
    for (const p of forward(word, pos, e.inner)) out.push(...forward(word, p, rest));
    out.push(...forward(word, pos, rest));
    return out;
  }
  for (const m of e.members) if (m !== "" && word.startsWith(m, pos)) out.push(...forward(word, pos + m.length, rest));
  return out;
}

/** Whether `els` can end exactly at `pos`, read backward. */
function backward(word: string, pos: number, els: Element[]): boolean {
  if (els.length === 0) return true;
  const e = els[els.length - 1]!;
  const rest = els.slice(0, -1);
  if (e.kind === "edge") return pos === 0 && backward(word, pos, rest);
  if (e.kind === "lit") return pos >= e.text.length && word.slice(pos - e.text.length, pos) === e.text && backward(word, pos - e.text.length, rest);
  if (e.kind === "opt") return backward(word, pos, [...rest, ...e.inner]) || backward(word, pos, rest);
  return e.members.some((m) => m !== "" && pos >= m.length && word.slice(pos - m.length, pos) === m && backward(word, pos - m.length, rest));
}

const fits = (word: string, start: number, end: number, e: Env): boolean => backward(word, start, e.before) && forward(word, end, e.after).length > 0;

/** The word after one rule. */
export function applyRule(word: string, r: Rule): string {
  const applies = (start: number, end: number) => r.envs.some((e) => fits(word, start, end, e)) && !r.exceptions.some((e) => fits(word, start, end, e));
  const into = (i: number) => r.replacements[r.replacements.length === 1 ? 0 : i]!;
  let out = "";
  if (r.targets.length === 1 && r.targets[0] === "") {
    for (let i = 0; i <= word.length; i++) {
      if (applies(i, i)) out += into(0);
      if (i < word.length) out += word[i];
    }
    return out;
  }
  // Longest target first, so «ts» wins over «t».
  const order = r.targets.map((t, i) => [t, i] as const).sort((a, b) => b[0].length - a[0].length);
  // A match never takes a letter from its combining marks (e in e̞ is not e).
  const whole = (end: number) => end >= word.length || !/\p{M}/u.test(word[end]!);
  let i = 0;
  while (i < word.length) {
    const hit = order.find(([t]) => t !== "" && word.startsWith(t, i) && whole(i + t.length) && applies(i, i + t.length));
    if (hit === undefined) {
      out += word[i];
      i += 1;
    } else {
      out += into(hit[1]);
      i += hit[0].length;
    }
  }
  return out;
}

export type Derivation = { input: string; output: string; steps: { line: number; form: string }[] };

/** A word through every rule in order, with each step that changed it. */
export function derive(word: string, rules: readonly Rule[]): Derivation {
  let form = word.normalize("NFC");
  const steps: { line: number; form: string }[] = [];
  for (const r of rules) {
    const next = applyRule(form, r);
    if (next !== form) steps.push({ line: r.line, form: next });
    form = next;
  }
  return { input: word, output: form, steps };
}

/** The words to run, one a line (or several on a line), duplicates once, in order. */
export function wordsOf(source: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const w of source.normalize("NFC").split(/[\s,;]+/u)) {
    if (w !== "" && !seen.has(w)) {
      seen.add(w);
      out.push(w);
    }
  }
  return out;
}

/** Words typed with the form each should become: a line `kapa > kaba` (or
 * with `→` or a tab between) pairs a word with its attested reflex; other
 * lines are words as `wordsOf` reads them, with nothing expected. */
export function pairsOf(source: string): { input: string; expected: string | null }[] {
  const out: { input: string; expected: string | null }[] = [];
  const seen = new Set<string>();
  for (const raw of source.normalize("NFC").split(/\r?\n/u)) {
    const m = /^\s*(\S+)\s*(?:>|→|\t)\s*(\S*)\s*$/u.exec(raw);
    if (m !== null) {
      const expected = m[2] === "∅" || m[2] === "0" ? "" : m[2]!;
      if (!seen.has(`${m[1]}\u0001${expected}`)) {
        seen.add(`${m[1]}\u0001${expected}`);
        out.push({ input: m[1]!, expected });
      }
      continue;
    }
    for (const w of wordsOf(raw)) {
      if (!seen.has(`${w}\u0001`)) {
        seen.add(`${w}\u0001`);
        out.push({ input: w, expected: null });
      }
    }
  }
  return out;
}

/** How many words came out as their attested reflex, when any is given. */
export function expectedLine(d: readonly Derivation[], expected: readonly (string | null)[]): string {
  const checked = d.map((x, i) => [x.output, expected[i]] as const).filter(([, e]) => e !== null && e !== undefined);
  if (checked.length === 0) return "";
  const right = checked.filter(([o, e]) => o === e).length;
  return ` ${right} of ${checked.length} ${checked.length === 1 ? "word comes" : "words come"} out as expected (${Math.round((100 * right) / checked.length)}%).`;
}

/** The line over the results. */
export function resultLine(d: readonly Derivation[], r: Rules): string {
  const changed = d.filter((x) => x.output !== x.input).length;
  const ruleWord = `${r.rules.length} ${r.rules.length === 1 ? "rule" : "rules"}`;
  const head = `${ruleWord} over ${d.length} ${d.length === 1 ? "word" : "words"}: ${changed} changed.`;
  const bad = r.problems.length === 0 ? "" : ` ${r.problems.length} ${r.problems.length === 1 ? "line does" : "lines do"} not read and ${r.problems.length === 1 ? "was" : "were"} skipped.`;
  return head + bad;
}

/** Tab-separated: input, output, and the steps as «line: form». */
export function derivationsTsv(d: readonly Derivation[], expected?: readonly (string | null)[]): string {
  const withExpected = expected !== undefined && expected.some((e) => e !== null);
  const head = withExpected ? "input\toutput\texpected\tas expected\tsteps" : "input\toutput\tsteps";
  return [
    head,
    ...d.map((x, i) => {
      const steps = x.steps.map((s) => `${s.line}: ${s.form}`).join("; ");
      if (!withExpected) return [x.input, x.output, steps].join("\t");
      const e = expected![i] ?? null;
      return [x.input, x.output, e ?? "", e === null ? "" : x.output === e ? "yes" : "no", steps].join("\t");
    }),
  ].join("\n");
}

/** A starting set of rules that shows the notation. */
export const EXAMPLE_RULES = `* Categories: a capital letter and its sounds
V = a e i o u
C = p t k b d g s m n l r

* Rules run in order, each over the word as it was before it
p t k > b d g / V_V      ; voicing between vowels
s > h / #_               ; s becomes h at the start of a word
V > ∅ / _#               ; final vowels drop
e > i / _(C)i            ; raising before i, across a consonant or not
`;

/** A line of a transcription through the rules, word by word (white space
 * between words kept as one space). */
export function lineThrough(line: string, rules: readonly Rule[]): string {
  return line
    .trim()
    .split(/\s+/u)
    .filter((w) => w !== "")
    .map((w) => derive(w, rules).output)
    .join(" ");
}

/** The default name of the track the rules write. */
export const targetName = (source: string): string => `${source.trim() || "Track"} after the sound changes`;
