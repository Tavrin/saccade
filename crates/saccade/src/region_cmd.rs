//! Model-independent phrase/mask import and optional model cache/runtime plumbing.
use crate::agent::CliError;
use image::GenericImageView;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(clap::Subcommand)]
enum Operation {
    /// Freeze a manually accepted phrase region from an imported inclusion mask.
    Import {
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        phrase: String,
        /// New frozen-region JSON file; use it with localized-check --region.
        #[arg(long)]
        out: PathBuf,
    },
    /// Report honest text-to-mask and import capabilities without loading models.
    Status,
    /// Explicitly download hash-pinned model artifacts into a local cache.
    #[cfg(feature = "semantic-regions")]
    Cache {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        cache: PathBuf,
    },
    /// Load self-contained ONNX graphs; graph loading does not qualify inference/parity.
    #[cfg(feature = "semantic-regions")]
    RuntimeProbe {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        cache: PathBuf,
        #[arg(long)]
        library: PathBuf,
    },
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    match args.operation {
        Operation::Status => {
            crate::emit(&format!(
                "{}\n",
                serde_json::json!({"mask_import":"available","semantic_regions_feature":cfg!(feature="semantic-regions"),"text_to_mask":"unavailable","reason":"checkpoint-specific detector/SAM 2.1 ONNX export and source parity remain unqualified; import a reviewed inclusion mask","runtime":"ONNX Runtime 1.22 via ort 2.0.0-rc.10"})
            ))?;
        }
        Operation::Import {
            reference,
            mask,
            phrase,
            out,
        } => {
            if phrase.trim().is_empty() {
                return Err(CliError::usage("region phrase must be nonempty"));
            }
            let reference_bytes =
                std::fs::read(&reference).map_err(|e| CliError::io(e.to_string()))?;
            let image = image::load_from_memory(&reference_bytes)
                .map_err(|e| CliError::usage(e.to_string()))?;
            let mask_bytes = std::fs::read(mask).map_err(|e| CliError::io(e.to_string()))?;
            let mask =
                image::load_from_memory(&mask_bytes).map_err(|e| CliError::usage(e.to_string()))?;
            if mask.color() != image::ColorType::L8
                || mask.dimensions() != image.dimensions()
                || mask.as_bytes().iter().any(|&v| v != 0 && v != 255)
            {
                return Err(CliError::usage(
                    "import needs binary L8 inclusion mask with reference dimensions",
                ));
            }
            let region = saccade_core::localized::freeze(
                saccade_core::localized::digest(&reference_bytes),
                [image.width(), image.height()],
                mask.as_bytes()
                    .iter()
                    .map(|&v| u8::from(v == 255))
                    .collect(),
                BTreeMap::from([
                    ("method".into(), "phrase_mask_import".into()),
                    ("original_phrase".into(), phrase),
                    ("selection_status".into(), "explicit_import".into()),
                    (
                        "source_mask_sha256".into(),
                        saccade_core::localized::digest(&mask_bytes),
                    ),
                    ("model_inference".into(), "not_run".into()),
                ]),
            )?;
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out)
                .map_err(|e| CliError::io(e.to_string()))?;
            serde_json::to_writer_pretty(file, &region)?;
            crate::emit(&format!(
                "frozen inclusion region {}; model inference not run\n",
                region.mask_sha256
            ))?;
        }
        #[cfg(feature = "semantic-regions")]
        Operation::Cache { manifest, cache } => {
            let model = crate::parse_contract(
                &std::fs::read(manifest).map_err(|e| CliError::io(e.to_string()))?,
                "saccade-region-models.v1",
            )?;
            let paths = saccade_core::semantic::cache_models(&model, &cache)?;
            crate::emit(&format!(
                "{} hash-verified artifacts cached; inference/parity remain unqualified\n",
                paths.len()
            ))?;
        }
        #[cfg(feature = "semantic-regions")]
        Operation::RuntimeProbe {
            manifest,
            cache,
            library,
        } => {
            let model = crate::parse_contract(
                &std::fs::read(manifest).map_err(|e| CliError::io(e.to_string()))?,
                "saccade-region-models.v1",
            )?;
            let loaded = saccade_core::semantic::probe_runtime(&model, &cache, &library)?;
            crate::emit(&format!(
                "{} graphs loaded; source parity and text-to-mask inference remain unqualified\n",
                loaded.len()
            ))?;
        }
    }
    Ok(0)
}
