// The languages most like the project's own typology answers (src-tauri/src/typology.rs `alike`). Pure.

export type Alike = { glottocode: string; name: string; family: string; macroarea: string; compared: number; same: number };
export type Alikes = { languages: Alike[]; answered: number; least: number };

/** Agreement as a share, for the bar and the table. */
export const agreement = (a: Alike): number => (a.compared === 0 ? 0 : a.same / a.compared);

export const percentOf = (a: Alike): string => `${Math.round(100 * agreement(a))}%`;

/** The line above the list. */
export function alikeLine(r: Alikes, sourceName: string, language: string): string {
  if (r.answered < 5) return `Answer at least five ${sourceName} features (not ‘?’) to compare ${language || "the language"} with the others.`;
  if (r.languages.length === 0) return `No language in ${sourceName} is coded on ${r.least} or more of the ${r.answered} features answered.`;
  const top = r.languages[0]!;
  const families = new Set(r.languages.slice(0, 10).map((l) => l.family).filter((f) => f !== "" && f !== "isolate"));
  return `Compared on the ${r.answered} features answered (each language on those it is coded for, ${r.least} at least): ${top.name} agrees most, on ${top.same} of ${top.compared}. The closest ten come from ${families.size === 0 ? "no one family" : families.size === 1 ? `one family, ${[...families][0]}` : `${families.size} families`}. Shared features can come from common descent, contact or chance; read the list as a start, not a classification.`;
}

export function alikeTsv(r: Alikes): string {
  return ["glottocode\tname\tfamily\tmacroarea\tcompared\tagree\tshare", ...r.languages.map((l) => [l.glottocode, l.name, l.family, l.macroarea, l.compared, l.same, agreement(l).toFixed(3)].join("\t"))].join("\n") + "\n";
}
