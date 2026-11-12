import http from "node:http";
// The Worker on this computer, for the desktop app's ignored end-to-end test
// (feedback.rs, the_real_sender_talks_to_the_service). An in-memory bucket;
// GitHub and email are answered here instead of sent.
//   node services/feedback/dev/serve-local.mjs
import worker from "../src/index.js";
const items = new Map();
const env = {
  LINK_SECRET: "x", GITHUB_TOKEN: "t", FEEDBACK_REPO: "o/r", RESEND_API_KEY: "k", OWNER_EMAIL: "me@example.org",
  REPORTS: {
    async get(k) { if (!items.has(k)) return null; const b = items.get(k); return { body: new Response(b).body, httpMetadata: {}, json: async () => JSON.parse(new TextDecoder().decode(b)) }; },
    async put(k, v) { items.set(k, typeof v === "string" ? new TextEncoder().encode(v) : new Uint8Array(await new Response(v).arrayBuffer())); },
  },
};
const calls = [];
globalThis.fetch = async (url, init) => { calls.push({ url: String(url), body: JSON.parse(init.body) }); return Response.json({ number: 3 }); };
http.createServer(async (req, res) => {
  const chunks = []; for await (const c of req) chunks.push(c);
  const body = chunks.length ? Buffer.concat(chunks) : undefined;
  const r = await worker.fetch(new Request(`http://127.0.0.1:8799${req.url}`, { method: req.method, headers: req.headers, body: req.method === "GET" ? undefined : body }), env);
  res.writeHead(r.status, Object.fromEntries(r.headers)); res.end(Buffer.from(await r.arrayBuffer()));
  if (req.method === "PUT") { console.log(JSON.stringify({ stored: [...items.keys()], sizes: [...items.values()].map(v => v.length), calls: calls.map(c => c.url) })); }
}).listen(8799, () => console.log("ready"));
