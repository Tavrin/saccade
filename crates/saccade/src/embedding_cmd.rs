//! Optional explicitly pinned embedding commands and streaming exact flat index.
use crate::agent::CliError;
#[cfg(feature = "embeddings")]
use crate::general_cmd;
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct RuntimeArgs {
    /// Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing.
    #[arg(long)]
    model: PathBuf,
    /// Content-addressed model cache; downloads require --download-model.
    #[arg(long)]
    cache: PathBuf,
    /// Explicit ONNX Runtime 1.22 dynamic library, CPU execution only.
    #[arg(long)]
    library: PathBuf,
    /// Explicitly allow the pinned export to be downloaded to the cache.
    #[arg(long)]
    download_model: bool,
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
    /// Build a streaming exact flat index, up to 100000 images and 512 MiB vectors.
    Build {
        dir: PathBuf,
        #[command(flatten)]
        runtime: RuntimeArgs,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Search an existing index; model/preprocessing must exactly match the index.
    Query {
        index: PathBuf,
        image: PathBuf,
        #[command(flatten)]
        runtime: RuntimeArgs,
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
        let value = enabled::similar(&args.a, &args.b, &args.runtime, &args.out)?;
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
    #[cfg(feature = "embeddings")]
    {
        match args.operation {
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
                let value = enabled::calibrate(&corpus, &runtime, &out)?;
                general_cmd::emit_document(value, Some(&out), json)
            }
            Operation::Build {
                dir,
                runtime,
                out,
                json,
            } => {
                let value = enabled::build(&dir, &runtime, &out)?;
                general_cmd::emit_document(value, Some(&out), json)
            }
            Operation::Query {
                index,
                image,
                runtime,
                top,
                out,
                json,
            } => {
                let value = enabled::query(&index, &image, &runtime, top, &out)?;
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
    use std::{
        io::{Read, Write},
        path::Path,
    };
    const MAX_VECTOR_BYTES: u64 = 512 * 1024 * 1024;
    fn engine(runtime: &RuntimeArgs) -> Result<(e::Engine, String), CliError> {
        let bytes = input::bytes(&runtime.model, 65536)?;
        let model: e::Model = serde_json::from_slice(&bytes)?;
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
        let model: e::Model = serde_json::from_slice(&input::bytes(model_path, 65536)?)?;
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
    pub(super) fn calibrate(
        path: &Path,
        runtime: &RuntimeArgs,
        out: &Path,
    ) -> Result<Value, CliError> {
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
        runtime: &RuntimeArgs,
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
    pub(super) fn build(dir: &Path, runtime: &RuntimeArgs, out: &Path) -> Result<Value, CliError> {
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
    pub(super) fn query(
        index: &Path,
        image: &Path,
        runtime: &RuntimeArgs,
        top: usize,
        out: &Path,
    ) -> Result<Value, CliError> {
        if !(1..=100).contains(&top) {
            return Err(CliError::usage("top must be 1..100"));
        }
        let metadata = index.join(format!("{}.json", e::INDEX_SCHEMA));
        let bytes = input::bytes(&metadata, 128 * 1024 * 1024)?;
        let document: Value = serde_json::from_slice(&bytes)?;
        let (mut engine, model_id) = engine(runtime)?;
        if document["schema"] != e::INDEX_SCHEMA
            || document["model_contract_sha256"] != model_id
            || document["dimensions"].as_u64() != Some(engine.model().dimensions as u64)
            || document["vector_file"] != "vectors.bin"
            || document["vector_encoding"] != "f32-little-endian-row-major-l2-normalized"
        {
            return Err(CliError::new(
                "index_mismatch",
                "index/model identity or encoding mismatch",
            ));
        }
        let rows = document["rows"]
            .as_array()
            .ok_or_else(|| CliError::usage("index rows missing"))?;
        let dims = engine.model().dimensions;
        let expected = rows.len() as u64 * dims as u64 * 4;
        if rows.is_empty() || rows.len() > 100000 || expected > MAX_VECTOR_BYTES {
            return Err(CliError::usage("index size/count invalid"));
        }
        let vectors = index.join("vectors.bin");
        let file = std::fs::File::open(&vectors).map_err(|e| CliError::io(e.to_string()))?;
        if file
            .metadata()
            .map_err(|e| CliError::io(e.to_string()))?
            .len()
            != expected
            || document["vectors_sha256"].as_str()
                != Some(input::sha256(&vectors, MAX_VECTOR_BYTES)?.as_str())
        {
            return Err(CliError::new(
                "index_mismatch",
                "index vector size/hash mismatch",
            ));
        }
        general_cmd::prepare_out(
            out,
            &[
                index,
                image,
                &runtime.model,
                &runtime.cache,
                &runtime.library,
            ],
        )?;
        let image_bytes = input::bytes(image, input::MAX_BYTES)?;
        let query = engine.embed(&input::decode(&image_bytes)?)?;
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = vec![0u8; dims * 4];
        let mut hits: Vec<(usize, f64)> = Vec::new();
        for id in 0..rows.len() {
            reader
                .read_exact(&mut buffer)
                .map_err(|e| CliError::io(e.to_string()))?;
            let vector: Vec<_> = buffer
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            let cosine = e::cosine(&query, &vector)?;
            hits.push((id, cosine));
            hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            hits.truncate(top);
        }
        let results:Vec<_>=hits.into_iter().map(|(id,value)|json!({"row":id,"source":rows[id],"cosine":value,"band":e::band(engine.model(),value)})).collect();
        Ok(
            json!({"schema":e::QUERY_SCHEMA,"operation":"index_query","verdict":"unknown","counts":{"indexed":rows.len(),"returned":results.len(),"index_errors":document["errors"].as_array().map_or(0,Vec::len)},"query_sha256":saccade_core::localized::digest(&image_bytes),"index_metadata_sha256":saccade_core::localized::digest(&bytes),"model_contract_sha256":model_id,"results":results,"limitations":["retrieval scores are conditional on supplied export/preprocessing/calibration","indexed file names are provenance; current source bytes are not revalidated by query","failed build entries are not searchable and remain in the index error list"]}),
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
        let runtime = RuntimeArgs {
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
            &RuntimeArgs {
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
