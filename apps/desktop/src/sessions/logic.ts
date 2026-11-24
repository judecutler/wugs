// The Sessions panel's rules (src-tauri/src/field_sessions.rs): the fieldwork
// log filtered, summed up, copied as a table and printed. Pure, npm-tested.

export type SessionView = { id: string; date: string; place: string; method: string; notes: string; people: [string, string][]; texts: [string, string][] };
export type SessionsView = { sessions: SessionView[]; people: [string, string][] };

/** Methods offered as the field is typed; any other is kept as written. */
export const METHODS = ["elicitation", "recording", "transcription with a speaker", "translation with a speaker", "checking", "participant observation", "interview", "word list", "picture task", "story retelling"];

const fold = (s: string): string => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** The sessions whose date, place, method, notes, people or texts hold the query
 * (case and accents ignored), and only those with `person` when one is picked. */
export function shownSessions(all: readonly SessionView[], query: string, person: string | null): SessionView[] {
  const q = fold(query.trim());
  return all.filter((s) => {
    if (person !== null && !s.people.some(([id]) => id === person)) return false;
    if (q === "") return true;
    return [s.date, s.place, s.method, s.notes, ...s.people.map(([, n]) => n), ...s.texts.map(([, t]) => t)].some((x) => fold(x).includes(q));
  });
}

/** Today as `YYYY-MM-DD` in the person's own calendar. */
export function today(now: Date = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${p(now.getMonth() + 1)}-${p(now.getDate())}`;
}

/** "2026-11-24" as "Tue 24 Nov 2026" (English, as the rest of the app). */
export function dateLabel(date: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (m === null) return date === "" ? "No date" : date;
  const d = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  const day = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][d.getUTCDay()];
  const month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][d.getUTCMonth()];
  return `${day} ${d.getUTCDate()} ${month} ${d.getUTCFullYear()}`;
}

/** The line above the log: how many sessions, over how many days, with how many people. */
export function sessionsLine(all: readonly SessionView[], shown: number): string {
  if (all.length === 0) return "No sessions logged yet. Log each sitting: when and where, how, who took part, and what to follow up.";
  const days = new Set(all.map((s) => s.date).filter((d) => d !== "")).size;
  const people = new Set(all.flatMap((s) => s.people.map(([id]) => id))).size;
  const head = `${all.length} ${all.length === 1 ? "session" : "sessions"} on ${days} ${days === 1 ? "day" : "days"}, with ${people} ${people === 1 ? "person" : "people"}.`;
  return shown < all.length ? `${head} ${shown} shown.` : head;
}

/** Each person's sessions, most first: who has been worked with how often. */
export function perPerson(all: readonly SessionView[]): { id: string; name: string; sessions: number; last: string }[] {
  const by = new Map<string, { id: string; name: string; sessions: number; last: string }>();
  for (const s of all) {
    for (const [id, name] of s.people) {
      const row = by.get(id) ?? { id, name, sessions: 0, last: "" };
      row.sessions += 1;
      if (s.date > row.last) row.last = s.date;
      by.set(id, row);
    }
  }
  return [...by.values()].sort((a, b) => b.sessions - a.sessions || a.name.localeCompare(b.name));
}

const clean = (s: string) => s.replace(/[\t\r\n]+/gu, " ").trim();

/** The log as a table: a row per session. */
export function sessionsTsv(sessions: readonly SessionView[]): string {
  const head = ["date", "place", "method", "people", "texts", "notes"];
  const rows = sessions.map((s) => [s.date, s.place, s.method, s.people.map(([, n]) => n).join("; "), s.texts.map(([, t]) => t).join("; "), s.notes].map(clean).join("\t"));
  return [head.join("\t"), ...rows].join("\n") + "\n";
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The log as one printable HTML page, oldest first, as a field notebook reads. */
export function sessionsSheet(sessions: readonly SessionView[], o: { language: string; generator: string }): string {
  const title = `${o.language.trim() || "Project"}: fieldwork log`;
  const ordered = [...sessions].sort((a, b) => (a.date === "" ? 1 : 0) - (b.date === "" ? 1 : 0) || a.date.localeCompare(b.date));
  const entry = (s: SessionView) => {
    const facts = [s.place, s.method, s.people.map(([, n]) => n).join(", ")].filter((x) => x !== "").map(esc).join(" · ");
    const texts = s.texts.length === 0 ? "" : `<p class="texts">Texts: ${s.texts.map(([, t]) => esc(t)).join(", ")}</p>`;
    const notes = s.notes === "" ? "" : `<div class="notes">${s.notes.split(/\n+/u).map((p) => `<p>${esc(p)}</p>`).join("")}</div>`;
    return `<section><h2>${esc(dateLabel(s.date))}</h2>${facts === "" ? "" : `<p class="facts">${facts}</p>`}${texts}${notes}</section>`;
  };
  const style = `@page{margin:15mm}body{font-family:"Charis SIL","Gentium Plus","Doulos SIL",serif;font-size:11pt;margin:0 auto;max-width:180mm}
h1{font-size:16pt;margin:0 0 10pt}h2{font-size:12.5pt;margin:14pt 0 2pt;break-after:avoid}section{break-inside:avoid;border-bottom:1px solid #ccc;padding-bottom:6pt}
.facts{color:#444;margin:0 0 4pt}.texts{margin:0 0 4pt;font-style:italic}.notes p{margin:0 0 4pt}`;
  return `<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>${esc(title)}</title><meta name="generator" content="${esc(o.generator)}"><style>${style}</style></head>
<body><h1>${esc(title)}</h1>${ordered.map(entry).join("\n")}</body></html>
`;
}
