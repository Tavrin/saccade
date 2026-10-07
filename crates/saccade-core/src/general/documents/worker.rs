//! Fresh Linux process per operation, with OS allocation/CPU/file caps and a parent deadline.
#[cfg(feature = "documents")]
use serde::Deserialize;
use serde::Serialize;

/// Fixed caps. No input or environment variable can raise these limits.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Caps {
    /// Encoded bytes per document.
    pub encoded_bytes: u64,
    /// Worker virtual address space (includes allocator, libraries and stack).
    pub memory_bytes: u64,
    /// Parent-enforced wall time per operation, including startup.
    pub wall_time_ms: u64,
    /// Wall time for a complete file-pair document comparison.
    pub operation_wall_time_ms: u64,
    /// Virtual address space for the comparison process.
    pub operation_memory_bytes: u64,
    /// CPU seconds for the comparison process.
    pub operation_cpu_seconds: u64,
    /// Total admitted input raster pixels across a comparison.
    pub total_pixels: u64,
    /// Maximum input raster area admitted to parent-side comparison.
    pub comparison_pixels: u64,
    /// Aggregate bytes written by a complete comparison, including scratch and indexes.
    pub output_bytes: u64,
    /// OS-enforced CPU seconds per worker.
    pub cpu_seconds: u64,
    /// Pages per document.
    pub pages: usize,
    /// Raster width or height in pixels.
    pub dimension: u32,
    /// Pixels per raster or embedded image.
    pub pixels: u64,
    /// Dictionary/array, XML and indirect-reference depth.
    pub object_depth: usize,
    /// PDF objects or SVG nodes.
    pub objects: usize,
    /// Aggregate decoded PDF stream bytes.
    pub decompressed_bytes: u64,
}
/// Default caps preserve all earlier byte, page, object and raster limits.
pub const CAPS: Caps = Caps {
    encoded_bytes: super::super::input::MAX_BYTES,
    memory_bytes: 512 * 1024 * 1024,
    wall_time_ms: 15_000,
    cpu_seconds: 10,
    operation_wall_time_ms: 60_000,
    operation_memory_bytes: 1024 * 1024 * 1024,
    operation_cpu_seconds: 60,
    total_pixels: 8 * 1024 * 1024,
    comparison_pixels: 1024 * 1024,
    output_bytes: 64 * 1024 * 1024,
    pages: 500,
    dimension: 16384,
    pixels: super::super::input::MAX_PIXELS,
    object_depth: 64,
    objects: 100000,
    decompressed_bytes: 64 * 1024 * 1024,
};

pub(super) fn error(code: &'static str) -> crate::Error {
    crate::Error::Document { code }
}

#[cfg(feature = "documents")]
mod process {
    use super::*;
    use std::io::{Read, Seek, Write};
    use std::process::{Command, Stdio};
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Job {
        dpi: f64,
        page: Option<usize>,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Reply {
        count: usize,
        width: u32,
        height: u32,
        error: Option<String>,
    }

    fn execute(encoded: &[u8], job: Job) -> crate::Result<(Reply, Vec<u8>)> {
        if encoded.len() as u64 > CAPS.encoded_bytes {
            return Err(error("document_encoded_limit"));
        }
        if !job.dpi.is_finite() || !(36.0..=600.0).contains(&job.dpi) {
            return Err(error("document_invalid_request"));
        }
        if job.page.is_some_and(|p| p >= CAPS.pages) {
            return Err(error("document_page_limit"));
        }
        if !cfg!(target_os = "linux") {
            return Err(error("document_isolation_unavailable"));
        }
        // Embedders supply a trusted documents-enabled CLI, never an input-supplied executable.
        let configured = WORKER
            .with(|worker| worker.borrow().clone())
            .or_else(|| std::env::var_os("SACCADE_DOCUMENT_WORKER").map(std::path::PathBuf::from));
        let current = std::env::current_exe().map_err(|_| error("document_worker_unavailable"))?;
        let launcher = if current.file_stem().is_some_and(|n| n == "saccade") {
            current
        } else if let Some(exe) = &configured {
            // Embedders provide a trusted CLI implementing both internal entry points.
            exe.clone()
        } else {
            current
                .parent()
                .ok_or_else(|| error("document_worker_unavailable"))?
                .join("saccade")
        };
        let exe = configured.unwrap_or_else(|| launcher.clone());
        let mut source = tempfile::tempfile().map_err(|_| error("document_worker_io"))?;
        serde_json::to_writer(&mut source, &job).map_err(|_| error("document_worker_io"))?;
        source
            .write_all(b"\n")
            .and_then(|()| source.write_all(encoded))
            .map_err(|_| error("document_worker_io"))?;
        source.rewind().map_err(|_| error("document_worker_io"))?;
        let mut output = tempfile::tempfile().map_err(|_| error("document_worker_io"))?;
        let started = std::time::Instant::now();
        let mut child = Command::new("/usr/bin/prlimit")
            .args([
                format!("--as={}", CAPS.memory_bytes),
                format!("--cpu={0}:{0}", CAPS.cpu_seconds),
                format!("--fsize={}", CAPS.pixels * 4 + 4096),
                "--core=0".into(),
                "--nofile=32".into(),
                "--".into(),
            ])
            .arg(launcher)
            .arg("--document-launch")
            .arg(exe)
            .env_clear()
            .env("RAYON_NUM_THREADS", "1")
            .stdin(Stdio::from(source))
            .stdout(Stdio::from(
                output
                    .try_clone()
                    .map_err(|_| error("document_worker_io"))?,
            ))
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| error("document_isolation_unavailable"))?;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error("document_worker_io"));
                }
            }
            if started.elapsed().as_millis() >= u128::from(CAPS.wall_time_ms) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error("document_wall_time_limit"));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        if !status.success() {
            return Err(error("document_worker_resource_limit"));
        }
        output.rewind().map_err(|_| error("document_worker_io"))?;
        let mut bytes = Vec::new();
        output
            .take(CAPS.pixels * 4 + 4097)
            .read_to_end(&mut bytes)
            .map_err(|_| error("document_worker_io"))?;
        let split = bytes
            .iter()
            .take(4096)
            .position(|&b| b == b'\n')
            .ok_or_else(|| error("document_worker_protocol"))?;
        let reply: Reply = serde_json::from_slice(&bytes[..split])
            .map_err(|_| error("document_worker_protocol"))?;
        if let Some(code) = &reply.error {
            // Never forward arbitrary worker diagnostics or untrusted text.
            if !CODES.contains(&code.as_str()) {
                return Err(error("document_worker_protocol"));
            }
            return Err(error(
                CODES
                    .iter()
                    .copied()
                    .find(|known| *known == code)
                    .ok_or_else(|| error("document_worker_protocol"))?,
            ));
        }
        let pixels = bytes.split_off(split + 1);
        if reply.count == 0
            || reply.count > CAPS.pages
            || reply.width > CAPS.dimension
            || reply.height > CAPS.dimension
            || u64::from(reply.width) * u64::from(reply.height) > CAPS.pixels
            || pixels.len() as u64 != u64::from(reply.width) * u64::from(reply.height) * 4
            || (job.page.is_some() && (reply.width == 0 || reply.height == 0))
            || (job.page.is_none() && (reply.width != 0 || reply.height != 0))
        {
            return Err(error("document_worker_protocol"));
        }
        Ok((reply, pixels))
    }
    const CODES: &[&str] = &[
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
    ];

    pub(super) fn count(encoded: &[u8]) -> crate::Result<usize> {
        Ok(execute(
            encoded,
            Job {
                dpi: super::super::DEFAULT_DPI,
                page: None,
            },
        )?
        .0
        .count)
    }
    pub(super) fn page(encoded: &[u8], dpi: f64, page: usize) -> crate::Result<image::RgbaImage> {
        let (reply, bytes) = execute(
            encoded,
            Job {
                dpi,
                page: Some(page),
            },
        )?;
        image::RgbaImage::from_raw(reply.width, reply.height, bytes)
            .ok_or_else(|| error("document_worker_protocol"))
    }
    pub(super) fn serve() -> u8 {
        use std::io::BufRead;
        if revoke_descriptors().is_err() {
            return 2;
        }
        // Refuse direct invocation unless exec was already admitted under every OS cap.
        let bounded = std::fs::read_to_string("/proc/self/limits")
            .ok()
            .is_some_and(|limits| {
                [
                    ("Max address space", CAPS.memory_bytes),
                    ("Max cpu time", CAPS.cpu_seconds),
                    ("Max file size", CAPS.pixels * 4 + 4096),
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
                                .all(|n| n.parse::<u64>().is_ok_and(|n| n <= *maximum))
                        })
                })
            });
        if !bounded {
            return 2;
        }
        let run = || -> crate::Result<(Reply, Vec<u8>)> {
            let mut input = std::io::BufReader::new(std::io::stdin().lock());
            let mut header = Vec::new();
            input
                .by_ref()
                .take(4096)
                .read_until(b'\n', &mut header)
                .map_err(|_| error("document_malformed"))?;
            let job: Job =
                serde_json::from_slice(&header).map_err(|_| error("document_invalid_request"))?;
            let mut encoded = Vec::new();
            input
                .take(CAPS.encoded_bytes + 1)
                .read_to_end(&mut encoded)
                .map_err(|_| error("document_malformed"))?;
            if encoded.len() as u64 > CAPS.encoded_bytes {
                return Err(error("document_encoded_limit"));
            }
            let format =
                super::super::format(&encoded).ok_or_else(|| error("document_malformed"))?;
            let count = match format {
                super::super::Format::Pdf => {
                    super::super::preflight::pdf(&encoded)?;
                    let pdf = hayro::Pdf::new(std::sync::Arc::new(encoded.clone()))
                        .map_err(|_| error("document_malformed"))?;
                    if pdf.len() > CAPS.objects {
                        return Err(error("document_objects_limit"));
                    }
                    let count = pdf.pages().len();
                    if count == 0 || count > CAPS.pages {
                        return Err(error("document_page_limit"));
                    }
                    // Inspect every page even for a count request, before later allocations.
                    for page in pdf.pages().iter() {
                        let (w, h) = page.render_dimensions();
                        super::super::dimensions(
                            f64::from(w) * job.dpi / 72.,
                            f64::from(h) * job.dpi / 72.,
                        )?;
                    }
                    count
                }
                super::super::Format::Svg => {
                    let text =
                        std::str::from_utf8(&encoded).map_err(|_| error("document_malformed"))?;
                    let doc = roxmltree::Document::parse(text)
                        .map_err(|_| error("document_malformed"))?;
                    if doc.descendants().count() > CAPS.objects {
                        return Err(error("document_objects_limit"));
                    }
                    if doc
                        .descendants()
                        .any(|n| n.ancestors().count() > CAPS.object_depth)
                    {
                        return Err(error("document_depth_limit"));
                    }
                    1
                }
            };
            let image = if let Some(page) = job.page {
                Some(super::super::rasterize(
                    &mut super::super::Local,
                    &encoded,
                    super::super::Request {
                        format,
                        dpi: job.dpi,
                        page,
                    },
                )?)
            } else {
                None
            };
            let (width, height, pixels) = image
                .map(|i| (i.width(), i.height(), i.into_raw()))
                .unwrap_or((0, 0, Vec::new()));
            Ok((
                Reply {
                    count,
                    width,
                    height,
                    error: None,
                },
                pixels,
            ))
        };
        let result = std::panic::catch_unwind(run)
            .unwrap_or_else(|_| Err(error("document_worker_resource_limit")));
        let (reply, pixels) = match result {
            Ok(v) => v,
            Err(e) => {
                let code = if let crate::Error::Document { code } = e {
                    code.to_string()
                } else {
                    "document_unsupported".into()
                };
                (
                    Reply {
                        count: 0,
                        width: 0,
                        height: 0,
                        error: Some(code),
                    },
                    Vec::new(),
                )
            }
        };
        let mut output = std::io::stdout().lock();
        if serde_json::to_writer(&mut output, &reply).is_err()
            || output.write_all(b"\n").is_err()
            || output.write_all(&pixels).is_err()
        {
            2
        } else {
            0
        }
    }
}
#[cfg(feature = "documents")]
pub(super) fn count(encoded: &[u8]) -> crate::Result<usize> {
    process::count(encoded)
}
#[cfg(feature = "documents")]
pub(super) fn page(encoded: &[u8], dpi: f64, page: usize) -> crate::Result<image::RgbaImage> {
    process::page(encoded, dpi, page)
}
/// Internal worker entry point. Call only from a trusted executable invoked under OS limits.
#[cfg(feature = "documents")]
pub fn serve() -> u8 {
    process::serve()
}

thread_local! {
    static WORKER: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
    static OPERATION: std::cell::RefCell<Option<(u64, u64)>> = const { std::cell::RefCell::new(None) };
}
/// Scoped cumulative budgets for the supervised document comparison thread.
pub struct OperationBudget(Option<(u64, u64)>);
impl Drop for OperationBudget {
    fn drop(&mut self) {
        OPERATION.with(|budget| *budget.borrow_mut() = self.0);
    }
}
/// Start fixed operation-wide pixel and write budgets; nested use preserves the prior budget.
pub fn operation_budget() -> OperationBudget {
    OperationBudget(
        OPERATION.with(|budget| budget.replace(Some((CAPS.total_pixels, CAPS.output_bytes)))),
    )
}
/// Whether the current thread belongs to a supervised document comparison.
pub fn operation_is_active() -> bool {
    OPERATION.with(|budget| budget.borrow().is_some())
}
/// Admit raster area before parent-side encoding, registration or FLIP allocation.
pub fn charge_pixels(pixels: u64) -> crate::Result<()> {
    OPERATION.with(|budget| {
        if let Some((remaining, _)) = budget.borrow_mut().as_mut() {
            if pixels > CAPS.comparison_pixels || pixels > *remaining {
                return Err(error("document_total_pixel_limit"));
            }
            *remaining -= pixels;
        }
        Ok(())
    })
}
/// Charge every requested write before it reaches disk (including report-index rows).
pub fn charge_output(bytes: u64) -> crate::Result<()> {
    OPERATION.with(|budget| {
        if let Some((_, remaining)) = budget.borrow_mut().as_mut() {
            if bytes > *remaining {
                return Err(error("document_output_byte_limit"));
            }
            *remaining -= bytes;
        }
        Ok(())
    })
}
/// Revoke all inherited descriptors except standard input/output/error before rendering.
/// Internal entry points call this only in a fresh, single-threaded process before opening owned IO.
/// Linux procfs enumerates even descriptors above the newly installed NOFILE limit.
/// Refuse on other platforms or if enumeration/closing is unavailable.
pub fn revoke_descriptors() -> crate::Result<()> {
    #[cfg(target_os = "linux")]
    {
        // Collect first: the directory iterator's own descriptor is gone before the sweep.
        // EBADF is allowed only for that already-closed enumeration descriptor.
        let descriptors = std::fs::read_dir("/proc/self/fd")
            .map_err(|_| error("document_isolation_unavailable"))?
            .map(|entry| {
                entry
                    .ok()
                    .and_then(|e| e.file_name().to_str()?.parse::<i32>().ok())
                    .ok_or_else(|| error("document_isolation_unavailable"))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        for fd in descriptors.into_iter().filter(|fd| *fd > 2) {
            match nix::unistd::close(fd) {
                Ok(()) | Err(nix::errno::Errno::EBADF) => {}
                Err(_) => return Err(error("document_isolation_unavailable")),
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    Err(error("document_isolation_unavailable"))
}
/// Internal trusted launcher: revoke descriptors before exec, including custom worker adapters.
#[cfg(feature = "documents")]
pub fn launch() -> u8 {
    if revoke_descriptors().is_err() {
        return 2;
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let Some(exe) = std::env::args_os().nth(2) else {
            return 2;
        };
        let _ = std::process::Command::new(exe)
            .arg("--document-worker")
            .env_clear()
            .env("RAYON_NUM_THREADS", "1")
            .exec();
    }
    2
}

/// Invocation-scoped trusted worker configuration without changing the process environment.
pub struct WorkerGuard(Option<std::path::PathBuf>);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        WORKER.with(|worker| *worker.borrow_mut() = self.0.take());
    }
}
/// Configure a trusted adapter for a supervised operation.
pub fn worker_scope(path: Option<&std::path::Path>) -> WorkerGuard {
    WorkerGuard(WORKER.with(|worker| worker.replace(path.map(std::path::Path::to_path_buf))))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod operation_tests {
    use super::*;
    #[test]
    fn operation_budget_is_cumulative_and_refuses_before_writes() {
        let _scope = operation_budget();
        for _ in 0..CAPS.total_pixels / CAPS.comparison_pixels {
            charge_pixels(CAPS.comparison_pixels).unwrap();
        }
        assert!(matches!(
            charge_pixels(1),
            Err(crate::Error::Document {
                code: "document_total_pixel_limit"
            })
        ));
        charge_output(CAPS.output_bytes - 1).unwrap();
        assert!(matches!(
            charge_output(2),
            Err(crate::Error::Document {
                code: "document_output_byte_limit"
            })
        ));
        charge_output(1).unwrap();
    }
}
