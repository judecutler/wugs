// Pure helpers for the Conflicts panel (roadmap item 24). No DOM.

/** "this device · 14:02" / "another device · 3 Nov, 09:15" */
export function versionMeta(v: { mine: boolean; atMs: number }, now: number): string {
  const who = v.mine ? "this device" : "another device";
  if (v.atMs <= 0) return who;
  const d = new Date(v.atMs);
  const today = new Date(now);
  const sameDay = d.getFullYear() === today.getFullYear() && d.getMonth() === today.getMonth() && d.getDate() === today.getDate();
  const time = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  return sameDay ? `${who} · ${time}` : `${who} · ${d.getDate()} ${months[d.getMonth()]}, ${time}`;
}

/** The JSON a typed value is sent as. */
export const typedRaw = (text: string): string => JSON.stringify(text.normalize("NFC"));

/** Two versions that read the same are confusing; number them ("1", "2") then. */
export function needsNumbers(labels: string[]): boolean {
  return new Set(labels).size !== labels.length;
}

/** "1 field" / "3 fields" */
export const fieldsLabel = (n: number): string => (n === 1 ? "1 field needs a decision" : `${n} fields need a decision`);
