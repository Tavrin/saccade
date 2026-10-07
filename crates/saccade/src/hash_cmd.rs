//! Perceptual hashing and near-duplicate commands.
use crate::{agent::CliError, general_cmd};
use saccade_core::general::{hashing, input};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct HashArgs {
    /// Files or directories; each unique input is decoded once.
    #[arg(required=true,num_args=1..)]
    files: Vec<PathBuf>,
    /// New or empty output directory.
    #[arg(long, default_value = "hash-report")]
    out: PathBuf,
    /// Emit a bounded JSON artifact receipt.
    #[arg(long)]
    json: bool,
}
#[derive(Clone, Copy, clap::ValueEnum, Default)]
pub(crate) enum Algorithm {
    Ahash,
    Dhash,
    #[default]
    Phash,
}
impl From<Algorithm> for hashing::Algorithm {
    fn from(value: Algorithm) -> Self {
        match value {
            Algorithm::Ahash => Self::Ahash,
            Algorithm::Dhash => Self::Dhash,
            Algorithm::Phash => Self::Phash,
        }
    }
}
#[derive(clap::Args)]
pub(crate) struct DedupeArgs {
    /// Directory of images; never deletes originals.
    dir: PathBuf,
    /// Algorithm for the Hamming index.
    #[arg(long, value_enum, default_value = "phash")]
    algorithm: Algorithm,
    /// Largest perceptual-hash distance in bits, 0-64 (0 = identical hashes; larger = looser); clusters use transitive connectivity.
    #[arg(long, default_value = "6")]
    threshold: u32,
    /// New or empty output directory.
    #[arg(long, default_value = "dedupe-report")]
    out: PathBuf,
    /// Emit a bounded JSON artifact receipt.
    #[arg(long)]
    json: bool,
}
pub(crate) fn run_hash(args: HashArgs) -> Result<u8, CliError> {
    let document = measure(&args.files, &args.out, None)?;
    general_cmd::emit_document(document, Some(&args.out), args.json)
}
pub(crate) fn run_dedupe(args: DedupeArgs) -> Result<u8, CliError> {
    let document = measure(
        &[args.dir],
        &args.out,
        Some((args.algorithm.into(), args.threshold)),
    )?;
    general_cmd::emit_document(document, Some(&args.out), args.json)
}
pub(crate) fn measure(
    inputs: &[PathBuf],
    out: &Path,
    dedupe: Option<(hashing::Algorithm, u32)>,
) -> Result<Value, CliError> {
    if inputs.is_empty() || inputs.len() > 100000 || dedupe.is_some_and(|(_, t)| t > 64) {
        return Err(CliError::usage(
            "hash requires inputs, <=100000 images and a Hamming radius in 0..64",
        ));
    }
    let mut files = std::collections::BTreeSet::new();
    let mut path_bytes = 0usize;
    for input in inputs {
        let candidates = if input.is_dir() {
            input::files(input, 100000)?
        } else {
            vec![input.clone()]
        };
        for path in candidates {
            let canonical = saccade_core::paths::canonicalize(&path)
                .map_err(|e| CliError::io(e.to_string()))?;
            if !files.contains(&canonical) {
                path_bytes = path_bytes.saturating_add(canonical.as_os_str().len());
                if files.len() >= 100000 || path_bytes > 64 * 1024 * 1024 {
                    return Err(CliError::usage(
                        "hash index input memory/count limit exceeded",
                    ));
                }
                files.insert(canonical);
            }
        }
    }
    general_cmd::prepare_out(
        out,
        &inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
    )?;
    let mut entries = Vec::new();
    let mut hashes = Vec::new();
    let mut errors = Vec::new();
    for file in files {
        let name = saccade_core::paths::cwd(&file, false);
        let measured = (|| -> Result<_, CliError> {
            let bytes = input::bytes(&file, input::MAX_BYTES)?;
            let image = input::decode(&bytes)?;
            let h = hashing::hash(&image);
            Ok((
                h,
                json!({"path":name,"encoded_sha256":saccade_core::localized::digest(&bytes),"dimensions":[image.width(),image.height()],"ahash":format!("{:016x}",h.ahash),"dhash":format!("{:016x}",h.dhash),"phash":format!("{:016x}",h.phash)}),
            ))
        })();
        match measured {
            Ok((h, v)) => {
                entries.push(v);
                hashes.push(h);
            }
            Err(e) => errors.push(json!({"path":name,"code":e.code,"message":e.message})),
        }
    }
    let limitations = [
        "perceptual hashes are candidate retrieval, not semantic identity",
        "hash collisions and flat-image matches need visual review",
        "all algorithms composite transparency over white; hash version is tied to preprocessing",
        "100000 image limit; BK-tree search can degrade on adversarial hash distributions",
    ];
    let schema = if dedupe.is_some() {
        hashing::DEDUPE_SCHEMA
    } else {
        hashing::HASH_SCHEMA
    };
    let mut value = json!({"schema":schema,"operation":if dedupe.is_some(){"dedupe"}else{"hash"},"verdict":if errors.is_empty()&&!entries.is_empty(){"pass"}else{"regression"},"counts":{"images":entries.len(),"errors":errors.len()},"preprocessing":"saccade-hash-white-triangle-srgb-luma-v1; phash DC bit zero","entries":entries,"errors":errors,"limitations":limitations});
    if let Some((algorithm, threshold)) = dedupe {
        let values: Vec<_> = hashes.iter().map(|h| h.get(algorithm)).collect();
        let clusters = hashing::clusters(&values, threshold)?;
        let duplicates = clusters.iter().filter(|c| c.len() > 1).count();
        value["algorithm"] = serde_json::to_value(algorithm)?;
        value["threshold"] = json!(threshold);
        value["clustering"] =
            json!("transitive connected components; members need not all be within threshold");
        value["counts"]["duplicate_clusters"] = json!(duplicates);
        value["clusters"] = json!(clusters);
    }
    Ok(value)
}
#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<Value> {
    let props = json!({"operation":{"enum":["hash","dedupe"],"type":"string"},"files":{"type":"array","minItems":1,"maxItems":100000,"items":{"type":"string"}},"out":{"type":"string"},"algorithm":{"enum":["ahash","dhash","phash"]},"threshold":{"type":"integer","minimum":0,"maximum":64}});
    vec![
        json!({"type":"object","properties":props,"required":["operation","files","out"],"additionalProperties":false}),
    ]
}
