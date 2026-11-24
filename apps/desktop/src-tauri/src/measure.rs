//! Measurements for every segment of a track: duration, pitch, intensity and
//! formants as one CSV table, the job phoneticians write Praat scripts for.
//! The numbers come from the `acoustic-analysis` capability (Praat when it
//! is found, else wugs's built-in analysis), the same frames the Timeline's
//! overlays draw, so a value in the table is a value on the screen.

use std::path::Path;

use serde::Serialize;

use crate::capability::{self, Capability, Provider};
use crate::praat_analysis::{self, Chunk, Job, CHUNK_MS, MAX_CHUNKS};
use crate::session::{self, Session};

/// One segment to measure.
#[derive(Clone, Debug)]
pub struct Target {
    pub segment: String,
    pub label: String,
    pub start_ms: f64,
    pub end_ms: f64,
}

/// What one segment measured. Each value is None where no frame gave one
/// (silence, a voiceless stretch, past the end of the recording).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measured {
    pub frames: usize,
    pub f0_mean: Option<f64>,
    pub f0_min: Option<f64>,
    pub f0_max: Option<f64>,
    /// Voiced frames out of all frames, in percent.
    pub voiced_percent: Option<f64>,
    /// Mean intensity, averaged as energy (Praat's default for Get mean).
    pub intensity_db: Option<f64>,
    /// F1–F3 at the frame nearest the middle.
    pub mid: [Option<f64>; 3],
    /// F1–F3 averaged over the middle half (the steady part of a vowel).
    pub middle_half: [Option<f64>; 3],
    /// The pitch contour at ten points spread evenly through the segment
    /// (5%, 15% … 95% of the way), each from the voiced frame nearest that
    /// point within a twentieth of the segment (at least one frame); None
    /// where the voice is off there. What tone and intonation studies plot.
    pub contour: [Option<f64>; CONTOUR_POINTS],
    /// F1 and F2 at 20% and 80% of the segment (`[[F1, F2] at 20%, [F1, F2]
    /// at 80%]`), each the mean of the frames within a twentieth of the
    /// segment (at least one frame step) of that point: the two ends of a
    /// vowel's trajectory, what diphthong and vowel-inherent spectral change
    /// studies plot.
    pub edges: [[Option<f64>; 2]; 2],
}

/// How many time-normalized pitch points each segment gets.
pub const CONTOUR_POINTS: usize = 10;

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// The chunks `targets` need, in order.
pub fn chunks_for(targets: &[Target]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for t in targets {
        let first = (t.start_ms.max(0.0) / f64::from(CHUNK_MS)).floor() as u32;
        let last = ((t.end_ms.max(0.0) - 1e-9).max(0.0) / f64::from(CHUNK_MS)).floor() as u32;
        out.extend(first..=last);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// One segment's measurements from the analysis frames whose centres fall
/// inside it.
pub fn measure(target: &Target, chunks: &[Chunk]) -> Measured {
    struct Frame {
        t: f64,
        pitch: Option<f32>,
        db: Option<f32>,
        f: [Option<f32>; 3],
    }
    let frames: Vec<Frame> = chunks
        .iter()
        .flat_map(|c| {
            (0..c.pitch.len()).map(move |i| Frame {
                t: c.start_ms + (i as f64 + 0.5) * c.step_ms,
                pitch: c.pitch[i],
                db: c.intensity.get(i).copied().flatten(),
                f: [c.f1.get(i).copied().flatten(), c.f2.get(i).copied().flatten(), c.f3.get(i).copied().flatten()],
            })
        })
        .filter(|f| f.t >= target.start_ms && f.t < target.end_ms)
        .collect();
    if frames.is_empty() {
        return Measured::default();
    }
    let pitch: Vec<f64> = frames.iter().filter_map(|f| f.pitch.map(f64::from)).collect();
    let energy: Vec<f64> = frames.iter().filter_map(|f| f.db.map(|d| 10f64.powf(f64::from(d) / 10.0))).collect();
    let mid_t = (target.start_ms + target.end_ms) / 2.0;
    let nearest = frames.iter().min_by(|a, b| (a.t - mid_t).abs().total_cmp(&(b.t - mid_t).abs())).expect("not empty");
    let quarter = (target.end_ms - target.start_ms) / 4.0;
    let middle: Vec<&Frame> = frames.iter().filter(|f| f.t >= target.start_ms + quarter && f.t < target.end_ms - quarter).collect();
    let formant_mean = |k: usize| {
        let v: Vec<f64> = middle.iter().filter_map(|f| f.f[k].map(f64::from)).collect();
        mean(&v)
    };
    let span = target.end_ms - target.start_ms;
    let step = chunks.first().map_or(10.0, |c| c.step_ms);
    let reach = (span / (2.0 * CONTOUR_POINTS as f64)).max(step);
    let contour = std::array::from_fn(|i| {
        let at = target.start_ms + span * (i as f64 + 0.5) / CONTOUR_POINTS as f64;
        frames
            .iter()
            .filter_map(|f| Some(((f.t - at).abs(), f.pitch?)))
            .filter(|(d, _)| *d <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| f64::from(p))
    });
    let edge = |at: f64| -> [Option<f64>; 2] {
        let near: Vec<&Frame> = frames.iter().filter(|f| (f.t - at).abs() <= reach).collect();
        std::array::from_fn(|k| mean(&near.iter().filter_map(|f| f.f[k].map(f64::from)).collect::<Vec<_>>()))
    };
    let edges = [edge(target.start_ms + 0.2 * span), edge(target.start_ms + 0.8 * span)];
    Measured {
        frames: frames.len(),
        f0_mean: mean(&pitch),
        f0_min: pitch.iter().copied().reduce(f64::min),
        f0_max: pitch.iter().copied().reduce(f64::max),
        voiced_percent: Some(100.0 * pitch.len() as f64 / frames.len() as f64),
        intensity_db: mean(&energy).map(|e| 10.0 * e.log10()),
        mid: nearest.f.map(|v| v.map(f64::from)),
        middle_half: [formant_mean(0), formant_mean(1), formant_mean(2)],
        contour,
        edges,
    }
}

/// A CSV cell: quoted when it holds a comma, quote or line break.
fn cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

fn num(v: Option<f64>, places: usize) -> String {
    v.map(|x| format!("{x:.places$}")).unwrap_or_default()
}

pub const COLUMNS: &[&str] = &[
    "text", "track", "segment", "label", "start_s", "end_s", "duration_ms", "f0_mean_hz", "f0_min_hz", "f0_max_hz", "voiced_percent", "intensity_db", "f1_mid_hz", "f2_mid_hz",
    "f3_mid_hz", "f1_mean_hz", "f2_mean_hz", "f3_mean_hz", "f0_05pct_hz", "f0_15pct_hz", "f0_25pct_hz", "f0_35pct_hz", "f0_45pct_hz", "f0_55pct_hz", "f0_65pct_hz",
    "f0_75pct_hz", "f0_85pct_hz", "f0_95pct_hz", "f1_20pct_hz", "f2_20pct_hz", "f1_80pct_hz", "f2_80pct_hz", "frames", "analysed_by",
];

/// The table: a header, then a row per target in time order.
pub fn table(text: &str, track: &str, analysed_by: &str, rows: &[(Target, Measured)]) -> String {
    let mut out = COLUMNS.join(",");
    out.push('\n');
    for (t, m) in rows {
        let mut cells = vec![
            cell(text),
            cell(track),
            cell(&t.segment),
            cell(&t.label),
            format!("{:.3}", t.start_ms / 1000.0),
            format!("{:.3}", t.end_ms / 1000.0),
            format!("{:.0}", t.end_ms - t.start_ms),
            num(m.f0_mean, 1),
            num(m.f0_min, 1),
            num(m.f0_max, 1),
            num(m.voiced_percent, 0),
            num(m.intensity_db, 1),
            num(m.mid[0], 0),
            num(m.mid[1], 0),
            num(m.mid[2], 0),
            num(m.middle_half[0], 0),
            num(m.middle_half[1], 0),
            num(m.middle_half[2], 0),
        ];
        cells.extend(m.contour.iter().map(|v| num(*v, 1)));
        cells.extend(m.edges.iter().flatten().map(|v| num(*v, 0)));
        cells.extend([m.frames.to_string(), cell(analysed_by)]);
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

/// Everything the measuring needs from the project, read on the session thread.
pub struct Prepared {
    pub job: Job,
    pub text: String,
    pub track: String,
    pub targets: Vec<Target>,
    /// Segments on the track with no times, which cannot be measured.
    pub untimed: usize,
    pub pin: Option<String>,
}

pub fn prepare(s: &mut Session, text_id: &str, tier_id: &str, source: &str) -> Result<Prepared, String> {
    let detail = session::get_text(s, text_id)?;
    let track = detail.tiers.iter().find(|t| t.id == tier_id).map(|t| t.name.clone()).ok_or("That track is not in this text.")?;
    let mut targets = Vec::new();
    let mut untimed = 0;
    for seg in detail.segments.iter().filter(|x| x.tier == tier_id) {
        match (seg.start_ms, seg.end_ms) {
            (Some(a), Some(b)) if b > a => targets.push(Target { segment: seg.id.clone(), label: seg.content.clone(), start_ms: a, end_ms: b }),
            _ => untimed += 1,
        }
    }
    targets.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms).then(a.end_ms.total_cmp(&b.end_ms)));
    let job = praat_analysis::job(s, source)?;
    let pin = capability::project_pin(session::project(s)?, Capability::AcousticAnalysis)?;
    Ok(Prepared { job, text: detail.title, track, targets, untimed, pin })
}

/// Chunks from the provider, at most `MAX_CHUNKS` a call.
pub fn analyse(provider: &Provider, job: &Job, chunks: &[u32]) -> Result<Vec<Chunk>, String> {
    let mut out = Vec::new();
    for group in chunks.chunks(MAX_CHUNKS) {
        out.extend(match provider {
            Provider::Praat(exe) => praat_analysis::analyse(exe, job, group)?,
            Provider::VoiceAnalysis => crate::voice_analysis::analyse(&job.wav, &job.voice, group)?,
            _ => return Err("Acoustic analysis has no provider here.".into()),
        });
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasureReport {
    pub path: String,
    pub measured: usize,
    pub untimed: usize,
    /// Segments with no voiced frame at all.
    pub unvoiced: usize,
    pub analysed_by: String,
}

/// Every segment measured, with what analysed them.
pub struct Computed {
    pub rows: Vec<(Target, Measured)>,
    pub analysed_by: String,
}

/// Measure `p`'s segments, off the session thread.
pub fn compute(p: &Prepared, app_data: &Path) -> Result<Computed, String> {
    if p.targets.is_empty() {
        return Err(if p.untimed > 0 { "This track's segments have no times, so there is nothing to measure.".into() } else { "This track has no segments to measure.".into() });
    }
    let provider = capability::resolve(Capability::AcousticAnalysis, p.pin.as_deref(), app_data)?;
    let analysed_by = match &provider {
        Provider::Praat(exe) => crate::praat::version_of(exe).unwrap_or_else(|_| "Praat".into()),
        other => capability::provider_name(other.id()),
    };
    let analysed_by = format!("{analysed_by}; {}", p.job.voice.describe());
    let chunks = analyse(&provider, &p.job, &chunks_for(&p.targets))?;
    let rows: Vec<(Target, Measured)> = p.targets.iter().map(|t| (t.clone(), measure(t, &chunks))).collect();
    Ok(Computed { rows, analysed_by })
}

/// Measure `p`, off the session thread, and write the table into `folder`.
pub fn run(p: Prepared, app_data: &Path, folder: &Path) -> Result<MeasureReport, String> {
    let Computed { rows, analysed_by } = compute(&p, app_data)?;
    let unvoiced = rows.iter().filter(|(_, m)| m.f0_mean.is_none()).count();
    let body = table(&p.text, &p.track, &analysed_by, &rows);
    let stem = format!("{} {} measurements", crate::interchange::file_stem(&p.text, "text"), crate::interchange::file_stem(&p.track, "track"));
    let path = crate::interchange::write_new(folder, &stem, "csv", body.as_bytes())?;
    Ok(MeasureReport { path: path.display().to_string(), measured: rows.len(), untimed: p.untimed, unvoiced, analysed_by })
}

/// One measured segment for the Vowel space and Pitch contours panels.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointView {
    pub segment: String,
    pub label: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub f0: Option<f64>,
    /// F1–F3 over the middle half, else at the middle frame.
    pub f1: Option<f64>,
    pub f2: Option<f64>,
    pub f3: Option<f64>,
    /// F0 at 5%, 15% … 95% of the segment (`Measured::contour`), for the Pitch contours panel.
    pub contour: Vec<Option<f64>>,
    /// F1 and F2 at 20% (`onset`) and 80% (`offset`) of the segment, for the Vowel space's trajectories.
    pub onset: [Option<f64>; 2],
    pub offset: [Option<f64>; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointsView {
    pub points: Vec<PointView>,
    pub untimed: usize,
    pub analysed_by: String,
}

pub fn points(p: &Prepared, app_data: &Path) -> Result<PointsView, String> {
    let Computed { rows, analysed_by } = compute(p, app_data)?;
    let points = rows
        .into_iter()
        .map(|(t, m)| {
            let f = |k: usize| m.middle_half[k].or(m.mid[k]);
            PointView { segment: t.segment, label: t.label, start_ms: t.start_ms, end_ms: t.end_ms, f0: m.f0_mean, f1: f(0), f2: f(1), f3: f(2), contour: m.contour.to_vec(), onset: m.edges[0], offset: m.edges[1] }
        })
        .collect();
    Ok(PointsView { points, untimed: p.untimed, analysed_by })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(index: u32, frames: Vec<(Option<f32>, Option<f32>, [Option<f32>; 3])>) -> Chunk {
        Chunk {
            index,
            start_ms: f64::from(index) * f64::from(CHUNK_MS),
            step_ms: 10.0,
            pitch: frames.iter().map(|f| f.0).collect(),
            intensity: frames.iter().map(|f| f.1).collect(),
            f1: frames.iter().map(|f| f.2[0]).collect(),
            f2: frames.iter().map(|f| f.2[1]).collect(),
            f3: frames.iter().map(|f| f.2[2]).collect(),
        }
    }

    fn target(a: f64, b: f64) -> Target {
        Target { segment: "s1".into(), label: "a, \"b\"".into(), start_ms: a, end_ms: b }
    }

    #[test]
    fn a_segment_reads_the_frames_inside_it() {
        // Ten frames: 0–40 ms voiced at 100–140 Hz and 60 dB, then unvoiced at 40 dB.
        let frames = (0..10)
            .map(|i| {
                let voiced = i < 5;
                (voiced.then_some(100.0 + 10.0 * i as f32), Some(if voiced { 60.0 } else { 40.0 }), [Some(500.0 + i as f32), Some(1500.0), None])
            })
            .collect();
        let c = [chunk(0, frames)];
        let m = measure(&target(0.0, 100.0), &c);
        assert_eq!(m.frames, 10);
        assert_eq!(m.f0_mean, Some(120.0));
        assert_eq!((m.f0_min, m.f0_max), (Some(100.0), Some(140.0)));
        assert_eq!(m.voiced_percent, Some(50.0));
        // Energy average: 10·log10((5·10⁶ + 5·10⁴) / 10).
        assert!((m.intensity_db.unwrap() - 57.03).abs() < 0.01, "{:?}", m.intensity_db);
        // The middle is 50 ms: frames at 45 and 55 are equally near, the first wins.
        assert_eq!(m.mid, [Some(504.0), Some(1500.0), None]);
        // The middle half is 25–75 ms: frames at 25, 35, 45, 55, 65.
        assert_eq!(m.middle_half, [Some(504.0), Some(1500.0), None]);
        // The contour: tenths of 100 ms at 5, 15 … 95 ms; the voice stops after 45 ms.
        assert_eq!(m.contour, [Some(100.0), Some(110.0), Some(120.0), Some(130.0), Some(140.0), Some(140.0), None, None, None, None]);
        // The trajectory's ends: frames within 10 ms of 20 ms (15, 25) and of 80 ms (75, 85).
        assert_eq!(m.edges, [[Some(501.5), Some(1500.0)], [Some(507.5), Some(1500.0)]]);
        // A stretch past the frames measures nothing.
        assert_eq!(measure(&target(200.0, 300.0), &c), Measured::default());
    }

    #[test]
    fn segments_across_chunks_ask_for_both() {
        let ts = [target(9_990.0, 10_010.0), target(25_000.0, 30_000.0), target(5.0, 9.0)];
        assert_eq!(chunks_for(&ts), [0, 1, 2]);
        assert_eq!(chunks_for(&[target(10_000.0, 20_000.0)]), [1]);
    }

    #[test]
    fn the_table_quotes_labels_and_leaves_missing_values_empty() {
        let rows = [(target(1000.0, 1250.0), Measured { frames: 25, f0_mean: Some(118.25), voiced_percent: Some(80.0), ..Default::default() })];
        let t = table("Text, one", "utterance", "Acoustic analysis (built in)", &rows);
        let mut lines = t.lines();
        assert_eq!(lines.next().unwrap().split(',').count(), COLUMNS.len());
        assert_eq!(lines.next().unwrap(), "\"Text, one\",utterance,s1,\"a, \"\"b\"\"\",1.000,1.250,250,118.2,,,80,,,,,,,,,,,,,,,,,,,,,,25,Acoustic analysis (built in)");
    }

    /// The tutorial project's utterance track, measured end to end with the
    /// built-in analysis (or Praat, where it is installed): one row per timed
    /// segment, a voice in the counted words, and the table in the folder.
    #[test]
    fn the_tutorial_track_is_measured() {
        let dir = std::env::temp_dir().join(format!("wugs-measure-{}", wugs_model::ulid()));
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("project"), "tester").unwrap();
        let text = session::list_texts(&mut s).unwrap().remove(0);
        let detail = session::get_text(&mut s, &text.id).unwrap();
        let tier = detail.tiers.iter().find(|t| t.utterance).unwrap().id.clone();
        let source = text.recording.clone().expect("the tutorial is linked");
        let timed = detail.segments.iter().filter(|x| x.tier == tier && x.start_ms.is_some()).count();
        // Inside the project is refused.
        assert!(crate::interchange::outside_project(&mut s, &dir.join("project")).is_err());
        let p = prepare(&mut s, &text.id, &tier, &source).unwrap();
        // The Vowel space panel's points: one per timed segment, with F1 and F2.
        let pts = points(&p, &dir.join("app-data")).unwrap();
        assert_eq!(pts.points.len(), timed);
        assert!(pts.points.iter().filter(|x| x.f1.is_some() && x.f2.is_some()).count() >= 2);
        // And their pitch contours: ten points each, voiced in the counted words.
        assert!(pts.points.iter().all(|x| x.contour.len() == CONTOUR_POINTS));
        assert!(pts.points.iter().any(|x| x.contour.iter().filter(|v| v.is_some()).count() >= 5), "a spoken word has a contour");
        let report = run(p, &dir.join("app-data"), &out).unwrap();
        assert_eq!(report.measured, timed);
        let csv = std::fs::read_to_string(&report.path).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), timed + 1);
        assert!(lines[1].contains("wʌn tuː θɹiː"), "{}", lines[1]);
        // A man counting: his pitch is in a speaking range, and F1 below F2.
        let cells: Vec<&str> = lines[1].split(',').collect();
        let f0: f64 = cells[7].parse().unwrap();
        assert!((70.0..250.0).contains(&f0), "{f0}");
        let (f1, f2): (f64, f64) = (cells[15].parse().unwrap(), cells[16].parse().unwrap());
        assert!(f1 < f2, "{f1} {f2}");
        std::fs::remove_dir_all(dir).ok();
    }
}
