// The wugs feedback receiver: a Cloudflare Worker. The desktop app sends a
// report here; the Worker keeps it in R2, files an issue in a private GitHub
// repository and emails the maintainer. A screenshot or video goes up in a
// second request, streamed straight into R2, and is linked from the issue.
//
// Every step is idempotent on the report id the app chose, so an app that
// lost a response sends the same report again and nothing is filed twice.
//
// Bindings (wrangler.toml): REPORTS (R2 bucket), LIMITER (rate limit,
// optional). Secrets: LINK_SECRET (signs upload and download links),
// GITHUB_TOKEN + FEEDBACK_REPO ("owner/repo"), RESEND_API_KEY + OWNER_EMAIL.
// GitHub and email are each skipped when their secrets are unset.

export const MAX_REPORT_BYTES = 64 * 1024;
export const MAX_MESSAGE_CHARS = 10_000;
export const MAX_DETAILS_BYTES = 32 * 1024;
// Cloudflare's free plan refuses request bodies over 100 MB.
export const MAX_ATTACHMENT_BYTES = 95 * 1024 * 1024;

const ID = /^[0-9A-Za-z]{16,40}$/;
const EMAIL = /^[^\s@<>]{1,64}@[^\s@<>]{1,190}$/;

export default {
  async fetch(request, env) {
    try {
      return await route(request, env);
    } catch (e) {
      console.error(e);
      return json(500, { error: "The feedback service had a problem. Your report is kept and will be sent again." });
    }
  },
};

async function route(request, env) {
  const url = new URL(request.url);
  const parts = url.pathname.split("/").filter(Boolean);
  if (parts[0] !== "v1") return json(404, { error: "Not found." });

  if (parts[1] === "reports" && parts.length === 3 && request.method === "POST") {
    if (!(await allowed(request, env))) return json(429, { error: "Too many reports from here just now. Try again in a minute." });
    return receiveReport(request, env, parts[2]);
  }
  if (parts[1] === "reports" && parts[3] === "attachment" && parts.length === 4 && request.method === "PUT") {
    if (!(await allowed(request, env))) return json(429, { error: "Too many uploads from here just now. Try again in a minute." });
    return receiveAttachment(request, env, parts[2]);
  }
  if (parts[1] === "files" && parts.length === 3 && request.method === "GET") {
    return sendFile(url, env, parts[2]);
  }
  return json(404, { error: "Not found." });
}

async function allowed(request, env) {
  if (!env.LIMITER) return true;
  const key = request.headers.get("CF-Connecting-IP") ?? "unknown";
  const { success } = await env.LIMITER.limit({ key });
  return success;
}

/** Checks a report the app sent. Returns the clean report or an error line. */
export function parseReport(body) {
  if (body === null || typeof body !== "object" || Array.isArray(body)) return { error: "The report is not an object." };
  const message = typeof body.message === "string" ? body.message.trim() : "";
  if (message === "") return { error: "The report has no message." };
  if (message.length > MAX_MESSAGE_CHARS) return { error: `The message is longer than ${MAX_MESSAGE_CHARS} characters.` };
  const contact = typeof body.contact === "string" ? body.contact.trim() : "";
  if (contact !== "" && !EMAIL.test(contact)) return { error: "The reply address is not an email address." };
  const client = typeof body.client === "string" ? body.client.slice(0, 80) : "";
  let details = null;
  if (body.details !== undefined && body.details !== null) {
    if (typeof body.details !== "object" || Array.isArray(body.details)) return { error: "The technical details are not an object." };
    if (JSON.stringify(body.details).length > MAX_DETAILS_BYTES) return { error: "The technical details are too long." };
    details = body.details;
  }
  let attachment = null;
  if (body.attachment !== undefined && body.attachment !== null) {
    const a = body.attachment;
    const name = typeof a?.name === "string" ? safeName(a.name) : "";
    if (name === "" || !Number.isInteger(a.bytes) || a.bytes < 1) return { error: "The attachment needs a name and a size." };
    if (a.bytes > MAX_ATTACHMENT_BYTES) return { error: "The attachment is larger than 95 MB." };
    attachment = { name, bytes: a.bytes };
  }
  return { report: { message, contact, client, details, attachment } };
}

/** A file name safe for a storage key and a download header. */
export function safeName(name) {
  const base = String(name).split(/[\\/]/).pop() ?? "";
  return base.replace(/[^\p{L}\p{N}._ -]/gu, "_").replace(/^[. ]+/, "").slice(0, 120);
}

async function receiveReport(request, env, id) {
  if (!ID.test(id)) return json(400, { error: "That is not a report id." });
  const declared = Number(request.headers.get("Content-Length") ?? "0");
  if (declared > MAX_REPORT_BYTES) return json(413, { error: "The report is too long." });
  const text = await request.text();
  if (text.length > MAX_REPORT_BYTES) return json(413, { error: "The report is too long." });
  let body;
  try {
    body = JSON.parse(text);
  } catch {
    return json(400, { error: "The report is not JSON." });
  }
  const parsed = parseReport(body);
  if (parsed.error) return json(400, { error: parsed.error });

  const key = `reports/${id}/report.json`;
  const stored = await env.REPORTS.get(key);
  let record;
  if (stored) {
    record = await stored.json();
  } else {
    record = { id, received: new Date().toISOString(), ...parsed.report, issue: null, emailed: false, file: null };
    await env.REPORTS.put(key, JSON.stringify(record), { httpMetadata: { contentType: "application/json" } });
  }
  // Finish whatever an earlier attempt left undone.
  if (record.issue === null && env.GITHUB_TOKEN && env.FEEDBACK_REPO) {
    record.issue = await fileIssue(env, record);
    await env.REPORTS.put(key, JSON.stringify(record));
  }
  if (!record.emailed && env.RESEND_API_KEY && env.OWNER_EMAIL) {
    await sendEmail(env, record);
    record.emailed = true;
    await env.REPORTS.put(key, JSON.stringify(record));
  }
  const out = { id, received: record.received };
  if (record.attachment && record.file === null) out.upload = { token: await sign(env, `upload:${id}`) };
  return json(stored ? 200 : 201, out);
}

async function receiveAttachment(request, env, id) {
  if (!ID.test(id)) return json(400, { error: "That is not a report id." });
  const token = (request.headers.get("Authorization") ?? "").replace(/^Bearer /, "");
  if (!(await verify(env, `upload:${id}`, token))) return json(403, { error: "That upload link is not valid." });
  const key = `reports/${id}/report.json`;
  const stored = await env.REPORTS.get(key);
  if (!stored) return json(404, { error: "Send the report before its attachment." });
  const record = await stored.json();
  if (!record.attachment) return json(400, { error: "This report has no attachment." });
  if (record.file !== null) return json(200, { id, file: "already received" });
  const length = Number(request.headers.get("Content-Length") ?? "NaN");
  if (length !== record.attachment.bytes) return json(400, { error: "The attachment's size does not match the report." });
  if (request.body === null) return json(400, { error: "The attachment is empty." });

  const fileKey = `reports/${id}/${record.attachment.name}`;
  // Streamed: a 95 MB video never sits in the Worker's memory.
  await env.REPORTS.put(fileKey, request.body, {
    httpMetadata: { contentType: request.headers.get("Content-Type") ?? "application/octet-stream" },
  });
  record.file = fileKey;
  await env.REPORTS.put(key, JSON.stringify(record));
  const link = await fileLink(request.url, env, id);
  if (record.issue !== null && env.GITHUB_TOKEN && env.FEEDBACK_REPO) {
    await github(env, `/repos/${env.FEEDBACK_REPO}/issues/${record.issue}/comments`, {
      body: `Attachment: [${mdText(record.attachment.name)}](${link}) (${sizeWords(record.attachment.bytes)}). The link works for anyone who has it; keep it in this repository.`,
    });
  }
  return json(201, { id, file: "received" });
}

async function sendFile(url, env, id) {
  if (!ID.test(id)) return json(404, { error: "Not found." });
  if (!(await verify(env, `file:${id}`, url.searchParams.get("sig") ?? ""))) return json(403, { error: "That link is not valid." });
  const stored = await env.REPORTS.get(`reports/${id}/report.json`);
  if (!stored) return json(404, { error: "Not found." });
  const record = await stored.json();
  if (record.file === null) return json(404, { error: "The attachment has not arrived yet." });
  const file = await env.REPORTS.get(record.file);
  if (!file) return json(404, { error: "Not found." });
  const name = record.attachment.name;
  return new Response(file.body, {
    headers: {
      "Content-Type": file.httpMetadata?.contentType ?? "application/octet-stream",
      "Content-Disposition": `attachment; filename="${name.replace(/"/g, "_")}"; filename*=UTF-8''${encodeURIComponent(name)}`,
      "Cache-Control": "private, no-store",
    },
  });
}

async function fileLink(requestUrl, env, id) {
  const base = new URL(requestUrl);
  return `${base.origin}/v1/files/${id}?sig=${await sign(env, `file:${id}`)}`;
}

/** The issue text. The person's words go in a fence, so markdown and
 * @mentions in them do nothing. */
export function issueText(record) {
  const firstLine = record.message.split("\n")[0].trim();
  const title = `Feedback: ${firstLine.length > 70 ? `${firstLine.slice(0, 69)}…` : firstLine}`;
  const lines = [fence(record.message, "text"), ""];
  lines.push(`- Received: ${record.received}`);
  if (record.client) lines.push(`- From: ${mdText(record.client)}`);
  lines.push(`- Reply to: ${record.contact ? `\`${record.contact.replace(/`/g, "")}\`` : "no address given"}`);
  if (record.attachment) lines.push(`- Attachment: ${mdText(record.attachment.name)} (${sizeWords(record.attachment.bytes)}), linked in a comment once it arrives`);
  lines.push(`- Report id: \`${record.id}\``);
  if (record.details) lines.push("", "Technical details:", "", fence(JSON.stringify(record.details, null, 2), "json"));
  return { title, body: lines.join("\n") };
}

function fence(text, lang) {
  const longest = Math.max(2, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
  const marks = "`".repeat(longest + 1);
  return `${marks}${lang}\n${text}\n${marks}`;
}

/** Plain text for a markdown line: no links, emphasis or mentions. */
function mdText(s) {
  return String(s).replace(/[\\`*_[\]<>#@|~]/g, (c) => `\\${c}`);
}

function sizeWords(bytes) {
  return bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} kB`;
}

async function fileIssue(env, record) {
  const { title, body } = issueText(record);
  const issue = await github(env, `/repos/${env.FEEDBACK_REPO}/issues`, { title, body, labels: ["feedback"] });
  return issue.number;
}

async function github(env, path, payload) {
  const r = await fetch(`https://api.github.com${path}`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.GITHUB_TOKEN}`,
      Accept: "application/vnd.github+json",
      "User-Agent": "wugs-feedback",
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  });
  if (!r.ok) throw new Error(`GitHub answered ${r.status}: ${await r.text()}`);
  return r.json();
}

async function sendEmail(env, record) {
  const { title } = issueText(record);
  const text = [
    record.message,
    "",
    `Reply to: ${record.contact || "no address given"}`,
    record.issue !== null ? `Issue: https://github.com/${env.FEEDBACK_REPO}/issues/${record.issue}` : null,
    record.attachment ? `Attachment: ${record.attachment.name}, linked in the issue once it arrives.` : null,
    record.details ? "Technical details are in the issue." : null,
  ]
    .filter((l) => l !== null)
    .join("\n");
  const r = await fetch("https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.RESEND_API_KEY}`,
      "Content-Type": "application/json",
      // A retry after a lost answer never sends a second email.
      "Idempotency-Key": `wugs-feedback/${record.id}`,
    },
    body: JSON.stringify({
      from: env.EMAIL_FROM || "wugs feedback <onboarding@resend.dev>",
      to: [env.OWNER_EMAIL],
      subject: title,
      text,
      ...(record.contact ? { reply_to: record.contact } : {}),
    }),
  });
  if (!r.ok) throw new Error(`Resend answered ${r.status}: ${await r.text()}`);
}

async function hmacKey(env) {
  if (!env.LINK_SECRET) throw new Error("LINK_SECRET is not set");
  return crypto.subtle.importKey("raw", new TextEncoder().encode(env.LINK_SECRET), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
}

export async function sign(env, what) {
  const mac = await crypto.subtle.sign("HMAC", await hmacKey(env), new TextEncoder().encode(what));
  return [...new Uint8Array(mac)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function verify(env, what, hex) {
  if (!/^[0-9a-f]{64}$/.test(hex)) return false;
  const mac = new Uint8Array(hex.match(/../g).map((h) => parseInt(h, 16)));
  return crypto.subtle.verify("HMAC", await hmacKey(env), mac, new TextEncoder().encode(what));
}

function json(status, value) {
  return new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } });
}
