// The published site's search (src-tauri/src/site-search.js) is a plain
// browser script, so it is loaded here into a fresh context and its pure
// functions are called directly.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

type Found = {
  entries: { id: string; headword: string; rank: number }[];
  sentences: { file: string; n: number; rank: number }[];
  more: { entries: number; sentences: number };
};
type Search = {
  wugsFold(s: string): string;
  wugsFind(index: unknown, q: string, limit: number): Found;
  wugsMark(text: string, q: string): string[];
};

const code = readFileSync(new URL('../../src-tauri/src/site-search.js', import.meta.url), 'utf8');
const ctx: Record<string, unknown> = {};
runInNewContext(code, ctx);
const vm = ctx as unknown as Search;
// Values made in the other context have its own prototypes, so they are copied over plainly.
const plain = <T>(x: T): T => JSON.parse(JSON.stringify(x));
const s: Search = {
  wugsFold: (x) => vm.wugsFold(x),
  wugsFind: (i, q, l) => plain(vm.wugsFind(i, q, l)),
  wugsMark: (t, q) => plain(vm.wugsMark(t, q)),
};

const INDEX = {
  entries: [
    ['lex1', 'θɹiː', null, 'three'],
    ['lex2', 'tuː', 1, 'two'],
    ['lex3', 'tuː', 2, 'too; also'],
    ['lex4', 'café', null, 'coffee house'],
  ],
  sentences: [
    ['text-1.html', 1, 'wʌn tuː θɹiː', 'one two three', 'Counting'],
    ['text-1.html', 2, 'fɔɹ faɪv', 'four five', 'Counting'],
    ['text-2.html', 1, 'Le café est fermé', 'The café is closed', 'Town'],
  ],
};

test('folding ignores case and accents', () => {
  assert.equal(s.wugsFold('Café FERMÉ'), 'cafe ferme');
  assert.equal(s.wugsFold('θɹiː'), 'θɹiː', 'IPA letters are letters, not accents');
});

test('headwords come before glosses, whole matches before partial', () => {
  const found = s.wugsFind(INDEX, 'tuː', 10);
  assert.deepEqual(found.entries.map((e) => e.id), ['lex2', 'lex3']);
  assert.deepEqual(found.sentences.map((x) => [x.file, x.n]), [['text-1.html', 1]]);
  const byGloss = s.wugsFind(INDEX, 'three', 10);
  assert.deepEqual(byGloss.entries.map((e) => e.id), ['lex1'], 'a gloss finds its headword');
  assert.equal(byGloss.sentences.length, 1, 'and the sentence whose translation has it');
});

test('a reader without the accents still finds the word', () => {
  const found = s.wugsFind(INDEX, 'cafe', 10);
  assert.deepEqual(found.entries.map((e) => e.id), ['lex4']);
  assert.deepEqual(found.sentences.map((x) => x.file), ['text-2.html']);
});

test('an empty query finds nothing, and the limit says how many more', () => {
  assert.equal(s.wugsFind(INDEX, '   ', 10).entries.length, 0);
  const one = s.wugsFind(INDEX, 't', 1);
  assert.equal(one.entries.length, 1);
  assert.ok(one.more.entries >= 1);
});

test('the match is marked in the original characters', () => {
  assert.deepEqual(s.wugsMark('Le café est fermé', 'CAFE'), ['Le ', 'café', ' est fermé']);
  // Decomposed text keeps its combining accent inside the mark.
  assert.deepEqual(s.wugsMark('Le café est', 'café'), ['Le ', 'café', ' est']);
  assert.deepEqual(s.wugsMark('nothing here', 'zzz'), ['nothing here']);
});
