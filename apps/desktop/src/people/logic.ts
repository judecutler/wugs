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
