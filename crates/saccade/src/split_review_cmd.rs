//! Explicit dataset split and photo burst candidate review.
use crate::{agent::CliError, general_cmd};
use saccade_core::general::{input, split_review};
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct Args {
    /// saccade-split-manifest.v1; paths resolve relative to this file.
    manifest: PathBuf,
    /// Inclusive perceptual-hash distance, 0..64.
    #[arg(long, default_value = "6")]
    hash_threshold: u32,
    /// Explicitly enable a provisioned embedding route; never downloads.
    #[arg(long)]
    embeddings: bool,
    /// Inclusive raw cosine candidate threshold.
    #[arg(long, default_value = "0.95")]
    cosine_threshold: f64,
    /// New or empty output directory outside the dataset.
    #[arg(long, default_value = "split-review-report")]
    out: PathBuf,
    /// Emit the complete versioned review, including limitations.
    #[arg(long)]
    json: bool,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let manifest: split_review::Manifest =
        serde_json::from_slice(&input::bytes(&args.manifest, 1024 * 1024)?)?;
    let root = args
        .manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let paths = manifest.resolve(root)?;
    let mut inputs: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    inputs.push(root);
    general_cmd::prepare_out(&args.out, &inputs)?;
    let report = if args.embeddings {
        #[cfg(feature = "embeddings")]
        {
            let cfg = crate::wave7_cmd::config()?;
            let contract = cfg.embedding_contract.as_ref().ok_or_else(|| {
                CliError::usage(
                    "embedding contract not configured; provision with models pull embedding",
                )
            })?;
            let library = cfg
                .runtime_library
                .as_ref()
                .ok_or_else(|| CliError::usage("ONNX runtime library not configured"))?;
            let model = saccade_core::general::embedding::parse_model(&input::bytes(
                contract,
                2 * 1024 * 1024,
            )?)?;
            let identity = saccade_core::localized::digest(&serde_json::to_vec(&model)?);
            let engine =
                saccade_core::general::embedding::Engine::load(model, &cfg.dir, library, false)?;
            let mut route = Route { engine, identity };
            split_review::review(
                &manifest,
                root,
                args.hash_threshold,
                args.cosine_threshold,
                Some(&mut route),
            )?
        }
        #[cfg(not(feature = "embeddings"))]
        {
            return Err(CliError::new(
                "feature_unavailable",
                "split-review --embeddings requires the embeddings feature and a provisioned model",
            ));
        }
    } else {
        split_review::review(
            &manifest,
            root,
            args.hash_threshold,
            args.cosine_threshold,
            None,
        )?
    };
    let file = general_cmd::persist_document(&report, &args.out)?;
    std::fs::write(args.out.join("pairs.csv"), split_review::csv(&report)?)
        .map_err(|e| CliError::io(e.to_string()))?;
    if args.json {
        crate::emit(&format!(
            "{}\n",
            saccade_core::report_links::decorate(&report)?
        ))?;
    } else {
        crate::emit(&format!(
            "{}\nRoutes: hash, geometric; embedding {}. Read limitations in {}.\n",
            report["summary"].as_str().unwrap_or("Review complete"),
            if args.embeddings {
                "used"
            } else {
                "not enabled"
            },
            file.display()
        ))?;
    }
    Ok(u8::from(report["verdict"] == "review_required"))
}
#[cfg(feature = "embeddings")]
struct Route {
    engine: saccade_core::general::embedding::Engine,
    identity: String,
}
#[cfg(feature = "embeddings")]
impl split_review::EmbeddingRoute for Route {
    fn identity(&self) -> &str {
        &self.identity
    }
    fn embed(&mut self, image: &image::RgbaImage) -> saccade_core::Result<Vec<f32>> {
        self.engine.embed(image)
    }
}
