import { test } from "node:test";
import assert from "node:assert/strict";
import { bestReady, choiceOptions, pinTarget, statusLine } from "./logic.ts";
import type { CapabilityView, ProviderRow } from "../api.ts";

const row = (id: string, ready: boolean, score: number | null = null): ProviderRow => ({ id, name: id.toUpperCase(), kind: "model", ready, note: ready ? "Downloaded." : "Not downloaded.", score });
const view = (over: Partial<CapabilityView>): CapabilityView => ({ id: "phone-transcription", title: "Phone transcription", machine: "automatic", pin: null, effective: null, problem: null, options: [], ...over });

test("automatic matches the backend: best score, then unscored", () => {
  assert.equal(bestReady([row("a", true), row("b", true, 0.3), row("c", true, 0.2), row("d", false, 0.1)])?.id, "c");
  assert.equal(bestReady([row("a", true), row("d", false, 0.1)])?.id, "a");
  assert.equal(bestReady([row("d", false)]), null);
});

test("the picker names what Automatic runs and why unready entries cannot", () => {
  const v = view({ effective: "a", options: [row("a", true), row("b", false)] });
  assert.deepEqual(
    choiceOptions(v).map((o) => o.label),
    ["Automatic (A)", "A", "B (not downloaded)"],
  );
  assert.equal(choiceOptions(view({ options: [row("b", false)], problem: "No model" }))[0].label, "Automatic (nothing ready yet)");
});

test("the status line says what runs, the pin, or the problem", () => {
  assert.equal(statusLine(view({ effective: "a", options: [row("a", true)] })), "Phone transcription uses A on this computer.");
  assert.match(statusLine(view({ effective: "a", pin: "a", options: [row("a", true)] })), /pinned to A: everyone on it/);
  assert.equal(statusLine(view({ problem: "Download one." })), "Download one.");
});

test("only an unpinned capability with something running can be pinned", () => {
  assert.equal(pinTarget(view({ effective: "a" })), "a");
  assert.equal(pinTarget(view({ effective: "a", pin: "a" })), null);
  assert.equal(pinTarget(view({ problem: "x" })), null);
});
