import assert from "node:assert/strict";
import { test } from "node:test";
import { parsePattern, readName, tidyDate, writesFor } from "./names.ts";

const ok = (s: string) => {
  const p = parsePattern(s);
  if ("error" in p) throw new Error(p.error);
  return p;
};

test("a pattern reads each file name's details", () => {
  const p = ok("{date}_{participants}_{genre}");
  assert.deepEqual(p.keys, ["date", "participants", "genre"]);
  assert.deepEqual(readName(p, "20260312_Ana_story.wav"), [["date", "2026-03-12"], ["participants", "Ana"], ["genre", "story"]]);
  // The last piece takes the rest; underscores inside it read as spaces.
  assert.deepEqual(readName(p, "2026-03-13_Ben_life_story.WAV"), [["date", "2026-03-13"], ["participants", "Ben"], ["genre", "life story"]]);
  assert.equal(readName(p, "interview.wav"), null, "a name that does not fit");
  assert.deepEqual(readName(ok("KAN-{skip}-{place}"), "C:\\\\rec\\\\KAN-007-Kelani.wav"), [["place", "Kelani"]]);
});

test("a pattern that cannot be read says why", () => {
  assert.match((parsePattern("date_genre") as { error: string }).error, /at least one detail/);
  assert.match((parsePattern("{date}{genre}") as { error: string }).error, /between them/);
  assert.match((parsePattern("{date}_{date}") as { error: string }).error, /twice/);
  assert.match((parsePattern("{date}_{genre") as { error: string }).error, /not closed/);
  assert.deepEqual(ok("{Recorded By} {Place}").keys, ["recorded_by", "place"]);
});

test("only empty details are written unless replacing", () => {
  const read: [string, string][] = [["date", "2026-03-12"], ["genre", "story"]];
  assert.deepEqual(writesFor(read, [["genre", "song"]], false), [["date", "2026-03-12"]]);
  assert.deepEqual(writesFor(read, [["genre", "song"], ["date", "2026-03-12"]], true), [["genre", "story"]]);
  assert.equal(tidyDate("2026.03.12"), "2026-03-12");
  assert.equal(tidyDate("March"), "March");
});
