import assert from "node:assert/strict";
import { afterEach, beforeEach, test } from "node:test";
import worker, { issueText, MAX_ATTACHMENT_BYTES, parseReport, safeName, sign } from "../src/index.js";

// An in-memory stand-in for an R2 bucket.
function bucket() {
  const items = new Map();
  return {
    items,
    async get(key) {
      if (!items.has(key)) return null;
      const { bytes, meta } = items.get(key);
      return {
        body: new Response(bytes).body,
        httpMetadata: meta,
        json: async () => JSON.parse(new TextDecoder().decode(bytes)),
      };
    },
    async put(key, value, opts = {}) {
      const bytes = typeof value === "string" ? new TextEncoder().encode(value) : new Uint8Array(await new Response(value).arrayBuffer());
      items.set(key, { bytes, meta: opts.httpMetadata ?? items.get(key)?.meta });
    },
  };
}

let calls;
let failGitHub;
const realFetch = globalThis.fetch;
beforeEach(() => {
  calls = [];
  failGitHub = false;
  globalThis.fetch = async (url, init) => {
    calls.push({ url: String(url), body: JSON.parse(init.body) });
    if (String(url).startsWith("https://api.github.com")) {
      if (failGitHub) return new Response("down", { status: 503 });
      return Response.json({ number: 7 });
    }
    return Response.json({ id: "email" });
  };
});
afterEach(() => {
  globalThis.fetch = realFetch;
});

const ENV = () => ({
  REPORTS: bucket(),
  LINK_SECRET: "s3cret",
  GITHUB_TOKEN: "t",
  FEEDBACK_REPO: "owner/wugs-feedback",
  RESEND_API_KEY: "r",
  OWNER_EMAIL: "me@example.org",
});
const ID = "01JC0000000000000000000000";
const post = (env, body, id = ID) =>
  worker.fetch(new Request(`https://fb.example/v1/reports/${id}`, { method: "POST", body: JSON.stringify(body) }), env);

test("a report becomes one private issue and one email", async () => {
  const env = ENV();
  const r = await post(env, { message: "The timeline froze\nafter import", contact: "gsi@example.edu", client: "wugs 0.1.0 (windows)" });
  assert.equal(r.status, 201);
  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "https://api.github.com/repos/owner/wugs-feedback/issues");
  assert.equal(calls[0].body.title, "Feedback: The timeline froze");
  assert.deepEqual(calls[0].body.labels, ["feedback"]);
  assert.equal(calls[1].url, "https://api.resend.com/emails");
  assert.deepEqual(calls[1].body.to, ["me@example.org"]);
  assert.equal(calls[1].body.reply_to, "gsi@example.edu");
  assert.match(calls[1].body.text, /issues\/7/);
});

test("sending the same report again files nothing new", async () => {
  const env = ENV();
  await post(env, { message: "hello" });
  const again = await post(env, { message: "hello" });
  assert.equal(again.status, 200);
  assert.equal(calls.length, 2);
});

test("a retry finishes what a failed attempt left undone", async () => {
  const env = ENV();
  failGitHub = true;
  const first = await post(env, { message: "hello" });
  assert.equal(first.status, 500);
  failGitHub = false;
  calls = [];
  const second = await post(env, { message: "hello" });
  assert.equal(second.status, 200);
  assert.deepEqual(calls.map((c) => new URL(c.url).host), ["api.github.com", "api.resend.com"]);
});

test("an attachment streams into storage and is linked from the issue", async () => {
  const env = ENV();
  const video = new Uint8Array(1000).fill(7);
  const r = await post(env, { message: "see video", attachment: { name: "clip.mp4", bytes: video.length } });
  const { upload } = await r.json();
  assert.ok(upload.token);
  calls = [];
  const put = await worker.fetch(
    new Request(`https://fb.example/v1/reports/${ID}/attachment`, {
      method: "PUT",
      body: video,
      headers: { Authorization: `Bearer ${upload.token}`, "Content-Length": String(video.length), "Content-Type": "video/mp4" },
    }),
    env,
  );
  assert.equal(put.status, 201);
  assert.equal(env.REPORTS.items.get(`reports/${ID}/clip.mp4`).bytes.length, 1000);
  assert.match(calls[0].url, /issues\/7\/comments$/);
  const link = calls[0].body.body.match(/\((https:[^)]+)\)/)[1];
  const got = await worker.fetch(new Request(link), env);
  assert.equal(got.status, 200);
  assert.equal((await got.arrayBuffer()).byteLength, 1000);
  assert.match(got.headers.get("Content-Disposition"), /clip\.mp4/);

  const tampered = await worker.fetch(new Request(link.replace(/sig=../, "sig=00")), env);
  assert.equal(tampered.status, 403);
});

test("uploads need the report's own token and its declared size", async () => {
  const env = ENV();
  await post(env, { message: "x", attachment: { name: "a.png", bytes: 10 } });
  const bad = await worker.fetch(
    new Request(`https://fb.example/v1/reports/${ID}/attachment`, { method: "PUT", body: new Uint8Array(10), headers: { Authorization: `Bearer ${"0".repeat(64)}` } }),
    env,
  );
  assert.equal(bad.status, 403);
  const token = await sign(env, `upload:${ID}`);
  const wrongSize = await worker.fetch(
    new Request(`https://fb.example/v1/reports/${ID}/attachment`, { method: "PUT", body: new Uint8Array(5), headers: { Authorization: `Bearer ${token}`, "Content-Length": "5" } }),
    env,
  );
  assert.equal(wrongSize.status, 400);
});

test("reports are checked before anything is stored", () => {
  assert.match(parseReport({ message: " " }).error, /no message/);
  assert.match(parseReport({ message: "x", contact: "not an address" }).error, /email/);
  assert.match(parseReport({ message: "x", attachment: { name: "v.mp4", bytes: MAX_ATTACHMENT_BYTES + 1 } }).error, /95 MB/);
  assert.match(parseReport({ message: "x".repeat(10_001) }).error, /longer/);
  assert.equal(parseReport({ message: "ok", details: { os: "windows" } }).report.details.os, "windows");
});

test("the person's words cannot ping anyone or render as markdown", () => {
  const { body, title } = issueText({ id: ID, received: "now", message: "hi @someone ```x``` [link](http://e)", contact: "", client: "", details: null, attachment: null });
  assert.match(body, /^````text\nhi @someone ```x``` \[link\]\(http:\/\/e\)\n````/);
  assert.equal(title, "Feedback: hi @someone ```x``` [link](http://e)");
});

test("file names are made safe", () => {
  assert.equal(safeName("C:\\Users\\me\\Videos\\Screen Recording 1.mp4"), "Screen Recording 1.mp4");
  assert.equal(safeName("../../etc/passwd"), "passwd");
  assert.equal(safeName("a\"b;c.mov"), "a_b_c.mov");
});

test("unknown routes and bad ids are refused", async () => {
  const env = ENV();
  assert.equal((await worker.fetch(new Request("https://fb.example/"), env)).status, 404);
  assert.equal((await post(env, { message: "x" }, "../x")).status, 404);
  assert.equal((await post(env, { message: "x" }, "short")).status, 400);
});
