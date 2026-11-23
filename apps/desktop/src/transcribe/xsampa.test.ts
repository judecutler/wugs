import assert from "node:assert/strict";
import { test } from "node:test";
import { preview, xsampaToIpa } from "./xsampa.ts";

test("letters, signs and the longest match", () => {
  assert.equal(xsampaToIpa("S@'n"), "ʃəʲn");
  assert.equal(xsampaToIpa("\"dZ{m"), "ˈdʒæm");
  assert.equal(xsampaToIpa("t_hO:k"), "tʰɔːk");
  assert.equal(xsampaToIpa("r\\`"), "ɻ");
  assert.equal(xsampaToIpa("r\\"), "ɹ");
  assert.equal(xsampaToIpa("G\\_<"), "ʛ");
  assert.equal(xsampaToIpa("b_<a"), "ɓa");
  assert.equal(xsampaToIpa("t`a"), "ʈa");
  assert.equal(xsampaToIpa("t_S"), "t͡ʃ");
  assert.equal(xsampaToIpa("p_>"), "pʼ");
  assert.equal(xsampaToIpa("a~"), "ã", "a combining mark composes with its letter");
  assert.equal(xsampaToIpa("ga"), "ɡa", "g is IPA's script g");
  assert.equal(xsampaToIpa("!\\a |\\a ||\\a"), "ǃa ǀa ǁa");
});

test("what is not X-SAMPA stays", () => {
  assert.equal(xsampaToIpa("ʃə-ma. ka"), "ʃə-ma. ka");
  assert.equal(xsampaToIpa("ŋ😀"), "ŋ😀");
  // Converting twice changes nothing: the IPA it writes is not X-SAMPA.
  const once = xsampaToIpa("\"tSE.r\\i:");
  assert.equal(once, "ˈtʃɛ.ɹiː");
  assert.equal(xsampaToIpa(once), once);
});

test("the preview shows only a change", () => {
  assert.equal(preview("kata"), "");
  assert.equal(preview("kAta"), "→ kɑta");
});
