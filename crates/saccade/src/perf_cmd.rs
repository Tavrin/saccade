//! Run performance options and the ablation CLI.
use crate::agent::CliError;
use clap::Args;
use std::path::PathBuf;

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
        if let Some(n) = &self.perf_name {
            opts.name.clone_from(n);
        }
        if let Some(n) = &self.perf_noise {
            opts.noise = Some(n.clone());
        }
        if let Some(k) = self.perf_noise_k {
            opts.k = k;
        }
        if let Some(v) = self.perf_resolution {
            opts.resolution_ms = Some(v);
        }
        if let Some(v) = self.perf_resolution_ticks {
            opts.resolution_ticks = Some(v);
        }
        if let Some(v) = self.perf_min_delta_ms {
            opts.min_delta_ms = Some(v);
        }
        if let Some(v) = self.perf_min_delta_pct {
            opts.min_delta_pct = Some(v);
        }
        opts.validate()?;
        Ok(())
    }
}
#[derive(Args)]
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
