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
let failComment;
const realFetch = globalThis.fetch;
beforeEach(() => {
  calls = [];
  failGitHub = false;
  failComment = false;
  globalThis.fetch = async (url, init) => {
    calls.push({ url: String(url), body: JSON.parse(init.body) });
    if (String(url).startsWith("https://api.github.com")) {
      if (failGitHub || (failComment && String(url).endsWith("/comments"))) return new Response("down", { status: 503 });
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
const post = (env, body, id = ID) => {
  const text = JSON.stringify(body);
  return worker.fetch(
    new Request(`https://fb.example/v1/reports/${id}`, { method: "POST", body: text, headers: { "Content-Length": String(new TextEncoder().encode(text).length) } }),
    env,
  );
};
const upload = (env, token, bytes, id = ID) =>
  worker.fetch(
    new Request(`https://fb.example/v1/reports/${id}/attachment`, {
      method: "PUT",
      body: bytes,
      headers: { Authorization: `Bearer ${token}`, "Content-Length": String(bytes.length), "Content-Type": "video/mp4" },
    }),
    env,
  );

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

test("a GitHub failure never stops the email, and a repeat files only the issue", async () => {
  const env = ENV();
  failGitHub = true;
  const first = await post(env, { message: "hello" });
  assert.equal(first.status, 201, "the maintainer was told by email");
  assert.deepEqual(calls.map((c) => new URL(c.url).host), ["api.github.com", "api.resend.com"]);
  failGitHub = false;
  calls = [];
  await post(env, { message: "hello" });
  assert.deepEqual(calls.map((c) => new URL(c.url).host), ["api.github.com"]);
});

test("when nobody could be told, the app is asked to try again", async () => {
  const env = ENV();
  delete env.RESEND_API_KEY;
  failGitHub = true;
  assert.equal((await post(env, { message: "hello" })).status, 503);
  failGitHub = false;
  assert.equal((await post(env, { message: "hello" })).status, 200);
});

test("an attachment streams into storage and is linked from the issue", async () => {
  const env = ENV();
  const video = new Uint8Array(1000).fill(7);
  const r = await post(env, { message: "see video", attachment: { name: "clip.mp4", bytes: video.length } });
  const { upload: up } = await r.json();
  const upload = up;
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
  assert.equal(env.REPORTS.items.get(`reports/${ID}/file-clip.mp4`).bytes.length, 1000);
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

test("a file named in any script downloads", async () => {
  const env = ENV();
  const r = await post(env, { message: "x", attachment: { name: "画面録画 2026-11-12.mov", bytes: 4 } });
  const { upload: up } = await r.json();
  calls = [];
  assert.equal((await upload(env, up.token, new Uint8Array(4))).status, 201);
  const link = calls[0].body.body.match(/\((https:[^)]+)\)/)[1];
  const got = await worker.fetch(new Request(link), env);
  assert.equal(got.status, 200);
  assert.match(got.headers.get("Content-Disposition"), /filename\*=UTF-8''%E7%94%BB/);
  assert.equal(got.headers.get("X-Content-Type-Options"), "nosniff");
});

test("a lost attachment comment is posted on the next try", async () => {
  const env = ENV();
  const r = await post(env, { message: "x", attachment: { name: "a.mp4", bytes: 3 } });
  const { upload: up } = await r.json();
  failComment = true;
  // GitHub is down for the comment, but the email carries the link instead.
  const first = await upload(env, up.token, new Uint8Array(3));
  assert.equal(first.status, 201);
  assert.ok(calls.some((c) => c.url === "https://api.resend.com/emails" && /^Attachment: /.test(c.body.subject)));
});

test("an attachment nobody could be told about is retried until someone is", async () => {
  const env = ENV();
  delete env.RESEND_API_KEY;
  const r = await post(env, { message: "x", attachment: { name: "a.mp4", bytes: 3 } });
  const { upload: up } = await r.json();
  failComment = true;
  assert.equal((await upload(env, up.token, new Uint8Array(3))).status, 503);
  failComment = false;
  calls = [];
  assert.equal((await upload(env, up.token, new Uint8Array(3))).status, 201);
  assert.match(calls[0].url, /issues\/7\/comments$/);
});

test("a day's reports and uploads are capped", async () => {
  const env = { ...ENV(), DAILY_REPORTS: "2", DAILY_UPLOAD_MB: "1" };
  assert.equal((await post(env, { message: "a" }, "01JC0000000000000000000001")).status, 201);
  assert.equal((await post(env, { message: "b" }, "01JC0000000000000000000002")).status, 201);
  assert.equal((await post(env, { message: "c" }, "01JC0000000000000000000003")).status, 503);
  // A repeat of one already received is not a new report.
  assert.equal((await post(env, { message: "a" }, "01JC0000000000000000000001")).status, 200);
});

test("a report without a length is refused before it is read", async () => {
  const env = ENV();
  const r = await worker.fetch(new Request(`https://fb.example/v1/reports/${ID}`, { method: "POST", body: "{}" }), env);
  assert.equal(r.status, 411);
});

test("the app's version line cannot add links or headings to the issue", () => {
  const { body } = issueText({ id: ID, received: "now", message: "m", contact: "", client: "wugs\n# https://evil.example", details: null, attachment: null });
  assert.match(body, /- From: `wugs # https:\/\/evil\.example`/);
});
