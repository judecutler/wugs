import assert from "node:assert/strict";
import { test } from "node:test";
import { fileName, outcomeLines, recordingTip, sendBlocker, systemOf } from "./logic.ts";

test("the recording tip names the keys for the person's own system", () => {
  assert.equal(systemOf("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"), "windows");
  assert.equal(systemOf("Macintosh; Intel Mac OS X"), "mac");
  assert.equal(systemOf("X11; Linux x86_64"), "linux");
  assert.match(recordingTip("windows"), /Windows \+ Shift \+ R/);
  assert.match(recordingTip("mac"), /Shift \+ Command \+ 5/);
});

test("the form can be sent once it says what happened", () => {
  assert.equal(sendBlocker("  ", ""), "Write what happened first.");
  assert.equal(sendBlocker("It froze", ""), null);
  assert.match(sendBlocker("It froze", "maria at uni") ?? "", /email/);
  assert.equal(sendBlocker("It froze", "maria@uni.edu"), null);
  assert.match(sendBlocker("x".repeat(10_001), "") ?? "", /under 10,000/);
});

test("the toast says plainly whether it went", () => {
  assert.deepEqual(outcomeLines({ sent: true, waiting: 0, problem: null }).lines, ["Thank you. Your feedback was sent."]);
  const offline = outcomeLines({ sent: false, waiting: 1, problem: "Could not reach the feedback service." });
  assert.equal(offline.tone, "warn");
  assert.match(offline.lines[0], /saved on this computer/);
  assert.match(outcomeLines({ sent: false, waiting: 0, problem: "The report has no message." }).lines[0], /could not be sent/);
});

test("an attachment is shown by its own name", () => {
  assert.equal(fileName("C:\\Users\\a\\Videos\\Screen Recordings\\clip.mp4"), "clip.mp4");
  assert.equal(fileName("/home/a/clip.webm"), "clip.webm");
});
