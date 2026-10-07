//! Closed, offline replay transport; pack files are data, never shell commands.
use crate::agent::CliError;
use saccade_core::{
    replay::{self, File, Model, Operation, Pack, Recipe, Role},
    wave7::models,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Action,
}
#[derive(clap::Subcommand)]
enum Action {
    /// Record a local compare/text recipe, exact inputs, models, binary and report.
    Pack {
        /// saccade-replay-recipe.v1 JSON; paths resolve against its parent.
        recipe: PathBuf,
        /// New destination, outside every input (must not exist).
        #[arg(long)]
        out: PathBuf,
        /// Require a fresh execution to match an existing report before packing it.
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Verify every identity, re-execute offline and compare the complete report identity.
    Verify {
        /// Pack directory containing pack.json and pack.sha256.
        pack: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
fn error(e: replay::ReplayError) -> CliError {
    CliError::new(e.code, e.message)
}
fn io(e: std::io::Error) -> CliError {
    CliError::io(e.to_string())
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, CliError> {
    models::read_bounded(path, limit).map_err(crate::wave7_cmd::error)
}
fn document<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, CliError> {
    Ok(serde_json::from_slice(&read(path, 16 << 20)?)?)
}
fn write<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?).map_err(io)
}
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

struct Builder<'a> {
    root: &'a Path,
    files: Vec<File>,
    total: u64,
    nodes: usize,
}
impl Builder<'_> {
    fn add(&mut self, relative: &str, role: Role, bytes: &[u8]) -> Result<(), CliError> {
        self.total = self.total.saturating_add(bytes.len() as u64);
        if bytes.len() as u64 > replay::MAX_FILE_BYTES
            || self.total > replay::MAX_PACK_BYTES
            || self.files.len() >= replay::MAX_FILES
        {
            return Err(CliError::new(
                "replay_pack_changed",
                "pack exceeds file/byte bound",
            ));
        }
        if self.files.iter().any(|f| f.path == relative) {
            return Err(CliError::usage("duplicate pack file"));
        }
        let path = replay::safe_path(self.root, relative).map_err(error)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(&path, bytes).map_err(io)?;
        self.files.push(File {
            path: relative.into(),
            bytes: bytes.len() as u64,
            sha256: models::digest(bytes),
            role,
        });
        Ok(())
    }
    fn copy(
        &mut self,
        from: &Path,
        relative: &str,
        role: Role,
        depth: usize,
    ) -> Result<(), CliError> {
        self.nodes += 1;
        if self.nodes > replay::MAX_FILES {
            return Err(CliError::usage("pack tree exceeds entry bound"));
        }
        if depth > 32 {
            return Err(CliError::usage("pack directory depth exceeds 32"));
        }
        let meta = std::fs::symlink_metadata(from).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound
                && matches!(role, Role::Model | Role::Runtime)
            {
                CliError::new(
                    "not_reproducible_here",
                    "not reproducible here: selected OCR artifact/runtime missing",
                )
            } else {
                io(e)
            }
        })?;
        if meta.is_dir() {
            let destination = replay::safe_path(self.root, relative).map_err(error)?;
            std::fs::create_dir_all(destination).map_err(io)?;
            let mut children = std::fs::read_dir(from)
                .map_err(io)?
                .take(replay::MAX_FILES + 1)
                .collect::<Result<Vec<_>, _>>()
                .map_err(io)?;
            if children.len() > replay::MAX_FILES {
                return Err(CliError::usage("pack directory exceeds entry bound"));
            }
            children.sort_by_key(|e| e.file_name());
            for child in children {
                let name = child
                    .file_name()
                    .into_string()
                    .map_err(|_| CliError::usage("pack filenames must be UTF-8"))?;
                self.copy(
                    &child.path(),
                    &format!("{relative}/{name}"),
                    role,
                    depth + 1,
                )?;
            }
        } else if meta.is_file() {
            self.add(relative, role, &read(from, replay::MAX_FILE_BYTES)?)?;
        } else {
            return Err(CliError::new(
                "unsafe_path",
                "pack sources must be regular files/directories without links",
            ));
        }
        Ok(())
    }
    fn input(&mut self, base: &Path, path: &mut PathBuf, slot: &str) -> Result<(), CliError> {
        let source = base.join(&*path);
        let relative = if source.is_dir() {
            format!("inputs/{slot}")
        } else {
            let name = source
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| CliError::usage("input filename must be UTF-8"))?;
            format!("inputs/{slot}/{name}")
        };
        self.copy(&source, &relative, Role::Input, 0)?;
        if source.is_file()
            && source
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| {
                    matches!(
                        e.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "exr" | "hdr"
                    )
                })
        {
            let parent = source.parent().unwrap_or(Path::new("."));
            let stem = source
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| CliError::usage("raster stem must be UTF-8"))?;
            let names = [
                "saccade-meta.json".into(),
                "saccade-perf.json".into(),
                "gpu_clock.json".into(),
                format!("{stem}.saccade-meta.json"),
                format!("{stem}.object-id.json"),
                format!("{stem}.object-id.png"),
                format!("{stem}.material-id.json"),
                format!("{stem}.material-id.png"),
                format!("{stem}.material-id.exr"),
            ];
            for name in names {
                let companion = parent.join(&name);
                if companion.try_exists().map_err(io)? {
                    self.copy(&companion, &format!("inputs/{slot}/{name}"), Role::Input, 0)?;
                }
            }
        }
        *path = relative.into();
        Ok(())
    }
}

fn stage_ocr(
    builder: &mut Builder<'_>,
    base: &Path,
    spec: &mut replay::Ocr,
) -> Result<Vec<Model>, CliError> {
    let registry_path = base.join(&spec.registry);
    if !registry_path.is_file() {
        return Err(CliError::new(
            "not_reproducible_here",
            "not reproducible here: selected OCR registry missing",
        ));
    }
    let registry = models::Registry::load(&registry_path).map_err(crate::wave7_cmd::error)?;
    let raw = registry.contracts.get(&spec.contract_id).ok_or_else(|| {
        CliError::new(
            "not_reproducible_here",
            "selected registry OCR id unavailable",
        )
    })?;
    // The first recipe version supports the default Rust OCR route; no provider or subprocess adapters.
    let mut contract: saccade_core::general::ocr::Contract = serde_json::from_value(raw.clone())?;
    saccade_core::general::ocr::validate(&contract)?;
    let cache = registry_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(&contract.cache);
    let mut identities = Vec::new();
    for artifact in [
        &contract.detection,
        &contract.recognition,
        &contract.dictionary,
    ] {
        let relative = format!("ocr/models/{}", artifact.sha256);
        builder.copy(&cache.join(&artifact.sha256), &relative, Role::Model, 0)?;
        let copied = builder
            .files
            .last()
            .ok_or_else(|| CliError::usage("missing copied model"))?;
        if copied.sha256 != artifact.sha256 || copied.bytes != artifact.bytes {
            return Err(CliError::new(
                "replay_model_changed",
                "selected OCR model violates registry pin",
            ));
        }
        identities.push(Model {
            contract_id: spec.contract_id.clone(),
            role: artifact.role.clone(),
            revision: artifact.version.clone(),
            sha256: artifact.sha256.clone(),
            license: artifact.license.clone(),
            source: artifact.url.clone(),
        });
    }
    let library_source = base.join(&spec.library);
    let runtime_root = library_source
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| CliError::usage("runtime library needs its release directory"))?;
    let pin: serde_json::Value = serde_json::from_str(models::RUNTIME_PIN_JSON)?;
    let files = pin["files"]
        .as_array()
        .ok_or_else(|| CliError::usage("invalid shipped runtime pin"))?;
    for file in files {
        let path = file["path"]
            .as_str()
            .ok_or_else(|| CliError::usage("invalid shipped runtime path"))?;
        let source = if path == "lib/libonnxruntime.so.1.22.0" {
            library_source.clone()
        } else {
            runtime_root.join(path)
        };
        builder.copy(&source, &format!("ocr/runtime/{path}"), Role::Runtime, 0)?;
        let copied = builder
            .files
            .last()
            .ok_or_else(|| CliError::usage("missing runtime file"))?;
        if file["sha256"] != copied.sha256 || file["bytes"] != copied.bytes {
            return Err(CliError::new(
                "replay_runtime_changed",
                "recipe v1 requires the shipped ONNX Runtime 1.22.0 files and licence pins",
            ));
        }
    }
    builder.add(
        "ocr/provenance.json",
        Role::Config,
        &serde_json::to_vec_pretty(&serde_json::json!({"contract_id":spec.contract_id,"license_evidence":contract.license_evidence,"artifacts":identities}))?,
    )?;
    builder.add(
        "ocr/runtime-pin.json",
        Role::Config,
        &serde_json::to_vec_pretty(&pin)?,
    )?;
    contract.cache = "models".into();
    let mut frozen = models::Registry::empty();
    frozen
        .contracts
        .insert(spec.contract_id.clone(), serde_json::to_value(contract)?);
    builder.add(
        "ocr/registry.json",
        Role::Config,
        &serde_json::to_vec_pretty(&frozen)?,
    )?;
    spec.registry = "ocr/registry.json".into();
    spec.library = "ocr/runtime/lib/libonnxruntime.so.1.22.0".into();
    Ok(identities)
}

fn argv(recipe: &Recipe) -> Result<Vec<String>, CliError> {
    recipe.validate().map_err(error)?;
    let path = |p: &Path| -> Result<String, CliError> {
        p.to_str()
            .map(str::to_owned)
            .ok_or_else(|| CliError::usage("recipe paths must be UTF-8"))
    };
    let mut args = match &recipe.run {
        Operation::Compare {
            a,
            b,
            threshold,
            metric,
            ppd,
        } => vec![
            "compare".into(),
            path(a)?,
            path(b)?,
            "--threshold".into(),
            threshold.to_string(),
            "--metric".into(),
            metric.clone(),
            "--ppd".into(),
            ppd.to_string(),
            "--config".into(),
            "compare.toml".into(),
        ],
        Operation::Text {
            a,
            b,
            a_source,
            b_source,
            ocr,
            expect_text,
            readable_confidence,
            moved_px,
        } => {
            let mut args = vec![
                "text".into(),
                path(a)?,
                path(b)?,
                "--readable-confidence".into(),
                readable_confidence.to_string(),
                "--moved-px".into(),
                moved_px.to_string(),
            ];
            for (flag, source) in [("--a-source", a_source), ("--b-source", b_source)] {
                if let Some(source) = source {
                    args.extend([flag.into(), path(source)?]);
                }
            }
            if let Some(ocr) = ocr {
                args.extend(["--ocr-contract".into(), path(&ocr.registry)?]);
            }
            for text in expect_text {
                args.push(format!("--expect-text={text}"));
            }
            args
        }
    };
    args.extend(["--out".into(), "run".into(), "--json".into()]);
    Ok(args)
}
fn recipe_paths(recipe: &Recipe) -> Vec<&Path> {
    match &recipe.run {
        Operation::Compare { a, b, .. } => vec![a, b],
        Operation::Text {
            a,
            b,
            a_source,
            b_source,
            ocr,
            ..
        } => {
            let mut p: Vec<&Path> = vec![a, b];
            p.extend(a_source.as_deref());
            p.extend(b_source.as_deref());
            if let Some(o) = ocr {
                p.extend([o.registry.as_path(), o.library.as_path()]);
            }
            p
        }
    }
}
fn validate_snapshot(root: &Path, recipe: &Recipe) -> Result<(), CliError> {
    match &recipe.run {
        Operation::Compare { .. } => {
            if !read(&root.join("compare.toml"), 16 << 20)?.is_empty() {
                return Err(CliError::new(
                    "replay_config_changed",
                    "recipe v1 requires isolated default compare config",
                ));
            }
        }
        Operation::Text {
            ocr: Some(spec), ..
        } => {
            if spec.registry != Path::new("ocr/registry.json")
                || spec.library != Path::new("ocr/runtime/lib/libonnxruntime.so.1.22.0")
            {
                return Err(CliError::new(
                    "replay_config_changed",
                    "frozen OCR paths changed",
                ));
            }
            let registry = models::Registry::load(&root.join(&spec.registry))
                .map_err(crate::wave7_cmd::error)?;
            if registry.contracts.len() != 1 || !registry.models.is_empty() {
                return Err(CliError::new(
                    "replay_config_changed",
                    "frozen registry must contain exactly the selected OCR contract",
                ));
            }
            let raw = registry.contracts.get(&spec.contract_id).ok_or_else(|| {
                CliError::new("replay_model_changed", "selected OCR contract id changed")
            })?;
            let contract: saccade_core::general::ocr::Contract =
                serde_json::from_value(raw.clone())?;
            saccade_core::general::ocr::validate(&contract)?;
            if contract.cache != Path::new("models") {
                return Err(CliError::new(
                    "replay_config_changed",
                    "frozen model cache must be inside the pack",
                ));
            }
            let pin: serde_json::Value = serde_json::from_str(models::RUNTIME_PIN_JSON)?;
            let files = pin["files"]
                .as_array()
                .ok_or_else(|| CliError::usage("invalid shipped runtime pin"))?;
            for file in files {
                let relative = file["path"]
                    .as_str()
                    .ok_or_else(|| CliError::usage("invalid shipped runtime path"))?;
                let path =
                    replay::safe_path(root, &format!("ocr/runtime/{relative}")).map_err(error)?;
                let bytes = read(&path, replay::MAX_FILE_BYTES)?;
                if file["sha256"] != models::digest(&bytes) || file["bytes"] != bytes.len() as u64 {
                    return Err(CliError::new(
                        "replay_runtime_changed",
                        "runtime differs from the shipped native-code pin",
                    ));
                }
            }
        }
        _ => (),
    }
    Ok(())
}

fn execute(
    root: &Path,
    recipe: &Recipe,
    executable: &Path,
) -> Result<(u8, String, serde_json::Value), CliError> {
    let args = argv(recipe)?;
    validate_snapshot(root, recipe)?;
    std::fs::create_dir_all(root.join("home")).map_err(io)?;
    std::fs::create_dir_all(root.join("tmp")).map_err(io)?;
    // Only the running, already verified CLI is executed. The embedded executable is never trusted as a command.
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(root)
        .env_clear()
        .env("HOME", root.join("home"))
        .env("TMPDIR", root.join("tmp"))
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("OMP_NUM_THREADS", "1")
        .env("RAYON_NUM_THREADS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Operation::Text {
        ocr: Some(spec), ..
    } = &recipe.run
    {
        if !cfg!(feature = "ocr") {
            return Err(CliError::new(
                "not_reproducible_here",
                "not reproducible here: OCR feature unavailable",
            ));
        }
        command.env("ORT_DYLIB_PATH", root.join(&spec.library));
    }
    let mut child = command.spawn().map_err(|_| {
        CliError::new(
            "not_reproducible_here",
            "not reproducible here: exact CLI cannot start in this environment",
        )
    })?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(io)? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(300) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CliError::new(
                "not_reproducible_here",
                "not reproducible here: replay exceeded 300 seconds",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let exit = match status.code() {
        Some(0) => 0,
        Some(1) => 1,
        _ => {
            return Err(CliError::new(
                "not_reproducible_here",
                "not reproducible here: measurement could not run; install compatible native dependencies",
            ));
        }
    };
    let report = match recipe.run {
        Operation::Compare { .. } => "run/saccade-report.v1.json",
        Operation::Text { .. } => "run/saccade-text.v1.json",
    };
    let value = document(&root.join(report))?;
    Ok((exit, report.into(), value))
}
fn report_id(value: &serde_json::Value) -> Result<String, CliError> {
    Ok(
        saccade_core::evidence::case::Measurement::report_identity_value(value)?
            .as_str()
            .into(),
    )
}
fn schema_ids(value: &serde_json::Value, ids: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(id) = map.get("schema").and_then(serde_json::Value::as_str) {
                ids.insert(id.into());
            }
            for v in map.values() {
                schema_ids(v, ids);
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                schema_ids(v, ids);
            }
        }
        _ => (),
    }
}

fn pack(
    recipe_path: &Path,
    out: &Path,
    recorded: Option<&Path>,
) -> Result<(Pack, String), CliError> {
    let mut recipe: Recipe = document(recipe_path)?;
    recipe.validate().map_err(error)?;
    let base = recipe_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let inputs: Vec<PathBuf> = recipe_paths(&recipe)
        .into_iter()
        .map(|p| base.join(p))
        .collect();
    let mut guarded: Vec<&Path> = inputs.iter().map(PathBuf::as_path).collect();
    guarded.push(recipe_path);
    if let Some(report) = recorded {
        guarded.push(report);
    }
    saccade_core::run::guard_output_dir(out, &guarded, &[])?;
    if out.exists() {
        return Err(CliError::new(
            "not_empty_out_dir",
            "pack destination must not exist",
        ));
    }
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(io)?;
    let parent = parent.canonicalize().map_err(io)?;
    let parent = parent.as_path();
    let temporary = tempfile::tempdir_in(parent).map_err(io)?;
    let root = temporary.path();
    let mut builder = Builder {
        root,
        files: vec![],
        total: 0,
        nodes: 0,
    };
    let mut models = vec![];
    match &mut recipe.run {
        Operation::Compare { a, b, .. } => {
            builder.input(base, a, "a")?;
            builder.input(base, b, "b")?;
            builder.add("compare.toml", Role::Config, b"")?;
        }
        Operation::Text {
            a,
            b,
            a_source,
            b_source,
            ocr,
            ..
        } => {
            builder.input(base, a, "a")?;
            builder.input(base, b, "b")?;
            if let Some(p) = a_source {
                builder.input(base, p, "a-source")?;
            }
            if let Some(p) = b_source {
                builder.input(base, p, "b-source")?;
            }
            if let Some(spec) = ocr {
                models = stage_ocr(&mut builder, base, spec)?;
            }
        }
    }
    builder.add(
        "recipe.json",
        Role::Config,
        &serde_json::to_vec_pretty(&recipe)?,
    )?;
    let executable = std::env::current_exe().map_err(io)?;
    builder.copy(&executable, "tool/saccade", Role::Tool, 0)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.join("tool/saccade"),
            std::fs::Permissions::from_mode(0o755),
        )
        .map_err(io)?;
    }
    let build = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"git_commit":env!("SACCADE_GIT_COMMIT"),"git_dirty":env!("SACCADE_GIT_DIRTY"),"profile":env!("SACCADE_BUILD_PROFILE"),"rustc":env!("SACCADE_RUSTC_VERSION"),"platform":platform(),"core_features":saccade_core::COMPILED_FEATURES});
    builder.add(
        "tool/build.json",
        Role::Tool,
        &serde_json::to_vec_pretty(&build)?,
    )?;
    let (measurement_exit, report, value) = execute(root, &recipe, &executable)?;
    let identity = report_id(&value)?;
    let comparison_id = replay::comparison_id(&value).map_err(error)?;
    if value["report_id"].as_str() != Some(&identity) {
        return Err(CliError::new(
            "replay_report_changed",
            "generated report identity does not match its content",
        ));
    }
    if let Some(recorded) = recorded {
        let previous: serde_json::Value = document(recorded)?;
        if replay::comparison_id(&previous).map_err(error)? != comparison_id {
            return Err(CliError::new(
                "replay_report_mismatch",
                "fresh execution differs from supplied report",
            ));
        }
        builder.add(
            "recorded-report.json",
            Role::Report,
            &read(recorded, 16 << 20)?,
        )?;
    }
    saccade_core::manifest::write(&root.join("run"), Default::default())?;
    // Inventory the generated report bundle, including the Lane C manifest/index.
    let run = root.join("run");
    let bundle = tempfile::tempdir_in(parent).map_err(io)?;
    std::fs::rename(&run, bundle.path().join("run")).map_err(io)?;
    builder.copy(&bundle.path().join("run"), "run", Role::Report, 0)?;
    let mut ids: BTreeSet<String> = [
        replay::RECIPE_SCHEMA,
        replay::PACK_SCHEMA,
        replay::RESULT_SCHEMA,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    schema_ids(&value, &mut ids);
    if let Operation::Text { ocr: Some(_), .. } = &recipe.run {
        ids.insert(models::REGISTRY_SCHEMA.into());
        ids.insert(saccade_core::general::ocr::SCHEMA.into());
    }
    for id in ids {
        builder.add(
            &format!("schemas/{id}.schema.json"),
            Role::Schema,
            saccade_core::schema_catalog::get(&id)?.as_bytes(),
        )?;
    }
    builder.files.sort_by(|a, b| a.path.cmp(&b.path));
    let pack = Pack {
        schema: replay::PACK_SCHEMA.into(),
        files: builder.files,
        models,
        argv: argv(&recipe)?,
        report,
        report_id: identity,
        comparison_id,
        measurement_exit,
        platform: platform(),
    };
    replay::verify_files(root, &pack).map_err(error)?;
    write(&root.join("pack.json"), &pack)?;
    let seal = models::digest(&read(&root.join("pack.json"), 16 << 20)?);
    std::fs::write(root.join("pack.sha256"), &seal).map_err(io)?;
    // Home/tmp are not evidence and are recreated in a fresh workspace at verify time.
    std::fs::remove_dir_all(root.join("home")).map_err(io)?;
    std::fs::remove_dir_all(root.join("tmp")).map_err(io)?;
    std::fs::rename(root, out).map_err(io)?;
    Ok((pack, seal))
}
fn verify(root: &Path) -> Result<(Pack, String, String), CliError> {
    let root = root.canonicalize().map_err(io)?;
    let raw = read(
        &replay::safe_path(&root, "pack.json").map_err(error)?,
        16 << 20,
    )?;
    let seal = read(
        &replay::safe_path(&root, "pack.sha256").map_err(error)?,
        128,
    )?;
    let hash = models::digest(&raw);
    if seal != hash.as_bytes() {
        return Err(CliError::new(
            "replay_pack_changed",
            "pack manifest seal changed",
        ));
    }
    let pack: Pack = serde_json::from_slice(&raw)?;
    replay::verify_files(&root, &pack).map_err(error)?;
    if pack.platform != platform() {
        return Err(CliError::new(
            "not_reproducible_here",
            "not reproducible here: binary platform differs",
        ));
    }
    let executable = std::env::current_exe().map_err(io)?;
    let tool = pack
        .files
        .iter()
        .find(|f| f.path == "tool/saccade" && f.role == Role::Tool)
        .ok_or_else(|| CliError::new("replay_pack_changed", "exact CLI identity missing"))?;
    if models::digest(&read(&executable, replay::MAX_FILE_BYTES)?) != tool.sha256 {
        return Err(CliError::new(
            "replay_tool_changed",
            "running CLI differs from the recorded executable",
        ));
    }
    let recipe: Recipe = document(&root.join("recipe.json"))?;
    if argv(&recipe)? != pack.argv {
        return Err(CliError::new(
            "replay_config_changed",
            "recorded command differs from recipe",
        ));
    }
    let workspace = tempfile::tempdir().map_err(io)?;
    // Copy only inventoried, verified files. Added unrecorded sidecars/files cannot influence execution.
    let mut builder = Builder {
        root: workspace.path(),
        files: vec![],
        total: 0,
        nodes: 0,
    };
    for f in &pack.files {
        if matches!(
            f.role,
            Role::Input | Role::Config | Role::Model | Role::Runtime
        ) {
            builder.copy(&root.join(&f.path), &f.path, f.role, 0)?;
            if builder
                .files
                .last()
                .is_none_or(|copied| copied.sha256 != f.sha256)
            {
                return Err(CliError::new(
                    f.role.changed(),
                    "pack changed while staging replay",
                ));
            }
        }
    }
    for path in recipe_paths(&recipe) {
        let relative = path
            .to_str()
            .ok_or_else(|| CliError::usage("non-UTF-8 recipe path"))?;
        let p = replay::safe_path(workspace.path(), relative).map_err(error)?;
        if !p.exists() {
            return Err(CliError::new(
                "replay_pack_changed",
                "recipe references uninventoried input",
            ));
        }
    }
    let recorded_path = replay::safe_path(&root, &pack.report).map_err(error)?;
    if !pack
        .files
        .iter()
        .any(|f| f.path == pack.report && f.role == Role::Report)
    {
        return Err(CliError::new(
            "replay_pack_changed",
            "report is outside the recorded inventory",
        ));
    }
    let recorded: serde_json::Value = document(&recorded_path)?;
    if report_id(&recorded)? != pack.report_id
        || recorded["report_id"].as_str() != Some(&pack.report_id)
    {
        return Err(CliError::new(
            "replay_report_changed",
            "recorded report identity changed",
        ));
    }
    let (exit, _, value) = execute(workspace.path(), &recipe, &executable)?;
    if replay::comparison_id(&recorded).map_err(error)? != pack.comparison_id {
        return Err(CliError::new(
            "replay_report_changed",
            "recorded comparison identity changed",
        ));
    }
    if replay::comparison_id(&value).map_err(error)? != pack.comparison_id
        || exit != pack.measurement_exit
    {
        return Err(CliError::new(
            "replay_report_mismatch",
            "re-executed report or measurement exit differs from recorded evidence",
        ));
    }
    Ok((pack, hash, report_id(&value)?))
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let (operation, json, pack, seal, replayed_report_id) = match args.operation {
        Action::Pack {
            recipe,
            out,
            report,
            json,
        } => {
            let (p, h) = pack(&recipe, &out, report.as_deref())?;
            {
                let id = p.report_id.clone();
                ("pack", json, p, h, id)
            }
        }
        Action::Verify { pack, json } => {
            let (p, h, replayed_id) = verify(&pack)?;
            ("verify", json, p, h, replayed_id)
        }
    };
    let receipt = replay::Receipt {
        schema: replay::RESULT_SCHEMA.into(),
        operation: operation.into(),
        status: "reproduced".into(),
        pack_sha256: seal,
        report_id: pack.report_id,
        replayed_report_id,
        measurement_exit: pack.measurement_exit,
    };
    if json {
        crate::emit(&format!("{}\n", serde_json::to_string(&receipt)?))?;
    } else {
        crate::emit(&format!(
            "reproduced: {} (measurement exit {})\n",
            receipt.report_id, receipt.measurement_exit
        ))?;
    }
    Ok(0)
}
