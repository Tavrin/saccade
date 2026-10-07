//! Supervise the complete comparison, including parent-side FLIP and report writes.
use super::*;
use documents::worker::{CAPS, operation_budget, revoke_descriptors};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::io::Seek;
use std::io::{Read, Write};
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};

const REPLY_BYTES: u64 = 8 * 1024 * 1024;
const JOB_BYTES: u64 = 512 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    a: PathBuf,
    b: PathBuf,
    out: PathBuf,
    scratch: PathBuf,
    page_map: Option<PathBuf>,
    dpi: Option<f64>,
    align: Option<general_cmd::Align>,
    resample: Option<general_cmd::Resample>,
    threshold: f64,
    metric: crate::MetricArg,
    source_refs: Vec<String>,
    report_index: Option<PathBuf>,
    worker: Option<PathBuf>,
}
fn refusal(code: &'static str) -> CliError {
    CliError::new(code, "document operation refused by its resource boundary")
}
#[cfg(target_os = "linux")]
fn stop(child: &mut std::process::Child) {
    // The operation owns its process group; render descendants remain in that group.
    if let Ok(pid) = i32::try_from(child.id()) {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    let _ = child.kill();
    let _ = child.wait();
}
#[cfg(target_os = "linux")]
fn supervise(
    child: &mut std::process::Child,
    deadline: std::time::Duration,
) -> Result<(), CliError> {
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Also clean up any descendants left by an abnormal worker exit.
                stop(child);
                return if status.success() {
                    Ok(())
                } else {
                    Err(refusal("document_operation_resource_limit"))
                };
            }
            Ok(None) => {}
            Err(_) => {
                stop(child);
                return Err(refusal("document_worker_io"));
            }
        }
        if started.elapsed() >= deadline {
            stop(child);
            return Err(refusal("document_operation_wall_time_limit"));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
pub(super) fn measure(
    a: &Path,
    b: &Path,
    out: &Path,
    options: &general_cmd::CompareArgs,
    threshold: f64,
    metric: crate::MetricArg,
) -> Result<Value, CliError> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (a, b, out, options, threshold, metric);
        Err(refusal("document_isolation_unavailable"))
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        // Parent ownership ensures SIGKILL/resource failures cannot leak named PNG scratch.
        let scratch = tempfile::tempdir().map_err(|_| refusal("document_worker_io"))?;
        let (source_refs, report_index) = saccade_core::report_links::current_context();
        let job = Job {
            a: a.into(),
            b: b.into(),
            out: out.into(),
            scratch: scratch.path().into(),
            page_map: options.page_map.clone(),
            dpi: options.dpi,
            align: options.align,
            resample: options.resample,
            threshold,
            metric,
            source_refs,
            report_index,
            worker: std::env::var_os("SACCADE_DOCUMENT_WORKER").map(PathBuf::from),
        };
        let bytes = serde_json::to_vec(&job)?;
        if bytes.len() as u64 > JOB_BYTES {
            return Err(refusal("document_invalid_request"));
        }
        let mut input = tempfile::tempfile().map_err(|_| refusal("document_worker_io"))?;
        input
            .write_all(&bytes)
            .and_then(|()| input.rewind())
            .map_err(|_| refusal("document_worker_io"))?;
        let mut output = tempfile::tempfile().map_err(|_| refusal("document_worker_io"))?;
        let mut child = Command::new("/usr/bin/prlimit")
            .args([
                format!("--as={}", CAPS.operation_memory_bytes),
                format!("--cpu={0}:{0}", CAPS.operation_cpu_seconds),
                format!("--fsize={}", CAPS.output_bytes.max(CAPS.pixels * 4 + 4096)),
                "--core=0".into(),
                "--nofile=32".into(),
                "--".into(),
            ])
            .arg(std::env::current_exe().map_err(|_| refusal("document_worker_unavailable"))?)
            .arg("--document-operation")
            .env_clear()
            .env("RAYON_NUM_THREADS", "1")
            .stdin(Stdio::from(input))
            .stdout(Stdio::from(
                output
                    .try_clone()
                    .map_err(|_| refusal("document_worker_io"))?,
            ))
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|_| refusal("document_isolation_unavailable"))?;
        supervise(
            &mut child,
            std::time::Duration::from_millis(CAPS.operation_wall_time_ms),
        )?;
        output.rewind().map_err(|_| refusal("document_worker_io"))?;
        let mut bytes = Vec::new();
        output
            .take(REPLY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| refusal("document_worker_io"))?;
        if bytes.len() as u64 > REPLY_BYTES {
            return Err(refusal("document_worker_protocol"));
        }
        let reply: Value =
            serde_json::from_slice(&bytes).map_err(|_| refusal("document_worker_protocol"))?;
        if let Some(error) = reply.get("error") {
            let code = error["code"]
                .as_str()
                .and_then(|code| CODES.iter().copied().find(|known| *known == code))
                .ok_or_else(|| refusal("document_worker_protocol"))?;
            return Err(CliError::new(
                code,
                error["message"]
                    .as_str()
                    .unwrap_or("document operation refused"),
            ));
        }
        let value = reply
            .get("result")
            .filter(|v| v["schema"] == documents::SCHEMA)
            .ok_or_else(|| refusal("document_worker_protocol"))?;
        Ok(value.clone())
    }
}
#[cfg(target_os = "linux")]
const CODES: &[&str] = &[
    "io",
    "usage",
    "config",
    "unsafe_path",
    "not_empty_out_dir",
    "feature_unavailable",
    "document_encoded_limit",
    "document_page_limit",
    "document_objects_limit",
    "document_dimensions_limit",
    "document_depth_limit",
    "document_decompressed_limit",
    "document_malformed",
    "document_unsupported",
    "document_invalid_request",
    "document_worker_resource_limit",
    "document_wall_time_limit",
    "document_worker_protocol",
    "document_worker_io",
    "document_worker_unavailable",
    "document_isolation_unavailable",
    "document_page_map_required",
    "document_page_map_invalid",
    "document_total_pixel_limit",
    "document_output_byte_limit",
];
/// Internal entry point: refuse unbounded execution before reading the job.
pub(crate) fn serve() -> u8 {
    if revoke_descriptors().is_err() {
        return 2;
    }
    let bounded = std::fs::read_to_string("/proc/self/limits")
        .ok()
        .is_some_and(|limits| {
            [
                ("Max address space", CAPS.operation_memory_bytes),
                ("Max cpu time", CAPS.operation_cpu_seconds),
                (
                    "Max file size",
                    CAPS.output_bytes.max(CAPS.pixels * 4 + 4096),
                ),
                ("Max core file size", 0),
                ("Max open files", 32),
            ]
            .iter()
            .all(|(name, maximum)| {
                limits
                    .lines()
                    .find_map(|line| line.strip_prefix(name))
                    .is_some_and(|line| {
                        line.split_whitespace()
                            .take(2)
                            .all(|v| v.parse::<u64>().is_ok_and(|n| n <= *maximum))
                    })
            })
        });
    if !bounded {
        return 2;
    }
    let _budget = operation_budget();
    let run = || -> Result<Value, CliError> {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(JOB_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| refusal("document_worker_io"))?;
        if bytes.len() as u64 > JOB_BYTES {
            return Err(refusal("document_invalid_request"));
        }
        let job: Job =
            serde_json::from_slice(&bytes).map_err(|_| refusal("document_invalid_request"))?;
        saccade_core::report_links::context(job.source_refs, job.report_index)?;
        let options = general_cmd::CompareArgs {
            page_map: job.page_map,
            dpi: job.dpi,
            align: job.align,
            resample: job.resample,
            ..Default::default()
        };
        // Worker override is trusted embedding configuration; pass it explicitly instead of retaining the environment.
        let _worker = documents::worker::worker_scope(job.worker.as_deref());
        let value = super::measure_local(
            &job.a,
            &job.b,
            &job.out,
            &options,
            job.threshold,
            job.metric,
            &job.scratch,
        )?;
        general_cmd::persist_document(&value, &job.out)?;
        Ok(value)
    };
    let result = std::panic::catch_unwind(run)
        .unwrap_or_else(|_| Err(refusal("document_worker_resource_limit")));
    let reply = match result {
        Ok(value) => json!({"result":value}),
        Err(error) => json!({"error":{"code":error.code,"message":error.message}}),
    };
    let Ok(bytes) = serde_json::to_vec(&reply) else {
        return 2;
    };
    if bytes.len() as u64 > REPLY_BYTES {
        return 2;
    }
    // The bounded supervisor protocol is separate from artifact/scratch quota,
    // so an exhausted write budget can still return its stable refusal code.
    if std::io::stdout().write_all(&bytes).is_err() {
        2
    } else {
        0
    }
}

#[cfg(all(test, target_os = "linux"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    #[test]
    fn operation_deadline_kills_computation_and_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("descendant.pid");
        let mut child = Command::new("/bin/sh")
            .args(["-c", "sleep 60 & echo $! > \"$1\"; wait", "test"])
            .arg(&marker)
            .process_group(0)
            .spawn()
            .unwrap();
        let ready = std::time::Instant::now();
        while !std::fs::read_to_string(&marker).is_ok_and(|s| !s.trim().is_empty()) {
            assert!(ready.elapsed().as_secs() < 2);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let descendant = std::fs::read_to_string(&marker)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let started = std::time::Instant::now();
        assert_eq!(
            supervise(&mut child, std::time::Duration::from_millis(20))
                .unwrap_err()
                .code,
            "document_operation_wall_time_limit"
        );
        assert!(started.elapsed().as_secs() < 2);
        assert!(child.try_wait().unwrap().is_some());
        let stopped = std::time::Instant::now();
        loop {
            let state = std::fs::read_to_string(format!("/proc/{descendant}/stat"));
            if state.as_ref().is_err()
                || state.is_ok_and(|s| {
                    s.split_once(") ")
                        .is_some_and(|(_, tail)| tail.starts_with('Z'))
                })
            {
                break;
            }
            assert!(
                stopped.elapsed().as_secs() < 2,
                "descendant survived group kill"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
