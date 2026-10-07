//! Input contract for frame sequences extracted by an external tool.
//!
//! A [`FrameMap`](crate::frame_map::FrameMap) lists, for every extracted frame, its frame index, its
//! presentation timestamp in seconds and the file holding it. [`check`](crate::frame_map::check) reports
//! what the map does and does not support. It never resamples, interpolates or
//! guesses: a gap, a variable frame rate or a sequence that never settles is an
//! explicit state with reasons, and conclusions that need uniform time are
//! flagged unusable rather than approximated.
//!
//! State precedence in [`Check::state`](crate::frame_map::Check::state): `invalid_files`, `missing_frames`,
//! `variable_frame_rate`, `constant_frame_rate`, `single_frame`. The per-step
//! rate is the timestamp step divided by the index step, so a dropped frame is
//! reported as a gap and does not by itself make the rate "variable".
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path};

/// Map discriminator.
pub const SCHEMA: &str = "saccade-frame-map.v1";
/// Check report discriminator.
pub const CHECK_SCHEMA: &str = "saccade-frame-map-check.v1";
/// Largest accepted frame count.
pub const MAX_FRAMES: usize = 100_000;
/// Gaps and file problems listed individually; totals are always exact.
pub const LIST_LIMIT: usize = 32;

/// One extracted frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Frame index in the source sequence.
    pub index: u64,
    /// Presentation time in seconds from the sequence start.
    pub timestamp_s: f64,
    /// Relative path of the frame image.
    pub file: String,
    /// Expected SHA-256 (hex) of the file, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Frame map: index, timestamp and file for each frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameMap {
    /// Version discriminator.
    pub schema: String,
    /// Frame rate the producer believes it extracted at, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nominal_fps: Option<f64>,
    /// Frames in increasing index and timestamp order.
    pub frames: Vec<Frame>,
}

/// Check settings.
#[derive(Debug, Clone, Copy)]
pub struct CheckPolicy {
    /// Relative spread of per-step rate, in percent, still called constant.
    pub rate_tolerance_pct: f64,
}

impl Default for CheckPolicy {
    fn default() -> Self {
        Self {
            rate_tolerance_pct: 1.0,
        }
    }
}

/// A run of missing frame indices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gap {
    /// Last present index before the gap.
    pub after_index: u64,
    /// Number of indices absent.
    pub missing: u64,
}

/// Timing summary.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Timing {
    /// `constant`, `variable` or `single_frame`.
    pub state: String,
    /// Median seconds per frame step; null for a single frame.
    pub median_step_s: Option<f64>,
    /// Smallest seconds per frame step.
    pub min_step_s: Option<f64>,
    /// Largest seconds per frame step.
    pub max_step_s: Option<f64>,
    /// 1 / median step.
    pub effective_fps: Option<f64>,
    /// Producer-declared rate.
    pub nominal_fps: Option<f64>,
    /// Whether the declared rate matches the effective rate within tolerance; null when either is unknown.
    pub nominal_agrees: Option<bool>,
}

/// File verification.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Files {
    /// Whether files were looked up (a root directory was given).
    pub checked: bool,
    /// Total missing files.
    pub missing_count: u64,
    /// Up to 32 missing paths.
    pub missing: Vec<String>,
    /// Total files whose hash differs from the map.
    pub hash_mismatch_count: u64,
    /// Up to 32 paths with a hash mismatch.
    pub hash_mismatch: Vec<String>,
}

/// What the sequence supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Usable {
    /// Frame-by-frame comparison by index: contiguous, files verified when checked.
    pub index_aligned: bool,
    /// Analyses that convert frames to seconds with one rate.
    pub uniform_time: bool,
}

/// Event-relative settling interpreted with the map's timestamps.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settle {
    /// `settled`, `never_settled` or `change_outside_sequence`.
    pub state: String,
    /// Position of the declared change in the sequence.
    pub change_position: u64,
    /// Position at which every tile settled, if it did.
    pub settle_position: Option<u64>,
    /// Timestamp of the change frame.
    pub change_timestamp_s: Option<f64>,
    /// Timestamp of the settling frame.
    pub settle_timestamp_s: Option<f64>,
    /// Elapsed seconds from the map's own timestamps; null unless settled.
    pub elapsed_s: Option<f64>,
    /// Seconds in the settling report, which assumes a fixed rate.
    pub report_elapsed_s: Option<f64>,
    /// Whether the report's fixed-rate conversion matches the map's timing.
    pub fixed_rate_assumption_holds: bool,
}

/// Check result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    /// Version discriminator.
    pub schema: String,
    /// Overall state; see the module documentation.
    pub state: String,
    /// Number of frames.
    pub frames: u64,
    /// First frame index.
    pub first_index: u64,
    /// Last frame index.
    pub last_index: u64,
    /// True when no index is missing.
    pub contiguous: bool,
    /// Total missing indices.
    pub missing_total: u64,
    /// Up to 32 gaps.
    pub gaps: Vec<Gap>,
    /// Timing summary.
    pub timing: Timing,
    /// File verification.
    pub files: Files,
    /// What analyses the sequence supports.
    pub usable_for: Usable,
    /// Settling interpretation when a report was supplied.
    pub settle: Option<Settle>,
    /// Plain reasons behind any non-constant, non-contiguous or unusable state.
    pub reasons: Vec<String>,
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::Config(msg.into())
}

fn safe_relative(file: &str) -> bool {
    let p = Path::new(file);
    !file.is_empty()
        && !p.is_absolute()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Check structure and, when `root` is given, the files. See the module documentation.
///
/// `settling` is an optional `saccade-settling.v1` report; its frame count must
/// equal the map's. Structural defects (empty map, unsafe path, duplicate or
/// out-of-order index or timestamp, non-finite time) are errors.
pub fn check(
    map: &FrameMap,
    root: Option<&Path>,
    policy: CheckPolicy,
    settling: Option<&Value>,
) -> Result<Check> {
    let tol = policy.rate_tolerance_pct;
    if map.schema != SCHEMA
        || map.frames.is_empty()
        || map.frames.len() > MAX_FRAMES
        || !tol.is_finite()
        || !(0.0..=50.0).contains(&tol)
        || map.nominal_fps.is_some_and(|f| !f.is_finite() || f <= 0.0)
    {
        return Err(invalid(
            "frame map needs its schema, 1..100000 frames and a rate tolerance of 0..50 percent",
        ));
    }
    for (i, f) in map.frames.iter().enumerate() {
        if !f.timestamp_s.is_finite() || f.timestamp_s < 0.0 || !safe_relative(&f.file) {
            return Err(invalid(format!(
                "frame {i}: needs a finite non-negative timestamp and a relative path inside the map root"
            )));
        }
        if i > 0 {
            let p = &map.frames[i - 1];
            if f.index <= p.index || f.timestamp_s <= p.timestamp_s {
                return Err(invalid(format!(
                    "frame {i}: index and timestamp must strictly increase"
                )));
            }
        }
    }
    let mut gaps = Vec::new();
    let mut missing_total = 0;
    let mut steps = Vec::new();
    for w in map.frames.windows(2) {
        let di = w[1].index - w[0].index;
        if di > 1 {
            missing_total += di - 1;
            gaps.push(Gap {
                after_index: w[0].index,
                missing: di - 1,
            });
        }
        steps.push((w[1].timestamp_s - w[0].timestamp_s) / di as f64);
    }
    let contiguous = missing_total == 0;
    gaps.truncate(LIST_LIMIT);
    let mut sorted = steps.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted.get(sorted.len() / 2).copied();
    let spread_ok =
        median.is_some_and(|m| steps.iter().all(|s| ((s - m) / m).abs() * 100.0 <= tol));
    let effective = median.map(|m| 1.0 / m);
    let timing = Timing {
        state: match (median, spread_ok) {
            (None, _) => "single_frame",
            (_, true) => "constant",
            _ => "variable",
        }
        .into(),
        median_step_s: median,
        min_step_s: sorted.first().copied(),
        max_step_s: sorted.last().copied(),
        effective_fps: effective,
        nominal_fps: map.nominal_fps,
        nominal_agrees: map
            .nominal_fps
            .zip(effective)
            .map(|(n, e)| ((n - e) / n).abs() * 100.0 <= tol),
    };
    let mut files = Files {
        checked: root.is_some(),
        missing_count: 0,
        missing: Vec::new(),
        hash_mismatch_count: 0,
        hash_mismatch: Vec::new(),
    };
    if let Some(root) = root {
        for f in &map.frames {
            let path = root.join(&f.file);
            let meta = std::fs::symlink_metadata(&path);
            if !meta.as_ref().is_ok_and(|m| m.file_type().is_file()) {
                files.missing_count += 1;
                if files.missing.len() < LIST_LIMIT {
                    files.missing.push(f.file.clone());
                }
            } else if let Some(want) = &f.sha256 {
                let got = crate::run::sha256_file(&path)?;
                if !got.eq_ignore_ascii_case(want.trim_start_matches("sha256:")) {
                    files.hash_mismatch_count += 1;
                    if files.hash_mismatch.len() < LIST_LIMIT {
                        files.hash_mismatch.push(f.file.clone());
                    }
                }
            }
        }
    }
    let files_bad = files.missing_count + files.hash_mismatch_count > 0;
    let mut reasons = Vec::new();
    if files_bad {
        reasons.push(format!(
            "{} file(s) missing and {} with a different hash",
            files.missing_count, files.hash_mismatch_count
        ));
    }
    if !contiguous {
        reasons.push(format!(
            "{missing_total} frame index(es) are absent; frames are not compared across gaps"
        ));
    }
    if timing.state == "variable" {
        reasons.push(
            "frame steps differ beyond tolerance; seconds cannot be derived from one frame rate"
                .into(),
        );
    }
    if timing.nominal_agrees == Some(false) {
        reasons.push("declared nominal_fps does not match the timestamps".into());
    }
    let state = if files_bad {
        "invalid_files"
    } else if !contiguous {
        "missing_frames"
    } else if timing.state == "variable" {
        "variable_frame_rate"
    } else if timing.state == "single_frame" {
        "single_frame"
    } else {
        "constant_frame_rate"
    };
    let usable_for = Usable {
        index_aligned: contiguous && !files_bad,
        uniform_time: state == "constant_frame_rate" && timing.nominal_agrees != Some(false),
    };
    let settle = settling
        .map(|r| interpret_settling(map, r, &timing, tol))
        .transpose()?;
    if let Some(s) = &settle {
        match s.state.as_str() {
            "never_settled" => reasons.push(
                "the sequence never settled after the change; no settling time exists".into(),
            ),
            "change_outside_sequence" => {
                reasons.push("the declared change frame is outside the sequence".into())
            }
            _ => {}
        }
        if !s.fixed_rate_assumption_holds && s.report_elapsed_s.is_some() {
            reasons.push(
                "the settling report's seconds assume a fixed rate that these timestamps do not support; use elapsed_s"
                    .into(),
            );
        }
    }
    Ok(Check {
        schema: CHECK_SCHEMA.into(),
        state: state.into(),
        frames: map.frames.len() as u64,
        first_index: map.frames[0].index,
        last_index: map.frames[map.frames.len() - 1].index,
        contiguous,
        missing_total,
        gaps,
        timing,
        files,
        usable_for,
        settle,
        reasons,
    })
}

fn interpret_settling(map: &FrameMap, report: &Value, timing: &Timing, tol: f64) -> Result<Settle> {
    let count = report["global_error"]
        .as_array()
        .map(Vec::len)
        .ok_or_else(|| invalid("settling report needs global_error"))?;
    if count != map.frames.len() {
        return Err(invalid(format!(
            "settling report covers {count} frames but the map lists {}",
            map.frames.len()
        )));
    }
    let change = report["policy"]["change_frame"]
        .as_u64()
        .ok_or_else(|| invalid("settling report needs policy.change_frame"))?;
    let report_fps = report["policy"]["fps"].as_f64();
    let holds = timing.state == "constant"
        && report_fps
            .zip(timing.effective_fps)
            .is_some_and(|(r, e)| ((r - e) / r).abs() * 100.0 <= tol);
    let at = |p: u64| map.frames.get(p as usize).map(|f| f.timestamp_s);
    let settle_position = report["settle_frame"].as_u64();
    let (state, elapsed, change_ts, settle_ts) = if change as usize >= count {
        ("change_outside_sequence", None, None, None)
    } else if let Some(s) = settle_position {
        let (c, t) = (at(change), at(s));
        ("settled", c.zip(t).map(|(c, t)| t - c), c, t)
    } else {
        ("never_settled", None, at(change), None)
    };
    Ok(Settle {
        state: state.into(),
        change_position: change,
        settle_position,
        change_timestamp_s: change_ts,
        settle_timestamp_s: settle_ts,
        elapsed_s: elapsed,
        report_elapsed_s: report["time_to_settle_seconds"].as_f64(),
        fixed_rate_assumption_holds: holds,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(frames: &[(u64, f64)]) -> FrameMap {
        FrameMap {
            schema: SCHEMA.into(),
            nominal_fps: None,
            frames: frames
                .iter()
                .map(|(i, t)| Frame {
                    index: *i,
                    timestamp_s: *t,
                    file: format!("f{i}.png"),
                    sha256: None,
                })
                .collect(),
        }
    }

    fn state(m: &FrameMap) -> Check {
        check(m, None, CheckPolicy::default(), None).unwrap()
    }

    #[test]
    fn constant_variable_and_gapped_sequences_get_distinct_states() {
        let constant = state(&map(&[(0, 0.0), (1, 0.5), (2, 1.0), (3, 1.5)]));
        assert_eq!(constant.state, "constant_frame_rate");
        assert!(constant.usable_for.uniform_time && constant.usable_for.index_aligned);
        assert_eq!(constant.timing.effective_fps, Some(2.0));
        let variable = state(&map(&[(0, 0.0), (1, 0.5), (2, 1.5), (3, 2.0)]));
        assert_eq!(variable.state, "variable_frame_rate");
        assert!(variable.usable_for.index_aligned && !variable.usable_for.uniform_time);
        // A dropped frame keeps a steady rate but is reported as a gap.
        let gapped = state(&map(&[(0, 0.0), (1, 0.5), (3, 1.5), (4, 2.0)]));
        assert_eq!(gapped.state, "missing_frames");
        assert_eq!(
            gapped.gaps,
            [Gap {
                after_index: 1,
                missing: 1
            }]
        );
        assert_eq!(gapped.timing.state, "constant");
        assert!(!gapped.usable_for.index_aligned);
        assert_eq!(state(&map(&[(5, 1.0)])).state, "single_frame");
    }

    #[test]
    fn structural_defects_and_unsafe_paths_are_errors() {
        let policy = CheckPolicy::default();
        for bad in [
            map(&[]),
            map(&[(0, 0.0), (0, 1.0)]),
            map(&[(0, 1.0), (1, 1.0)]),
            map(&[(0, f64::NAN)]),
        ] {
            assert!(check(&bad, None, policy, None).is_err());
        }
        let mut escaped = map(&[(0, 0.0)]);
        escaped.frames[0].file = "../x.png".into();
        assert!(check(&escaped, None, policy, None).is_err());
    }

    #[test]
    fn files_are_verified_by_presence_and_hash() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f0.png"), b"a").unwrap();
        let mut m = map(&[(0, 0.0), (1, 0.5)]);
        m.frames[0].sha256 = Some("00".repeat(32));
        let c = check(&m, Some(dir.path()), CheckPolicy::default(), None).unwrap();
        assert_eq!(
            (c.files.missing.clone(), c.files.hash_mismatch.clone()),
            (vec!["f1.png".to_string()], vec!["f0.png".to_string()])
        );
        assert_eq!(c.state, "invalid_files");
    }

    #[test]
    fn settling_uses_map_timestamps_and_names_never_settled() {
        let m = map(&[(0, 0.0), (1, 0.1), (2, 0.5), (3, 0.6)]);
        let report = |settle: Value| json!({"policy":{"change_frame":1,"fps":10.0},"global_error":[0,0,0,0],"settle_frame":settle,"time_to_settle_seconds":0.1});
        let s = check(&m, None, CheckPolicy::default(), Some(&report(json!(3))))
            .unwrap()
            .settle
            .unwrap();
        assert_eq!(s.state, "settled");
        assert!((s.elapsed_s.unwrap() - 0.5).abs() < 1e-9);
        assert!(!s.fixed_rate_assumption_holds);
        let never = check(&m, None, CheckPolicy::default(), Some(&report(Value::Null))).unwrap();
        let s = never.settle.unwrap();
        assert_eq!((s.state.as_str(), s.elapsed_s), ("never_settled", None));
        assert!(never.reasons.iter().any(|r| r.contains("never settled")));
        let short = json!({"policy":{"change_frame":1,"fps":10.0},"global_error":[0,0]});
        assert!(check(&m, None, CheckPolicy::default(), Some(&short)).is_err());
    }
}
