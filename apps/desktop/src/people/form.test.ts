import { test } from "node:test";
import assert from "node:assert/strict";
import { consentForm, FORM_TERMS, termExplained } from "./form.ts";
import { termLabel } from "./logic.ts";

test("the form names every term the People panel records, with a meaning and Yes and No", () => {
  const html = consentForm({ language: "Tagalog", name: "", roles: [] });
  for (const t of FORM_TERMS) {
    assert.ok(html.includes(termLabel(t)), t);
    assert.ok(termExplained(t).length > 20, t);
    assert.ok(html.includes(termExplained(t)), t);
  }
  assert.equal(html.match(/☐ Yes/g)?.length, FORM_TERMS.length);
  assert.ok(html.includes("<h1>Consent to record and share: Tagalog</h1>"));
});

test("a blank form leaves the name line empty; a person's form fills it, escaped", () => {
  const blank = consentForm({ language: "", name: "", roles: [] });
  assert.ok(blank.includes('<span>Name</span><span class="line"></span>'));
  assert.ok(blank.includes("<h1>Consent to record and share</h1>"));
  const theirs = consentForm({ language: "", name: "Ana <Reyes>", roles: ["speaker", "consultant"] });
  assert.ok(theirs.includes('<span class="line">Ana &lt;Reyes&gt;</span>'));
  assert.ok(theirs.includes('<span class="line">speaker, consultant</span>'));
  assert.ok(theirs.includes("<title>Consent to record and share (Ana &lt;Reyes&gt;)</title>"));
});

test("the form says consent can be withdrawn and leaves room for oral consent", () => {
  const html = consentForm({ language: "", name: "", roles: [] });
  assert.ok(html.includes("change your mind later"));
  assert.ok(html.includes("agreed aloud"));
  assert.ok(html.includes("ethics board"));
});
