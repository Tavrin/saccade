//! Verdicts over externally acquired timings. No commands are executed here.
use crate::{
    Error, Result,
    paired_stats::{Generator, hodges_lehmann, quantile},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
/// Imported experiment contract.
pub const SESSION_SCHEMA: &str = "saccade-timing-session.v1";
/// Timing analysis contract.
pub const REPORT_SCHEMA: &str = "saccade-timing-ab.v1";
/// One independently acquired pair. Frames within a run are averaged before inference.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// Unique pair identity.
    pub id: String,
    /// Contiguous resampling block identity; correlated pairs share a block.
    pub block: String,
    /// Actual acquisition order: ab or ba.
    pub order: String,
    /// A run (1..3 frame samples, milliseconds).
    pub a: Vec<f64>,
    /// B run (1..3 frame samples, milliseconds).
    pub b: Vec<f64>,
    /// Same-session unchanged-arm control pair.
    #[serde(default)]
    pub aa: bool,
}
/// Frozen analysis policy and externally acquired evidence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    /// Version discriminator.
    pub schema: String,
    /// External session identity, required to bind A/A to A/B.
    pub session: String,
    /// Practical equivalence band, percent.
    pub band_pct: f64,
    /// Declared maximum A/B pairs before acquisition.
    pub max_pairs: usize,
    /// Predeclared look counts; empty means a single fixed maximum.
    #[serde(default)]
    pub looks: Vec<usize>,
    /// Confidence chosen before acquisition.
    pub confidence: f64,
    /// Bootstrap seed chosen before acquisition.
    pub seed: u64,
    /// Whole-block bootstrap draws.
    pub resamples: usize,
    /// All acquired A/B and A/A pairs, in acquisition order.
    pub pairs: Vec<Pair>,
    /// Acquisition validation issues from external adapters.
    #[serde(default)]
    pub diagnostic_reasons: Vec<String>,
    /// Optional capture pairs for existing clock/readiness qualification.
    #[serde(default)]
    pub captures: Vec<[PathBuf; 2]>,
    /// Explicit CPU timing declaration.
    #[serde(default)]
    pub gpu_clocks_not_applicable: bool,
}
/// Full effect, policy, reasons and source identities.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Version discriminator.
    pub schema: String,
    /// faster, slower, equivalent or inconclusive.
    pub verdict: String,
    /// Exact practical equivalence band.
    pub band_pct: f64,
    /// Paired log-ratio Hodges–Lehmann percent shift.
    pub effect_pct: Option<f64>,
    /// Whole-block bootstrap interval.
    pub interval_pct: Option<[f64; 2]>,
    /// Conservative A/A bound (max absolute CI endpoint).
    pub noise_floor_pct: Option<f64>,
    /// Independent blocks used for A/B inference.
    pub blocks: usize,
    /// A/B pairs at the recorded look.
    pub pairs: usize,
    /// True when provenance/qualification is insufficient for acceptance.
    pub diagnostic_only: bool,
    /// Typed acquisition/qualification reasons.
    pub reasons: Vec<String>,
    /// Existing qualification payloads, including typed readiness reasons.
    pub qualification: Vec<Value>,
    /// Declared rule and multiplicity adjustment, retained for audit.
    pub stopping_rule: String,
    /// Actual per-look confidence after Bonferroni spending.
    pub look_confidence: f64,
    /// Immutable imported plan/evidence.
    pub session: Session,
    /// Exact external input SHA-256 values.
    pub sources: BTreeMap<String, String>,
}
fn invalid(s: &str) -> Error {
    Error::Config(format!("timing: {s}"))
}
fn mean(v: &[f64]) -> Result<f64> {
    if !(1..=3).contains(&v.len()) || v.iter().any(|x| !x.is_finite() || *x <= 0.) {
        return Err(invalid("each run needs 1..3 finite positive samples"));
    }
    Ok(v.iter().map(|x| x / v.len() as f64).sum::<f64>())
}
fn estimate(
    pairs: &[&Pair],
    confidence: f64,
    draws: usize,
    seed: u64,
) -> Result<(f64, [f64; 2], usize)> {
    let mut blocks: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for p in pairs {
        blocks
            .entry(&p.block)
            .or_default()
            .push(mean(&p.b)?.ln() - mean(&p.a)?.ln());
    }
    let all = blocks.values().flatten().copied().collect::<Vec<_>>();
    let groups = blocks.values().collect::<Vec<_>>();
    if groups.iter().map(|g| g.len()).max().unwrap_or(0) * groups.len() > 256 {
        return Err(invalid(
            "block imbalance exceeds bounded bootstrap sample budget (256 pairs per draw)",
        ));
    }
    let mut rng = Generator::new(seed);
    let mut boot = Vec::with_capacity(draws);
    for _ in 0..draws {
        let mut sample = Vec::new();
        for _ in 0..groups.len() {
            sample.extend(groups[rng.index(groups.len())?]);
        }
        boot.push(hodges_lehmann(&sample)?);
    }
    boot.sort_by(f64::total_cmp);
    let tail = (1. - confidence) / 2.;
    let pct = |x: f64| x.exp_m1() * 100.;
    let effect = pct(hodges_lehmann(&all)?);
    if !effect.is_finite() {
        return Err(invalid("timing ratio overflows percent representation"));
    }
    let interval = [
        pct(quantile(&boot, tail)?),
        pct(quantile(&boot, 1. - tail)?),
    ];
    if interval.iter().any(|v| !v.is_finite()) {
        return Err(invalid("bootstrap ratio overflows percent representation"));
    }
    Ok((effect, interval, groups.len()))
}
/// Analyze a session, refusing unsupported stopping/order declarations.
pub fn analyze(session: Session, sources: BTreeMap<String, String>, root: &Path) -> Result<Report> {
    if session.schema != SESSION_SCHEMA
        || session.session.trim().is_empty()
        || !session.band_pct.is_finite()
        || session.band_pct <= 0.
        || session.band_pct >= 100.
        || !(6..=128).contains(&session.max_pairs)
        || !(0.8..=0.99).contains(&session.confidence)
        || !(1024..=8192).contains(&session.resamples)
        || session.pairs.len() > 256
    {
        return Err(invalid(
            "invalid schema, session, band or bounded analysis plan",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut previous = None;
    let mut closed = BTreeSet::new();
    for p in &session.pairs {
        mean(&p.a)?;
        mean(&p.b)?;
        if p.id.is_empty()
            || !ids.insert(&p.id)
            || p.block.is_empty()
            || !matches!(p.order.as_str(), "ab" | "ba")
        {
            return Err(invalid(
                "pairs require unique IDs, block IDs and ab/ba orders",
            ));
        }
        if previous != Some(&p.block) {
            if let Some(prev) = previous {
                closed.insert(prev);
            }
            if closed.contains(&p.block) {
                return Err(invalid("blocks must be contiguous"));
            }
            previous = Some(&p.block);
        }
    }
    let ab = session.pairs.iter().filter(|p| !p.aa).collect::<Vec<_>>();
    let aa = session.pairs.iter().filter(|p| p.aa).collect::<Vec<_>>();
    let looks = if session.looks.is_empty() {
        vec![session.max_pairs]
    } else {
        session.looks.clone()
    };
    if looks.len() > 8
        || looks.windows(2).any(|w| w[0] >= w[1])
        || looks.iter().any(|n| *n < 6 || *n > session.max_pairs)
        || looks.last() != Some(&session.max_pairs)
        || ab.len() > session.max_pairs
    {
        return Err(invalid(
            "looks must increase to the declared maximum, with at most eight looks",
        ));
    }
    let confidence = 1. - (1. - session.confidence) / looks.len() as f64;
    let mut reasons = session.diagnostic_reasons.clone();
    if (1. - confidence) * 0.5 * (session.resamples as f64) < 10. {
        reasons.push("bootstrap_tail_underresolved".into());
    }
    let (mut effect, mut interval, mut blocks) = (None, None, 0);
    let mut floor = None;
    if ab.len() < 6 {
        reasons.push("too_few_pairs".into());
    }
    if aa.len() < 6 {
        reasons.push("too_few_same_session_aa_pairs".into());
    }
    if !ab.iter().any(|p| p.order == "ab") || !ab.iter().any(|p| p.order == "ba") {
        reasons.push("order_not_interleaved".into());
    }
    if ab.len() >= 6 {
        let (e, ci, n) = estimate(&ab, confidence, session.resamples, session.seed)?;
        effect = Some(e);
        interval = Some(ci);
        blocks = n;
        if n < 6 {
            reasons.push("too_few_independent_blocks".into());
        }
    }
    if aa.len() >= 6 {
        let (_, ci, n) = estimate(&aa, confidence, session.resamples, session.seed ^ 1)?;
        floor = Some(ci[0].abs().max(ci[1].abs()));
        if n < 6 {
            reasons.push("too_few_aa_blocks".into());
        }
        if floor.is_some_and(|f| f > session.band_pct) {
            reasons.push("aa_noise_floor_too_wide".into());
        }
    }
    if !looks.contains(&ab.len()) {
        reasons.push("not_a_declared_look".into());
    }
    // A look must not split a correlated block.
    if session
        .pairs
        .iter()
        .any(|p| p.aa && ab.iter().any(|q| q.block == p.block))
    {
        reasons.push("aa_ab_share_resampling_block".into());
    }
    #[allow(unused_mut)]
    let mut qualification = Vec::new();
    if !session.captures.is_empty() && session.captures.len() != session.pairs.len() {
        reasons.push("capture_pair_binding_incomplete".into());
    }
    #[cfg(feature = "graphics")]
    for (i, pair) in session.captures.iter().enumerate() {
        let opts = crate::perf::PerfOptions {
            gpu_clocks_not_applicable: session.gpu_clocks_not_applicable,
            ..Default::default()
        };
        for (side, dir) in pair.iter().enumerate() {
            if let Some(perf) =
                crate::perf::CapturePerf::read(&root.join(dir), "saccade-perf.json")?
            {
                let observation = session
                    .pairs
                    .get(i)
                    .ok_or_else(|| invalid("capture pair exceeds observations"))?;
                let observed = mean(if side == 0 {
                    &observation.a
                } else {
                    &observation.b
                })?;
                if (observed - perf.frame.value).abs() > 1e-6 * observed.max(perf.frame.value) {
                    reasons.push("capture_timing_binding_mismatch".into());
                }
                let path = root.join(dir).join("saccade-perf.json");
                // Capture identities remain visible in the qualification payload's source hashes.
                qualification.push(serde_json::json!({"pair_id":observation.id,"side":side,"perf_sha256":crate::localized::digest(&crate::evidence_quality::read(&path,4<<20)?)}));
            }
        }
        let (diff, errors) = crate::perf::pair(&root.join(&pair[0]), &root.join(&pair[1]), &opts)?;
        if !errors.is_empty() {
            reasons.push("capture_performance_errors".into());
        }
        if diff
            .as_ref()
            .is_none_or(|d| d.comparability != crate::perf::Comparability::Qualified)
        {
            reasons.push("capture_qualification_rejected".into());
        }
        qualification.push(serde_json::json!({"diff":diff,"errors":errors}));
    }
    #[cfg(not(feature = "graphics"))]
    {
        let _ = root;
        if !session.captures.is_empty() {
            reasons.push("graphics_qualification_unavailable".into());
        }
    }
    if session.captures.is_empty() {
        reasons.push("clock_readiness_provenance_unavailable".into());
    }
    let diagnostic_only = reasons.iter().any(|r| {
        r == "clock_readiness_provenance_unavailable"
            || r.starts_with("capture_")
            || r == "graphics_qualification_unavailable"
            || r == "external_order_unverified"
    });
    let blocking = reasons
        .iter()
        .any(|r| r != "clock_readiness_provenance_unavailable");
    let mut verdict = "inconclusive";
    if !blocking && let Some(ci) = interval {
        verdict = if ci[1] < -session.band_pct {
            "faster"
        } else if ci[0] > session.band_pct {
            "slower"
        } else if ci[0] >= -session.band_pct && ci[1] <= session.band_pct {
            "equivalent"
        } else {
            reasons.push("ci_spans_equivalence_band".into());
            "inconclusive"
        };
    }
    let count = ab.len();
    Ok(Report {
        schema: REPORT_SCHEMA.into(),
        verdict: verdict.into(),
        band_pct: session.band_pct,
        effect_pct: effect,
        interval_pct: interval,
        noise_floor_pct: floor,
        blocks,
        pairs: count,
        diagnostic_only,
        reasons,
        qualification,
        stopping_rule: format!(
            "predeclared_looks_bonferroni_block_percentile; max={}; looks={looks:?}; no opportunistic looks; bootstrap coverage diagnostic",
            session.max_pairs
        ),
        look_confidence: confidence,
        session,
        sources,
    })
}
/// Extract a numeric array, scalar, performance sidecar or arbitrary dotted JSON path.
pub fn extract(value: &Value, format: &str, path: Option<&str>, unit: &str) -> Result<Vec<f64>> {
    let v = match format {
        "perf" => {
            if unit != "ms" {
                return Err(invalid(
                    "performance sidecars declare milliseconds; do not rescale their unit",
                ));
            }
            let perf =
                crate::perf::CapturePerf::parse(&value.to_string(), "imported timing sidecar")?;
            return Ok(vec![perf.frame.value]);
        }
        "hyperfine" => &value["results"][0]["times"],
        "json" => {
            let p = path.ok_or_else(|| invalid("json extractor requires a path"))?;
            return numbers(
                &crate::arms::lookup(value, p).ok_or_else(|| invalid("JSON path absent"))?,
                unit,
            );
        }
        _ => return Err(invalid("unknown timing adapter")),
    };
    numbers(v, if format == "hyperfine" { "s" } else { unit })
}
fn numbers(v: &Value, unit: &str) -> Result<Vec<f64>> {
    let scale = match unit {
        "s" => 1000.,
        "ms" => 1.,
        "us" => 0.001,
        "ns" => 0.000001,
        _ => return Err(invalid("unit must be s, ms, us or ns")),
    };
    let values = if let Some(a) = v.as_array() {
        a.iter().collect::<Vec<_>>()
    } else {
        vec![v]
    };
    if values.is_empty() || values.len() > 128 {
        return Err(invalid("timing array must contain 1..128 values"));
    }
    values
        .into_iter()
        .map(|v| {
            v.as_f64()
                .map(|x| x * scale)
                .filter(|x| x.is_finite() && *x > 0.)
                .ok_or_else(|| invalid("nonpositive/nonfinite/nonnumeric timing"))
        })
        .collect()
}
/// Resolve bounded external run references in a session JSON before deserialization.
/// A run can be an array or {file, format, path?, unit?, index?}.
pub fn load(path: &Path, format: &str) -> Result<(Session, BTreeMap<String, String>)> {
    if !matches!(format, "session" | "csv") {
        return Err(invalid("input format must be session or csv"));
    }
    let bytes = crate::evidence_quality::read(path, 4 << 20)?;
    let mut sources =
        BTreeMap::from([(path.display().to_string(), crate::localized::digest(&bytes))]);
    if format == "csv" {
        let text = std::str::from_utf8(&bytes).map_err(|_| invalid("CSV must be UTF-8"))?;
        let mut lines = text.lines();
        if lines.next() != Some("id,block,order,a_ms,b_ms,aa") {
            return Err(invalid("CSV header must be id,block,order,a_ms,b_ms,aa"));
        }
        let mut pairs = Vec::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let c = line.split(',').collect::<Vec<_>>();
            if c.len() != 6 {
                return Err(invalid("CSV needs six unquoted columns"));
            }
            pairs.push(Pair {
                id: c[0].into(),
                block: c[1].into(),
                order: c[2].into(),
                a: vec![c[3].parse().map_err(|_| invalid("bad CSV timing"))?],
                b: vec![c[4].parse().map_err(|_| invalid("bad CSV timing"))?],
                aa: c[5].parse().map_err(|_| invalid("aa must be true/false"))?,
            });
        }
        let max_pairs = pairs.iter().filter(|p| !p.aa).count().max(6);
        return Ok((
            Session {
                schema: SESSION_SCHEMA.into(),
                session: path.display().to_string(),
                band_pct: 2.,
                max_pairs,
                looks: vec![],
                confidence: 0.95,
                seed: 1,
                resamples: 2048,
                pairs,
                diagnostic_reasons: vec![],
                captures: vec![],
                gpu_clocks_not_applicable: false,
            },
            sources,
        ));
    }
    let mut value: Value = serde_json::from_slice(&bytes)?;
    let root = path.parent().unwrap_or(Path::new("."));
    if let Some(pairs) = value["pairs"].as_array_mut() {
        if pairs.len() > 256 {
            return Err(invalid("too many pairs"));
        }
        for pair in pairs {
            for arm in ["a", "b"] {
                if pair[arm].is_object() {
                    let spec = &pair[arm];
                    if spec.as_object().is_some_and(|o| {
                        o.keys().any(|k| {
                            !matches!(k.as_str(), "file" | "format" | "path" | "unit" | "index")
                        })
                    }) {
                        return Err(invalid("unknown run-reference field"));
                    }
                    if spec.get("index").is_some_and(|v| v.as_u64().is_none()) {
                        return Err(invalid("run index must be an unsigned integer"));
                    }
                    let name = spec["file"]
                        .as_str()
                        .ok_or_else(|| invalid("run reference requires file"))?;
                    let file = root.join(name);
                    let bytes = crate::evidence_quality::read(&file, 4 << 20)?;
                    sources.insert(file.display().to_string(), crate::localized::digest(&bytes));
                    let doc: Value = serde_json::from_slice(&bytes)?;
                    let mut times = extract(
                        &doc,
                        spec["format"]
                            .as_str()
                            .ok_or_else(|| invalid("run reference requires string format"))?,
                        spec["path"].as_str(),
                        spec["unit"].as_str().unwrap_or("ms"),
                    )?;
                    if let Some(i) = spec["index"].as_u64() {
                        times = vec![
                            *times
                                .get(i as usize)
                                .ok_or_else(|| invalid("timing index out of bounds"))?,
                        ];
                    }
                    pair[arm] = serde_json::to_value(times)?;
                }
            }
        }
    }
    Ok((serde_json::from_value(value)?, sources))
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn session(shift: f64, noise: f64) -> Session {
        let mut pairs = Vec::new();
        for aa in [false, true] {
            for i in 0..24 {
                let jitter = ((i * 13 % 23) as f64 / 23. - 0.5) * noise;
                let a = 10. * (1. + jitter);
                pairs.push(Pair {
                    id: format!("{aa}-{i}"),
                    block: format!("{aa}-{i}"),
                    order: if i % 2 == 0 { "ab" } else { "ba" }.into(),
                    a: vec![a],
                    b: vec![a * (1. + if aa { 0. } else { shift })],
                    aa,
                });
            }
        }
        Session {
            schema: SESSION_SCHEMA.into(),
            session: "specimen-analysis".into(),
            band_pct: 2.,
            max_pairs: 24,
            looks: vec![],
            confidence: 0.95,
            seed: 4,
            resamples: 1024,
            pairs,
            diagnostic_reasons: vec![],
            captures: vec![],
            gpu_clocks_not_applicable: true,
        }
    }
    #[test]
    fn slowdown_under_load_never_faster_and_null_is_equivalent() {
        let r = analyze(session(0.1, 0.35), BTreeMap::new(), Path::new(".")).unwrap();
        assert_eq!(r.verdict, "slower");
        assert!(r.effect_pct.unwrap() > 9.9);
        assert!(r.diagnostic_only);
        assert_eq!(
            analyze(session(0., 0.35), BTreeMap::new(), Path::new("."))
                .unwrap()
                .verdict,
            "equivalent"
        );
    }
    #[test]
    fn wide_aa_refuses_and_blocks_are_independent_units() {
        let mut s = session(0.1, 0.35);
        for p in s.pairs.iter_mut().filter(|p| p.aa) {
            p.b[0] *= 1.2;
        }
        let r = analyze(s, BTreeMap::new(), Path::new(".")).unwrap();
        assert_eq!(r.verdict, "inconclusive");
        assert!(r.reasons.contains(&"aa_noise_floor_too_wide".into()));
        let mut s = session(0.1, 0.35);
        for p in s.pairs.iter_mut().filter(|p| !p.aa) {
            p.block = "one".into();
        }
        assert!(
            analyze(s, BTreeMap::new(), Path::new("."))
                .unwrap()
                .reasons
                .contains(&"too_few_independent_blocks".into())
        );
    }
    #[test]
    fn adapters_and_sequential_policy() {
        assert_eq!(
            extract(
                &serde_json::json!({"results":[{"times":[0.01,0.02]}]}),
                "hyperfine",
                None,
                "ms"
            )
            .unwrap(),
            vec![10., 20.]
        );
        assert_eq!(
            extract(
                &serde_json::json!({"measure":{"duration":3}}),
                "json",
                Some("measure.duration"),
                "ms"
            )
            .unwrap(),
            vec![3.]
        );
        let mut s = session(0.1, 0.);
        s.looks = vec![12, 24];
        let r = analyze(s, BTreeMap::new(), Path::new(".")).unwrap();
        assert_eq!(r.look_confidence, 0.975);
    }

    #[test]
    #[ignore = "prespecified repeated-peeking simulation; coordinator statistical acceptance gate"]
    fn repeated_peeking_null_familywise_directional_rate_is_bounded() {
        const TRIALS: usize = 128;
        let mut rng = Generator::new(0x0011_aabb_2026);
        let mut false_directions = 0;
        let mut conclusive_null_sessions = 0;
        let mut detected_shift_sessions = 0;
        for trial in 0..TRIALS {
            let mut plan = session(0., 0.35);
            plan.band_pct = 1.;
            plan.looks = vec![12, 24];
            plan.seed = trial as u64 + 400;
            // Prespecified bounded symmetric log-ratio noise, independent runs.
            // A and B share substantial machine load and independent residual noise.
            for pair in &mut plan.pairs {
                let residual = (rng.index(10001).unwrap() as f64 / 10000. - 0.5) * 0.04;
                pair.b[0] = pair.a[0] * residual.exp();
            }
            let mut any_false_direction = false;
            let mut any_conclusion = false;
            let mut any_detected_shift = false;
            for count in [12, 24] {
                let mut look = plan.clone();
                let mut seen = 0;
                look.pairs.retain(|p| {
                    if p.aa {
                        true
                    } else {
                        seen += 1;
                        seen <= count
                    }
                });
                let null = analyze(look.clone(), BTreeMap::new(), Path::new(".")).unwrap();
                assert_eq!(null.look_confidence, 0.975);
                any_false_direction |= matches!(null.verdict.as_str(), "faster" | "slower");
                any_conclusion |= null.verdict == "equivalent";
                for p in look.pairs.iter_mut().filter(|p| !p.aa) {
                    p.b[0] *= 1.1;
                }
                let shifted = analyze(look, BTreeMap::new(), Path::new(".")).unwrap();
                assert_ne!(shifted.verdict, "faster");
                any_detected_shift |= shifted.verdict == "slower";
            }
            false_directions += usize::from(any_false_direction);
            conclusive_null_sessions += usize::from(any_conclusion);
            detected_shift_sessions += usize::from(any_detected_shift);
        }
        eprintln!(
            "prespecified null peeking: {false_directions}/{TRIALS} false directions; {conclusive_null_sessions} conclusive nulls; {detected_shift_sessions} shifted detections; declared alpha=0.05"
        );
        // Count ANY false direction over all declared looks, even after a stop.
        assert!(false_directions as f64 / TRIALS as f64 <= 0.05);
        // These guards prevent passing simply by returning inconclusive every time.
        assert!(conclusive_null_sessions >= TRIALS / 2);
        assert!(detected_shift_sessions >= TRIALS / 2);
        let mut unlisted = session(0.1, 0.);
        unlisted.looks = vec![12, 24];
        let mut seen = 0;
        unlisted.pairs.retain(|p| {
            if p.aa {
                true
            } else {
                seen += 1;
                seen <= 18
            }
        });
        assert_eq!(
            analyze(unlisted, BTreeMap::new(), Path::new("."))
                .unwrap()
                .verdict,
            "inconclusive"
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod import_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn external_hyperfine_perf_json_paths_and_csv_bind_exact_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let hyper = tmp.path().join("h.json");
        std::fs::write(&hyper, br#"{"results":[{"times":[0.01,0.011]}]}"#).unwrap();
        let perf = tmp.path().join("saccade-perf.json");
        std::fs::write(&perf, br#"{"schema":"saccade-perf.v1","unit":"ms","frame":{"value":11,"samples":1,"stat":"mean"},"terms":[],"counters":{}}"#).unwrap();
        let path = tmp.path().join("lab.json");
        std::fs::write(&path, br#"{"readings":[{"latency_us":11000}]}"#).unwrap();
        let input = tmp.path().join("session.json");
        let mut session = json!({"schema":SESSION_SCHEMA,"session":"measurement-run-1","band_pct":2,"max_pairs":6,"confidence":0.95,"seed":1,"resamples":1024,"pairs":[{"id":"p1","block":"b1","order":"ab","a":{"file":"h.json","format":"hyperfine","index":0},"b":{"file":"saccade-perf.json","format":"perf","unit":"ms"}}]});
        std::fs::write(&input, serde_json::to_vec(&session).unwrap()).unwrap();
        let (s, hashes) = load(&input, "session").unwrap();
        assert_eq!(s.pairs[0].a, vec![10.]);
        assert_eq!(s.pairs[0].b, vec![11.]);
        assert_eq!(hashes.len(), 3);
        assert_eq!(
            hashes[&hyper.display().to_string()],
            crate::localized::digest(&std::fs::read(hyper).unwrap())
        );
        assert_eq!(
            analyze(s, hashes, tmp.path()).unwrap().verdict,
            "inconclusive"
        );
        session["pairs"][0]["b"] =
            json!({"file":"lab.json","format":"json","path":"readings.0.latency_us","unit":"us"});
        std::fs::write(&input, serde_json::to_vec(&session).unwrap()).unwrap();
        assert_eq!(load(&input, "session").unwrap().0.pairs[0].b, vec![11.]);
        let csv = tmp.path().join("session.csv");
        std::fs::write(
            &csv,
            "id,block,order,a_ms,b_ms,aa\np1,b1,ab,10,11,false\nc1,c1,ba,10,10,true\n",
        )
        .unwrap();
        assert_eq!(load(&csv, "csv").unwrap().0.pairs.len(), 2);
        assert!(load(&input, "unknown").is_err());
        session["pairs"][0]["a"]["index"] = json!(99);
        std::fs::write(&input, serde_json::to_vec(&session).unwrap()).unwrap();
        assert!(load(&input, "session").is_err());
    }
}
