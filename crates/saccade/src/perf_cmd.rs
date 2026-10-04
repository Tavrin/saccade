//! Run performance options and the ablation CLI.
use crate::agent::CliError;
use clap::Args;
use std::path::PathBuf;

pub(crate) fn check_image_noise_config(path: &std::path::Path) -> Result<(), CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        CliError::io(format!(
            "reading {}: {e}",
            saccade_core::paths::portable(path)
        ))
    })?;
    if saccade_core::perf::is_performance_noise_document(&text) {
        return Err(saccade_core::perf::wrong_noise_kind(
            "image",
            "noise BASE REPEAT... --kind image",
        )
        .into());
    }
    Ok(())
}

pub(crate) fn noise(
    dirs: &[PathBuf],
    out: &std::path::Path,
    opts: &saccade_core::perf::PerfOptions,
    absolute: bool,
) -> Result<serde_json::Value, CliError> {
    #[cfg(not(feature = "graphics"))]
    {
        let _ = (dirs, out, opts, absolute);
        Err(saccade_core::Error::FeatureUnavailable {
            feature: "graphics",
        }
        .into())
    }
    #[cfg(feature = "graphics")]
    {
        let mut record = saccade_core::perf::noise_record(dirs, opts)?;
        for reason in &record.reasons {
            if reason.contains("provenance is absent") {
                eprintln!("saccade noise: {}", crate::escape_control(reason));
            }
        }
        for (source, dir) in record.sources.iter_mut().zip(dirs) {
            source.source = saccade_core::paths::record(
                &dir.join(&opts.name),
                out.parent().unwrap_or(std::path::Path::new(".")),
                absolute,
            );
        }
        crate::local_cmd::write_value(out, &serde_json::to_value(&record)?)?;
        let mut value = crate::local_cmd::base_result("noise.performance");
        value["artifact"] = crate::local_cmd::reference(out)?;
        value["counts"] = serde_json::json!({"runs":dirs.len()});
        value["data"] = serde_json::json!({
            "kind":record.kind,"unit":record.unit,"comparability":record.comparability
        });
        value["limits"] = serde_json::json!(
            record
                .reasons
                .iter()
                .take(3)
                .map(|reason| crate::local_cmd::short(reason, 200))
                .collect::<Vec<_>>()
        );
        Ok(value)
    }
}

#[derive(Args)]
#[command(next_help_heading = "Performance")]
pub(crate) struct PerfArgs {
    /// Run performance sidecar file name (default saccade-perf.json).
    #[arg(long, value_name = "NAME")]
    pub perf_name: Option<String>,
    /// Noise JSON or TOML from unchanged-build repeats.
    #[arg(long, value_name = "FILE")]
    pub perf_noise: Option<PathBuf>,
    /// Repeat spread multiplier in the effective noise threshold (default 3).
    #[arg(long, value_name = "K")]
    pub perf_noise_k: Option<f64>,
    /// Timer quantum in ms; overrides the estimate from repeated captures.
    #[arg(long, value_name = "MS")]
    pub perf_resolution: Option<f64>,
    /// Minimum timer ticks in the noise threshold (default 2).
    #[arg(long, value_name = "N")]
    pub perf_resolution_ticks: Option<u32>,
    /// Minimum meaningful delta in ms (default 0.05).
    #[arg(long, value_name = "MS")]
    pub perf_min_delta_ms: Option<f64>,
    /// Minimum meaningful delta as a percentage of the baseline frame (default 0.5).
    #[arg(long, value_name = "PCT")]
    pub perf_min_delta_pct: Option<f64>,
}
impl PerfArgs {
    pub fn apply(&self, opts: &mut saccade_core::perf::PerfOptions) -> Result<(), CliError> {
        #[cfg(not(feature = "graphics"))]
        if self.perf_name.is_some()
            || self.perf_noise.is_some()
            || self.perf_noise_k.is_some()
            || self.perf_resolution.is_some()
            || self.perf_resolution_ticks.is_some()
            || self.perf_min_delta_ms.is_some()
            || self.perf_min_delta_pct.is_some()
        {
            return Err(saccade_core::Error::FeatureUnavailable {
                feature: "graphics",
            }
            .into());
        }
        if let Some(n) = &self.perf_name {
            opts.name.clone_from(n);
        }
        if let Some(n) = &self.perf_noise {
            opts.noise = Some(n.clone());
        }
        if let Some(k) = self.perf_noise_k {
            opts.k = k;
            opts.policy_sources.insert("noise_k".into(), "cli".into());
        }
        if let Some(v) = self.perf_resolution {
            opts.resolution_ms = Some(v);
            opts.policy_sources
                .insert("resolution_ms".into(), "cli".into());
        }
        if let Some(v) = self.perf_resolution_ticks {
            opts.resolution_ticks = Some(v);
            opts.policy_sources
                .insert("resolution_ticks".into(), "cli".into());
        }
        if let Some(v) = self.perf_min_delta_ms {
            opts.min_delta_ms = Some(v);
            opts.policy_sources
                .insert("min_delta_ms".into(), "cli".into());
        }
        if let Some(v) = self.perf_min_delta_pct {
            opts.min_delta_pct = Some(v);
            opts.policy_sources
                .insert("min_delta_pct".into(), "cli".into());
        }
        opts.validate()?;
        Ok(())
    }
}
#[derive(Args)]
#[cfg(feature = "graphics")]
pub(crate) struct AblateArgs {
    base: Option<PathBuf>,
    arms: Vec<PathBuf>,
    /// Base repeat directories. Accepts a directory or a quoted glob; repeatable.
    #[arg(long = "base", num_args = 1.., action = clap::ArgAction::Append, value_name = "RUN_DIR")]
    repeat_bases: Vec<String>,
    /// Labelled arm repeats, e.g. --arm 's2=s2_r*'; repeatable.
    #[arg(long = "arm", action = clap::ArgAction::Append, value_name = "LABEL=RUN_GLOB")]
    repeat_arms: Vec<String>,
    #[arg(long, default_value = "ablation")]
    pub out: PathBuf,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    /// Per-term deltas beyond noise to show per arm.
    #[arg(long, default_value_t = 5)]
    top: usize,
    #[command(flatten)]
    perf: PerfArgs,
}
#[cfg(feature = "graphics")]
pub(crate) fn ablate(args: AblateArgs, absolute: bool) -> Result<u8, CliError> {
    let mut cfg = crate::load_config(args.config.as_deref())?;
    cfg.record_absolute_paths = absolute;
    args.perf.apply(&mut cfg.perf)?;
    let model = if args.repeat_bases.is_empty() && args.repeat_arms.is_empty() {
        let base = args.base.as_ref().ok_or_else(|| {
            saccade_core::Error::Config(
                "ablate requires BASE and at least one ARM, or --base and --arm repeats".into(),
            )
        })?;
        if args.arms.is_empty() {
            return Err(
                saccade_core::Error::Config("ablate requires at least one arm".into()).into(),
            );
        }
        saccade_core::ablate::run(base, &args.arms, &args.out, &cfg, args.top)?
    } else {
        if args.base.is_some() || !args.arms.is_empty() {
            return Err(saccade_core::Error::Config(
                "use positional BASE ARM... or --base/--arm repeats, not both".into(),
            )
            .into());
        }
        let bases = expand_repeats(&args.repeat_bases)?;
        let mut groups: Vec<(String, Vec<PathBuf>)> = Vec::new();
        for spec in &args.repeat_arms {
            let (label, pattern) = spec.split_once('=').ok_or_else(|| {
                saccade_core::Error::Config("--arm expects LABEL=RUN_GLOB".into())
            })?;
            if label.is_empty() || pattern.is_empty() {
                return Err(saccade_core::Error::Config(
                    "--arm expects nonempty LABEL=RUN_GLOB".into(),
                )
                .into());
            }
            let paths = expand_repeats(&[pattern.to_string()])?;
            if let Some((_, existing)) = groups.iter_mut().find(|(name, _)| name == label) {
                existing.extend(paths);
            } else {
                groups.push((label.to_string(), paths));
            }
        }
        saccade_core::ablate::run_repeats(&bases, &groups, &args.out, &cfg, args.top)?
    };
    if args.json {
        let mut value =
            crate::local_cmd::analysis_result(&serde_json::to_value(&model)?, &args.out)?;
        value["data"] = serde_json::json!({"base_repeat_count":model.base_repeats.len(),"excluded_base_repeats":model.excluded_base_repeats,"base_stable":model.base_stability.as_ref().map(|s|s.stable),"repeat_qualification":model.repeat_qualification,"repeat_reasons":model.repeat_reasons.iter().filter(|r|r.contains("configuration_hash") || r.contains("not comparable")).take(5).collect::<Vec<_>>(),"arms":model.arms.iter().map(|arm| serde_json::json!({"label":arm.label,"flag":arm.flag,"repeat_count":arm.repeats.len(),"stable":arm.repeat_stability.as_ref().map(|s|s.stable),"max_flip_by_image":arm.repeat_stability.as_ref().map(|s|&s.max_flip_by_image),"validity_findings":arm.validity_findings,"next_actions":arm.next_actions,"excluded_repeats":arm.excluded_repeats,"reasons":arm.perf_diff.as_ref().map(|d|d.qualification_reasons.iter().filter(|r|r.contains("configuration_hash") || r.contains("gpu clock")).take(3).collect::<Vec<_>>()).unwrap_or_default(),"comparability":arm.perf_diff.as_ref().map(|d|d.comparability),"noise_comparability":arm.perf_diff.as_ref().map(|d|d.noise_comparability)})).collect::<Vec<_>>()});
        if model.arms.iter().any(|a| !a.validity_findings.is_empty()) {
            value["validity"] = serde_json::json!("invalid");
            value["validity_reasons"] =
                serde_json::json!(["arm output is not deterministic across repeats"]);
            value["next_actions"] = serde_json::json!([{"id":"repair-arm-repeats","kind":"repair_capture","reason_code":"arm_output_nondeterministic","priority":1,"requires":[],"arguments":{"artifact":value["artifact"]},"cli_argv":[],"expected_case_id":value["artifact"]["sha256"]}]);
        }
        crate::local_cmd::print(&value, true)?;
    } else {
        crate::emit(&model.text())?;
        crate::emit(&format!(
            "wrote {}\n",
            saccade_core::paths::cwd(&args.out.join("index.html"), absolute)
        ))?;
    }
    Ok(if model.arms.iter().any(|a| !a.errors.is_empty()) {
        2
    } else {
        0
    })
}

#[cfg(feature = "graphics")]
fn expand_repeats(patterns: &[String]) -> Result<Vec<PathBuf>, CliError> {
    let mut found = Vec::new();
    for pattern in patterns {
        let path = std::path::Path::new(pattern);
        if !pattern.contains(['*', '?', '[']) {
            found.push(path.to_path_buf());
            continue;
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let name = path.file_name().and_then(|s| s.to_str()).ok_or_else(|| {
            saccade_core::Error::Config(format!("invalid repeat glob {pattern:?}"))
        })?;
        let matcher = globset::Glob::new(name)
            .map_err(|e| {
                saccade_core::Error::Config(format!("invalid repeat glob {pattern:?}: {e}"))
            })?
            .compile_matcher();
        let entries = std::fs::read_dir(parent)
            .map_err(|e| CliError::io(format!("reading {}: {e}", parent.display())))?;
        let mut matches = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| matcher.is_match(n)))
            .collect::<Vec<_>>();
        matches.sort();
        if matches.is_empty() {
            return Err(saccade_core::Error::Config(format!(
                "repeat glob {pattern:?} matched no paths"
            ))
            .into());
        }
        found.extend(matches);
    }
    found.sort();
    found.dedup();
    Ok(found)
}
