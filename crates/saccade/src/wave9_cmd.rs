//! Local rendering evidence commands, without providers or implicit inspection.
use crate::agent::CliError;
use saccade_core::evidence_quality::{self as eq, reference, trial};
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct ReferenceArgs {
    #[command(flatten)]
    pub strict_arms: crate::arms_cmd::StrictArgs,
    /// Intended metadata variables, exact paths, prefixes or suffixes.
    #[arg(long = "intended-variable")]
    pub intended_variables: Vec<String>,
    #[arg(long)]
    pub config: Option<PathBuf>,
    pub render: PathBuf,
    pub reference: PathBuf,
    /// Additional independent reference seed images.
    #[arg(long = "seed-reference", conflicts_with = "variance")]
    pub seeds: Vec<PathBuf>,
    /// Native scalar image of sample-mean variance in linear luminance squared.
    #[arg(long)]
    pub variance: Option<PathBuf>,
    /// White pixels include the reference/fit scope.
    #[arg(long)]
    pub mask: Option<PathBuf>,
    #[arg(long)]
    pub policy: Option<PathBuf>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}
fn load(
    path: &Path,
    hashes: &mut std::collections::BTreeMap<String, String>,
) -> Result<image::DynamicImage, CliError> {
    let bytes = eq::read(path, 128 << 20)?;
    let image = eq::decode(&bytes, path)?;
    hashes.insert(
        path.to_string_lossy().into_owned(),
        saccade_core::localized::digest(&bytes),
    );
    Ok(image)
}
pub(crate) fn reference_value(args: &ReferenceArgs) -> Result<reference::Report, CliError> {
    if args.seeds.len() > 31 || (!args.seeds.is_empty() && args.variance.is_some()) {
        return Err(CliError::usage(
            "use at most 31 extra seeds, or a variance map",
        ));
    }
    let mut cfg = crate::load_config(args.config.as_deref())?;
    args.strict_arms.apply(&mut cfg.meta);
    cfg.meta
        .intended
        .extend(args.intended_variables.iter().cloned());
    saccade_core::arms::enforce(&args.reference, &args.render, &cfg)?;
    for seed in &args.seeds {
        saccade_core::arms::enforce(&args.reference, seed, &cfg)?;
    }
    let mut hashes = Default::default();
    let render = load(&args.render, &mut hashes)?;
    let reference_image = load(&args.reference, &mut hashes)?;
    let pixels = u64::from(reference_image.width()) * u64::from(reference_image.height());
    if pixels.saturating_mul(args.seeds.len() as u64 + 2) > 16_777_216 {
        return Err(CliError::usage("reference aggregate pixel budget exceeded"));
    }
    let seeds = args
        .seeds
        .iter()
        .map(|p| load(p, &mut hashes))
        .collect::<Result<Vec<_>, _>>()?;
    let variance = args
        .variance
        .as_ref()
        .map(|p| -> Result<Vec<f64>, CliError> {
            let img = load(p, &mut hashes)?;
            if (img.width(), img.height()) != (reference_image.width(), reference_image.height()) {
                return Err(CliError::usage("variance map dimensions differ"));
            }
            // Float scalar maps preserve native radiance units; integer maps are normalized.
            Ok(img.to_rgb32f().pixels().map(|p| f64::from(p[0])).collect())
        })
        .transpose()?;
    let excluded = args
        .mask
        .as_ref()
        .map(|p| -> Result<Vec<bool>, CliError> {
            let img = load(p, &mut hashes)?.to_luma8();
            if img.dimensions() != (reference_image.width(), reference_image.height()) {
                return Err(CliError::usage("mask dimensions differ"));
            }
            Ok(img.pixels().map(|p| p[0] == 0).collect())
        })
        .transpose()?;
    let policy = if let Some(p) = &args.policy {
        let bytes = eq::read(p, 1 << 20)?;
        hashes.insert(
            p.to_string_lossy().into_owned(),
            saccade_core::localized::digest(&bytes),
        );
        serde_json::from_slice(&bytes)?
    } else {
        reference::Policy::default()
    };
    let noise = if let Some(v) = &variance {
        reference::Noise::Variance(v)
    } else if seeds.is_empty() {
        reference::Noise::Estimate
    } else {
        reference::Noise::Seeds(&seeds)
    };
    let mut report = reference::compare(
        &render,
        &reference_image,
        noise,
        excluded.as_deref(),
        &policy,
        &Default::default(),
    )?;
    if cfg.meta.require_valid_arms {
        report.arm_validation = Some(saccade_core::arms::check_paths(
            &args.reference,
            &args.render,
            &cfg.meta,
        )?);
    }
    report.input_sha256 = hashes;
    Ok(report)
}
fn emit<T: serde::Serialize>(value: &T, json: bool) -> Result<(), CliError> {
    let text = if json {
        serde_json::to_string(value)?
    } else {
        serde_json::to_string_pretty(value)?
    };
    crate::emit(&format!("{text}\n"))
}
pub(crate) fn reference(args: ReferenceArgs) -> Result<u8, CliError> {
    let report = reference_value(&args)?;
    if let Some(p) = &args.out {
        // Never overwrite the captured input or a prior evidence artifact.
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(p)
            .map_err(|e| CliError::io(e.to_string()))?;
        let linked = saccade_core::report_links::decorate(&serde_json::to_value(&report)?)?;
        f.write_all(&serde_json::to_vec_pretty(&linked)?)
            .map_err(|e| CliError::io(e.to_string()))?;
        saccade_core::report_links::index(p, &linked)?;
    }
    emit(&report, args.json)?;
    Ok(if report.verdict == "within_noise_floor" {
        0
    } else {
        1
    })
}
#[derive(clap::Args)]
pub(crate) struct TrialArgs {
    #[command(subcommand)]
    pub operation: TrialOperation,
}
#[derive(clap::Subcommand)]
pub(crate) enum TrialOperation {
    /// Hash a plan and all inputs before decoding or showing images.
    Register {
        plan: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Lock inspection state and write the blind HTML gallery.
    Start {
        plan: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Persist one explicit judgment through the core vote store.
    Vote {
        plan: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        voter: String,
        #[arg(long)]
        item: String,
        #[arg(long)]
        answer: String,
    },
    /// Import the exported blind gallery judgments for an explicit voter.
    Import {
        plan: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        voter: String,
        judgments: PathBuf,
    },
}
pub(crate) fn trial(args: TrialArgs, json: bool) -> Result<u8, CliError> {
    let receipt = match args.operation {
        TrialOperation::Register { plan, out } => trial::register(&plan, &out)?,
        TrialOperation::Start { plan, out } => trial::start(&plan, &out)?,
        TrialOperation::Vote {
            plan,
            out,
            voter,
            item,
            answer,
        } => trial::vote(&plan, &out, &voter, &item, &answer)?,
        TrialOperation::Import {
            plan,
            out,
            voter,
            judgments,
        } => trial::import(&plan, &out, &voter, &eq::read(&judgments, 1 << 20)?)?,
    };
    emit(&receipt, json)?;
    Ok(0)
}
