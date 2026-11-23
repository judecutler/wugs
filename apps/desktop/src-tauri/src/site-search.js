// Search for a site wugs publishes ("Publish as a website…"). Plain script,
// no modules, so it runs from a page opened straight from disk. The pages
// load `search-index.js` first, which sets `WUGS_INDEX` to
// { entries: [[id, headword, homograph, glosses]], sentences: [[file, n, text, translation, title]] }.
// Matching ignores case and accents, so a reader without the language's
// keyboard still finds words.

function wugsFold(s) {
  return String(s).normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
}

// The rank of a match: 0 for the whole field, 1 for its start or a word's
// start, 2 anywhere inside; -1 for none.
function wugsRank(field, q) {
  const f = wugsFold(field);
  if (!q) return -1;
  if (f === q) return 0;
  const at = f.indexOf(q);
  if (at < 0) return -1;
  if (at === 0 || /[\s\-=.,;:'"‘’“”()]/u.test(f[at - 1])) return 1;
  return 2;
}

// Entries and sentences matching `query`, best first, at most `limit` of each.
function wugsFind(index, query, limit) {
  const q = wugsFold(query.trim());
  const entries = [];
  const sentences = [];
  if (!q) return { entries, sentences, more: { entries: 0, sentences: 0 } };
  for (const [i, e] of index.entries.entries()) {
    const h = wugsRank(e[1], q);
    const g = wugsRank(e[3], q);
    const best = h >= 0 ? h : g >= 0 ? 3 + g : -1;
    if (best >= 0) entries.push({ rank: best, order: i, id: e[0], headword: e[1], homograph: e[2], glosses: e[3] });
  }
  for (const [i, s] of index.sentences.entries()) {
    const t = wugsRank(s[2], q);
    const tr = wugsRank(s[3], q);
    const best = t >= 0 ? t : tr >= 0 ? 3 + tr : -1;
    if (best >= 0) sentences.push({ rank: best, order: i, file: s[0], n: s[1], text: s[2], translation: s[3], title: s[4] });
  }
  const byRank = (a, b) => a.rank - b.rank || a.order - b.order;
  entries.sort(byRank);
  sentences.sort(byRank);
  return {
    entries: entries.slice(0, limit),
    sentences: sentences.slice(0, limit),
    more: { entries: Math.max(0, entries.length - limit), sentences: Math.max(0, sentences.length - limit) },
  };
}

// `text` split around where `query` matches (case- and accent-blind), as
// [before, match, after] runs of the original characters.
function wugsMark(text, query) {
  const q = wugsFold(query.trim());
  if (!q) return [text];
  // Fold each character on its own, so a position in the folded text maps back.
  const chars = Array.from(text);
  let folded = '';
  const starts = [];
  for (let i = 0; i < chars.length; i++) {
    const f = wugsFold(chars[i]);
    for (let k = 0; k < f.length; k++) starts.push(i);
    folded += f;
  }
  const at = folded.indexOf(q);
  if (at < 0) return [text];
  const from = starts[at];
  const to = at + q.length < starts.length ? starts[at + q.length] : chars.length;
  return [chars.slice(0, from).join(''), chars.slice(from, to).join(''), chars.slice(to).join('')];
}

function wugsSearchPage() {
  const input = document.getElementById('q');
  const out = document.getElementById('results');
  if (!input || !out || typeof WUGS_INDEX === 'undefined') return;
  const LIMIT = 30;
  const line = (cls, text, query) => {
    const el = document.createElement('span');
    el.className = cls;
    const [a, m, b] = wugsMark(text, query);
    el.append(a);
    if (m !== undefined) {
      const mark = document.createElement('mark');
      mark.textContent = m;
      el.append(mark, b);
    }
    return el;
  };
  const section = (title, items, more, make) => {
    if (!items.length) return;
    const h = document.createElement('h2');
    h.textContent = title;
    const ul = document.createElement('ul');
    ul.className = 'hits';
    for (const it of items) ul.append(make(it));
    out.append(h, ul);
    if (more) {
      const p = document.createElement('p');
      p.className = 'count';
      p.textContent = more === 1 ? 'and 1 more; type more of the word to narrow it down.' : `and ${more} more; type more of the word to narrow it down.`;
      out.append(p);
    }
  };
  const show = () => {
    const query = input.value;
    const found = wugsFind(WUGS_INDEX, query, LIMIT);
    out.replaceChildren();
    const params = new URLSearchParams(location.search);
    if (query) params.set('q', query); else params.delete('q');
    history.replaceState(null, '', params.toString() ? `?${params}` : location.pathname);
    if (!query.trim()) return;
    if (!found.entries.length && !found.sentences.length) {
      const p = document.createElement('p');
      p.className = 'count';
      p.textContent = 'Nothing found.';
      out.append(p);
      return;
    }
    section('Dictionary', found.entries, found.more.entries, (e) => {
      const li = document.createElement('li');
      const a = document.createElement('a');
      a.href = `dictionary.html#${encodeURIComponent(e.id)}`;
      a.append(line('hw', e.headword, query));
      if (e.homograph) {
        const n = document.createElement('sub');
        n.textContent = String(e.homograph);
        a.append(n);
      }
      li.append(a, ' ', line('gl', e.glosses, query));
      return li;
    });
    section('In the texts', found.sentences, found.more.sentences, (s) => {
      const li = document.createElement('li');
      const a = document.createElement('a');
      a.href = `${s.file}#s${s.n}`;
      a.append(line('tx', s.text, query));
      li.append(a);
      if (s.translation) li.append(' ', line('ft', `‘${s.translation}’`, query));
      const where = document.createElement('span');
      where.className = 'count';
      where.textContent = ` ${s.title}, ${s.n}`;
      li.append(where);
      return li;
    });
  };
  input.addEventListener('input', show);
  const start = new URLSearchParams(location.search).get('q');
  if (start) {
    input.value = start;
    show();
  }
}

if (typeof document !== 'undefined') wugsSearchPage();
