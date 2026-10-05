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
        let row: Row = serde_json::from_str(&line)
            .map_err(|e| CliError::io(format!("history index line {}: {e}", i + 1)))?;
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

fn record(report_path: &Path, store: &Path) -> Result<Value, CliError> {
    let report = crate::read_report(report_path)?;
    if report.config.mode != Mode::Regression {
        return Err(CliError::usage(
            "history accepts comparison reports, not identity proofs",
        ));
    }
    let bytes = fs::read(report_path)
        .map_err(|e| CliError::io(format!("{}: {e}", report_path.display())))?;
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
    if existing.iter().any(|r| r.report_sha256 == report_hash) {
        return Ok(
            json!({"schema":SCHEMA,"operation":"record","recorded":false,"reason":"report_already_recorded","report_sha256":report_hash}),
        );
    }
    let samples: Vec<_> = report
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
        .collect();
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
    let row = Row {
        schema: SCHEMA.into(),
        report_sha256: report_hash.clone(),
        config_sha256: config_hash,
        generated_at_unix: report.generated_at_unix,
        samples,
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

fn performance_observation(
    report: &saccade_core::Report,
    run: &str,
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
        time: report.generated_at_unix,
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
    let mut groups: BTreeMap<String, Vec<saccade_core::onset::Observation>> = BTreeMap::new();
    let mut omitted = Vec::new();
    for row in rows {
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
        let report: saccade_core::Report = serde_json::from_slice(&bytes)?;
        match performance_observation(&report, &row.report_sha256) {
            Ok(o) => groups.entry(o.identity.clone()).or_default().push(o),
            Err(reason) => omitted.push(json!({"run":row.report_sha256,"reason":reason})),
        }
    }
    let mut partitions = Vec::new();
    for (identity, mut observations) in groups {
        observations.sort_by(|a, b| a.time.cmp(&b.time).then(a.run.cmp(&b.run)));
        let mut windows = std::collections::BTreeSet::new();
        observations.retain(|o| {
            if windows.insert(o.window.clone()) { true } else {
                omitted.push(json!({"run":o.run,"reason":"same_sample_window_not_an_independent_observation"})); false
            }
        });
        let truncated = observations.len().saturating_sub(limit);
        observations.drain(..truncated);
        if observations.len() < 10 {
            partitions.push(json!({"identity":identity,"status":"insufficient_history","observations":observations,"older_observations_omitted":truncated}));
        } else {
            let analysis = saccade_core::onset::detect(&observations)?;
            partitions.push(json!({"identity":identity,"status":"candidate_analysis","analysis":analysis,"older_observations_omitted":truncated}));
        }
    }
    Ok(
        json!({"schema":"saccade-onset.v1","operation":"onset","partitions":partitions,"excluded_observations":omitted,"limits":["Candidate intervals only; fresh qualified repeats are required. Never commit blame.","One record must represent an independent nightly statistic; timestamps do not establish independence. Missing observations are retained as gaps, not interpolated."]}),
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
        } => (record(&report, &store)?, json),
        HistoryOperation::Analyze {
            store,
            entry,
            limit,
            json,
        } => {
            let rows = read_index(&store)?;
            (analyze(&rows, entry.as_deref(), limit as usize), json)
        }
    };
    if json_output {
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&crate::local_cmd::bounded(value, 4096)?)?
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
        let mut out = format!("history: {} comparable groups\n", value["groups"]);
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
                "totals":{"pass":0,"fail":0,"error":0,"missing":0,"new":0,"total":0},"entries":[],
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
    fn distinct_capture_variation_crossing_threshold_is_flagged() {
        let mut rows = Vec::new();
        for (id, value) in [("a", 0.01), ("b", 0.03), ("c", 0.04)] {
            rows.push(Row {
                schema: SCHEMA.into(),
                report_sha256: id.into(),
                config_sha256: "cfg".into(),
                generated_at_unix: 0,
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
