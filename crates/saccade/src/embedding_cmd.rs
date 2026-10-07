//! Optional explicitly pinned embedding commands and streaming exact flat index.
use crate::agent::CliError;
#[cfg(feature = "embeddings")]
use crate::general_cmd;
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct RuntimeArgs {
    /// Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing.
    /// Default: SACCADE_MODELS_EMBEDDING_CONTRACT or [models].embedding_contract.
    #[arg(long)]
    model: Option<PathBuf>,
    /// Deprecated: set SACCADE_MODELS_DIR or [models].dir. Content-addressed model cache.
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library.
    /// Explicit ONNX Runtime 1.22 dynamic library, CPU execution only.
    #[arg(long)]
    library: Option<PathBuf>,
    /// Deprecated: provision with `saccade models pull embedding`. Still downloads
    /// the pinned export to the cache.
    #[arg(long)]
    download_model: bool,
}
/// Fully resolved runtime inputs.
#[cfg_attr(not(feature = "embeddings"), allow(dead_code))]
pub(crate) struct Runtime {
    pub(crate) model: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) library: PathBuf,
    pub(crate) download_model: bool,
}
impl RuntimeArgs {
    /// Flags (deprecated spellings) over the shared model configuration.
    #[cfg_attr(not(feature = "embeddings"), allow(dead_code))]
    fn resolve(&self) -> Result<Runtime, CliError> {
        if self.download_model {
            saccade_core::model_config::deprecated_download_flag("--download-model");
        }
        let cfg =
            crate::wave7_cmd::config()?.with_overrides(saccade_core::model_config::Overrides {
                dir: self.cache.as_deref(),
                runtime_library: self.library.as_deref(),
                embedding_contract: self.model.as_deref(),
                ..Default::default()
            });
        let need = |v: Option<PathBuf>, what: &str, hint: &str| {
            v.ok_or_else(|| CliError::usage(format!("{what} not configured; set {hint}")))
        };
        Ok(Runtime {
            model: need(
                cfg.embedding_contract.clone(),
                "embedding contract",
                "SACCADE_MODELS_EMBEDDING_CONTRACT, [models].embedding_contract or --model",
            )?,
            cache: cfg.dir.clone(),
            library: need(
                cfg.runtime_library.clone(),
                "ONNX runtime library",
                "SACCADE_MODELS_RUNTIME_LIBRARY, [models].runtime_library or --library",
            )?,
            download_model: self.download_model,
        })
    }
}
#[derive(clap::Args)]
pub(crate) struct SimilarArgs {
    a: PathBuf,
    b: PathBuf,
    #[command(flatten)]
    runtime: RuntimeArgs,
    #[arg(long, default_value = "similar-report")]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
#[derive(clap::Args)]
pub(crate) struct IndexArgs {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(clap::Subcommand)]
enum Operation {
    // wave11: external report index export needs no models.
    /// Export external report cross-links.
    Export {
        #[arg(long, default_value = "reports/index.jsonl")]
        index: PathBuf,
        #[arg(long, default_value = "jsonl")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Write exact Rust-preprocessed tensors for independent checkpoint/export parity.
    ExportInputs {
        dir: PathBuf,
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Run pinned export parity and fit/holdout calibration over a frozen corpus (heavy).
    Calibrate {
        corpus: PathBuf,
        #[command(flatten)]
        runtime: RuntimeArgs,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Build an exact index; --segmented supports larger archives and incremental updates.
    Build {
        dir: PathBuf,
        /// Use durable v2 segments (up to 1000000 images and 16 GiB vectors).
        #[arg(long)]
        segmented: bool,
        #[command(flatten)]
        runtime: RuntimeArgs,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Add/replace changed sources in an existing index, optionally pruning missing paths.
    Update {
        index: PathBuf,
        dir: PathBuf,
        /// Treat dir as the complete archive and remove absent sources.
        #[arg(long)]
        prune: bool,
        #[command(flatten)]
        runtime: RuntimeArgs,
        #[arg(long, default_value = "index-update-report")]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Search an existing index; model/preprocessing must exactly match the index.
    Query {
        index: PathBuf,
        #[arg(required_unless_present = "text", conflicts_with = "text")]
        image: Option<PathBuf>,
        /// Text query requires a pinned SigLIP 2 joint text/image model.
        #[arg(long)]
        text: Option<String>,
        #[command(flatten)]
        runtime: RuntimeArgs,
        /// Number of nearest matches to return (default 10).
        #[arg(long, default_value_t = 10)]
        top: usize,
        #[arg(long, default_value = "query-report")]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
pub(crate) fn similar(args: SimilarArgs) -> Result<u8, CliError> {
    #[cfg(feature = "embeddings")]
    {
        let value = enabled::similar(&args.a, &args.b, &args.runtime.resolve()?, &args.out)?;
        general_cmd::emit_document(value, Some(&args.out), args.json)
    }
    #[cfg(not(feature = "embeddings"))]
    {
        let _ = args;
        Err(CliError::new(
            "feature_unavailable",
            "similar requires embeddings; no model export is bundled",
        ))
    }
}
pub(crate) fn index(args: IndexArgs) -> Result<u8, CliError> {
    if let Operation::Export {
        index,
        format,
        out,
        json,
    } = &args.operation
    {
        return crate::wave11_cmd::export(
            index,
            if *json { "json" } else { format },
            out.as_deref(),
        );
    }
    #[cfg(feature = "embeddings")]
    {
        match args.operation {
            Operation::Export { .. } => Err(CliError::usage("export already dispatched")),
            Operation::ExportInputs {
                dir,
                model,
                out,
                json,
            } => {
                let value = enabled::export_inputs(&dir, &model, &out)?;
                general_cmd::emit_document(value, Some(&out), json)
            }
            Operation::Calibrate {
                corpus,
                runtime,
                out,
                json,
            } => {
                let value = enabled::calibrate(&corpus, &runtime.resolve()?, &out)?;
                general_cmd::emit_document(value, Some(&out), json)
            }
            Operation::Build {
                dir,
                segmented,
                runtime,
                out,
                json,
            } => {
                let value = if segmented {
                    enabled::build_segmented(&dir, &runtime.resolve()?, &out)?
                } else {
                    enabled::build(&dir, &runtime.resolve()?, &out)?
                };
                general_cmd::emit_document(value, Some(&out), json)
            }
            Operation::Update {
                index,
                dir,
                prune,
                runtime,
                out,
                json,
            } => {
                let value = enabled::update(&index, &dir, prune, &runtime.resolve()?, &out)?;
                general_cmd::emit_document(value, Some(&out), json)
            }
            Operation::Query {
                index,
                image,
                text,
                runtime,
                top,
                out,
                json,
            } => {
                let value = if let Some(text) = text {
                    enabled::query_text(&index, &text, &runtime.resolve()?, top, &out)?
                } else {
                    let image = image.ok_or_else(|| CliError::usage("image or --text required"))?;
                    enabled::query(&index, &image, &runtime.resolve()?, top, &out)?
                };
                general_cmd::emit_document(value, Some(&out), json)
            }
        }
    }
    #[cfg(not(feature = "embeddings"))]
    {
        let _ = args;
        Err(CliError::new(
            "feature_unavailable",
            "index requires embeddings; no model export is bundled",
        ))
    }
}
#[cfg(feature = "embeddings")]
mod enabled {
    use super::*;
    use saccade_core::general::{embedding as e, input};
    use serde_json::{Value, json};
    use std::{io::Write, path::Path};
    const MAX_VECTOR_BYTES: u64 = 512 * 1024 * 1024;
    fn engine(runtime: &Runtime) -> Result<(e::Engine, String), CliError> {
        let bytes = input::bytes(&runtime.model, 2 * 1024 * 1024)?;
        let model: e::Model = e::parse_model(&bytes)?;
        let identity = saccade_core::localized::digest(&serde_json::to_vec(&model)?);
        Ok((
            e::Engine::load(
                model,
                &runtime.cache,
                &runtime.library,
                runtime.download_model,
            )?,
            identity,
        ))
    }
    pub(super) fn export_inputs(
        dir: &Path,
        model_path: &Path,
        out: &Path,
    ) -> Result<Value, CliError> {
        let model: e::Model = e::parse_model(&input::bytes(model_path, 2 * 1024 * 1024)?)?;
        e::validate(&model)?;
        let files = input::files(dir, 128)?;
        if files.is_empty() {
            return Err(CliError::usage("export parity input directory is empty"));
        }
        general_cmd::prepare_out(out, &[dir, model_path])?;
        std::fs::create_dir(out.join("images")).map_err(|e| CliError::io(e.to_string()))?;
        std::fs::create_dir(out.join("tensors")).map_err(|e| CliError::io(e.to_string()))?;
        let mut samples = Vec::new();
        for (i, path) in files.iter().enumerate() {
            let bytes = input::bytes(path, input::MAX_BYTES)?;
            let image = input::decode(&bytes)?;
            let data = e::preprocess(&model, &image)?;
            let tensor: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
            let image_name = format!(
                "images/{i:04}.{}",
                path.extension().and_then(|s| s.to_str()).unwrap_or("bin")
            );
            let tensor_name = format!("tensors/{i:04}.f32le");
            std::fs::write(out.join(&image_name), &bytes)
                .map_err(|e| CliError::io(e.to_string()))?;
            std::fs::write(out.join(&tensor_name), &tensor)
                .map_err(|e| CliError::io(e.to_string()))?;
            samples.push(json!({"image":image_name,"sha256":saccade_core::localized::digest(&bytes),"tensor":tensor_name,"tensor_sha256":saccade_core::localized::digest(&tensor)}));
        }
        Ok(
            json!({"schema":saccade_core::general::embedding_qualification::EXPORT_INPUT_SCHEMA,"operation":"embedding_export_inputs","verdict":"unknown","counts":{"samples":samples.len()},"model_template":model,"samples":samples,"limits":["tensor preparation proves no graph/checkpoint parity; use independently executed checkpoint embeddings","model template artifact pin is replaced with the actual exported graph pin by the export job"]}),
        )
    }
    pub(super) fn calibrate(path: &Path, runtime: &Runtime, out: &Path) -> Result<Value, CliError> {
        use saccade_core::general::embedding_qualification as q;
        let bytes = input::bytes(path, 16 * 1024 * 1024)?;
        let corpus: q::Corpus = serde_json::from_slice(&bytes)?;
        let (mut engine, _) = engine(runtime)?;
        general_cmd::prepare_out(
            out,
            &[path, &runtime.model, &runtime.cache, &runtime.library],
        )?;
        let (model, receipt) = q::qualify(
            &corpus,
            &saccade_core::localized::digest(&bytes),
            path.parent().unwrap_or(Path::new(".")),
            &mut engine,
        )?;
        if receipt["verdict"] == "pass" {
            general_cmd::write_new(&out.join("model.json"), &serde_json::to_value(model)?)?;
        }
        Ok(receipt)
    }
    pub(super) fn similar(
        a: &Path,
        b: &Path,
        runtime: &Runtime,
        out: &Path,
    ) -> Result<Value, CliError> {
        let (mut engine, model_id) = engine(runtime)?;
        general_cmd::prepare_out(
            out,
            &[a, b, &runtime.model, &runtime.library, &runtime.cache],
        )?;
        let aa = input::bytes(a, input::MAX_BYTES)?;
        let bb = input::bytes(b, input::MAX_BYTES)?;
        let av = engine.embed(&input::decode(&aa)?)?;
        let bv = engine.embed(&input::decode(&bb)?)?;
        let score = e::cosine(&av, &bv)?;
        Ok(
            json!({"schema":e::SIMILAR_SCHEMA,"operation":"similar","verdict":"unknown","counts":{"images":2},"inputs":{"a_sha256":saccade_core::localized::digest(&aa),"b_sha256":saccade_core::localized::digest(&bb)},"model_contract_sha256":model_id,"model":engine.model(),"cosine":score,"band":e::band(engine.model(),score),"calibration_status":if engine.model().calibration.is_some(){"supplied_calibration_requires_external_qualification"}else{"uncalibrated"},"limitations":["semantic similarity is conditional on the pinned export and preprocessing","cosine and supplied bands do not establish exact text or image identity","canonical export parity and built-in calibration are unqualified"]}),
        )
    }
    pub(super) fn build(dir: &Path, runtime: &Runtime, out: &Path) -> Result<Value, CliError> {
        let (mut engine, model_id) = engine(runtime)?;
        let files = input::files(dir, 100000)?;
        let max_rows =
            (MAX_VECTOR_BYTES / (engine.model().dimensions as u64 * 4)).min(100000) as usize;
        if files.len() > max_rows {
            return Err(CliError::usage(
                "flat index exceeds 100000 images or 512 MiB vector budget",
            ));
        }
        let path_bytes = files.iter().map(|p| p.as_os_str().len()).sum::<usize>();
        if path_bytes > 64 * 1024 * 1024 {
            return Err(CliError::usage("index paths exceed 64 MiB"));
        }
        general_cmd::prepare_out(
            out,
            &[dir, &runtime.model, &runtime.library, &runtime.cache],
        )?;
        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(out.join("vectors.bin"))
            .map_err(|e| CliError::io(e.to_string()))?;
        let mut writer = std::io::BufWriter::new(file);
        let mut rows = Vec::new();
        let mut errors = Vec::new();
        for path in files {
            let name = path
                .strip_prefix(dir)
                .map_err(|e| CliError::io(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            let result = (|| -> Result<_, CliError> {
                let bytes = input::bytes(&path, input::MAX_BYTES)?;
                let vector = engine.embed(&input::decode(&bytes)?)?;
                Ok((vector, saccade_core::localized::digest(&bytes)))
            })();
            match result {
                Ok((vector, digest)) => {
                    for v in vector {
                        writer
                            .write_all(&v.to_le_bytes())
                            .map_err(|e| CliError::io(e.to_string()))?;
                    }
                    rows.push(json!({"path":name,"encoded_sha256":digest}));
                }
                Err(e) => errors.push(json!({"path":name,"code":e.code,"message":e.message})),
            }
        }
        writer.flush().map_err(|e| CliError::io(e.to_string()))?;
        Ok(
            json!({"schema":e::INDEX_SCHEMA,"operation":"index_build","verdict":if errors.is_empty()&&!rows.is_empty(){"pass"}else{"regression"},"counts":{"images":rows.len(),"errors":errors.len()},"model_contract_sha256":model_id,"model":engine.model(),"vector_file":"vectors.bin","vector_encoding":"f32-little-endian-row-major-l2-normalized","vectors_sha256":input::sha256(&out.join("vectors.bin"),MAX_VECTOR_BYTES)?,"dimensions":engine.model().dimensions,"rows":rows,"errors":errors,"limitations":["exact flat cosine search, <=100000 rows and <=512 MiB vectors","index build pass means successful execution, not semantic qualification","failed images remain listed; they are unavailable to query","canonical model export and calibration are not bundled or qualified"]}),
        )
    }
    fn update_rows(
        index: &Path,
        dir: &Path,
        prune: bool,
        engine: &mut e::Engine,
    ) -> Result<Value, CliError> {
        use saccade_core::general::embedding_index as storage;
        let mut update = storage::Update::begin(index, engine.model().clone())?;
        update.sync_directory(dir, prune, |bytes| engine.embed(&input::decode(bytes)?))?;
        Ok(update.commit()?)
    }
    pub(super) fn build_segmented(
        dir: &Path,
        runtime: &Runtime,
        out: &Path,
    ) -> Result<Value, CliError> {
        let (mut engine, _) = engine(runtime)?;
        general_cmd::prepare_out(
            out,
            &[dir, &runtime.model, &runtime.library, &runtime.cache],
        )?;
        let mut receipt = update_rows(out, dir, false, &mut engine)?;
        receipt["operation"] = json!("index_build");
        if receipt["counts"]["images"] == 0 {
            receipt["verdict"] = json!("regression");
        }
        Ok(receipt)
    }
    pub(super) fn update(
        index: &Path,
        dir: &Path,
        prune: bool,
        runtime: &Runtime,
        out: &Path,
    ) -> Result<Value, CliError> {
        // Output safety is validated before opening the writer; no writes inside source archives.
        saccade_core::run::guard_output_dir(
            index,
            &[dir, &runtime.model, &runtime.library, &runtime.cache],
            &[
                "saccade-embedding-index.v2.json",
                "saccade-embedding-index.v1.json",
            ],
        )?;
        let snapshot = saccade_core::general::embedding_index::Index::load(index)?;
        let (mut engine, model_id) = engine(runtime)?;
        if snapshot.model_id() != model_id {
            return Err(CliError::new(
                "index_mismatch",
                "update model differs from index",
            ));
        }
        general_cmd::prepare_out(
            out,
            &[index, dir, &runtime.model, &runtime.library, &runtime.cache],
        )?;
        update_rows(index, dir, prune, &mut engine)
    }
    pub(super) fn query_text(
        index: &Path,
        text: &str,
        runtime: &Runtime,
        top: usize,
        out: &Path,
    ) -> Result<Value, CliError> {
        let model = e::parse_model(&input::bytes(&runtime.model, 2 * 1024 * 1024)?)?;
        if model.text.is_none() {
            return Err(CliError::new(
                "text_embedding_unavailable",
                "image-only model cannot answer text queries",
            ));
        }
        query_common(index, None, Some(text), runtime, top, out)
    }
    pub(super) fn query(
        index: &Path,
        image: &Path,
        runtime: &Runtime,
        top: usize,
        out: &Path,
    ) -> Result<Value, CliError> {
        query_common(index, Some(image), None, runtime, top, out)
    }
    fn query_common(
        index: &Path,
        image: Option<&Path>,
        text: Option<&str>,
        runtime: &Runtime,
        top: usize,
        out: &Path,
    ) -> Result<Value, CliError> {
        if !(1..=100).contains(&top) {
            return Err(CliError::usage("top must be 1..100"));
        }
        let snapshot = saccade_core::general::embedding_index::Index::load(index)?;
        let (mut engine, model_id) = engine(runtime)?;
        if snapshot.model_id() != model_id {
            return Err(CliError::new(
                "index_mismatch",
                "query model differs from index",
            ));
        }
        let mut inputs = vec![index, &runtime.model, &runtime.cache, &runtime.library];
        if let Some(image) = image {
            inputs.push(image);
        }
        general_cmd::prepare_out(out, &inputs)?;
        let (vector, query_hash) = if let Some(text) = text {
            (
                engine.embed_text(text)?,
                saccade_core::localized::digest(text.as_bytes()),
            )
        } else {
            let bytes = input::bytes(
                image.ok_or_else(|| CliError::usage("query image missing"))?,
                input::MAX_BYTES,
            )?;
            (
                engine.embed(&input::decode(&bytes)?)?,
                saccade_core::localized::digest(&bytes),
            )
        };
        let hits = snapshot.query(&model_id, &vector, top)?;
        let results: Vec<_> = hits["hits"].as_array().ok_or_else(|| CliError::usage("query hits missing"))?.iter()
            .map(|h| json!({"row":h["row_index"],"source":h["row"],"cosine":h["cosine"],"band":if text.is_some(){None}else{e::band(engine.model(), h["cosine"].as_f64().unwrap_or(0.))}})).collect();
        Ok(
            json!({"schema":e::QUERY_SCHEMA,"operation":"index_query","verdict":"unknown","counts":{"indexed":snapshot.len(),"returned":results.len(),"index_errors":snapshot.error_count()},"query_sha256":query_hash,"index_metadata_sha256":snapshot.metadata_sha256(),"model_contract_sha256":model_id,"results":results,"limitations":["exact retrieval is conditional on the pinned model contract; no semantic accuracy qualification","source paths are provenance; query does not revalidate current source bytes"]}),
        )
    }
    #[cfg(feature = "mcp")]
    pub(super) fn measure(
        op: &str,
        paths: &std::collections::BTreeMap<String, PathBuf>,
        top: usize,
        out: &Path,
    ) -> Result<Value, CliError> {
        let get = |key: &str| {
            paths
                .get(key)
                .ok_or_else(|| CliError::usage(format!("missing {key}")))
        };
        if op == "embedding_export_inputs" {
            return export_inputs(get("dir")?, get("model")?, out);
        }
        let runtime = Runtime {
            model: get("model")?.clone(),
            cache: get("cache")?.clone(),
            library: get("library")?.clone(),
            download_model: false,
        };
        match op {
            "similar" => similar(get("a")?, get("b")?, &runtime, out),
            "embedding_calibrate" => calibrate(get("corpus")?, &runtime, out),
            "index_build" => build(get("dir")?, &runtime, out),
            "index_query" => query(get("index")?, get("image")?, &runtime, top, out),
            _ => Err(CliError::usage("invalid embedding operation")),
        }
    }
}
#[cfg(feature = "mcp")]
pub(crate) fn measure(
    op: &str,
    paths: &std::collections::BTreeMap<String, PathBuf>,
    top: usize,
    out: &std::path::Path,
) -> Result<serde_json::Value, CliError> {
    #[cfg(feature = "embeddings")]
    {
        enabled::measure(op, paths, top, out)
    }
    #[cfg(not(feature = "embeddings"))]
    {
        let _ = (op, paths, top, out);
        Err(CliError::new(
            "feature_unavailable",
            "embedding operation requires embeddings",
        ))
    }
}
#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<serde_json::Value> {
    use serde_json::json;
    [
        ("similar", vec!["a", "b"]),
        ("index_build", vec!["dir"]),
        ("index_query", vec!["index", "image"]),
        ("embedding_calibrate", vec!["corpus"]),
        ("embedding_export_inputs", vec!["dir"]),
    ]
    .into_iter()
    .map(|(op, fields)| {
        let mut props = serde_json::Map::new();
        let runtime_fields = if op == "embedding_export_inputs" {
            vec!["out", "model"]
        } else {
            vec!["out", "model", "cache", "library"]
        };
        let mut required = vec!["operation"];
        required.extend(runtime_fields.iter().copied());
        for key in runtime_fields.into_iter().chain(fields.iter().copied()) {
            props.insert(key.into(), json!({"type":"string"}));
        }
        required.extend(fields);
        props.insert("operation".into(), json!({"const":op,"type":"string"}));
        if op == "index_query" {
            props.insert(
                "top".into(),
                json!({"type":"integer","minimum":1,"maximum":100}),
            );
        }
        json!({"type":"object","properties":props,"required":required,"additionalProperties":false})
    })
    .collect()
}

pub(crate) fn routed(
    a: &std::path::Path,
    b: &std::path::Path,
    model: &std::path::Path,
    cache: &std::path::Path,
    library: &std::path::Path,
    out: &std::path::Path,
) -> Result<serde_json::Value, CliError> {
    #[cfg(feature = "embeddings")]
    {
        enabled::similar(
            a,
            b,
            &Runtime {
                model: model.into(),
                cache: cache.into(),
                library: library.into(),
                download_model: false,
            },
            out,
        )
    }
    #[cfg(not(feature = "embeddings"))]
    {
        let _ = (a, b, model, cache, library, out);
        Err(CliError::new(
            "feature_unavailable",
            "same-content requires embeddings; no silent fallback",
        ))
    }
}

#[cfg(feature = "mcp")]
pub(crate) const AUTHORITY_SCHEMA: &str = "saccade-onnx-runtime-authority.v1";

#[cfg(feature = "mcp")]
pub(crate) fn authorize_runtime(library: &std::path::Path) -> Result<(), CliError> {
    use saccade_core::general::input;
    let home = std::env::var_os("HOME").ok_or_else(|| {
        CliError::new(
            "execution_authorization_required",
            "MCP native runtime needs operator-owned configuration",
        )
    })?;
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&home).join(".config"));
    let dir = config_root.join("saccade");
    let config = dir.join("onnx-runtime.json");
    let denied = || {
        CliError::new(
            "execution_authorization_required",
            "MCP native ONNX runtime must match operator-owned onnx-runtime.json path and SHA-256; input roots do not authorize arbitrary libraries",
        )
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let owner = std::fs::metadata(&home).map_err(|_| denied())?.uid();
        for path in [&config_root, &dir, &config] {
            let metadata = std::fs::symlink_metadata(path).map_err(|_| denied())?;
            if metadata.file_type().is_symlink()
                || metadata.uid() != owner
                || metadata.mode() & 0o022 != 0
            {
                return Err(denied());
            }
        }
    }
    let value: serde_json::Value =
        serde_json::from_slice(&input::bytes(&config, 65536).map_err(|_| denied())?)
            .map_err(|_| denied())?;
    if value["schema"] != AUTHORITY_SCHEMA {
        return Err(denied());
    }
    let pinned = value["library"].as_str().ok_or_else(denied)?;
    let pinned = saccade_core::paths::canonicalize(pinned).map_err(|_| denied())?;
    let requested = saccade_core::paths::canonicalize(library).map_err(|_| denied())?;
    if pinned != requested
        || value["sha256"].as_str()
            != Some(
                input::sha256(&requested, 512 * 1024 * 1024)
                    .map_err(|_| denied())?
                    .as_str(),
            )
    {
        return Err(denied());
    }
    Ok(())
}
