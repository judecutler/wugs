//! Brown's (1973) fourteen grammatical morphemes counted in CHAT's `%mor`
//! tier, session by session (text by text) for each speaker's track, as
//! CLAN's KIDEVAL reports them: how often each is used, the start of an
//! emergence profile. Mastery needs obligatory contexts, which `%mor`
//! alone does not mark, so these are uses, not percentages supplied.

use serde::Serialize;

use crate::concordance::{each_segment, Read, Scope};
use crate::session::{project, Session};

/// The fourteen, in Brown's order of acquisition, as `%mor` writes them.
pub const MORPHEMES: [&str; 14] = [
    "Present progressive -ing",
    "in",
    "on",
    "Plural -s",
    "Irregular past",
    "Possessive 's",
    "Copula, full form",
    "Articles",
    "Regular past -ed",
    "Third person -s",
    "Irregular third person",
    "Auxiliary, full form",
    "Copula, contracted",
    "Auxiliary, contracted",
];

/// Which of the fourteen one `%mor` part is (by index), if any; `clitic`
/// when it followed a `~` (a contracted form).
fn which(part: &str, clitic: bool, out: &mut [usize; 14]) {
    let Some((pre, rest)) = part.split_once('|') else { return };
    let pos = pre.rsplit('#').next().unwrap_or(pre);
    let main = rest.split('=').next().unwrap_or(rest);
    let cut = main.find(['-', '&']).unwrap_or(main.len());
    let stem = &main[..cut];
    // Each code with the mark before it: `-` for a suffix, `&` for fusion.
    let tail = &main[cut..];
    let marks: Vec<(usize, char)> = tail.char_indices().filter(|(_, c)| *c == '-' || *c == '&').collect();
    let codes: Vec<(char, &str)> = marks.iter().enumerate().map(|(n, (at, mark))| (*mark, &tail[at + 1..marks.get(n + 1).map_or(tail.len(), |(e, _)| *e)])).collect();
    let has = |m: char, c: &str| codes.iter().any(|(mm, cc)| *mm == m && *cc == c);
    let first = pos.split(':').next().unwrap_or(pos);
    if has('-', "PRESP") {
        out[0] += 1;
    }
    if first == "prep" && stem.eq_ignore_ascii_case("in") {
        out[1] += 1;
    }
    if first == "prep" && stem.eq_ignore_ascii_case("on") {
        out[2] += 1;
    }
    if first == "n" && has('-', "PL") {
        out[3] += 1;
    }
    if matches!(first, "v" | "part") && has('&', "PAST") {
        out[4] += 1;
    }
    if (first == "poss" && stem == "s") || has('-', "POSS") {
        out[5] += 1;
    }
    if first == "cop" {
        out[if clitic { 12 } else { 6 }] += 1;
    }
    if pos == "det:art" || first == "art" {
        out[7] += 1;
    }
    if has('-', "PAST") {
        out[8] += 1;
    }
    if first == "v" && has('-', "3S") {
        out[9] += 1;
    }
    if first == "v" && has('&', "3S") {
        out[10] += 1;
    }
    if first == "aux" {
        out[if clitic { 13 } else { 11 }] += 1;
    }
}

/// The fourteen counted in one `%mor` line.
pub fn count(mor: &str) -> [usize; 14] {
    let mut out = [0; 14];
    for item in mor.split_whitespace() {
        for (i, part) in item.split('~').enumerate() {
            for p in part.split('$') {
                which(p, i > 0, &mut out);
            }
        }
    }
    out
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrownSession {
    pub text: String,
    pub title: String,
    /// Utterances with a `%mor` tier.
    pub utterances: usize,
    pub counts: [usize; 14],
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrownTrack {
    pub tier: String,
    pub name: String,
    pub sessions: Vec<BrownSession>,
}

/// Each transcription track with `%mor` in it, text by text (by title).
pub fn brown(session: &mut Session) -> Result<Vec<BrownTrack>, String> {
    let p = project(session)?;
    let mut tracks: Vec<BrownTrack> = Vec::new();
    each_segment(p, Read::Words, &Scope::all(), |seg| {
        let Some(mor) = p.proj.field_str(&seg.id, "x.chat.mor")?.filter(|m| !m.trim().is_empty()) else {
            return Ok(());
        };
        let t = match tracks.iter().position(|t| t.tier == seg.tier) {
            Some(i) => i,
            None => {
                tracks.push(BrownTrack { tier: seg.tier.clone(), name: p.proj.field_str(&seg.tier, "name")?.unwrap_or_default(), sessions: Vec::new() });
                tracks.len() - 1
            }
        };
        let sessions = &mut tracks[t].sessions;
        let s = match sessions.iter().position(|s| s.text == seg.text) {
            Some(i) => i,
            None => {
                sessions.push(BrownSession { text: seg.text.clone(), title: seg.title.clone(), utterances: 0, counts: [0; 14] });
                sessions.len() - 1
            }
        };
        let c = count(&mor);
        let s = &mut sessions[s];
        s.utterances += 1;
        for (a, b) in s.counts.iter_mut().zip(c) {
            *a += b;
        }
        Ok(())
    })?;
    for t in &mut tracks {
        t.sessions.sort_by(|a, b| a.title.cmp(&b.title));
    }
    tracks.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_morpheme_is_found_in_mor() {
        let c = count("pro:sub|I~aux|be&1S part|go-PRESP prep|in det:art|the n|park-PL .");
        assert_eq!((c[0], c[1], c[3], c[7], c[13]), (1, 1, 1, 1, 1));
        let c = count("pro:per|he v|go&PAST prep|on det:art|a n:prop|Mommy~poss|s n|toy .");
        assert_eq!((c[4], c[2], c[5], c[7]), (1, 1, 1, 1));
        let c = count("pro:dem|that cop|be&3S v|jump-PAST v|run-3S v|do&3S aux|be&3S pro:dem|that~cop|be&3S .");
        assert_eq!((c[6], c[8], c[9], c[10], c[11], c[12]), (1, 1, 1, 1, 1, 1));
        assert_eq!(count("."), [0; 14]);
        assert_eq!(count("n|dog-PL"), { let mut x = [0; 14]; x[3] = 1; x });
    }

    #[test]
    fn sessions_by_track() {
        let dir = std::env::temp_dir().join(format!("wugs-brown-{}", wugs_model::ulid()));
        let mut s: crate::session::Session = None;
        crate::session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        for (name, mor) in [("one.cha", "n|doggie-PL ."), ("two.cha", "det:art|the n|doggie-PL .")] {
            let file = dir.join(name);
            std::fs::write(&file, format!("@Begin\n*CHI:\tsomething .\n%mor:\t{mor}\n*MOT:\tyes .\n@End\n")).unwrap();
            crate::interchange::import_file(&mut s, file.to_str().unwrap()).unwrap();
        }
        let r = brown(&mut s).unwrap();
        assert_eq!(r.len(), 1, "MOT has no %mor");
        assert_eq!(r[0].name, "CHI");
        let per: Vec<(&str, usize, usize)> = r[0].sessions.iter().map(|x| (x.title.as_str(), x.counts[3], x.counts[7])).collect();
        assert_eq!(per, vec![("one", 1, 0), ("two", 1, 1)]);
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
