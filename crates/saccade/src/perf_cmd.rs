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
    base: PathBuf,
    #[arg(required = true, num_args = 1..)]
    arms: Vec<PathBuf>,
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
    let model = saccade_core::ablate::run(&args.base, &args.arms, &args.out, &cfg, args.top)?;
    if args.json {
        let mut value =
            crate::local_cmd::analysis_result(&serde_json::to_value(&model)?, &args.out)?;
        value["data"] = serde_json::json!({"arms":model.arms.iter().map(|arm| serde_json::json!({"label":arm.label,"flag":arm.flag,"reasons":arm.perf_diff.as_ref().map(|d|{let mut reasons=d.qualification_reasons.clone(); if d.noise_comparability != saccade_core::perf::Comparability::Qualified {reasons.push("repeat noise unavailable".into());} reasons}).unwrap_or_default(),"comparability":arm.perf_diff.as_ref().map(|d|d.comparability),"noise_comparability":arm.perf_diff.as_ref().map(|d|d.noise_comparability),"actions":if arm.flag == "INCONCLUSIVE" {vec!["record and qualify warmup on both captures", "recapture both arms with matching configuration", "capture unchanged-build repeats; supply --perf-noise FILE"]} else {Vec::new()}})).collect::<Vec<_>>()});
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
