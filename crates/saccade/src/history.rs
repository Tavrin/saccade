//! Opt-in, local history of comparable report measurements.
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use fs2::FileExt;
use saccade_core::report::{Metric, Mode, Status};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::agent::CliError;

const SCHEMA: &str = "saccade-history.v1";
const INDEX: &str = "index.jsonl";
const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Args)]
pub(crate) struct HistoryArgs {
    #[command(subcommand)]
    operation: HistoryOperation,
}

#[derive(Subcommand)]
enum HistoryOperation {
    /// Find candidate performance onsets in qualified, comparable history observations.
    Onset {
        #[arg(long)]
        store: PathBuf,
        /// Most recent distinct observations per partition; exact DP is bounded to 120.
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u16).range(10..=120))]
        limit: u16,
        #[arg(long)]
        json: bool,
    },
    /// Add one existing comparison report to the local history store.
    Record {
        report: PathBuf,
        /// Producer-assigned independent capture run, never an image or report hash.
        #[arg(long, requires = "environment_id")]
        run_id: Option<String>,
        /// Frozen browser/device, fonts, viewport, warmup and temporal protocol identity.
        #[arg(long, requires = "run_id")]
        environment_id: Option<String>,
        /// Declare an unchanged-build repeat eligible for normal-variation advice.
        #[arg(long, requires = "run_id")]
        unchanged_build: bool,
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Show measured variation and threshold advice for comparable entries.
    Analyze {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        entry: Option<String>,
        /// Diagnose sustained anchor-relative drift in recorded run order.
        #[arg(long)]
        drift: bool,
        /// New file containing the complete witness for the selected groups.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u8).range(1..=20))]
        limit: u8,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    schema: String,
    report_sha256: String,
    config_sha256: String,
    generated_at_unix: u64,
    samples: Vec<Sample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trial: Option<Trial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Trial {
    run_id: String,
    environment_id: String,
    unchanged_build: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    entry: String,
    baseline_sha256: String,
    capture_sha256: String,
    metric: Metric,
    threshold: f64,
    value: f64,
}

type GroupKey = (String, String, String, String, String);
type TrialGroups<'a> = BTreeMap<(GroupKey, String), Vec<(&'a Row, &'a Sample, &'a Trial)>>;
type Observations = (f64, BTreeMap<String, f64>);

fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn index_path(store: &Path) -> PathBuf {
    store.join(INDEX)
}

fn read_index(store: &Path) -> Result<Vec<Row>, CliError> {
    let path = index_path(store);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file =
        fs::File::open(&path).map_err(|e| CliError::io(format!("{}: {e}", path.display())))?;
    if file
        .metadata()
        .map_err(|e| CliError::io(e.to_string()))?
        .len()
        > MAX_INDEX_BYTES
    {
        return Err(CliError::new(
            "output_budget",
            "history index exceeds 64 MiB; archive old records",
        ));
    }
    let mut rows = Vec::new();
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| CliError::io(format!("history index line {}: {e}", i + 1)))?;
        let row: Row = crate::parse_contract(line.as_bytes(), SCHEMA)
            .map_err(|e| CliError::new(e.code, format!("history index line {}: {e}", i + 1)))?;
        if row.schema != SCHEMA {
            return Err(CliError::new(
                "version_skew",
                format!(
                    "history index line {} has unsupported schema {}",
                    i + 1,
                    row.schema
                ),
            ));
        }
        rows.push(row);
    }
    Ok(rows)
}

fn report_samples(report: &saccade_core::Report) -> Vec<Sample> {
    report
        .entries
        .iter()
        .filter_map(|e| {
            if !matches!(e.status, Status::Pass | Status::Fail) {
                return None;
            }
            if e.capture_validity.status == saccade_core::meta::Validity::Invalid {
                return None;
            }
            let (Some(value), Some(baseline_sha256), Some(capture_sha256)) = (
                e.value,
                e.baseline_sha256.as_ref(),
                e.capture_sha256.as_ref(),
            ) else {
                return None;
            };
            if !value.is_finite() || !e.threshold.is_finite() {
                return None;
            }
            Some(Sample {
                entry: e.name.clone(),
                baseline_sha256: baseline_sha256.clone(),
                capture_sha256: capture_sha256.clone(),
                metric: e.metric_used,
                threshold: e.threshold,
                value,
            })
        })
        .collect()
}

fn verified_rows(store: &Path) -> Result<Vec<Row>, CliError> {
    let mut rows = read_index(store)?;
    for row in &mut rows {
        if row.report_sha256.len() != 64
            || !row
                .report_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(CliError::io("invalid history object identity"));
        }
        let path = store
            .join("objects")
            .join(format!("{}.json", row.report_sha256));
        let bytes =
            fs::read(&path).map_err(|e| CliError::io(format!("{}: {e}", path.display())))?;
        if hash(&bytes) != row.report_sha256 {
            return Err(CliError::io("history object content hash mismatch"));
        }
        let report: saccade_core::Report = crate::parse_contract(&bytes, "saccade-report.v1")?;
        row.samples = report_samples(&report);
        row.config_sha256 = hash(&serde_json::to_vec(&report.config)?);
        row.generated_at_unix = report.generated_at_unix;
    }
    Ok(rows)
}

#[cfg(test)]
fn record(report_path: &Path, store: &Path) -> Result<Value, CliError> {
    record_trial(report_path, store, None)
}

fn record_trial(report_path: &Path, store: &Path, trial: Option<Trial>) -> Result<Value, CliError> {
    if trial
        .as_ref()
        .is_some_and(|t| t.run_id.trim().is_empty() || t.environment_id.trim().is_empty())
    {
        return Err(CliError::usage(
            "run and environment identities must be nonempty",
        ));
    }
    let bytes = fs::read(report_path)
        .map_err(|e| CliError::io(format!("{}: {e}", report_path.display())))?;
    let report: saccade_core::Report = crate::parse_contract(&bytes, "saccade-report.v1")?;
    if report.config.mode != Mode::Regression {
        return Err(CliError::usage(
            "history accepts comparison reports, not identity proofs",
        ));
    }
    let report_hash = hash(&bytes);
    let config_hash = hash(&serde_json::to_vec(&report.config)?);
    fs::create_dir_all(store.join("objects"))
        .map_err(|e| CliError::io(format!("creating history store: {e}")))?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(store.join("record.lock"))
        .map_err(|e| CliError::io(format!("opening history lock: {e}")))?;
    lock.lock_exclusive()
        .map_err(|e| CliError::io(format!("locking history store: {e}")))?;
    let existing = read_index(store)?;
    if existing.iter().any(|r| match (&trial, &r.trial) {
        (Some(a), Some(b)) => a.run_id == b.run_id && a.environment_id == b.environment_id,
        (None, None) => r.report_sha256 == report_hash,
        _ => false,
    }) {
        // Re-recording also upgrades legacy relative-path provenance.
        crate::last_good::record_origin(store, &report_hash, report_path)?;
        return Ok(
            json!({"schema":SCHEMA,"operation":"record","recorded":false,"reason":"report_already_recorded","report_sha256":report_hash}),
        );
    }
    let samples = report_samples(&report);
    if samples.is_empty() {
        return Err(CliError::new(
            "nothing_compared",
            "report has no finite paired measurements with image hashes",
        ));
    }
    let object = store.join("objects").join(format!("{report_hash}.json"));
    if !object.exists() {
        let mut temp = tempfile::NamedTempFile::new_in(store.join("objects"))
            .map_err(|e| CliError::io(format!("creating history object: {e}")))?;
        temp.write_all(&bytes)
            .map_err(|e| CliError::io(format!("writing history object: {e}")))?;
        temp.as_file()
            .sync_all()
            .map_err(|e| CliError::io(format!("syncing history object: {e}")))?;
        match temp.persist_noclobber(&object) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(CliError::io(format!("{}: {}", object.display(), e.error))),
        }
    }
    if hash(&fs::read(&object).map_err(|e| CliError::io(format!("{}: {e}", object.display())))?)
        != report_hash
    {
        return Err(CliError::io(format!(
            "history object content differs: {}",
            object.display()
        )));
    }
    // Retain the origin needed to resolve relative capture provenance.
    crate::last_good::record_origin(store, &report_hash, report_path)?;
    let row = Row {
        schema: SCHEMA.into(),
        report_sha256: report_hash.clone(),
        config_sha256: config_hash,
        generated_at_unix: report.generated_at_unix,
        samples,
        trial,
    };
    let mut index = OpenOptions::new()
        .create(true)
        .append(true)
        .open(index_path(store))
        .map_err(|e| CliError::io(format!("opening history index: {e}")))?;
    let mut line = serde_json::to_vec(&row)?;
    line.push(b'\n');
    let old_len = index
        .metadata()
        .map_err(|e| CliError::io(format!("reading history index length: {e}")))?
        .len();
    let written = index
        .write(&line)
        .map_err(|e| CliError::io(format!("writing history index: {e}")))?;
    if written != line.len() {
        index
            .set_len(old_len)
            .map_err(|e| CliError::io(format!("rolling back short history append: {e}")))?;
        return Err(CliError::io("short history index append"));
    }
    index
        .sync_all()
        .map_err(|e| CliError::io(format!("syncing history index: {e}")))?;
    Ok(json!({"schema":SCHEMA,"operation":"record","recorded":true,
        "report_sha256":report_hash,"samples":row.samples.len()}))
}

fn analyze(rows: &[Row], entry: Option<&str>, limit: usize) -> Value {
    // Group only measurements with identical reference content and effective
    // run settings. Repeated capture hashes count once, not as fresh evidence.
    let mut groups: BTreeMap<GroupKey, Observations> = BTreeMap::new();
    for row in rows {
        // Declared trials belong to the independent-run analysis. Revision
        // measurements must not also train legacy artifact tolerance advice.
        if row.trial.is_some() {
            continue;
        }
        for sample in &row.samples {
            if entry.is_some_and(|name| name != sample.entry) {
                continue;
            }
            let key = (
                sample.entry.clone(),
                sample.baseline_sha256.clone(),
                row.config_sha256.clone(),
                format!("{:?}", sample.metric).to_lowercase(),
                sample.threshold.to_bits().to_string(),
            );
            let (_, observed) = groups
                .entry(key)
                .or_insert_with(|| (sample.threshold, BTreeMap::new()));
            observed
                .entry(sample.capture_sha256.clone())
                .or_insert(sample.value);
        }
    }
    let total = groups.len();
    let mut findings = Vec::new();
    for ((name, baseline, config, metric, _), (threshold, values)) in groups {
        let (min, max) = values
            .values()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
                (lo.min(v), hi.max(v))
            });
        let count = values.len();
        let span = if count == 0 { 0.0 } else { max - min };
        let verdict = if count < 3 {
            "insufficient_history"
        } else if min <= threshold && max > threshold {
            "flaky_threshold"
        } else if span > 0.0 && span >= threshold {
            "noise_dominates_threshold"
        } else {
            "stable_observed"
        };
        let action = if matches!(verdict, "flaky_threshold" | "noise_dominates_threshold") {
            "quarantine_and_remeasure"
        } else if verdict == "insufficient_history" {
            "capture_more_distinct_repeats"
        } else {
            "keep_threshold"
        };
        // An observed maximum plus one measured span is a provisional suggestion,
        // never an automatic gate change.
        let suggested = if count >= 3 && max.is_finite() {
            Some((max + span).min(1.0))
        } else {
            None
        };
        findings.push(
            json!({"entry":name,"baseline_sha256":baseline,"config_sha256":config,
            "metric":metric,"threshold":threshold,"distinct_captures":count,
            "observed_min":if count == 0 { None } else { Some(min) },
            "observed_max":if count == 0 { None } else { Some(max) },
            "variation_span":span,"finding":verdict,"next_action":action,
            "suggested_threshold":suggested}),
        );
    }
    findings.sort_by(|a, b| a["entry"].as_str().cmp(&b["entry"].as_str()));
    findings.truncate(limit);
    json!({"schema":SCHEMA,"operation":"analyze","groups":total,
        "page":{"shown":findings.len(),"omitted":total.saturating_sub(findings.len())},
        "entries":findings,"limits":["Only distinct capture hashes under one baseline and configuration count as variation.",
        "Observed extrema are not a statistical confidence bound; threshold advice requires review."]})
}

fn median(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn run_analysis(rows: &[Row], entry: Option<&str>, limit: usize, drift: bool) -> Value {
    // Image identity and independent capture identity are deliberately separate.
    let mut groups: TrialGroups<'_> = BTreeMap::new();
    for row in rows {
        let Some(trial) = &row.trial else { continue };
        for sample in &row.samples {
            if entry.is_some_and(|e| e != sample.entry) || !sample.value.is_finite() {
                continue;
            }
            let key = (
                sample.entry.clone(),
                sample.baseline_sha256.clone(),
                row.config_sha256.clone(),
                format!("{:?}", sample.metric),
                sample.threshold.to_bits().to_string(),
            );
            let group = groups
                .entry((key, trial.environment_id.clone()))
                .or_default();
            if !group.iter().any(|(_, _, t)| t.run_id == trial.run_id) {
                group.push((row, sample, trial));
            }
        }
    }
    let total = groups.len();
    let entries: Vec<_> = groups.into_iter().take(limit).map(|((key, environment), runs)| {
        let noise: Vec<_> = runs.iter().filter(|(_, _, t)| t.unchanged_build).map(|(_, s, _)| s.value).collect();
        let threshold = runs[0].1.threshold;
        let lo = noise.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = noise.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let enough = noise.len() >= 10;
        let span = if noise.is_empty() { 0.0 } else { hi - lo };
        let flaky = enough && lo <= threshold && hi > threshold;
        let values: Vec<_> = runs.iter().map(|(_, s, _)| s.value).collect();
        let effect = if values.len() >= 10 { Some(median(&values[values.len()-5..]) - median(&values[..5])) } else { None };
        // Keep the same baseline anchor. Sustained, mostly increasing changes
        // smaller than a per-revision tolerance still accumulate here.
        // Equal repeated captures and a settled plateau preserve a sustained
        // displacement. Collapse adjacent equals for the trend check while the
        // endpoint medians retain independent runs and the fixed anchor.
        let mut levels = values.clone();
        levels.dedup();
        let recent = &levels[levels.len().saturating_sub(10)..];
        let increasing = recent.windows(2).filter(|w| w[1] > w[0]).count();
        let candidate = drift && enough && effect.is_some_and(|e| e > span.max(1e-9))
            && recent.len() >= 2 && increasing * 4 >= (recent.len() - 1) * 3;
        let suggestion = (enough && !candidate).then_some((hi + span).min(1.0));
        json!({"entry":key.0,"baseline_sha256":key.1,"config_sha256":key.2,"environment_id":environment,
            "independent_runs":runs.len(),"unchanged_build_runs":noise.len(),
            "unique_image_hashes":runs.iter().map(|(_,s,_)| &s.capture_sha256).collect::<std::collections::BTreeSet<_>>().len(),
            "normal_variation_span":enough.then_some(span),"threshold":threshold,"suggested_threshold":suggestion,
            "suggestion_basis":"observed unchanged-build maximum plus range; uncalibrated, review required",
            "drift":if !drift {"not_requested"} else if !enough {"insufficient_unchanged_build_repeats"} else if candidate {"candidate"} else {"not_detected"},
            "anchor_relative_effect":effect,"chronology":"recorded_sequence","policy_changed":false,
            "recommendation":if candidate {"investigate_drift_with_fresh_matched_repeats"} else if flaky {"quarantine_and_remeasure"} else if !enough {"collect_unchanged_build_repeats"} else {"keep_threshold"},
            "evidence":runs.iter().map(|(r,s,t)| json!({"run_id":t.run_id,"report_sha256":r.report_sha256,"capture_sha256":s.capture_sha256,"value":s.value,"unchanged_build":t.unchanged_build})).collect::<Vec<_>>()})
    }).collect();
    json!({"groups":total,"entries":entries,"limits":["Run independence and unchanged build are producer declarations, not inferred from identical pixels.","Scalar history cannot localize variation or diagnose delayed fonts. Drift is a heuristic candidate, not causality.","Environment identity must include all capture conditions. Legacy hash-only advice is provisional artifact variation, not learned normal variation."]})
}

fn json_preview(mut value: Value) -> Result<Value, CliError> {
    if value["operation"] != "analyze" {
        return crate::local_cmd::bounded(value, 4096);
    }
    let total = value["run_analysis"]["groups"].as_u64().unwrap_or(0);
    if let Some(entries) = value["run_analysis"]["entries"].as_array_mut() {
        for entry in entries {
            if let Some(evidence) = entry["evidence"].as_array_mut() {
                let original = evidence.len();
                if original > 6 {
                    let tail = evidence.split_off(original - 3);
                    evidence.truncate(3);
                    evidence.extend(tail);
                }
                let omitted = original - evidence.len();
                entry["evidence_omitted"] = json!(omitted);
            }
        }
    }
    loop {
        let shown = value["run_analysis"]["entries"]
            .as_array()
            .map_or(0, Vec::len);
        if value.get("run_analysis").is_some() {
            value["run_analysis"]["page"] = json!({"shown":shown,"omitted":total.saturating_sub(shown as u64),"witness":"JSON is a bounded preview; --out preserves every run in the selected groups"});
        }
        match crate::local_cmd::bounded(value.clone(), 4096) {
            Ok(value) => return Ok(value),
            Err(error) => {
                let entries = value["run_analysis"]["entries"].as_array_mut();
                if let Some(entries) = entries
                    && entries.len() > 1
                {
                    entries.pop();
                } else {
                    return Err(error);
                }
            }
        }
    }
}

fn performance_observation(
    report: &saccade_core::Report,
    run: &str,
    sequence: u64,
) -> Result<saccade_core::onset::Observation, String> {
    use saccade_core::perf::Comparability;
    let p = report.perf_diff.as_ref().ok_or("performance_missing")?;
    if p.comparability != Comparability::Qualified
        || p.noise_comparability != Comparability::Qualified
    {
        return Err(format!(
            "performance_or_repeat_noise_unqualified: {:?}",
            p.qualification_reasons
        ));
    }
    let context = p
        .context_after
        .as_ref()
        .ok_or("performance_context_missing")?;
    if ["window_complete", "clock_qualified", "warmup_complete"]
        .iter()
        .any(|key| context.qualification.checks.get(*key) != Some(&Some(true)))
        || context.qualification.status != Comparability::Qualified
        || context
            .qualification
            .checks
            .values()
            .any(|v| *v != Some(true))
    {
        return Err("performance_context_unqualified".into());
    }
    let identity = p
        .history_identity
        .as_ref()
        .ok_or("history_comparison_identity_not_recorded_by_producer")?;
    let window = context
        .sample_window
        .as_ref()
        .ok_or("sample_window_missing")?;
    let milliseconds = p
        .frame
        .after
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or("positive_frame_timing_missing")?;
    let repeat_range_ms = p
        .frame
        .noise_floor
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or("repeat_range_missing")?;
    let floor_ms = p
        .frame
        .noise_threshold
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or("materiality_floor_missing")?;
    let commits: std::collections::BTreeSet<_> = report
        .entries
        .iter()
        .filter_map(|e| e.capture_provenance.get("capture.source_head"))
        .collect();
    Ok(saccade_core::onset::Observation {
        run: run.into(),
        commit: if commits.len() == 1 {
            commits.first().map(|v| (*v).clone())
        } else {
            None
        },
        time: sequence,
        identity: identity.as_str().into(),
        window: window.hash.as_str().into(),
        milliseconds,
        repeat_range_ms,
        floor_ms,
        min_delta_pct: p.min_delta_pct,
    })
}

fn onset_value(store: &Path, limit: usize) -> Result<Value, CliError> {
    let rows = read_index(store)?;
    let mut groups: BTreeMap<String, Vec<(saccade_core::onset::Observation, Option<u64>)>> =
        BTreeMap::new();
    let mut omitted = Vec::new();
    for (sequence, row) in rows.into_iter().enumerate() {
        if row.report_sha256.len() != 64
            || !row.report_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(CliError::io("invalid history object identity"));
        }
        let path = store
            .join("objects")
            .join(format!("{}.json", row.report_sha256));
        let bytes =
            fs::read(&path).map_err(|e| CliError::io(format!("{}: {e}", path.display())))?;
        if hash(&bytes) != row.report_sha256 {
            return Err(CliError::io("history object content hash mismatch"));
        }
        // Read the original object, not cached numerical fields in the index.
        let report: saccade_core::Report = crate::parse_contract(&bytes, "saccade-report.v1")?;
        match performance_observation(&report, &row.report_sha256, sequence as u64) {
            Ok(o) => {
                // Capture timestamps are producer evidence. When a partition lacks
                // numeric timestamps, preserve the store's append sequence instead
                // of mixing timestamp and sequence units or sorting report hashes.
                let times: Option<std::collections::BTreeSet<u64>> = report
                    .entries
                    .iter()
                    .map(|e| e.capture_provenance.get("capture.timestamp")?.parse().ok())
                    .collect();
                let time = times
                    .filter(|t| t.len() == 1)
                    .and_then(|t| t.first().copied());
                groups
                    .entry(o.identity.clone())
                    .or_default()
                    .push((o, time));
            }
            Err(reason) => omitted.push(json!({"run":row.report_sha256,"reason":reason})),
        }
    }
    let mut partitions = Vec::new();
    for (identity, records) in groups {
        let capture_chronology = records.iter().all(|(_, time)| time.is_some());
        let chronology = if capture_chronology {
            "capture_timestamp"
        } else {
            "recorded_sequence"
        };
        let mut observations: Vec<_> = records
            .into_iter()
            .map(|(mut o, time)| {
                if capture_chronology {
                    o.time = time.unwrap_or(o.time);
                }
                o
            })
            .collect();
        // Stable sorting keeps recorded chronology when capture timestamps tie.
        observations.sort_by_key(|o| o.time);
        let mut windows = std::collections::BTreeSet::new();
        observations.retain(|o| {
            if windows.insert(o.window.clone()) { true } else {
                omitted.push(json!({"run":o.run,"reason":"same_sample_window_not_an_independent_observation"})); false
            }
        });
        let truncated = observations.len().saturating_sub(limit);
        observations.drain(..truncated);
        if observations.len() < 10 {
            partitions.push(json!({"identity":identity,"chronology":chronology,"status":"insufficient_history","observations":observations,"older_observations_omitted":truncated}));
        } else {
            let analysis = saccade_core::onset::detect(&observations)?;
            partitions.push(json!({"identity":identity,"chronology":chronology,"status":"candidate_analysis","analysis":analysis,"older_observations_omitted":truncated}));
        }
    }
    Ok(
        json!({"schema":"saccade-onset.v1","operation":"onset","partitions":partitions,"excluded_observations":omitted,"limits":["Candidate intervals only; fresh qualified repeats are required. Never commit blame.","One record must represent an independent nightly statistic; timestamps do not establish independence. Numeric producer capture timestamps order complete partitions, otherwise store append sequence is used; report generation time and hashes never order measurements. Missing observations are not interpolated."]}),
    )
}
fn run_onset(store: &Path, limit: usize, json_output: bool) -> Result<u8, CliError> {
    let value = onset_value(store, limit)?;
    if json_output {
        crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
    } else {
        let mut text =
            String::from("Performance onset candidates (fresh qualified repeats required):\n");
        let mut count = 0;
        for partition in value["partitions"].as_array().into_iter().flatten() {
            if partition["status"] == "insufficient_history" {
                text.push_str(&format!(
                    "{}: insufficient qualified history\n",
                    partition["identity"]
                ));
            }
            for candidate in partition["analysis"]["evidence"]["candidates"]
                .as_array()
                .into_iter()
                .flatten()
            {
                count += 1;
                text.push_str(&format!("candidate onset at run {} / commit {}; interval after run {} / commit {}; effect {} ms ({}%).\n", candidate["first_changed"]["run"], candidate["first_changed"]["commit"], candidate["last_before"]["run"], candidate["last_before"]["commit"], candidate["effect_ms"], candidate["effect_pct"]));
            }
        }
        text.push_str(&format!("{count} candidates; {} excluded observations. Use --json for the numerical witness and exclusion reasons.\n", value["excluded_observations"].as_array().map_or(0, Vec::len)));
        crate::emit(&text)?;
    }
    Ok(0)
}

pub(crate) fn run(args: HistoryArgs) -> Result<u8, CliError> {
    let (value, json_output) = match args.operation {
        HistoryOperation::Onset { store, limit, json } => {
            return run_onset(&store, limit as usize, json);
        }
        HistoryOperation::Record {
            report,
            store,
            json,
            run_id,
            environment_id,
            unchanged_build,
        } => (
            record_trial(
                &report,
                &store,
                run_id
                    .zip(environment_id)
                    .map(|(run_id, environment_id)| Trial {
                        run_id,
                        environment_id,
                        unchanged_build,
                    }),
            )?,
            json,
        ),
        HistoryOperation::Analyze {
            store,
            entry,
            drift,
            out,
            limit,
            json,
        } => {
            let rows = verified_rows(&store)?;
            (
                {
                    let mut value = analyze(&rows, entry.as_deref(), limit as usize);
                    value["run_analysis"] =
                        run_analysis(&rows, entry.as_deref(), limit as usize, drift);
                    value["policy_changed"] = json!(false);
                    value["history_store"] = json!(store);
                    if let Some(path) = out {
                        let file = OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)
                            .map_err(|e| CliError::io(e.to_string()))?;
                        serde_json::to_writer_pretty(file, &value)?;
                        value["evidence_artifact"] = crate::local_cmd::reference(&path)?;
                    }
                    value
                },
                json,
            )
        }
    };
    if json_output {
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&json_preview(value)?)?
        ))?;
    } else if value["operation"] == "record" {
        crate::emit(&format!(
            "history: {} ({})\n",
            if value["recorded"] == true {
                "recorded"
            } else {
                "already recorded"
            },
            value["report_sha256"].as_str().unwrap_or("unknown")
        ))?;
    } else {
        let mut out = format!(
            "history: {} artifact groups, {} capture-run groups\n",
            value["groups"], value["run_analysis"]["groups"]
        );
        for item in value["run_analysis"]["entries"]
            .as_array()
            .into_iter()
            .flatten()
        {
            out.push_str(&format!(
                "{}: {} independent runs; drift {}; recommendation {}\n",
                item["entry"].as_str().unwrap_or("?"),
                item["independent_runs"],
                item["drift"].as_str().unwrap_or("?"),
                item["recommendation"].as_str().unwrap_or("?")
            ));
        }
        for item in value["entries"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "{}: {} ({} distinct captures, span {:.5}, threshold {:.5})\n",
                item["entry"].as_str().unwrap_or("?"),
                item["finding"].as_str().unwrap_or("?"),
                item["distinct_captures"].as_u64().unwrap_or(0),
                item["variation_span"].as_f64().unwrap_or(0.0),
                item["threshold"].as_f64().unwrap_or(0.0)
            ));
        }
        crate::emit(&out)?;
    }
    Ok(0)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn repeated_and_settled_anchor_drift_remains_detected() {
        let mut rows = Vec::new();
        for i in 0..30 {
            rows.push(Row {
                schema: SCHEMA.into(),
                report_sha256: format!("report{i}"),
                config_sha256: "config".into(),
                generated_at_unix: 0,
                trial: Some(Trial {
                    run_id: format!("run{i}"),
                    environment_id: "env".into(),
                    unchanged_build: i < 10,
                }),
                samples: vec![Sample {
                    entry: "ui.png".into(),
                    baseline_sha256: "anchor".into(),
                    capture_sha256: format!("image{i}"),
                    metric: Metric::Mean,
                    threshold: 0.5,
                    value: if i < 10 {
                        0.0
                    } else {
                        ((i - 10) / 2 + 1) as f64 * 0.001
                    },
                }],
            });
        }
        assert_eq!(
            run_analysis(&rows, None, 10, true)["entries"][0]["drift"],
            "candidate"
        );
        for i in 30..45 {
            let mut row = rows.last().expect("row").clone();
            row.trial.as_mut().expect("trial").run_id = format!("run{i}");
            rows.push(row);
        }
        assert_eq!(
            run_analysis(&rows, None, 10, true)["entries"][0]["drift"],
            "candidate"
        );
        for row in &mut rows {
            row.samples[0].value = 0.0;
        }
        assert_eq!(
            run_analysis(&rows, None, 10, true)["entries"][0]["drift"],
            "not_detected"
        );
    }
    #[test]
    fn onset_reads_hash_bound_reports_partitions_hardware_and_keeps_exclusions() {
        let dir = tempfile::tempdir().expect("store");
        fs::create_dir_all(dir.path().join("objects")).expect("objects");
        let context: Value = serde_json::from_str(include_str!(
            "../../saccade-core/tests/fixtures/perf/context.json"
        ))
        .expect("context");
        let mut index = String::new();
        let mut last_hash = String::new();
        for i in 0..33 {
            let mut context = context.clone();
            let window = if i == 31 { 0 } else { i };
            context["sample_window"]["hash"] =
                json!(saccade_core::evidence::canonical::Digest::of_bytes(
                    format!("window{window}").as_bytes()
                ));
            if i == 32 {
                context["hardware"]["gpu"] = "other-gpu".into();
            }
            let identity = saccade_core::evidence::canonical::Digest::of_bytes(if i == 32 {
                b"other-hardware"
            } else {
                b"hardware"
            });
            let report = json!({"schema":"saccade-report.v1","tool_version":"fixture","generated_at_unix":i * 86400,
                "config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},
                "totals":{"pass":0,"fail":0,"error":0,"missing":0,"new":0,"total":0},
                "entries":[{"name":"frame.png","status":"pass","metric_used":"mean","threshold":0.01,"value":0.0,"metrics":null,"properties":null,"paths":{},"error":null,"capture_provenance":{"capture.timestamp":(i * 86400).to_string()}}],
                "perf_diff":{"schema":"saccade-perf-diff.v1","history_identity":identity,"unit":"ms","noise_k":3.0,
                    "comparability":if i == 30 { "unknown" } else { "qualified" },"noise_comparability":"qualified","context_after":context,
                    "frame":{"before":10.0,"after":if i < 15 {10.0} else {12.0},"delta":null,"delta_pct":null,"noise_floor":0.04,"noise_threshold":0.12,"beyond_noise":null,"status":"paired"},
                    "unattributed_before":null,"unattributed_after":null,"terms":[],"warnings":[]}});
            let bytes = serde_json::to_vec(&report).expect("report");
            last_hash = hash(&bytes);
            fs::write(
                dir.path().join("objects").join(format!("{last_hash}.json")),
                bytes,
            )
            .expect("object");
            let row = Row {
                schema: SCHEMA.into(),
                report_sha256: last_hash.clone(),
                config_sha256: "config".into(),
                generated_at_unix: i * 86400,
                trial: None,
                samples: vec![],
            };
            index.push_str(&serde_json::to_string(&row).expect("row"));
            index.push('\n');
        }
        fs::write(index_path(dir.path()), index).expect("index");
        let result = onset_value(dir.path(), 60).expect("onset");
        assert_eq!(
            result["partitions"].as_array().expect("partitions").len(),
            2
        );
        assert_eq!(
            result["excluded_observations"]
                .as_array()
                .expect("excluded")
                .len(),
            2
        );
        let schema: Value = serde_json::from_slice(
            &fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../saccade-core/schemas/saccade-onset.v1.schema.json"),
            )
            .expect("schema file"),
        )
        .expect("schema");
        jsonschema::validator_for(&schema)
            .expect("validator")
            .validate(&result)
            .expect("populated onset validates");
        let active = result["partitions"]
            .as_array()
            .expect("partitions")
            .iter()
            .find(|p| p["status"] == "candidate_analysis")
            .expect("analysis");
        assert_eq!(
            active["analysis"]["evidence"]["candidates"][0]["first_changed"]["time"],
            15 * 86400
        );
        fs::write(
            dir.path().join("objects").join(format!("{last_hash}.json")),
            b"tampered",
        )
        .expect("tamper fixture");
        assert!(onset_value(dir.path(), 60).is_err());
    }

    #[test]
    fn onset_uses_capture_or_record_chronology_not_report_time() {
        for (capture_times, tied) in [(true, false), (false, false), (true, true)] {
            let dir = tempfile::tempdir().expect("store");
            fs::create_dir_all(dir.path().join("objects")).expect("objects");
            let context: Value = serde_json::from_str(include_str!(
                "../../saccade-core/tests/fixtures/perf/context.json"
            ))
            .expect("context");
            let mut rows = Vec::new();
            for i in 0..60u64 {
                let mut context = context.clone();
                context["sample_window"]["hash"] =
                    json!(saccade_core::evidence::canonical::Digest::of_bytes(
                        format!("window{i}").as_bytes()
                    ));
                let entries = if capture_times {
                    json!([{
                        "name":"frame.png","status":"pass","metric_used":"mean","threshold":0.01,
                        "value":0.0,"metrics":null,"properties":null,"paths":{},"error":null,
                        "capture_provenance":{"capture.timestamp":(if tied { 0 } else { i * 86400 }).to_string()}
                    }])
                } else {
                    json!([])
                };
                let report = json!({"schema":"saccade-report.v1","tool_version":"fixture","generated_at_unix":(60-i)*86400,
                    "config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},
                    "totals":{"pass":0,"fail":0,"error":0,"missing":0,"new":0,"total":0},"entries":entries,
                    "perf_diff":{"schema":"saccade-perf-diff.v1","history_identity":saccade_core::evidence::canonical::Digest::of_bytes(b"hardware"),"unit":"ms","noise_k":3.0,
                        "comparability":"qualified","noise_comparability":"qualified","context_after":context,
                        "frame":{"before":10.0,"after":if i < 30 {10.0} else {12.0},"delta":null,"delta_pct":null,"noise_floor":0.04,"noise_threshold":0.12,"beyond_noise":null,"status":"paired"},
                        "unattributed_before":null,"unattributed_after":null,"terms":[],"warnings":[]}});
                let bytes = serde_json::to_vec(&report).expect("report");
                let digest = hash(&bytes);
                fs::write(
                    dir.path().join("objects").join(format!("{digest}.json")),
                    bytes,
                )
                .expect("object");
                rows.push(Row {
                    schema: SCHEMA.into(),
                    report_sha256: digest,
                    config_sha256: "config".into(),
                    generated_at_unix: (60 - i) * 86400,
                    trial: None,
                    samples: vec![],
                });
            }
            if capture_times && !tied {
                rows.reverse();
            }
            let index = rows
                .iter()
                .map(|row| serde_json::to_string(row).expect("row") + "\n")
                .collect::<String>();
            fs::write(index_path(dir.path()), index).expect("index");
            let result = onset_value(dir.path(), 60).expect("onset");
            let evidence = &result["partitions"][0]["analysis"]["evidence"];
            assert_eq!(evidence["segment_ends"], json!([30, 60]), "{result}");
            assert_eq!(
                evidence["candidates"][0]["first_changed"]["time"],
                if tied {
                    0
                } else if capture_times {
                    30 * 86400
                } else {
                    30
                }
            );
        }
    }

    #[test]
    fn onset_output_has_shipped_schema() {
        let dir = tempfile::tempdir().expect("store");
        let result = onset_value(dir.path(), 60).expect("onset");
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../saccade-core/schemas/saccade-onset.v1.schema.json");
        let schema: Value =
            serde_json::from_slice(&fs::read(path).expect("shipped onset schema")).expect("schema");
        let validator = jsonschema::validator_for(&schema).expect("validator");
        validator.validate(&result).expect("onset validates");
    }

    #[test]
    fn record_is_content_addressed_and_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (baseline, capture, out, store) = (
            dir.path().join("baseline"),
            dir.path().join("capture"),
            dir.path().join("report"),
            dir.path().join("history"),
        );
        fs::create_dir_all(&baseline).expect("baseline dir");
        fs::create_dir_all(&capture).expect("capture dir");
        let image = image::RgbImage::from_pixel(32, 32, image::Rgb([70, 80, 90]));
        image.save(baseline.join("ui.png")).expect("baseline png");
        image.save(capture.join("ui.png")).expect("capture png");
        saccade_core::run::run(
            &baseline,
            &capture,
            &out,
            &saccade_core::config::RunConfig::default(),
        )
        .expect("comparison");
        let path = out.join(saccade_core::report::REPORT_FILE_NAME);
        assert_eq!(record(&path, &store).expect("record")["recorded"], true);
        assert_eq!(record(&path, &store).expect("dedupe")["recorded"], false);
        let rows = read_index(&store).expect("index");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].samples.len(), 1);
        let object = store
            .join("objects")
            .join(format!("{}.json", rows[0].report_sha256));
        assert_eq!(
            fs::read(object).expect("object"),
            fs::read(&path).expect("report")
        );
        let preexisting = dir.path().join("preexisting-history");
        fs::create_dir_all(preexisting.join("objects")).expect("objects");
        fs::copy(
            &path,
            preexisting
                .join("objects")
                .join(format!("{}.json", rows[0].report_sha256)),
        )
        .expect("preexisting object");
        assert_eq!(
            record(&path, &preexisting).expect("matching object")["recorded"],
            true
        );
        assert_eq!(
            read_index(&preexisting).expect("preexisting index").len(),
            1
        );
        let concurrent = dir.path().join("concurrent-history");
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let path = out.join(saccade_core::report::REPORT_FILE_NAME);
                let store = concurrent.clone();
                std::thread::spawn(move || record(&path, &store).expect("concurrent record"))
            })
            .collect();
        let recorded = handles
            .into_iter()
            .map(|h| h.join().expect("thread")["recorded"] == true)
            .filter(|v| *v)
            .count();
        assert_eq!(recorded, 1);
        assert_eq!(read_index(&concurrent).expect("concurrent index").len(), 1);
    }

    #[test]
    fn independent_stable_runs_and_anchor_drift_split_environments() {
        let mut rows = Vec::new();
        for i in 0..30 {
            rows.push(Row {
                schema: SCHEMA.into(),
                report_sha256: format!("report{i}"),
                config_sha256: "cfg".into(),
                generated_at_unix: 30 - i,
                trial: Some(Trial {
                    run_id: format!("run{i}"),
                    environment_id: "browser-fonts-warmup".into(),
                    unchanged_build: i < 10,
                }),
                samples: vec![Sample {
                    entry: "ui.png".into(),
                    baseline_sha256: "anchor".into(),
                    capture_sha256: if i < 10 {
                        "same".into()
                    } else {
                        format!("image{i}")
                    },
                    metric: Metric::Mean,
                    threshold: 0.5,
                    value: if i < 10 {
                        0.01
                    } else {
                        0.01 + (i - 9) as f64 * 0.001
                    },
                }],
            });
        }
        let stable = run_analysis(&rows[..10], None, 10, true);
        assert_eq!(stable["entries"][0]["independent_runs"], 10);
        assert_eq!(stable["entries"][0]["unique_image_hashes"], 1);
        let result = run_analysis(&rows, None, 10, true);
        assert_eq!(result["entries"][0]["drift"], "candidate"); // Recent drift is not diluted by a stable prefix.
        rows.drain(..5);
        let result = run_analysis(&rows, None, 10, true);
        assert_eq!(
            result["entries"][0]["drift"],
            "insufficient_unchanged_build_repeats"
        );
        // Ten stable repeats followed by a long monotone sub-threshold drift.
        for i in 30..50 {
            let mut row = rows.last().expect("row").clone();
            row.trial.as_mut().expect("trial").run_id = format!("run{i}");
            row.samples[0].value = i as f64 * 0.001;
            rows.push(row);
        }
        for row in &mut rows[..10] {
            row.trial.as_mut().expect("trial").unchanged_build = true;
            row.samples[0].value = 0.01;
        }
        assert_eq!(
            run_analysis(&rows, None, 10, true)["entries"][0]["drift"],
            "candidate"
        );
        let mut duplicate = rows[0].clone();
        duplicate.samples[0].value = 0.9;
        rows.push(duplicate);
        assert_eq!(
            run_analysis(&rows, None, 10, true)["entries"][0]["independent_runs"],
            45
        );
        rows.last_mut()
            .expect("row")
            .trial
            .as_mut()
            .expect("trial")
            .environment_id = "other-fonts".into();
        assert_eq!(run_analysis(&rows, None, 10, true)["groups"], 2);
    }

    #[test]
    fn distinct_capture_variation_crossing_threshold_is_flagged() {
        let mut rows = Vec::new();
        for (id, value) in [("a", 0.01), ("b", 0.03), ("c", 0.04)] {
            rows.push(Row {
                schema: SCHEMA.into(),
                report_sha256: id.into(),
                config_sha256: "cfg".into(),
                generated_at_unix: 0,
                trial: None,
                samples: vec![Sample {
                    entry: "ui.png".into(),
                    baseline_sha256: "base".into(),
                    capture_sha256: id.into(),
                    metric: Metric::Mean,
                    threshold: 0.02,
                    value,
                }],
            });
        }
        let result = analyze(&rows, None, 10);
        assert_eq!(result["entries"][0]["finding"], "flaky_threshold");
        assert_eq!(result["entries"][0]["distinct_captures"], 3);
        assert_eq!(result["entries"][0]["observed_min"], 0.01);
        assert_eq!(result["entries"][0]["observed_max"], 0.04);
        rows.push(rows[0].clone());
        assert_eq!(
            analyze(&rows, None, 10)["entries"][0]["distinct_captures"],
            3
        );
    }
}
