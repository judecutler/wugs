// Unclear passages (src-tauri/src/unclear.rs): the line over the list, a
// time in words, and a sheet to take to the next session. Pure: no DOM.

export type UnclearRow = { textId: string; textTitle: string; tierId: string; segmentId: string; content: string; startMs: number | null; endMs: number | null; markers: string[] };
export type Unclear = { checked: number; total: number; rows: UnclearRow[] };

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** A time as m:ss.s, or h:mm:ss for an hour and more. */
export function clock(ms: number): string {
  const s = ms / 1000;
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(Math.floor(sec)).padStart(2, "0")}` : `${m}:${sec.toFixed(1).padStart(4, "0")}`;
}

/** Where a row is heard, in words. */
export const spanOf = (r: UnclearRow): string => (r.startMs === null ? "untimed" : r.endMs === null ? clock(r.startMs) : `${clock(r.startMs)}–${clock(r.endMs)}`);

export function unclearLine(u: Unclear): string {
  if (u.checked === 0) return "No segment is transcribed yet.";
  if (u.total === 0) return `None of ${u.checked} transcribed segments is marked unclear. Mark a doubtful passage with (?), [unclear], (inaudible) or CHAT's xxx and it is listed here to take back to a speaker.`;
  const more = u.total > u.rows.length ? ` The first ${u.rows.length} are listed.` : "";
  return `${u.total} of ${u.checked} transcribed segments ${u.total === 1 ? "has" : "have"} a passage marked unclear.${more} Print the list to go through with a speaker at the next session.`;
}

/** The sheet: each text's unclear segments with their times, a box to tick and room to write. */
export function unclearSheet(rows: readonly UnclearRow[], o: { language: string; date: string; generator: string }): string {
  const title = `${o.language.trim() || "Project"}: unclear passages to check`;
  const groups: string[] = [];
  let current: string | null = null;
  let items: string[] = [];
  const flush = () => {
    if (current !== null) groups.push(`<h2>${esc(current || "Untitled text")}</h2><ol>${items.join("")}</ol>`);
    items = [];
  };
  for (const r of rows) {
    if (r.textTitle !== current) {
      flush();
      current = r.textTitle;
    }
    items.push(`<li><span class="box"></span><div><div class="where">${esc(spanOf(r))}</div><div class="body">${esc(r.content)}</div><div class="answer"></div></div></li>`);
  }
  flush();
  const style = `@page{margin:15mm}body{font-family:"Charis SIL","Gentium Plus","Doulos SIL",serif;font-size:11pt;margin:0 auto;max-width:180mm}
h1{font-size:16pt;margin:0 0 2pt}h2{font-size:12.5pt;margin:14pt 0 4pt;break-after:avoid}.meta{color:#555;margin:0 0 10pt}
ol{list-style:none;padding:0;margin:0}li{display:flex;gap:8pt;padding:5pt 0;border-bottom:1px solid #ccc;break-inside:avoid}
.box{flex:none;width:10pt;height:10pt;border:1px solid #333;margin-top:2pt}.where{color:#555;font-size:9.5pt}.body{margin-top:1pt}
.answer{height:22pt}`;
  return `<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>${esc(title)}</title><meta name="generator" content="${esc(o.generator)}"><style>${style}</style></head>
<body><h1>${esc(title)}</h1><p class="meta">${esc(o.date)}. Speaker: ______________________</p>${groups.join("\n")}</body></html>
`;
}
