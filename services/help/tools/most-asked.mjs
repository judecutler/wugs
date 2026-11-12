// The "most asked" report: which parts of the guide people ask about most,
// with recent questions under each, from questions people agreed to share.
// Needs CLOUDFLARE_ACCOUNT_ID and an API token with Account Analytics: Read
// (CLOUDFLARE_API_TOKEN, or CLOUDFLARE_API_KEY as the repository stores it).
// Usage: node tools/most-asked.mjs [days, default 30] [--counts-only]
// --counts-only leaves the questions out: what CI prints in a public
// repository's run summary. People agreed to share their questions with
// the maintainers, not with everyone.

const countsOnly = process.argv.includes("--counts-only");
const days = Number(process.argv.slice(2).find((a) => !a.startsWith("--")) ?? 30);
if (!Number.isInteger(days) || days < 1 || days > 90) throw new Error("days must be 1 to 90 (Analytics Engine keeps 3 months)");
const account = process.env.CLOUDFLARE_ACCOUNT_ID;
const token = process.env.CLOUDFLARE_API_TOKEN ?? process.env.CLOUDFLARE_API_KEY;
if (!account || !token) throw new Error("set CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN");

async function sql(query) {
  const r = await fetch(`https://api.cloudflare.com/client/v4/accounts/${account}/analytics_engine/sql`, {
    method: "POST",
    headers: { authorization: `Bearer ${token}` },
    body: query,
  });
  if (!r.ok) throw new Error(`Analytics Engine ${r.status}: ${await r.text()}`);
  return (await r.json()).data;
}

const since = `timestamp > NOW() - INTERVAL '${days}' DAY`;
const top = await sql(
  `SELECT index1 AS section, SUM(_sample_interval) AS asked FROM wugs_help_asked WHERE ${since} GROUP BY section ORDER BY asked DESC LIMIT 20`,
);
const recent = await sql(
  `SELECT index1 AS section, blob1 AS question, blob3 AS version FROM wugs_help_asked WHERE ${since} ORDER BY timestamp DESC LIMIT 500`,
);

const lines = [`# Most asked, last ${days} days`, ""];
if (top.length === 0) lines.push("No shared questions yet.");
for (const { section, asked } of top) {
  lines.push(`## ${section} (${asked})`, "");
  if (countsOnly) continue;
  for (const q of recent.filter((r) => r.section === section).slice(0, 5)) {
    lines.push(`- ${q.question.replace(/\s+/gu, " ")}${q.version ? ` (${q.version})` : ""}`);
  }
  lines.push("");
}
console.log(lines.join("\n"));
