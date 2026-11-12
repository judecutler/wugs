// The wugs help bot: a Cloudflare Worker that answers questions about the app
// from its user guide. The app sends a question (and the last few turns);
// the Worker picks the guide sections that match it, asks a small hosted
// model to answer from those sections only, and returns the answer with the
// sections it used.
//
// Bindings (wrangler.toml): AI (Workers AI), ASKED (Analytics Engine, the
// "most asked" report; written only when the person agreed), LIMITER (rate
// limit, optional). Vars: MODEL, GUIDE_URL. Optional: PROVIDER = "deepseek"
// with the secret DEEPSEEK_API_KEY answers through DeepSeek instead.

import { search, sections } from "./guide.js";

export const MAX_QUESTION_CHARS = 1000;
export const MAX_TURNS = 6;
export const MAX_TURN_CHARS = 2000;
const VERSION = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?$/;
const GUIDE_TTL_SECONDS = 3600;

export default {
  async fetch(request, env, ctx) {
    try {
      return await route(request, env, ctx);
    } catch (e) {
      console.error(e);
      return json(502, { error: "The help service had a problem answering. Try again in a moment, or use Feedback." });
    }
  },
};

async function route(request, env, ctx) {
  const url = new URL(request.url);
  if (url.pathname !== "/v1/ask") return json(404, { error: "Not found." });
  if (request.method !== "POST") return json(405, { error: "Use POST." });
  if (env.LIMITER) {
    const { success } = await env.LIMITER.limit({ key: request.headers.get("CF-Connecting-IP") ?? "unknown" });
    if (!success) return json(429, { error: "Too many questions from here just now. Try again in a minute." });
  }
  let body;
  try {
    body = await request.json();
  } catch {
    return json(400, { error: "The question did not arrive whole." });
  }
  const ask = parseAsk(body);
  if (typeof ask === "string") return json(400, { error: ask });

  const guide = sections(await guideText(env, ask.version, ctx));
  const picked = search(guide, [...ask.history.filter((t) => t.role === "user").map((t) => t.content), ask.question].join("\n"));
  const answer = await complete(env, messages(picked, ask));
  if (ask.share && env.ASKED) {
    env.ASKED.writeDataPoint({
      indexes: [(picked[0]?.title ?? "(no section)").slice(0, 96)],
      blobs: [ask.question, picked[0]?.title ?? "", ask.version ?? ""],
    });
  }
  return json(200, { answer, sources: picked.map(({ title, anchor }) => ({ title, anchor })) });
}

/** Checks what the app sent. Returns the clean request or an error line. */
export function parseAsk(body) {
  if (body === null || typeof body !== "object") return "The question did not arrive whole.";
  const question = typeof body.question === "string" ? body.question.trim() : "";
  if (question === "") return "Ask a question.";
  if (question.length > MAX_QUESTION_CHARS) return `Questions can be up to ${MAX_QUESTION_CHARS} characters.`;
  const raw = Array.isArray(body.history) ? body.history : [];
  const history = raw
    .filter((t) => t !== null && typeof t === "object" && (t.role === "user" || t.role === "assistant") && typeof t.content === "string")
    .slice(-MAX_TURNS)
    .map((t) => ({ role: t.role, content: t.content.slice(0, MAX_TURN_CHARS) }));
  const version = typeof body.version === "string" && VERSION.test(body.version) ? body.version : null;
  return { question, history, version, share: body.share === true };
}

/** The prompt: the rules, the guide sections, the last turns, the question. */
export function messages(picked, ask) {
  const excerpts =
    picked.length === 0
      ? "(No part of the guide matched this question.)"
      : picked.map((s) => `## ${s.title}\n${s.body}`).join("\n\n");
  const system = [
    "You are the help assistant inside wugs, a desktop app for linguists (fieldwork kit Tongues, corpus kit Scrolls).",
    "Answer only from the user guide excerpts below. Use the app's exact names for buttons, panels and menus as the excerpts write them.",
    "If the excerpts do not answer the question, say you are not sure, and suggest the Feedback button in the top bar so the people who make wugs can help.",
    "Never invent a feature, setting or keyboard shortcut. Keep answers short: a few sentences, or numbered steps when there are steps.",
    "",
    "User guide excerpts:",
    excerpts,
  ].join("\n");
  return [{ role: "system", content: system }, ...ask.history, { role: "user", content: ask.question }];
}

async function complete(env, msgs) {
  if (env.PROVIDER === "deepseek" && env.DEEPSEEK_API_KEY) {
    const r = await fetch("https://api.deepseek.com/chat/completions", {
      method: "POST",
      headers: { "content-type": "application/json", authorization: `Bearer ${env.DEEPSEEK_API_KEY}` },
      body: JSON.stringify({ model: env.DEEPSEEK_MODEL || "deepseek-chat", messages: msgs, max_tokens: 600, temperature: 0.2 }),
    });
    if (!r.ok) throw new Error(`deepseek ${r.status}`);
    const data = await r.json();
    return String(data.choices?.[0]?.message?.content ?? "").trim();
  }
  const out = await env.AI.run(env.MODEL, { messages: msgs, max_tokens: 600, temperature: 0.2 });
  return String(out?.response ?? "").trim();
}

/** The guide for the asker's version (the tag `v<version>`), falling back to
 * `main` when that tag has no guide; cached for an hour. A new guide reaches
 * the bot without a redeploy. */
async function guideText(env, version, ctx) {
  const refs = version === null ? ["main"] : [`v${version}`, "main"];
  for (const ref of refs) {
    const url = env.GUIDE_URL.replace("{ref}", ref);
    const cache = globalThis.caches?.default;
    const hit = cache ? await cache.match(url) : undefined;
    if (hit) return hit.text();
    const r = await fetch(url);
    if (!r.ok) continue;
    const text = await r.text();
    if (cache) {
      const keep = new Response(text, { headers: { "cache-control": `max-age=${GUIDE_TTL_SECONDS}` } });
      ctx?.waitUntil?.(cache.put(url, keep));
    }
    return text;
  }
  throw new Error("user guide unreachable");
}

function json(status, value) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}
