//! Bounded local batch intake with shared CLI/in-process workers and immutable receipts.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};

/// One durable result per input identity.
pub const ROW_SCHEMA: &str = "saccade-batch-row.v1";
/// Folder/list intake contract.
pub const INPUT_SCHEMA: &str = "saccade-batch-input.v1";
/// Resume configuration contract.
pub const RUN_SCHEMA: &str = "saccade-batch-run.v1";
/// Bounded CLI summary; rows retain their original section schemas.
pub const RESULT_SCHEMA: &str = "saccade-batch-result.v1";
const MAX_BYTES: u64 = 64 << 20;
const MAX_ITEMS: usize = 1000;

/// An existing single-image command and its explicit extra arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    /// analyze-media, inspect, compare, text-quality, tofu, watermark or mask-metrics.
    pub command: String,
    /// Existing command flags. Output location, provider calls and downloads are forbidden.
    #[serde(default)]
    pub args: Vec<String>,
}
/// An explicit item; relative paths resolve against the intake manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Original input (never modified).
    pub path: PathBuf,
    /// Optional paired reference.
    #[serde(default)]
    pub reference: Option<PathBuf>,
    /// Explicit omission still produces a row.
    #[serde(default)]
    pub skip: bool,
    /// Optional tighter item deadline, at most the run timeout.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intake {
    schema: String,
    inputs: Vec<Input>,
}
/// Resource limits and command selection. No provider dispatch is admitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    /// Commands applied in order to every item.
    pub sections: Vec<Section>,
    /// Simultaneously active items, 1–16.
    pub concurrency: usize,
    /// Whole-item deadline, including hashing, probe and every section, 1–300000 ms.
    pub timeout_ms: u64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            sections: vec![Section {
                command: "analyze-media".into(),
                args: vec![],
            }],
            concurrency: 2,
            timeout_ms: 30_000,
        }
    }
}
fn invalid(s: impl Into<String>) -> Error {
    Error::Config(s.into())
}
fn io(e: std::io::Error) -> Error {
    Error::Io {
        context: "batch IO".into(),
        source: e,
    }
}
fn read(path: &Path) -> Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)
        .map_err(io)?
        .file_type()
        .is_file()
    {
        return Err(invalid("batch reads only regular files, never symlinks"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(io)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(invalid("batch input exceeds 64 MiB"));
    }
    Ok(bytes)
}
fn executable_hash(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(io)?;
    let mut h = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(io)?;
        if n == 0 {
            break;
        }
        h.update(&buffer[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("batch paths must be UTF-8"))
}
fn absolute(path: &Path, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}
/// Read a recursively sorted folder (including unsupported files), or a versioned JSON list.
/// Symlinks and non-regular entries are retained as skipped rows; never followed.
pub fn intake(source: &Path, reference_dir: Option<&Path>) -> Result<Vec<Input>> {
    let cwd = std::env::current_dir().map_err(io)?;
    let source = absolute(source, &cwd);
    let inputs = if source.is_dir() {
        let mut inputs = Vec::new();
        for e in walkdir::WalkDir::new(&source)
            .follow_links(false)
            .sort_by_file_name()
        {
            let e = e.map_err(|e| invalid(e.to_string()))?;
            if e.file_type().is_dir() {
                continue;
            }
            inputs.push(Input {
                path: e.path().to_owned(),
                reference: reference_dir.map(|d| {
                    absolute(d, &cwd).join(e.path().strip_prefix(&source).unwrap_or(e.path()))
                }),
                skip: !e.file_type().is_file(),
                timeout_ms: None,
            });
            if inputs.len() > MAX_ITEMS {
                return Err(invalid("batch intake exceeds 1000 items"));
            }
        }
        inputs
    } else {
        let manifest: Intake = serde_json::from_slice(&read(&source)?)?;
        if manifest.schema != INPUT_SCHEMA {
            return Err(invalid(
                "unsupported batch intake schema; upgrade for newer versions",
            ));
        }
        if reference_dir.is_some() {
            return Err(invalid("reference-dir applies only to folders"));
        }
        let base = source.parent().unwrap_or(&cwd);
        manifest
            .inputs
            .into_iter()
            .map(|mut i| {
                i.path = absolute(&i.path, base);
                i.reference = i.reference.map(|p| absolute(&p, base));
                i
            })
            .collect()
    };
    if inputs.is_empty() || inputs.len() > MAX_ITEMS {
        return Err(invalid("batch needs 1–1000 inputs"));
    }
    Ok(inputs)
}
fn validate(options: &Options) -> Result<()> {
    if !(1..=16).contains(&options.concurrency)
        || !(1..=300_000).contains(&options.timeout_ms)
        || options.sections.is_empty()
        || options.sections.len() > 16
    {
        return Err(invalid(
            "batch limits: 1–16 workers/sections, timeout 1–300000 ms",
        ));
    }
    let mut names = BTreeSet::new();
    for s in &options.sections {
        if !matches!(
            s.command.as_str(),
            "analyze-media"
                | "inspect"
                | "compare"
                | "text-quality"
                | "tofu"
                | "watermark"
                | "mask-metrics"
        ) || !names.insert(&s.command)
        {
            return Err(invalid("unsupported or repeated batch section"));
        }
        if s.args.len() > 128
            || s.args.iter().any(|a| {
                a.len() > 8192
                    || a == "--"
                    || a == "--out"
                    || a.starts_with("--out=")
                    || a.starts_with("--json")
                    || a.starts_with("--run")
                    || a.starts_with("--allow-download")
                    || a.starts_with("--description")
            })
        {
            return Err(invalid(
                "batch section cannot select output, dispatch providers or download",
            ));
        }
    }
    Ok(())
}
/// Probe an input in a killable CLI worker and create a bounded thumbnail.
/// The returned status distinguishes an unknown format from broken supported bytes.
pub fn probe(path: &Path, thumbnail: &Path) -> Result<Value> {
    let bytes = match read(path) {
        Ok(b) => b,
        Err(e) => return Ok(json!({"status":"corrupt","error":e.to_string()})),
    };
    let format = match image::guess_format(&bytes) {
        Ok(f) => f,
        Err(_) => {
            let known = path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                matches!(
                    s.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "exr" | "hdr" | "webp" | "avif"
                )
            });
            return Ok(
                json!({"status":if known {"corrupt"} else {"unsupported"},"error":"unrecognized image bytes"}),
            );
        }
    };
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 << 20);
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    reader.limits(limits);
    match reader.decode() {
        Ok(image) => {
            image
                .thumbnail(160, 160)
                .to_rgb8()
                .save(thumbnail)
                .map_err(|e| invalid(e.to_string()))?;
            Ok(json!({"status":"ok","dimensions":[image.width(),image.height()]}))
        }
        Err(image::ImageError::Unsupported(e)) => {
            Ok(json!({"status":"unsupported","error":e.to_string()}))
        }
        Err(e) => Ok(json!({"status":"corrupt","error":e.to_string()})),
    }
}
fn invoke(executable: &Path, args: &[String], dir: &Path, deadline: Instant) -> Result<Value> {
    if Instant::now() >= deadline {
        return Ok(json!({"status":"timed-out"}));
    }
    let stdout = tempfile::tempfile_in(dir).map_err(io)?;
    let stderr = tempfile::tempfile_in(dir).map_err(io)?;
    let mut cmd = Command::new(executable);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().map_err(io)?)
        .stderr(stderr.try_clone().map_err(io)?);
    // Separate group also contains external decoder children on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(io)?;
    let status = loop {
        if stdout.metadata().map_err(io)?.len() > MAX_BYTES
            || stderr.metadata().map_err(io)?.len() > MAX_BYTES
        {
            let _ = child.kill();
            child.wait().map_err(io)?;
            return Ok(json!({"status":"partial","error":"worker output exceeds 64 MiB"}));
        }
        if let Some(status) = child.try_wait().map_err(io)? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            child.wait().map_err(io)?;
            break None;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let Some(status) = status else {
        return Ok(json!({"status":"timed-out"}));
    };
    use std::io::{Seek, SeekFrom};
    let mut stdout = stdout;
    stdout.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut data = Vec::new();
    stdout
        .take(MAX_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(io)?;
    if data.len() as u64 > MAX_BYTES {
        return Ok(json!({"status":"partial","error":"section output exceeds 64 MiB"}));
    }
    // Existing commands may print a short footer after the first JSON document.
    let result = serde_json::Deserializer::from_slice(&data)
        .into_iter::<Value>()
        .next()
        .and_then(std::result::Result::ok);
    let unavailable = result.as_ref().is_some_and(has_failure);
    Ok(
        json!({"status":if (status.success() || status.code() == Some(1)) && result.is_some() && !unavailable {"ok"} else {"partial"},"exit_code":status.code(),"result":result,"error":if data.is_empty() {Some("section produced no JSON result")} else {None}}),
    )
}
fn has_failure(v: &Value) -> bool {
    match v {
        Value::Object(o) => o.iter().any(|(k, v)| {
            ((k == "status" || k == "state")
                && v.as_str().is_some_and(|s| {
                    matches!(
                        s,
                        "error"
                            | "failed"
                            | "unavailable"
                            | "partial"
                            | "unknown"
                            | "inconclusive"
                            | "unmeasurable"
                    )
                }))
                || has_failure(v)
        }),
        Value::Array(a) => a.iter().any(has_failure),
        _ => false,
    }
}
fn item(
    worker: &Worker,
    input: &Input,
    options: &Options,
    out: &Path,
    duplicate: bool,
    occurrence: usize,
) -> Result<Value> {
    let timeout_ms = input.timeout_ms.unwrap_or(options.timeout_ms);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let path = path_string(&input.path)?;
    let bytes = if input.skip {
        None
    } else {
        read(&input.path).ok()
    };
    let sha = bytes.as_ref().map(|b| format!("{:x}", Sha256::digest(b)));
    let reference_bytes = input.reference.as_deref().and_then(|p| read(p).ok());
    let reference_sha = reference_bytes
        .as_ref()
        .map(|b| format!("{:x}", Sha256::digest(b)));
    let key = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&json!([
            path,
            sha,
            input.reference,
            reference_sha,
            input.skip,
            occurrence,
            timeout_ms
        ]))?)
    );
    let receipt = out.join("rows").join(format!("{key}.json"));
    if receipt.is_file() {
        return Ok(serde_json::from_slice(&read(&receipt)?)?);
    }
    // Unique attempt directories prevent overwrites even when an earlier attempt died.
    let attempt = tempfile::Builder::new()
        .prefix("item-")
        .tempdir_in(out.join("attempts"))
        .map_err(io)?
        .keep();
    let snapshot = attempt.join(format!(
        "input.{}",
        input
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
    ));
    let reference_snapshot = attempt.join(format!(
        "reference.{}",
        input
            .reference
            .as_deref()
            .and_then(Path::extension)
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
    ));
    if let Some(bytes) = bytes {
        std::fs::write(&snapshot, bytes).map_err(io)?;
    }
    if let Some(bytes) = reference_bytes {
        std::fs::write(&reference_snapshot, bytes).map_err(io)?;
    }
    let thumb = attempt.join("thumbnail.png");
    let mut row = json!({"schema":ROW_SCHEMA,"id":key,"occurrence":occurrence,"timeout_ms":timeout_ms,"path":path,"content_sha256":sha,"reference":input.reference,"reference_sha256":reference_sha,"duplicate_basename":duplicate,"status":"skipped","sections":[],"thumbnail":null});
    if !input.skip {
        let probe = worker.invoke(
            &[
                "batch-probe".into(),
                path_string(if snapshot.is_file() {
                    &snapshot
                } else {
                    &input.path
                })?,
                path_string(&thumb)?,
                "--json".into(),
            ],
            &attempt,
            deadline,
        )?;
        let probe = probe
            .get("result")
            .filter(|v| {
                v["status"]
                    .as_str()
                    .is_some_and(|s| matches!(s, "ok" | "corrupt" | "unsupported"))
            })
            .unwrap_or(&probe);
        row["status"] = probe["status"].clone();
        row["probe"] = probe.clone();
        if probe["status"] == "ok" {
            row["thumbnail"] = json!(crate::paths::portable(
                thumb
                    .strip_prefix(out)
                    .map_err(|e| invalid(e.to_string()))?
            ));
            let mut sections = Vec::new();
            for (n, s) in options.sections.iter().enumerate() {
                let paired = matches!(
                    s.command.as_str(),
                    "compare" | "text-quality" | "mask-metrics"
                );
                if paired && input.reference.is_none() {
                    sections.push(json!({"command":s.command,"status":"skipped","error":"paired reference required"}));
                    continue;
                }
                let name = match s.command.as_str() {
                    "inspect" => "inspect-image",
                    "text-quality" => "text-legibility",
                    s => s,
                };
                let mut argv = vec![name.to_owned()];
                if paired
                    && s.command != "mask-metrics"
                    && let Some(p) = &input.reference
                {
                    argv.push(path_string(
                        if reference_snapshot.is_file() && s.command != "compare" {
                            &reference_snapshot
                        } else {
                            p
                        },
                    )?);
                }
                argv.push(path_string(if s.command == "compare" {
                    &input.path
                } else {
                    &snapshot
                })?);
                if s.command == "mask-metrics"
                    && let Some(p) = &input.reference
                {
                    argv.push(path_string(
                        if reference_snapshot.is_file() && s.command != "compare" {
                            &reference_snapshot
                        } else {
                            p
                        },
                    )?);
                }
                argv.extend(s.args.iter().cloned());
                if matches!(s.command.as_str(), "compare" | "inspect") {
                    argv.extend([
                        "--out".into(),
                        path_string(&attempt.join(format!("section-{n}")))?,
                    ]);
                }
                argv.push("--json".into());
                let mut section = worker.invoke(&argv, &attempt, deadline)?;
                if s.command == "compare" {
                    let current = read(&input.path)
                        .ok()
                        .map(|b| format!("{:x}", Sha256::digest(b)));
                    let reference = input
                        .reference
                        .as_deref()
                        .and_then(|p| read(p).ok())
                        .map(|b| format!("{:x}", Sha256::digest(b)));
                    if current != sha || reference != reference_sha {
                        section["status"] = json!("partial");
                        section["error"] = json!("input changed during comparison");
                    }
                }
                section["command"] = json!(s.command);
                sections.push(section);
            }
            let status = if sections.iter().any(|s| s["status"] == "timed-out") {
                "timed-out"
            } else if sections.iter().any(|s| s["status"] != "ok") {
                "partial"
            } else if duplicate {
                "duplicate-basename"
            } else {
                "ok"
            };
            row["status"] = json!(status);
            row["sections"] = json!(sections);
        }
    }
    let mut tmp = tempfile::NamedTempFile::new_in(out.join("rows")).map_err(io)?;
    tmp.write_all(&serde_json::to_vec(&row)?).map_err(io)?;
    tmp.as_file().sync_all().map_err(io)?;
    tmp.persist_noclobber(receipt).map_err(|e| io(e.error))?;
    Ok(row)
}
fn escaped(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn replace(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new("."))).map_err(io)?;
    temp.write_all(bytes).map_err(io)?;
    temp.as_file().sync_all().map_err(io)?;
    temp.persist(path).map_err(|e| io(e.error))?;
    Ok(())
}
/// Execute or resume a local batch. Completed receipts (including failures and
/// removed/changed inputs) are retained. Returns all historical rows in stable order.
/// CLI workers are killable processes; Python uses [`run_in_process`] instead.
pub fn run(
    executable: &Path,
    inputs: &[Input],
    options: &Options,
    out: &Path,
) -> Result<Vec<Value>> {
    validate(options)?;
    let executable = if executable.components().count() == 1 {
        std::env::var_os("PATH")
            .and_then(|p| {
                std::env::split_paths(&p)
                    .map(|d| d.join(executable))
                    .find(|p| p.is_file())
            })
            .ok_or_else(|| invalid("batch executable missing from PATH"))?
    } else {
        executable.to_owned()
    };
    let executable = &executable;
    run_with_worker(&Worker::Cli(executable.to_owned()), inputs, options, out)
}

/// Execute or resume using Rust core analysis without spawning a CLI.
/// `library` pins the actual loaded extension bytes for resume compatibility.
/// Deadlines are cooperative: checked before and after each bounded operation;
/// a running decode/analysis cannot be interrupted. Unsupported flags yield partial rows.
pub fn run_in_process(
    library: &Path,
    inputs: &[Input],
    options: &Options,
    out: &Path,
) -> Result<Vec<Value>> {
    validate(options)?;
    let config = crate::model_config::ModelConfig::resolve()?;
    // Parse and pin the same bounded registry bytes used by this analyzer.
    let registry_bytes = config
        .registry
        .as_deref()
        .map(|p| crate::wave7::models::read_bounded(p, 2 * 1024 * 1024))
        .transpose()
        .map_err(|e| invalid(e.to_string()))?;
    let analyzer = if let Some(bytes) = &registry_bytes {
        let registry: crate::wave7::models::Registry = serde_json::from_slice(bytes)?;
        registry.validate().map_err(|e| invalid(e.to_string()))?;
        crate::media::Analyzer::with_registry(
            crate::media::Profile::CpuLite,
            config.dir.clone(),
            false,
            registry,
        )
    } else {
        crate::media::Analyzer::new(crate::media::Profile::CpuLite, config.dir.clone(), false)
    }
    .map_err(|e| invalid(e.to_string()))?;
    let identity = json!({
        "backend":"in-process",
        "library":crate::paths::canonicalize(library).map_err(io)?,
        "library_sha256":executable_hash(library)?,
        "model_config":config.to_json(),
        "registry_sha256":registry_bytes.as_ref().map(|b| format!("{:x}", Sha256::digest(b))),
        "deadline":"cooperative-before-and-after-operation",
    });
    run_with_worker(
        &Worker::Native {
            analyzer: Box::new(analyzer),
            identity,
        },
        inputs,
        options,
        out,
    )
}

mod native;

enum Worker {
    Cli(PathBuf),
    Native {
        analyzer: Box<crate::media::Analyzer>,
        identity: Value,
    },
}
impl Worker {
    fn identity(&self) -> Result<Value> {
        match self {
            Self::Cli(executable) => Ok(
                json!({"executable":crate::paths::canonicalize(executable).map_err(io)?,"executable_sha256":executable_hash(executable)?}),
            ),
            Self::Native { identity, .. } => Ok(identity.clone()),
        }
    }
    fn invoke(&self, args: &[String], dir: &Path, deadline: Instant) -> Result<Value> {
        match self {
            Self::Cli(executable) => invoke(executable, args, dir, deadline),
            Self::Native { analyzer, .. } => {
                cooperative(deadline, || native::invoke(analyzer, args))
            }
        }
    }
}
fn cooperative(
    deadline: Instant,
    operation: impl FnOnce() -> Result<(i32, Value)>,
) -> Result<Value> {
    if Instant::now() >= deadline {
        return Ok(json!({"status":"timed-out"}));
    }
    let result = operation().and_then(|(exit_code, result)| {
        if serde_json::to_vec(&result)?.len() as u64 > MAX_BYTES {
            return Err(invalid("section output exceeds 64 MiB"));
        }
        Ok((exit_code, result))
    });
    // Never detach timed-out threads: active work completes before returning.
    if Instant::now() >= deadline {
        return Ok(json!({"status":"timed-out"}));
    }
    Ok(match result {
        Ok((exit_code, result)) => {
            json!({"status":if !matches!(exit_code, 0 | 1) || has_failure(&result) {"partial"} else {"ok"},"exit_code":exit_code,"result":result,"error":null})
        }
        Err(e) => json!({"status":"partial","exit_code":2,"result":null,"error":e.to_string()}),
    })
}

// Keep the sentinel inode stable: deleting it would let a competing run lock a
// different inode. Explicit unlock also releases flock when a duplicate (for
// example, inherited by a concurrently spawned process) still references it.
struct OutputLock(std::fs::File);
impl OutputLock {
    fn acquire(out: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(out.join(".batch-lock"))
            .map_err(io)?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(|e| {
            if e.raw_os_error() == fs2::lock_contended_error().raw_os_error() {
                invalid("batch output is already in use")
            } else {
                io(e)
            }
        })?;
        Ok(Self(file))
    }
}
impl Drop for OutputLock {
    fn drop(&mut self) {
        // Drop covers successful runs, early errors and unwinding. Closing the
        // owned handle remains the fallback if the OS rejects the unlock.
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

fn run_with_worker(
    worker: &Worker,
    inputs: &[Input],
    options: &Options,
    out: &Path,
) -> Result<Vec<Value>> {
    if inputs.is_empty() || inputs.len() > MAX_ITEMS {
        return Err(invalid("batch needs 1–1000 inputs"));
    }
    if inputs.iter().any(|i| {
        i.timeout_ms
            .is_some_and(|n| n == 0 || n > options.timeout_ms)
    }) {
        return Err(invalid(
            "item timeout must be positive and at most the run timeout",
        ));
    }
    std::fs::create_dir_all(out).map_err(io)?;
    let out = crate::paths::canonicalize(out).map_err(io)?;
    for input in inputs {
        for p in std::iter::once(&input.path).chain(input.reference.iter()) {
            if crate::paths::canonicalize(p)
                .unwrap_or_else(|_| p.to_owned())
                .starts_with(&out)
            {
                return Err(invalid(
                    "batch output must be separate from input directories",
                ));
            }
        }
    }
    for e in walkdir::WalkDir::new(&out).follow_links(false) {
        let e = e.map_err(|e| invalid(e.to_string()))?;
        if e.file_type().is_symlink() {
            return Err(invalid("batch output contains a symlink"));
        }
    }
    let _lock = OutputLock::acquire(&out)?;
    // A dedicated owned directory permits summary replacement, never unrelated files.
    let config_path = out.join("batch-run.json");
    let mut config = worker.identity()?;
    config["schema"] = json!(RUN_SCHEMA);
    config["options"] = json!(options);
    if config_path.exists() {
        if serde_json::from_slice::<Value>(&read(&config_path)?)? != config {
            return Err(invalid(
                "resume configuration differs; choose a new output directory",
            ));
        }
    } else {
        if std::fs::read_dir(&out)
            .map_err(io)?
            .any(|e| e.is_ok_and(|e| e.file_name() != ".batch-lock"))
        {
            return Err(invalid(
                "batch output must be empty or an owned batch directory",
            ));
        }
        crate::manifest::write_owned(&config_path, RUN_SCHEMA, &config)?;
    }
    std::fs::create_dir_all(out.join("rows")).map_err(io)?;
    std::fs::create_dir_all(out.join("attempts")).map_err(io)?;
    // Keep the guard through row/summary and manifest publication. On process
    // interruption the kernel releases the lock when its last handle closes.
    let mut counts = BTreeMap::new();
    for i in inputs {
        *counts.entry(i.path.file_name()).or_insert(0usize) += 1;
    }
    let mut occurrences = BTreeMap::new();
    let tasks: Vec<_> = inputs
        .iter()
        .map(|i| {
            let n = occurrences
                .entry((&i.path, &i.reference, i.skip))
                .or_insert(0usize);
            let occurrence = *n;
            *n += 1;
            (i, occurrence)
        })
        .collect();
    let queue = Mutex::new(tasks.into_iter());
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..options.concurrency {
            let (queue, counts, out, tx) = (&queue, &counts, &out, tx.clone());
            scope.spawn(move || {
                loop {
                    let input = match queue.lock() {
                        Ok(mut q) => q.next(),
                        Err(_) => {
                            let _ = tx.send(Err(invalid("batch queue poisoned")));
                            break;
                        }
                    };
                    let Some((input, occurrence)) = input else {
                        break;
                    };
                    let result = item(
                        worker,
                        input,
                        options,
                        out,
                        counts.get(&input.path.file_name()).is_some_and(|n| *n > 1),
                        occurrence,
                    );
                    if tx.send(result.map(|_| ())).is_err() {
                        break;
                    }
                }
            });
        }
    });
    drop(tx);
    for result in rx {
        result?;
    }
    let mut rows = Vec::new();
    for e in std::fs::read_dir(out.join("rows")).map_err(io)? {
        let p = e.map_err(io)?.path();
        if p.extension().is_some_and(|e| e == "json") {
            let row: Value = serde_json::from_slice(&read(&p)?)?;
            if row["schema"] != ROW_SCHEMA {
                return Err(invalid("unsupported batch row schema"));
            }
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| {
        a["path"]
            .as_str()
            .cmp(&b["path"].as_str())
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    let mut jsonl = Vec::new();
    let mut table = String::from("id,path,content_sha256,status,duplicate_basename,thumbnail\n");
    let mut html = String::from(
        "<!doctype html><meta charset=\"utf-8\"><title>Batch index</title><style>body{font:16px system-ui}article{display:inline-block;vertical-align:top;width:240px;padding:12px;overflow-wrap:anywhere}img{width:160px;height:160px;object-fit:contain}</style><h1>Batch index</h1>",
    );
    for row in &rows {
        serde_json::to_writer(&mut jsonl, row)?;
        jsonl.push(b'\n');
        table.push_str(
            &[
                "id",
                "path",
                "content_sha256",
                "status",
                "duplicate_basename",
                "thumbnail",
            ]
            .map(|k| {
                csv(&row[k]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| row[k].to_string()))
            })
            .join(","),
        );
        table.push('\n');
        html.push_str("<article>");
        if let Some(p) = row["thumbnail"].as_str() {
            html.push_str(&format!("<img src=\"{}\" alt=\"\">", escaped(p)));
        }
        html.push_str(&format!(
            "<p>{}</p><p>{}</p><a href=\"rows/{}.json\">Receipt</a></article>",
            escaped(row["path"].as_str().unwrap_or("")),
            escaped(row["status"].as_str().unwrap_or("")),
            escaped(row["id"].as_str().unwrap_or(""))
        ));
    }
    replace(&out.join("rows.jsonl"), &jsonl)?;
    replace(&out.join("rows.csv"), table.as_bytes())?;
    replace(&out.join("index.html"), html.as_bytes())?;
    crate::manifest::write(&out, Default::default())?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_lock_releases_with_a_duplicate_handle_and_on_early_error() -> Result<()> {
        let t = tempfile::tempdir().map_err(io)?;
        let sentinel = t.path().join(".batch-lock");
        std::fs::write(&sentinel, b"stale sentinel").map_err(io)?;
        let lock = OutputLock::acquire(t.path())?;
        // A clone deterministically models the shared file description of a
        // forked child without timing or reliance on parallel test ordering.
        let duplicate = lock.0.try_clone().map_err(io)?;
        assert!(
            matches!(OutputLock::acquire(t.path()), Err(Error::Config(s))
            if s == "batch output is already in use")
        );
        drop(lock);
        let next = OutputLock::acquire(t.path())?;
        // Closing an old duplicate must not release the next owner's lock.
        drop(duplicate);
        assert!(OutputLock::acquire(t.path()).is_err());
        drop(next);
        fn fail(out: &Path) -> Result<()> {
            let _lock = OutputLock::acquire(out)?;
            Err(invalid("fixture error after acquisition"))
        }
        assert!(fail(t.path()).is_err());
        drop(OutputLock::acquire(t.path())?);
        assert_eq!(std::fs::read(sentinel).map_err(io)?, b"stale sentinel");
        Ok(())
    }
    #[test]
    fn cooperative_deadline_waits_for_active_work_and_blocks_later_work() -> Result<()> {
        let called = std::sync::atomic::AtomicBool::new(false);
        let row = cooperative(Instant::now(), || {
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok((0, json!({"status":"ok"})))
        })?;
        assert_eq!(row["status"], "timed-out");
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
        let deadline = Instant::now() + Duration::from_millis(5);
        let row = cooperative(deadline, || {
            std::thread::sleep(Duration::from_millis(10));
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok((0, json!({"status":"ok"})))
        })?;
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(row["status"], "timed-out");
        assert!(row.get("result").is_none());
        let unavailable = cooperative(Instant::now() + Duration::from_secs(1), || {
            Ok((0, json!({"status":"unavailable"})))
        })?;
        assert_eq!(unavailable["status"], "partial");
        Ok(())
    }
    #[test]
    fn rejects_unrelated_outputs_and_unsafe_flags() -> Result<()> {
        let temp = tempfile::tempdir().map_err(io)?;
        let original = temp.path().join("original.txt");
        std::fs::write(&original, b"retain").map_err(io)?;
        let input = Input {
            path: original.clone(),
            reference: None,
            skip: false,
            timeout_ms: None,
        };
        assert!(
            run(
                Path::new("missing"),
                std::slice::from_ref(&input),
                &Options::default(),
                temp.path()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(original).map_err(io)?, b"retain");
        for flag in [
            "--out=original",
            "--allow-download",
            "--run",
            "--description",
            "--json",
            "--",
        ] {
            let mut options = Options::default();
            options.sections[0].args.push(flag.into());
            assert!(validate(&options).is_err());
        }
        Ok(())
    }
    #[test]
    fn probe_and_model_section_failures_remain_distinct() -> Result<()> {
        let t = tempfile::tempdir().map_err(io)?;
        let bad = t.path().join("bad.png");
        std::fs::write(&bad, b"bad").map_err(io)?;
        assert_eq!(
            probe(&bad, &t.path().join("thumb.png"))?["status"],
            "corrupt"
        );
        let unsupported = t.path().join("unsupported.bin");
        std::fs::write(&unsupported, b"unknown").map_err(io)?;
        assert_eq!(
            probe(&unsupported, &t.path().join("thumb.png"))?["status"],
            "unsupported"
        );
        assert!(has_failure(
            &json!({"faces":{"status":"failed","error":"missing model"}})
        ));
        assert!(!has_failure(&json!({"description":{"status":"skipped"}})));
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn deadline_kills_worker_and_resume_keeps_failure() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().map_err(io)?;
        let executable = t.path().join("fixture-worker");
        std::fs::write(&executable, b"#!/bin/sh\nsleep 10\n").map_err(io)?;
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .map_err(io)?;
        let image = t.path().join("input.png");
        std::fs::write(&image, b"original").map_err(io)?;
        let input = Input {
            path: image.clone(),
            reference: None,
            skip: false,
            timeout_ms: None,
        };
        let options = Options {
            timeout_ms: 25,
            ..Default::default()
        };
        let start = Instant::now();
        let rows = run(
            &executable,
            std::slice::from_ref(&input),
            &options,
            &t.path().join("out"),
        )?;
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(rows[0]["status"], "timed-out");
        let receipt = std::fs::read(t.path().join("out/rows.jsonl")).map_err(io)?;
        let again = run(
            &executable,
            std::slice::from_ref(&input),
            &options,
            &t.path().join("out"),
        )?;
        assert_eq!(rows, again);
        assert_eq!(
            receipt,
            std::fs::read(t.path().join("out/rows.jsonl")).map_err(io)?
        );
        assert_eq!(std::fs::read(&image).map_err(io)?, b"original");
        assert!(crate::manifest::verify(&t.path().join("out"))?.is_empty());
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod concurrency_tests {
    use super::*;
    #[test]
    fn concurrency_is_bounded() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().map_err(io)?;
        let exe = t.path().join("worker.py");
        std::fs::write(
            &exe,
            br##"#!/usr/bin/env python3
import os, time, pathlib
log=pathlib.Path(__file__).with_suffix('.events')
def event(kind):
    with log.open('a') as f: f.write(f'{kind} {os.getpid()}\n')
event('start')
time.sleep(0.08)
print('{"status":"corrupt"}')
event('end')
"##,
        )
        .map_err(io)?;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
        let inputs: Vec<_> = (0..6)
            .map(|n| Input {
                path: t.path().join(format!("{n}.png")),
                reference: None,
                skip: false,
                timeout_ms: None,
            })
            .collect();
        for i in &inputs {
            std::fs::write(&i.path, b"bytes").map_err(io)?;
        }
        let rows = run(
            &exe,
            &inputs,
            &Options {
                concurrency: 2,
                ..Default::default()
            },
            &t.path().join("out"),
        )?;
        assert_eq!(rows.len(), 6);
        let mut active = 0;
        let mut peak = 0;
        for line in std::fs::read_to_string(exe.with_extension("events"))
            .map_err(io)?
            .lines()
        {
            if line.starts_with("start") {
                active += 1;
                peak = peak.max(active);
            } else {
                active -= 1;
            }
        }
        assert_eq!(active, 0);
        assert_eq!(peak, 2);
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod protocol_tests {
    use super::*;
    #[test]
    fn incompatible_worker_error_retains_an_explicit_partial_row() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().map_err(io)?;
        let exe = t.path().join("old-worker");
        std::fs::write(&exe,b"#!/bin/sh\nprintf '%s' '{\"execution\":\"error\",\"errors\":[{\"code\":\"usage\"}]}'\nexit 2\n").map_err(io)?;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
        let image = t.path().join("image.png");
        std::fs::write(&image, b"bytes").map_err(io)?;
        let input = Input {
            path: image,
            reference: None,
            skip: false,
            timeout_ms: None,
        };
        let rows = run(&exe, &[input], &Options::default(), &t.path().join("out"))?;
        assert_eq!(rows[0]["status"], "partial");
        assert_eq!(rows[0]["probe"]["result"]["errors"][0]["code"], "usage");
        Ok(())
    }
}
