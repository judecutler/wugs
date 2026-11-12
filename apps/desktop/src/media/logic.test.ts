import { test } from "node:test";
import assert from "node:assert/strict";
import { CARD_TYPE, cardDrag, cards, isCardDrag, duration, normalized, wavePath } from "./logic.ts";

test("cards are the recordings with audio, marked for the text on screen", () => {
  const sources = [
    { id: "s1", label: "Counting", kind: "audio", access: null, hasMedia: true },
    { id: "s2", label: "", kind: "audio", access: "restricted", hasMedia: true },
    { id: "s3", label: "Scan", kind: "image", access: null, hasMedia: false },
    { id: "s4", label: "Lost", kind: "audio", access: null, hasMedia: false },
  ];
  const texts = [
    { id: "t1", title: "Counting to ten", recording: "s1" },
    { id: "t2", title: "Again", recording: "s1" },
    { id: "t3", title: "Fox", recording: null },
  ];
  const c = cards(sources, texts, "t1");
  assert.deepEqual(c.map((x) => x.id), ["s1", "s2"]);
  assert.equal(c[0]!.inThisText, true);
  assert.deepEqual(c[0]!.usedBy, ["Counting to ten", "Again"]);
  assert.equal(c[1]!.label, "(unnamed recording)");
  assert.equal(c[1]!.access, "restricted");
  assert.equal(cards(sources, texts, "t3")[0]!.inThisText, false);
});

test("durations read like a video editor's", () => {
  assert.equal(duration(4200), "0:04");
  assert.equal(duration(187_000), "3:07");
  assert.equal(duration(3_723_000), "1:02:03");
});

test("the waveform path goes along the maxima and back along the minima", () => {
  assert.equal(wavePath([], [], 100, 20), "M0 10 H100");
  assert.equal(wavePath([-1, -0.5], [1, 0.5], 100, 20), "M0 0 L100 5 L100 15 L0 20 Z");
  assert.equal(wavePath([-2], [2], 10, 20), "M5 0 L5 20 Z");
});

test("peaks are scaled so the loudest reaches the edge", () => {
  assert.deepEqual(normalized([-100, -50], [200, 0]), { mins: [-0.5, -0.25], maxs: [1, 0] });
  assert.deepEqual(normalized([0], [0]), { mins: [0], maxs: [0] });
});

test("a card drag is live from its start until its end, and lapses", () => {
  assert.equal(cardDrag.active(0), false);
  cardDrag.start(1000);
  assert.equal(cardDrag.active(2000), true);
  assert.equal(cardDrag.active(70_000), false, "a lost dragend never leaves the guard open");
  cardDrag.end();
  assert.equal(cardDrag.active(2000), false);
});

test("a card drag carries the card type and nothing to open", () => {
  assert.equal(isCardDrag(true, [CARD_TYPE]), true);
  assert.equal(isCardDrag(false, [CARD_TYPE]), false, "only a drag started in the app");
  assert.equal(isCardDrag(true, ["text/uri-list", CARD_TYPE]), false, "a link dragged in meanwhile");
  assert.equal(isCardDrag(true, ["text/plain"]), false);
  assert.equal(isCardDrag(true, ["Files", CARD_TYPE]), false);
});
