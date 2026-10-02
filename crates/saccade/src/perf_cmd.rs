//! Run performance options and the ablation CLI.
use crate::agent::CliError;
use clap::Args;
use std::path::PathBuf;

#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum NoiseKind {
    Image,
    Performance,
}

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
    json: bool,
    absolute: bool,
) -> Result<u8, CliError> {
    #[cfg(not(feature = "graphics"))]
    {
        let _ = (dirs, out, opts, json, absolute);
        Err(saccade_core::Error::FeatureUnavailable {
            feature: "graphics",
        }
        .into())
    }
    #[cfg(feature = "graphics")]
    {
        let mut record = saccade_core::perf::noise_record(dirs, opts)?;
        for (source, dir) in record.sources.iter_mut().zip(dirs) {
            source.source = saccade_core::paths::record(
                &dir.join(&opts.name),
                out.parent().unwrap_or(std::path::Path::new(".")),
                absolute,
            );
        }
        let text = serde_json::to_string_pretty(&record)?;
        std::fs::write(out, format!("{text}\n")).map_err(|e| {
            CliError::io(format!(
                "writing {}: {e}",
                saccade_core::paths::portable(out)
            ))
        })?;
        if json {
            crate::emit(&format!("{text}\n"))?;
        } else {
            crate::emit(&format!(
                "performance noise (ms): frame {:.6}; qualification {:?}\nwrote {}\n",
                record.perf_noise.frame,
                record.comparability,
                saccade_core::paths::cwd(out, absolute)
            ))?;
        }
        Ok(0)
    }
}

#[derive(Args)]
pub(crate) struct PerfArgs {
    /// Run performance sidecar file name (default saccade-perf.json).
    #[arg(long)]
    pub perf_name: Option<String>,
    /// Noise JSON or TOML from unchanged-build repeats.
    #[arg(long)]
    pub perf_noise: Option<PathBuf>,
    /// Repeat spread multiplier in the effective noise threshold (default 3).
    #[arg(long)]
    pub perf_noise_k: Option<f64>,
    /// Timer quantum in ms; overrides the estimate from repeated captures.
    #[arg(long)]
    pub perf_resolution: Option<f64>,
    /// Minimum timer ticks in the noise threshold (default 2).
    #[arg(long)]
    pub perf_resolution_ticks: Option<u32>,
    /// Minimum meaningful delta in ms (default 0.05).
    #[arg(long)]
    pub perf_min_delta_ms: Option<f64>,
    /// Minimum meaningful delta as a percentage of the baseline frame (default 0.5).
    #[arg(long)]
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
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&model)?))?;
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
