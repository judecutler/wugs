//! Collexeme analysis (Stefanowitsch and Gries 2003): which words fill one
//! slot of a construction more often than their frequency would have it.
//! The construction is a CQP query (`[lemma="give"] [] [upos="NOUN"]`) and
//! the slot one of its positions; for each word met there, the 2×2 table
//! is its uses in the slot, the slot's other fillers, its uses elsewhere
//! and every other word of the searched sentences. Strength is Fisher's
//! exact test as `−log10 p`, negative for a word that avoids the slot.

use std::collections::HashMap;

use serde::Serialize;

use crate::cqp::{parse, search};
use crate::session::{project, Session};

/// At most this many rows are returned, strongest first.
pub const LIMIT: usize = 1000;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollexemeRow {
    /// The commonest spelling of the word (or lemma) in the slot.
    pub item: String,
    /// Times it fills the slot.
    pub together: usize,
    /// Times it occurs in the searched sentences.
    pub freq: usize,
    /// Times it would fill the slot by chance.
    pub expected: f64,
    /// −log10 of Fisher's exact p, attraction positive and repulsion negative.
    pub strength: f64,
    pub log_likelihood: f64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collexemes {
    pub rows: Vec<CollexemeRow>,
    /// Matches of the construction whose slot holds a word.
    pub constructions: usize,
    /// Words (not punctuation) in the searched sentences.
    pub tokens: usize,
    /// Different words met in the slot.
    pub types: usize,
    /// The positions the query's matches have, so the panel can offer them.
    pub positions: usize,
}

/// ln Γ(x) by Lanczos (g = 7, n = 9), good to about 15 digits for x > 0.
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

fn ln_choose(n: f64, k: f64) -> f64 {
    ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0)
}

/// `log10 p` of Fisher's exact test, one-tailed in the direction the count
/// leans: `o11` uses of a word in `r1` slot fillers, `c1` uses in all, `n` words.
pub fn fisher_log10(o11: usize, r1: usize, c1: usize, n: usize) -> f64 {
    let (r1f, c1f, nf) = (r1 as f64, c1 as f64, n as f64);
    let ln_p = |k: usize| ln_choose(c1f, k as f64) + ln_choose(nf - c1f, (r1 - k) as f64) - ln_choose(nf, r1f);
    let lo = (r1 + c1).saturating_sub(n);
    let hi = r1.min(c1);
    let attracted = o11 as f64 >= r1f * c1f / nf.max(1.0);
    let ks: Box<dyn Iterator<Item = usize>> = if attracted { Box::new(o11..=hi) } else { Box::new((lo..=o11).rev()) };
    let (mut top, mut sum) = (f64::NEG_INFINITY, 0.0f64);
    for k in ks {
        let l = ln_p(k);
        if l > top {
            sum = sum * (top - l).exp() + 1.0;
            top = l;
        } else {
            sum += (l - top).exp();
        }
        if l < top - 60.0 {
            break;
        }
    }
    let ln_total = (top + sum.ln()).min(0.0);
    ln_total / std::f64::consts::LN_10
}

/// The words (or lemmas) filling position `slot` (from 1) of a query's matches.
pub fn collexemes(session: &mut Session, query: &str, slot: usize, lemma: bool, only_text: Option<&str>) -> Result<Collexemes, String> {
    let q = parse(query).map_err(|e| e.to_string())?;
    if slot == 0 {
        return Err("the slot is counted from 1".into());
    }
    let p = project(session)?;
    let mut freq: HashMap<String, usize> = HashMap::new();
    let mut in_slot: HashMap<String, (usize, Vec<(String, usize)>)> = HashMap::new();
    let (mut tokens, mut constructions, mut positions) = (0usize, 0usize, 0usize);
    let key_of = |t: &crate::cqp::Tok| -> Option<(String, String)> {
        if t.upos == "PUNCT" {
            return None;
        }
        let spelling = p.proj.nfc(if lemma { &t.lemma } else { &t.word });
        let k = wugs_model::fold_key(&spelling);
        (!k.is_empty()).then_some((k, spelling))
    };
    search(p, &q, only_text, |s| {
        for t in s.toks {
            if let Some((k, _)) = key_of(t) {
                tokens += 1;
                *freq.entry(k).or_default() += 1;
            }
        }
        for &(at, len) in &s.matches {
            positions = positions.max(len);
            if slot > len {
                continue;
            }
            let Some((k, spelling)) = key_of(&s.toks[at + slot - 1]) else { continue };
            constructions += 1;
            let e = in_slot.entry(k).or_default();
            e.0 += 1;
            match e.1.iter_mut().find(|(sp, _)| *sp == spelling) {
                Some((_, n)) => *n += 1,
                None => e.1.push((spelling, 1)),
            }
        }
        Ok(())
    })?;
    let types = in_slot.len();
    let mut rows: Vec<CollexemeRow> = in_slot
        .into_iter()
        .map(|(k, (together, spellings))| {
            let f = freq.get(&k).copied().unwrap_or(together);
            let expected = constructions as f64 * f as f64 / tokens.max(1) as f64;
            let log10p = fisher_log10(together, constructions, f, tokens);
            let strength = if together as f64 >= expected { -log10p } else { log10p };
            let (ll, _) = crate::collocates::scores(together, constructions, f, tokens);
            let item = spellings.iter().max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0))).map(|s| s.0.clone()).unwrap_or(k);
            CollexemeRow { item, together, freq: f, expected, strength: strength + 0.0, log_likelihood: ll }
        })
        .collect();
    rows.sort_by(|a, b| b.strength.total_cmp(&a.strength).then(b.together.cmp(&a.together)).then(a.item.cmp(&b.item)));
    rows.truncate(LIMIT);
    Ok(Collexemes { rows, constructions, tokens, types, positions })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fisher_matches_hand_worked_tables() {
        // [[3,1],[1,3]]: (16 + 1) / 70.
        assert!((10f64.powf(fisher_log10(3, 4, 4, 8)) - 17.0 / 70.0).abs() < 1e-12);
        // Two of three slots, four of eighteen words: (84 + 4) / 816.
        assert!((10f64.powf(fisher_log10(2, 3, 4, 18)) - 88.0 / 816.0).abs() < 1e-12);
        // Below chance: the left tail, P(k = 0) = 560 / 816.
        assert!((10f64.powf(fisher_log10(0, 3, 2, 18)) - 560.0 / 816.0).abs() < 1e-12);
        // Huge tables stay finite.
        let big = fisher_log10(900, 1000, 1000, 10_000_000);
        assert!(big.is_finite() && big < -1000.0, "{big}");
        assert!((ln_gamma(10.0) - 362_880f64.ln()).abs() < 1e-10);
    }

    #[test]
    fn the_slot_of_a_construction() {
        let dir = std::env::temp_dir().join(format!("wugs-collex-{}", wugs_model::ulid()));
        let mut session: Session = None;
        crate::session::create_project(&mut session, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut session).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let s = |words: [(&str, &str, &str); 3]| words.iter().enumerate().map(|(i, (f, l, u))| format!("{}\t{f}\t{l}\t{u}\t_\t_\t_\t_\t_\t_\n", i + 1)).collect::<String>() + "\n";
        let ud = [
            s([("they", "they", "PRON"), ("give", "give", "VERB"), ("books", "book", "NOUN")]),
            s([("we", "we", "PRON"), ("give", "give", "VERB"), ("books", "book", "NOUN")]),
            s([("you", "you", "PRON"), ("give", "give", "VERB"), ("money", "money", "NOUN")]),
            s([("books", "book", "NOUN"), ("are", "be", "AUX"), ("good", "good", "ADJ")]),
            s([("money", "money", "NOUN"), ("is", "be", "AUX"), ("good", "good", "ADJ")]),
            s([("we", "we", "PRON"), ("read", "read", "VERB"), ("books", "book", "NOUN")]),
        ]
        .concat();
        wugs_conllu::import_conllu(p, &ud, "giving", &tier).unwrap();
        let c = collexemes(&mut session, "[lemma=\"give\"] []", 2, true, None).unwrap();
        assert_eq!((c.constructions, c.tokens, c.types, c.positions), (3, 18, 2, 2));
        let book = &c.rows[0];
        assert_eq!((book.item.as_str(), book.together, book.freq), ("book", 2, 4));
        assert!((book.strength - -(88f64 / 816.0).log10()).abs() < 1e-9, "{}", book.strength);
        assert!((book.expected - 3.0 * 4.0 / 18.0).abs() < 1e-12);
        // Words, not lemmas: the spelling in the slot.
        let w = collexemes(&mut session, "[lemma=\"give\"] []", 2, false, None).unwrap();
        assert_eq!(w.rows[0].item, "books");
        // A slot past the query's length fills nothing.
        assert_eq!(collexemes(&mut session, "[lemma=\"give\"] []", 3, true, None).unwrap().constructions, 0);
        assert!(collexemes(&mut session, "[lemma=", 1, true, None).is_err());
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
