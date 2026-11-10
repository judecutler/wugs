// The morph-breakdown string a linguist types into a word, and its inverse.
// Pure: no DOM. Markers follow Leipzig: `-` affix boundary, `=` clitic, `~`
// reduplication, `<…>` infix, `∅` zero morph. An unmarked piece is a root.
//
//   köpek-ler     root + suffix          ka-köpek      prefix + root
//   köpek=ki      root + clitic          =ki           a clitic alone
//   re~redup      reduplicant + root     <in>fix       infix + root
//   un- do -ness  space-separated pieces carry their own marker
//
// In a joined word with `-` boundaries the root is the longest unmarked piece
// (ties go to the first, and `∅` only when nothing else is left); write the
// pieces apart with their markers to say otherwise: `un- do`.

export type MorphType = "root" | "prefix" | "suffix" | "infix" | "clitic" | "reduplication";
export type ParsedMorph = { surface: string; type: MorphType };
export type ParseResult = { ok: true; morphs: ParsedMorph[] } | { ok: false; error: string };

export const ZERO = "∅";

type Piece = { text: string; infix: boolean; left: string | null; right: string | null };

const DELIMITERS = new Set(["-", "=", "~"]);

function pieces(chunk: string): Piece[] | string {
  const out: Piece[] = [];
  let pendingDelim: string | null = null;
  let text = "";
  const push = (t: string, infix: boolean) => {
    out.push({ text: t, infix, left: pendingDelim, right: null });
    pendingDelim = null;
  };
  const flush = () => {
    if (text !== "") push(text, false);
    text = "";
  };
  const chars = [...chunk];
  for (let i = 0; i < chars.length; i += 1) {
    const c = chars[i]!;
    if (c === "<") {
      flush();
      const close = chars.indexOf(">", i);
      if (close === -1) return "An infix needs a closing >.";
      const inner = chars.slice(i + 1, close).join("");
      if (inner === "" || inner.includes("<")) return "An infix needs letters inside <…>.";
      push(inner, true);
      i = close;
    } else if (c === ">") {
      return "A > without a < before it.";
    } else if (DELIMITERS.has(c)) {
      const before = text !== "" || out.length > 0;
      flush();
      if (pendingDelim !== null) return `Two markers in a row (${pendingDelim}${c}): a morph is missing between them.`;
      if (before && out.length > 0) out[out.length - 1]!.right = c;
      pendingDelim = c;
    } else {
      text += c;
    }
  }
  flush();
  if (pendingDelim !== null && out.length > 0 && out[out.length - 1]!.right === null) out[out.length - 1]!.right = pendingDelim;
  if (out.length === 0) return "Nothing to break here but a marker.";
  return out;
}

const size = (s: string): number => [...s.normalize("NFC")].length;

function classify(list: Piece[]): MorphType[] {
  const last = list.length - 1;
  const types: (MorphType | null)[] = list.map((p, i) => {
    if (p.infix) return "infix";
    if (p.left === "=" || (i === last && p.right === "=")) return "clitic";
    if (p.right === "~" || (i === 0 && p.left === "~")) return "reduplication";
    const open = i === 0 && p.left === "-";
    const closed = i === last && p.right === "-";
    if (open && closed) return "infix";
    if (open) return "suffix";
    if (closed) return "prefix";
    // the halves of a root an infix sits inside (s<um>ulat), or the root it is attached to (<in>fix)
    if ((list[i - 1]?.infix && p.left === null) || (list[i + 1]?.infix && p.right === null)) return "root";
    return null;
  });
  const bare = types.flatMap((t, i) => (t === null ? [i] : []));
  const real = bare.filter((i) => list[i]!.text !== ZERO);
  const candidates = real.length > 0 ? real : bare;
  let root = -1;
  for (const i of candidates) if (root === -1 || size(list[i]!.text) > size(list[root]!.text)) root = i;
  return types.map((t, i) => t ?? (i === root ? "root" : i < root ? "prefix" : "suffix"));
}

/** Parse a typed breakdown. An empty string is a valid, empty analysis. */
export function parseBreakdown(input: string): ParseResult {
  const morphs: ParsedMorph[] = [];
  for (const chunk of input.split(/\s+/u).filter((c) => c !== "")) {
    const list = pieces(chunk);
    if (typeof list === "string") return { ok: false, error: list };
    const types = classify(list);
    list.forEach((p, i) => morphs.push({ surface: p.text.normalize("NFC"), type: types[i]! }));
  }
  return { ok: true, morphs };
}

const MARK: Record<string, (s: string) => string> = {
  prefix: (s) => `${s}-`,
  suffix: (s) => `-${s}`,
  infix: (s) => `<${s}>`,
  clitic: (s) => `=${s}`,
  reduplication: (s) => `${s}~`,
};

/** One morph with its marker, as written in a lexicon or typed back into the editor. */
export const marked = (m: ParsedMorph): string => (MARK[m.type] ?? ((s: string) => s))(m.surface);

/** The editor's text for an analysis: every piece carries its marker, separated by spaces,
 * so it parses back to the same types whatever their lengths. */
export const formatBreakdown = (morphs: readonly ParsedMorph[]): string => morphs.map(marked).join(" ");

/** The Leipzig word line: pieces joined at their markers, so `köpek` `-ler` reads `köpek-ler`. */
export function joinedBreakdown(morphs: readonly ParsedMorph[]): string {
  let out = "";
  for (const m of morphs) {
    const piece = marked(m);
    const glued = out === "" || /[-=~>]$/u.test(out) || /^[-=<]/u.test(piece);
    out += glued ? piece : ` ${piece}`;
  }
  return out;
}

/** Whether typing `input` would change the analysis `current` (compared as marked text). */
export function breakdownChanged(input: string, current: readonly ParsedMorph[]): boolean {
  const parsed = parseBreakdown(input);
  if (!parsed.ok) return true;
  return formatBreakdown(parsed.morphs) !== formatBreakdown(current.map((m) => ({ surface: m.surface.normalize("NFC"), type: m.type })));
}
