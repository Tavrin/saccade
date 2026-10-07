//! Timed subtitle/caption checks against timestamped, image-bound observations.
use crate::{Error, Result, text_quality};
use serde::{Deserialize, Serialize};

/// Additive report contract; older frame-map and legibility contracts are unchanged.
pub const SCHEMA: &str = "saccade-timed-text.v1";
/// Maximum cues or sampled frames in one bounded check.
pub const LIMIT: usize = 4096;

/// Optional explicit region-absence declaration alongside an unchanged OCR source contract.
pub const SOURCE_SCHEMA: &str = "saccade-timed-text-source.v1";
/// Image-bound imported OCR plus optional external empty-region annotation.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Timed-text source discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-timed-text-source.v1")))]
    pub schema: String,
    /// Existing image-bound OCR observations; complete must remain false.
    pub source: crate::ui_review::Source,
    /// Explicit producer annotation of empty text in this rectangle, not inferred from empty OCR.
    pub declared_empty_region_px: Option<[u32; 4]>,
}

/// One plain-text timed cue. Rich cue markup is deliberately rejected.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cue {
    /// File ordinal; identifiers do not need to be unique.
    pub ordinal: usize,
    /// Optional file identifier.
    pub id: Option<String>,
    /// Inclusive presentation start, seconds.
    pub start_s: f64,
    /// Exclusive presentation end, seconds.
    pub end_s: f64,
    /// Expected Unicode text, inert data.
    pub text: String,
}
fn invalid(message: &str) -> Error {
    Error::Config(message.into())
}
fn timestamp(value: &str, vtt: bool) -> Result<f64> {
    let separator = if vtt { '.' } else { ',' };
    let parts: Vec<_> = value.split(':').collect();
    if !(parts.len() == 3 || vtt && parts.len() == 2) {
        return Err(invalid("invalid subtitle timestamp"));
    }
    let (seconds, millis) = parts[parts.len() - 1]
        .split_once(separator)
        .ok_or_else(|| invalid("timestamp requires milliseconds"))?;
    let number = |s: &str| -> Result<u64> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("invalid timestamp digits"));
        }
        s.parse().map_err(|_| invalid("timestamp overflow"))
    };
    let minutes = parts[parts.len() - 2];
    if seconds.len() != 2 || minutes.len() != 2 || millis.len() != 3 {
        return Err(invalid(
            "timestamp requires two digit minutes/seconds and three digit milliseconds",
        ));
    }
    let s = number(seconds)?;
    let m = number(minutes)?;
    let h = if parts.len() == 3 {
        number(parts[0])?
    } else {
        0
    };
    if s >= 60 || m >= 60 || h > 9999 {
        return Err(invalid("timestamp outside supported range"));
    }
    Ok((h * 3600 + m * 60 + s) as f64 + number(millis)? as f64 / 1000.)
}
fn rich_text(content: &str) -> bool {
    let markup = content.split('<').skip(1).any(|tail| {
        tail.split_once('>').is_some_and(|(tag, _)| {
            let tag = tag.trim_start_matches('/');
            tag.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '!' || c == '?')
                || tag.contains(':') && tag.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
    });
    let entity = content.split('&').skip(1).any(|tail| {
        tail.split_once(';').is_some_and(|(name, _)| {
            !name.is_empty()
                && name.len() <= 32
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '#')
        })
    });
    markup || entity
}

/// Parse bounded UTF-8 SRT or WebVTT, accepting CRLF/BOM, identifiers and multiline cues.
/// Unsupported markup, styling and timestamp maps fail explicitly rather than changing text/time.
pub fn parse(bytes: &[u8]) -> Result<Vec<Cue>> {
    if bytes.len() > 4 << 20 {
        return Err(invalid("timed text exceeds 4 MiB"));
    }
    let raw = std::str::from_utf8(bytes).map_err(|_| invalid("timed text must be UTF-8"))?;
    let text = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    if text.contains('\r') {
        return Err(invalid("unsupported bare carriage return"));
    }
    let text = text
        .lines()
        .map(|line| if line.trim().is_empty() { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n");
    let mut blocks = text.split("\n\n").filter(|b| !b.trim().is_empty());
    let first = blocks.next().ok_or_else(|| invalid("empty timed text"))?;
    let vtt = first
        .lines()
        .next()
        .is_some_and(|l| l == "WEBVTT" || l.starts_with("WEBVTT ") || l.starts_with("WEBVTT\t"));
    let mut entries = Vec::new();
    if vtt {
        if first.lines().count() != 1 {
            return Err(invalid("unsupported WebVTT header metadata"));
        }
    } else {
        entries.push(first);
    }
    entries.extend(blocks);
    let mut cues = Vec::new();
    for block in entries {
        let lines: Vec<_> = block.trim_matches('\n').lines().collect();
        if vtt && lines[0].split_whitespace().next() == Some("NOTE") {
            continue;
        }
        let timing = usize::from(!lines[0].contains("-->"));
        let line = lines
            .get(timing)
            .ok_or_else(|| invalid("cue lacks timing"))?;
        let (start, end) = line
            .split_once("-->")
            .ok_or_else(|| invalid("cue requires --> timing"))?;
        let end_parts: Vec<_> = end.split_whitespace().collect();
        if end_parts.is_empty() {
            return Err(invalid("cue lacks end timestamp"));
        }
        // Position settings do not alter declared region or text. Validate supported keys.
        if !vtt && end_parts.len() != 1
            || end_parts.iter().skip(1).any(|s| {
                !["line:", "position:", "size:", "align:", "vertical:"]
                    .iter()
                    .any(|p| s.starts_with(p))
            })
        {
            return Err(invalid("unsupported cue timing settings"));
        }
        let start_s = timestamp(start.trim(), vtt)?;
        let end_s = timestamp(end_parts[0], vtt)?;
        let content = lines.get(timing + 1..).unwrap_or_default().join("\n");
        if end_s <= start_s
            || content.trim().is_empty()
            || content.len() > 65536
            || content.contains('\0')
            || rich_text(&content)
        {
            return Err(invalid(
                "cue requires positive duration and plain text without markup/entities",
            ));
        }
        if cues.len() >= LIMIT {
            return Err(invalid("too many cues (maximum 4096)"));
        }
        if cues.last().is_some_and(|c: &Cue| c.start_s > start_s) {
            return Err(invalid("cue start times must be nondecreasing"));
        }
        cues.push(Cue {
            ordinal: cues.len(),
            id: if timing == 1 {
                Some(lines[0].into())
            } else {
                None
            },
            start_s,
            end_s,
            text: content,
        });
    }
    if cues.is_empty() {
        return Err(invalid("no timed cues"));
    }
    Ok(cues)
}
/// Frozen sampling and matching policy.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Maximum onset discrepancy tolerated in seconds.
    pub timing_tolerance_s: f64,
    /// Search before/after each cue for exact text, seconds.
    pub search_s: f64,
    /// Maximum sample step and unsampled window edge allowed, seconds.
    pub maximum_gap_s: f64,
    /// Reused text-legibility pixel thresholds.
    pub legibility: text_quality::Policy,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            timing_tolerance_s: 0.3,
            search_s: 2.,
            maximum_gap_s: 0.5,
            legibility: text_quality::Policy::default(),
        }
    }
}
/// A timestamped OCR observation from one verified raster and declared region.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Source frame index.
    pub index: u64,
    /// Presentation timestamp.
    pub timestamp_s: f64,
    /// Exact encoded image identity.
    pub image_sha256: String,
    /// Exact imported source identity, if supplied.
    pub source_sha256: Option<String>,
    /// Confident region text; empty means a complete OCR producer observed no text.
    pub text: Option<String>,
    /// Explicit OCR adapter/availability reason.
    pub ocr_reason: String,
    /// Reused pixel measurements; absent when text-quality is not built.
    pub legibility: Option<text_quality::RegionResult>,
}
/// Per-cue result in the supplied sample set, never a continuous-video assertion.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueResult {
    /// Original expected cue.
    pub cue: Cue,
    /// aligned, failed, or insufficient_evidence.
    pub state: String,
    /// All findings: missing, text_mismatch, early, late, persists_after_end, interrupted, illegible.
    pub findings: Vec<String>,
    /// Sample indices whose exact region text was assigned to this cue.
    pub matching_frames: Vec<u64>,
    /// First exact matching sample timestamp.
    pub first_seen_s: Option<f64>,
    /// Last exact matching sample timestamp (not disappearance time).
    pub last_seen_s: Option<f64>,
    /// First matching sample minus expected onset; sampled estimate only.
    pub onset_offset_s: Option<f64>,
    /// Whether the expected window is covered by known OCR samples within the gap policy.
    pub window_covered: bool,
    /// Per-cue pixel result: legible, illegible, or insufficient_evidence.
    pub legibility_state: String,
    /// Explicit abstention explanations.
    pub reasons: Vec<String>,
}
/// Contiguous sampled run of unexpected nonempty region text.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extra {
    /// Index of the first report observation in this run.
    pub observation: usize,
    /// Source indices of every unexpected text sample in the run.
    pub frame_indices: Vec<u64>,
    /// First sampled sighting, not inferred continuous onset.
    pub first_seen_s: f64,
    /// Last sampled sighting, not disappearance time.
    pub last_seen_s: f64,
    /// Exact inert observed text.
    pub text: String,
}
/// Versioned timed-text evidence report.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Version discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-timed-text.v1")))]
    pub schema: String,
    /// Content-addressed identity, added at transport emission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// Optional caller backlinks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// aligned, failed, or insufficient_evidence; measured failures dominate abstentions.
    pub state: String,
    /// Exact timed-text input bytes.
    pub timed_text_sha256: String,
    /// Exact frame-map input bytes.
    pub frame_map_sha256: String,
    /// Frozen search, coverage and legibility thresholds.
    pub policy: Policy,
    /// Declared region in every frame's capture pixels; layout/size changes need separate runs.
    pub region_px: [u32; 4],
    /// Every sampled observation including unavailable OCR and pixel evidence.
    pub observations: Vec<Observation>,
    /// Every supplied cue, in file order.
    pub cues: Vec<CueResult>,
    /// Unexpected observed text grouped into contiguous runs under the gap policy.
    pub extra: Vec<Extra>,
    /// Interpretation limits.
    pub limitations: Vec<String>,
}
fn roundoff(a: f64, b: f64) -> f64 {
    8.0 * f64::EPSILON * a.abs().max(b.abs()).max(1.0)
}
fn gap_exceeds(from: f64, to: f64, limit: f64) -> bool {
    let delta = to - from;
    // Explicit zero tolerance remains a strict comparison of supplied floats.
    if limit == 0.0 {
        return delta > 0.0;
    }
    delta - limit > roundoff(from, to)
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
/// Evaluate exact whitespace-normalized Unicode against bounded observations.
/// Repeated text is assigned to the nearest expected interval; ties abstain.
pub fn check(
    cues: &[Cue],
    observations: &[Observation],
    policy: Policy,
) -> Result<(String, Vec<CueResult>, Vec<Extra>)> {
    if cues.is_empty()
        || cues.len() > LIMIT
        || observations.is_empty()
        || observations.len() > LIMIT
        || cues.len().saturating_mul(observations.len()) > 4_000_000
        || !policy.timing_tolerance_s.is_finite()
        || !(0.0..=60.).contains(&policy.timing_tolerance_s)
        || !policy.search_s.is_finite()
        || !(0.0..=60.).contains(&policy.search_s)
        || !policy.maximum_gap_s.is_finite()
        || !(0.001..=60.).contains(&policy.maximum_gap_s)
    {
        return Err(invalid(
            "invalid timed-text bounds or timing policy (maximum 4 million cue/sample pairs)",
        ));
    }
    let p = policy.legibility;
    if !p.minimum_contrast.is_finite()
        || !(1.0..=21.0).contains(&p.minimum_contrast)
        || !p.minimum_x_height_px.is_finite()
        || p.minimum_x_height_px <= 0.0
        || !p.minimum_sharpness.is_finite()
        || !(0.0..=1.0).contains(&p.minimum_sharpness)
        || !p.minimum_stroke_px.is_finite()
        || p.minimum_stroke_px <= 0.0
    {
        return Err(invalid("invalid text legibility thresholds"));
    }
    for (i, c) in cues.iter().enumerate() {
        if !c.start_s.is_finite()
            || !c.end_s.is_finite()
            || c.start_s < 0.
            || c.end_s <= c.start_s
            || c.text.trim().is_empty()
            || c.text.len() > 65536
            || i > 0 && c.start_s < cues[i - 1].start_s
        {
            return Err(invalid("invalid cue"));
        }
    }
    for (i, o) in observations.iter().enumerate() {
        if !o.timestamp_s.is_finite()
            || o.timestamp_s < 0.
            || o.text.as_ref().is_some_and(|t| t.len() > 65536)
            || i > 0
                && (o.timestamp_s <= observations[i - 1].timestamp_s
                    || o.index <= observations[i - 1].index)
        {
            return Err(invalid("invalid observation ordering/text"));
        }
    }
    let expected: Vec<_> = cues.iter().map(|c| normalized(&c.text)).collect();
    let texts: Vec<_> = observations
        .iter()
        .map(|o| o.text.as_ref().map(|t| normalized(t)))
        .collect();
    let mut assigned = vec![None; observations.len()];
    let mut ambiguous = vec![false; observations.len()];
    let mut extra: Vec<Extra> = Vec::new();
    for (j, (o, text)) in observations.iter().zip(&texts).enumerate() {
        let Some(text) = text.as_ref().filter(|t| !t.is_empty()) else {
            continue;
        };
        let mut best = f64::INFINITY;
        let mut owner = None;
        let mut best_active = false;
        let mut tie = false;
        for (i, c) in cues.iter().enumerate() {
            if text != &expected[i] {
                continue;
            }
            let active = o.timestamp_s >= c.start_s && o.timestamp_s < c.end_s;
            if o.timestamp_s < c.start_s && gap_exceeds(o.timestamp_s, c.start_s, policy.search_s)
                || o.timestamp_s >= c.end_s
                    && (policy.search_s == 0.0
                        || o.timestamp_s - c.end_s
                            >= policy.search_s - roundoff(o.timestamp_s, c.end_s))
            {
                continue;
            }
            let distance = if o.timestamp_s < c.start_s {
                c.start_s - o.timestamp_s
            } else if o.timestamp_s >= c.end_s {
                o.timestamp_s - c.end_s
            } else {
                0.
            };
            let uncertainty = roundoff(o.timestamp_s, c.end_s);
            if owner.is_none()
                || active && !best_active
                || active == best_active && distance < best - uncertainty
            {
                best = distance;
                owner = Some(i);
                best_active = active;
                tie = false;
            } else if active == best_active && (distance - best).abs() <= uncertainty {
                tie = true;
            }
        }
        if tie {
            ambiguous[j] = true;
        } else {
            assigned[j] = owner;
        }
        if owner.is_none() {
            if let Some(previous) = extra.last_mut().filter(|e| {
                e.frame_indices.last() == j.checked_sub(1).map(|p| &observations[p].index)
                    && normalized(&e.text) == *text
                    && !gap_exceeds(e.last_seen_s, o.timestamp_s, policy.maximum_gap_s)
            }) {
                previous.frame_indices.push(o.index);
                previous.last_seen_s = o.timestamp_s;
            } else {
                extra.push(Extra {
                    observation: j,
                    frame_indices: vec![o.index],
                    first_seen_s: o.timestamp_s,
                    last_seen_s: o.timestamp_s,
                    text: o.text.clone().unwrap_or_default(),
                });
            }
        }
    }
    let mut results = Vec::new();
    for (i, c) in cues.iter().enumerate() {
        let window: Vec<_> = observations
            .iter()
            .enumerate()
            .filter(|(_, o)| o.timestamp_s >= c.start_s && o.timestamp_s < c.end_s)
            .map(|(j, _)| j)
            .collect();
        let matches: Vec<_> = assigned
            .iter()
            .enumerate()
            .filter(|(_, a)| **a == Some(i))
            .map(|(j, _)| j)
            .collect();
        let precision = roundoff(c.start_s, c.end_s);
        let coarse_clock = precision >= policy.maximum_gap_s
            || policy.timing_tolerance_s > 0.0 && precision >= policy.timing_tolerance_s;
        let covered = !coarse_clock
            && !window.is_empty()
            && window.iter().all(|j| texts[*j].is_some() && !ambiguous[*j])
            && !gap_exceeds(
                c.start_s,
                observations[window[0]].timestamp_s,
                policy.maximum_gap_s,
            )
            && !gap_exceeds(
                observations[*window.last().unwrap_or(&window[0])].timestamp_s,
                c.end_s,
                policy.maximum_gap_s,
            )
            && window.windows(2).all(|w| {
                !gap_exceeds(
                    observations[w[0]].timestamp_s,
                    observations[w[1]].timestamp_s,
                    policy.maximum_gap_s,
                )
            });
        let first = matches.first().map(|j| observations[*j].timestamp_s);
        let last = matches.last().map(|j| observations[*j].timestamp_s);
        let offset = first.map(|t| t - c.start_s);
        let mut findings = Vec::new();
        let mut reasons = Vec::new();
        if matches.is_empty() && covered {
            findings.push(
                if window
                    .iter()
                    .any(|j| texts[*j].as_ref().is_some_and(|t| !t.is_empty()))
                {
                    "text_mismatch"
                } else {
                    "missing"
                }
                .into(),
            );
        }
        if first.is_some_and(|t| gap_exceeds(t, c.start_s, policy.timing_tolerance_s)) {
            findings.push("early".into());
        }
        let onset_covered = first.is_some_and(|first| {
            let span: Vec<_> = observations
                .iter()
                .enumerate()
                .filter(|(_, o)| o.timestamp_s >= c.start_s && o.timestamp_s <= first)
                .map(|(j, _)| j)
                .collect();
            !span.is_empty()
                && !gap_exceeds(
                    c.start_s,
                    observations[span[0]].timestamp_s,
                    policy.maximum_gap_s,
                )
                && span.iter().all(|j| texts[*j].is_some() && !ambiguous[*j])
                && span.windows(2).all(|w| {
                    !gap_exceeds(
                        observations[w[0]].timestamp_s,
                        observations[w[1]].timestamp_s,
                        policy.maximum_gap_s,
                    )
                })
        });
        if onset_covered
            && first.is_some_and(|t| gap_exceeds(c.start_s, t, policy.timing_tolerance_s))
        {
            findings.push("late".into());
        }
        if last.is_some_and(|t| gap_exceeds(c.end_s, t, policy.timing_tolerance_s)) {
            findings.push("persists_after_end".into());
        }
        if covered
            && last.is_some_and(|t| {
                gap_exceeds(t, c.end_s, policy.maximum_gap_s + policy.timing_tolerance_s)
            })
        {
            findings.push("ends_before_end".into());
        }
        if covered && !matches.is_empty() && window.iter().any(|j| assigned[*j] != Some(i)) {
            findings.push("interrupted".into());
        }
        let relevant = if matches.is_empty() {
            &window
        } else {
            &matches
        };
        let legibility = if relevant.iter().any(|j| {
            observations[*j]
                .legibility
                .as_ref()
                .is_some_and(|r| r.state == text_quality::State::Illegible)
        }) {
            "illegible"
        } else if !relevant.is_empty()
            && relevant.iter().all(|j| {
                observations[*j]
                    .legibility
                    .as_ref()
                    .is_some_and(|r| r.state == text_quality::State::Legible)
            })
        {
            "legible"
        } else {
            "insufficient_evidence"
        };
        // Empty/missing text is not pixel legibility evidence for the expected cue.
        let legibility = if matches.is_empty() {
            "insufficient_evidence"
        } else {
            legibility
        };
        if legibility == "illegible" {
            findings.push("illegible".into());
        }
        if first.is_some_and(|t| gap_exceeds(c.start_s, t, policy.timing_tolerance_s))
            && !onset_covered
        {
            reasons.push("sampled onset offset cannot establish lateness across unknown OCR or sampling gaps".into());
        }
        if coarse_clock {
            reasons.push("presentation clock precision is too coarse for timing/gap policy; use a sequence-relative origin".into());
        }
        if !covered {
            reasons.push("window lacks known unambiguous OCR samples within maximum_gap_s; unsampled times remain unverified".into());
        }
        if legibility == "insufficient_evidence" {
            reasons.push("expected text pixel legibility unavailable or insufficient".into());
        }
        if matches.windows(2).any(|w| {
            gap_exceeds(
                observations[w[0]].timestamp_s,
                observations[w[1]].timestamp_s,
                policy.maximum_gap_s,
            )
        }) {
            reasons.push("matching observations are separated by a sampling gap".into());
        }
        let state = if !findings.is_empty() {
            "failed"
        } else if !covered || !reasons.is_empty() {
            "insufficient_evidence"
        } else {
            "aligned"
        };
        results.push(CueResult {
            cue: c.clone(),
            state: state.into(),
            findings,
            matching_frames: matches.iter().map(|j| observations[*j].index).collect(),
            first_seen_s: first,
            last_seen_s: last,
            onset_offset_s: offset,
            window_covered: covered,
            legibility_state: legibility.into(),
            reasons,
        });
    }
    let state = if !extra.is_empty() || results.iter().any(|r| r.state == "failed") {
        "failed"
    } else if results.iter().all(|r| r.state == "aligned")
        && observations.iter().all(|o| o.text.is_some())
        && !ambiguous.contains(&true)
    {
        "aligned"
    } else {
        "insufficient_evidence"
    };
    Ok((state.into(), results, extra))
}
