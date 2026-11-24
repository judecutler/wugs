/** A term's answer: agreed, refused, or not asked (null). */
export type Answer = boolean | null;

/** Roles offered for a person; a role written elsewhere is kept and shown too. */
export const ROLES = ["speaker", "consultant", "interviewer", "researcher", "transcriber", "translator"];

const TERMS: Record<string, string> = {
  "archive-ok": "Deposit in an archive",
  "web-ok": "Publish online",
  "community-only": "Share within the community only",
  "AI-ok": "Use to build or test speech tools (AI)",
};

/** A consent term in words. */
export const termLabel = (term: string): string => TERMS[term] ?? term;

/** The details the People panel offers, in order: what an archive's actor
 * record (IMDI, ELAR, PARADISEC) asks for. Other keys a person has are shown too. */
export const DETAILS: readonly [string, string][] = [
  ["born", "Born (year)"],
  ["sex", "Sex or gender"],
  ["languages", "Languages"],
  ["from", "From (place)"],
  ["lives", "Lives in"],
  ["ethnic_group", "Ethnic group"],
  ["education", "Education"],
  ["notes", "Notes"],
];

/** A detail's name in words. */
export const detailLabel = (key: string): string => DETAILS.find(([k]) => k === key)?.[1] ?? key.replace(/_/g, " ");

/** The keys to show for a person: the offered ones, then any others they have. */
export const detailKeys = (details: readonly [string, string][]): string[] => [...DETAILS.map(([k]) => k), ...details.map(([k]) => k).filter((k) => !DETAILS.some(([d]) => d === k))];

type PersonLike = { name: string; roles: string[]; details: [string, string][]; consents: { covers: string; terms: [string, boolean | null][]; date: string }[]; deleted: boolean };

/** Everyone as a table for an archive's list of participants: name, roles,
 * every detail in use, then each person's consent records in words. */
export function peopleTsv(people: readonly PersonLike[], coverLabel: (id: string) => string): string {
  const live = people.filter((p) => !p.deleted);
  const keys = [...new Set([...DETAILS.map(([k]) => k), ...live.flatMap((p) => p.details.map(([k]) => k))])].filter((k) => live.some((p) => p.details.some(([d, v]) => d === k && v !== "")));
  const cell = (s: string) => s.replace(/[\t\r\n]+/g, " ");
  const consent = (p: PersonLike) =>
    p.consents
      .map((c) => {
        const said = c.terms.filter(([, a]) => a !== null).map(([t, a]) => `${termLabel(t)}: ${a ? "yes" : "no"}`);
        return `${coverLabel(c.covers)}${c.date ? ` (${c.date})` : ""}${said.length > 0 ? `: ${said.join(", ")}` : ""}`;
      })
      .join("; ");
  const head = ["Name", "Roles", ...keys.map(detailLabel), "Consent"];
  const rows = live.map((p) => [p.name, p.roles.join(", "), ...keys.map((k) => p.details.find(([d]) => d === k)?.[1] ?? ""), consent(p)].map(cell).join("\t"));
  return [head.join("\t"), ...rows].join("\n") + "\n";
}
