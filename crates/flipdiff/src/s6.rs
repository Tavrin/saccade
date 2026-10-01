//! Bisect/watch/ask command wiring and loopback-only inbox client.
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use clap::Args;
use flipdiff_core::bisect::{self, BisectOptions, Probe};
use flipdiff_core::inbox::{AskResult, Item, Question, ServeInfo};
use flipdiff_core::watch::WatchOptions;
use serde_json::Value;

use crate::agent::{CliError, DEFAULT_TOP_FAILING, result_value};

#[derive(Args)]
pub(crate) struct BisectArgs {
    /// Ordered run directories, oldest first (repeatable).
    #[arg(long, num_args = 1..)]
    pub runs: Vec<PathBuf>,
    /// One ordered run path per line.
    #[arg(long)]
    pub runs_from: Option<PathBuf>,
    /// Reference for existing runs (default: first run).
    #[arg(long)]
    pub good: Option<PathBuf>,
    /// Revision range good..bad; reads git history without changing checkout.
    #[arg(long)]
    pub git: Option<String>,
    /// User shell command. Use unquoted {rev} and {out} placeholders.
    #[arg(long)]
    pub capture_cmd: Option<String>,
    /// Reference images for command capture mode.
    #[arg(long)]
    pub reference: Option<PathBuf>,
    /// Explicit FLIP threshold relaxes native sample identity.
    #[arg(long)]
    pub threshold: Option<f64>,
    /// mean, p95, p99 or max (default max).
    #[arg(long)]
    pub metric: Option<String>,
    /// Select image names by glob.
    #[arg(long)]
    pub entries: Option<String>,
    /// Report directory, separate from inputs.
    #[arg(long, default_value = "bisect-report")]
    pub out: PathBuf,
    /// Print flipdiff-bisect.v1 JSON.
    #[arg(long)]
    pub json: bool,
}

pub(crate) fn options(
    threshold: Option<f64>,
    metric: Option<&str>,
    entries: Option<String>,
) -> Result<BisectOptions, CliError> {
    let metric = metric
        .map(|m| match m {
            "mean" => Ok(flipdiff_core::Metric::Mean),
            "p95" => Ok(flipdiff_core::Metric::P95),
            "p99" => Ok(flipdiff_core::Metric::P99),
            "max" => Ok(flipdiff_core::Metric::Max),
            _ => Err(CliError::usage("metric must be mean, p95, p99 or max")),
        })
        .transpose()?;
    if threshold.is_some_and(|t| !t.is_finite() || !(0.0..=1.0).contains(&t)) {
        return Err(CliError::usage(
            "threshold must be finite and between 0 and 1",
        ));
    }
    Ok(BisectOptions {
        threshold,
        metric,
        entries,
    })
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub(crate) fn bisect(args: BisectArgs) -> Result<u8, CliError> {
    let opts = options(args.threshold, args.metric.as_deref(), args.entries)?;
    let result = if let Some(range) = args.git {
        if !args.runs.is_empty() || args.runs_from.is_some() || args.good.is_some() {
            return Err(CliError::usage(
                "--git cannot be combined with existing-run options",
            ));
        }
        let capture_cmd = args
            .capture_cmd
            .ok_or_else(|| CliError::usage("--git needs --capture-cmd"))?;
        let reference = args
            .reference
            .ok_or_else(|| CliError::usage("--git needs --reference"))?;
        let (good, bad) = range
            .split_once("..")
            .filter(|(g, b)| {
                !g.is_empty()
                    && !b.is_empty()
                    && !b.starts_with('.')
                    && !g.starts_with('-')
                    && !b.starts_with('-')
            })
            .ok_or_else(|| CliError::usage("--git needs good..bad"))?;
        // Resolve both endpoints to immutable commits before passing the range.
        let resolve = |rev: &str| -> Result<String, CliError> {
            let output = std::process::Command::new("git")
                .args([
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{rev}^{{commit}}"),
                ])
                .output()
                .map_err(|e| CliError::io(format!("git rev-parse: {e}")))?;
            if !output.status.success() {
                return Err(CliError::usage(String::from_utf8_lossy(&output.stderr)));
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        };
        let (good, bad) = (resolve(good)?, resolve(bad)?);
        let output = std::process::Command::new("git")
            .args(["rev-list", "--reverse", &format!("{good}..{bad}")])
            .output()
            .map_err(|e| CliError::io(format!("git rev-list: {e}")))?;
        if !output.status.success() {
            return Err(CliError::usage(String::from_utf8_lossy(&output.stderr)));
        }
        let targets: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect();
        if !reference.is_dir() {
            return Err(CliError::usage("reference must be an existing directory"));
        }
        // Validate glob/config before executing any user command.
        let cfg = flipdiff_core::config::RunConfig {
            ignore: opts.entries.iter().cloned().collect(),
            ..Default::default()
        };
        cfg.validate()?;
        let out = bisect::prepare(&args.out, &[&reference])?;
        let mut result = bisect::search(&targets, |i, rev| {
            let capture = out.join(format!(
                "capture-{i}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos())
            ));
            std::fs::create_dir(&capture).map_err(core_io("creating fresh capture directory"))?;
            let log = std::fs::File::create(capture.join("capture.log"))
                .map_err(core_io("opening capture log"))?;
            let stderr = log.try_clone().map_err(core_io("opening capture stderr"))?;
            let command = capture_cmd
                .replace("{rev}", &shell_quote(rev))
                .replace("{out}", &shell_quote(&capture.to_string_lossy()));
            let status = std::process::Command::new("sh")
                .args(["-c", &command])
                .stdout(log)
                .stderr(stderr)
                .status()
                .map_err(core_io("running user capture command"))?;
            if !status.success() {
                return Ok(Probe {
                    index: i,
                    target: rev.into(),
                    verdict: "skip".into(),
                    report_dir: None,
                    reason: Some(format!(
                        "capture command exited {status}; log: {}",
                        capture.join("capture.log").display()
                    )),
                });
            }
            bisect::compare_probe(
                &reference,
                &capture,
                &bisect::probe_dir(&out, i)?,
                &opts,
                i,
                rev.into(),
            )
        })?;
        if result.last_good.is_none() {
            result.last_good = Some(good);
        }
        bisect::write_result(&out, &result)?;
        result
    } else {
        if args.capture_cmd.is_some() || args.reference.is_some() {
            return Err(CliError::usage("capture options require --git"));
        }
        let mut runs = args.runs;
        if let Some(file) = args.runs_from {
            if !runs.is_empty() {
                return Err(CliError::usage("use --runs or --runs-from"));
            }
            runs = std::fs::read_to_string(file)
                .map_err(|e| CliError::io(format!("reading runs file: {e}")))?
                .lines()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .collect();
        }
        bisect::runs(&runs, args.good.as_deref(), &args.out, &opts)?
    };
    if args.json {
        emit_line(&serde_json::to_string(&result)?)?;
    } else {
        emit_line(&result.text())?;
    }
    Ok(result.exit_code())
}

#[derive(Args)]
pub(crate) struct WatchArgs {
    pub baseline: PathBuf,
    pub capture: PathBuf,
    #[arg(long, default_value = "watch-report")]
    pub out: PathBuf,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long, default_value_t = 500)]
    pub debounce_ms: u64,
    #[arg(long)]
    pub once: bool,
    #[arg(long)]
    pub json: bool,
}

pub(crate) fn watch_text(report: &flipdiff_core::Report) -> String {
    let worst = crate::agent::failing_entries(report)
        .first()
        .copied()
        .or_else(|| {
            report
                .entries
                .iter()
                .max_by(|a, b| a.value.unwrap_or(0.0).total_cmp(&b.value.unwrap_or(0.0)))
        });
    let description = worst
        .and_then(|e| e.diagnostics.as_ref())
        .map(|d| d.description.as_str())
        .unwrap_or("no diagnosed change");
    let t = &report.totals;
    format!(
        "{}: {} pass, {} fail, {} error, {} missing, {} new; worst: {}; {}",
        if report.is_regression() {
            "regression"
        } else {
            "pass"
        },
        t.pass,
        t.fail,
        t.error,
        t.missing,
        t.new,
        worst.map_or("none", |e| e.name.as_str()),
        description
    )
}

pub(crate) fn watch(args: WatchArgs) -> Result<u8, CliError> {
    let opts = WatchOptions {
        baseline: args.baseline,
        capture: args.capture,
        out: args.out,
        config: crate::load_config(args.config.as_deref())?,
        debounce: Duration::from_millis(args.debounce_ms),
    };
    let stop = Arc::new(AtomicBool::new(false));
    if !args.once {
        let ctrl_stop = stop.clone();
        ctrlc::set_handler(move || ctrl_stop.store(true, Ordering::SeqCst))
            .map_err(|e| CliError::io(format!("installing Ctrl-C handler: {e}")))?;
    }
    let mut code = 2;
    let mut emit_error = None;
    let mut callback = |result: flipdiff_core::Result<flipdiff_core::Report>| {
        let output = match result {
            Ok(report) => {
                code = u8::from(report.is_regression());
                if args.json {
                    serde_json::to_string(&result_value(
                        &report,
                        &opts.out.join(flipdiff_core::report::REPORT_FILE_NAME),
                        DEFAULT_TOP_FAILING,
                        false,
                    ))
                    .map_err(CliError::from)
                } else {
                    Ok(watch_text(&report))
                }
            }
            Err(e) => {
                code = 2;
                let e: CliError = e.into();
                if args.json {
                    serde_json::to_string(&e.value()).map_err(CliError::from)
                } else {
                    Ok(format!("error: {e}"))
                }
            }
        };
        if let Err(e) = output.and_then(|line| {
            emit_line(&if args.json {
                line
            } else {
                crate::escape_control(&line)
            })
        }) {
            emit_error = Some(e);
            stop.store(true, Ordering::SeqCst);
        }
    };
    if args.once {
        callback(flipdiff_core::watch::once(&opts));
    } else {
        flipdiff_core::watch::run(opts.clone(), stop.clone(), &mut callback)?;
    }
    if let Some(e) = emit_error {
        return Err(e);
    }
    Ok(code)
}

#[derive(Args)]
pub(crate) struct AskArgs {
    /// Local serve URL; only literal 127.0.0.1 is allowed.
    #[arg(long)]
    pub serve: String,
    #[arg(long)]
    pub question: String,
    #[arg(long, value_delimiter = ',')]
    pub answers: Vec<String>,
    #[arg(long)]
    pub context: Option<String>,
    #[arg(long)]
    pub link: Option<String>,
    #[arg(long)]
    pub from: Option<String>,
    /// Serve's discovery directory (default platform cache).
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long)]
    pub wait: bool,
    #[arg(long, default_value_t = 600)]
    pub timeout: u64,
    #[arg(long)]
    pub json: bool,
}

/// Strict loopback client, no DNS, proxies or redirect following.
pub(crate) struct InboxClient {
    port: u16,
    token: String,
}
impl InboxClient {
    pub fn new(serve: &str, cache: &Path) -> Result<Self, CliError> {
        let port = serve
            .strip_prefix("http://127.0.0.1:")
            .and_then(|s| s.strip_suffix('/').or(Some(s)))
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|p| *p != 0)
            .ok_or_else(|| CliError::usage("serve URL must be http://127.0.0.1:<port>"))?;
        let path = cache.join("serve.json");
        let meta = std::fs::symlink_metadata(&path).map_err(|e| {
            CliError::io(format!("reading serve discovery {}: {e}", path.display()))
        })?;
        if !meta.is_file() || meta.len() > 4096 {
            return Err(CliError::usage(
                "serve discovery must be a small regular file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(CliError::usage("serve discovery must be private (0600)"));
            }
        }
        let info: ServeInfo = serde_json::from_slice(
            &std::fs::read(path)
                .map_err(|e| CliError::io(format!("reading serve discovery: {e}")))?,
        )?;
        if info.port != port
            || info.token.len() != 32
            || !info.token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(CliError::usage(
                "serve discovery port/token mismatch; use this serve process's --cache-dir",
            ));
        }
        Ok(Self {
            port,
            token: info.token,
        })
    }
    fn request(&self, path: &str, body: Option<&Value>) -> Result<Value, CliError> {
        let mut stream = TcpStream::connect_timeout(
            &SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port).into(),
            Duration::from_secs(3),
        )
        .map_err(|e| CliError::io(format!("connecting to serve: {e}")))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| CliError::io(e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| CliError::io(e.to_string()))?;
        let body_text = body
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_default();
        let auth = if body.is_some() {
            format!(
                "Origin: http://127.0.0.1:{}\r\nX-Flipdiff-Token: {}\r\n",
                self.port, self.token
            )
        } else {
            String::new()
        };
        let request = format!(
            "{} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body_text}",
            if body.is_some() { "POST" } else { "GET" },
            self.port,
            body_text.len()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| CliError::io(format!("sending inbox request: {e}")))?;
        let mut response = Vec::new();
        stream
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut response)
            .map_err(|e| CliError::io(format!("reading inbox response: {e}")))?;
        if response.len() > 2 * 1024 * 1024 {
            return Err(CliError::io("inbox response too large"));
        }
        let split = response
            .windows(4)
            .position(|b| b == b"\r\n\r\n")
            .ok_or_else(|| CliError::io("invalid HTTP response"))?;
        let headers = String::from_utf8_lossy(&response[..split]);
        if headers.split_whitespace().nth(1) != Some("200") {
            return Err(CliError::io(format!(
                "inbox HTTP {}: {}",
                headers.lines().next().unwrap_or("error"),
                String::from_utf8_lossy(&response[split + 4..])
            )));
        }
        Ok(serde_json::from_slice(&response[split + 4..])?)
    }
    pub fn get(&self, id: &str) -> Result<Item, CliError> {
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CliError::usage("invalid inbox id"));
        }
        Ok(serde_json::from_value(
            self.request(&format!("/api/inbox/{id}"), None)?,
        )?)
    }
    pub fn ask(
        &self,
        mut question: Question,
        wait: bool,
        timeout: u64,
    ) -> Result<AskResult, CliError> {
        if let Some(link) = &mut question.link {
            for host in ["127.0.0.1", "localhost"] {
                let origin = format!("http://{host}:{}", self.port);
                if let Some(local) = link.strip_prefix(&origin).filter(|p| p.starts_with('/')) {
                    *link = local.to_owned();
                    break;
                }
            }
        }
        flipdiff_core::inbox::validate(&question)?;
        let response = self.request("/api/inbox", Some(&serde_json::to_value(question)?))?;
        let id = response["id"]
            .as_str()
            .ok_or_else(|| CliError::io("inbox response missing id"))?;
        let start = Instant::now();
        let deadline = Duration::from_secs(timeout);
        loop {
            let item = self.get(id)?;
            let timed_out = wait && item.status == "open" && start.elapsed() >= deadline;
            if !wait || item.status == "answered" || timed_out {
                return Ok(AskResult {
                    schema: "flipdiff-ask-result.v1".into(),
                    id: id.into(),
                    status: item.status,
                    answer: item.answer,
                    note: item.note,
                    timed_out,
                    url: format!("http://127.0.0.1:{}/inbox#{id}", self.port),
                });
            }
            std::thread::sleep(
                Duration::from_millis(100).min(deadline.saturating_sub(start.elapsed())),
            );
        }
    }
}

pub(crate) fn ask(args: AskArgs) -> Result<u8, CliError> {
    let client = InboxClient::new(
        &args.serve,
        &args
            .cache_dir
            .unwrap_or_else(flipdiff_core::serve::default_cache_dir),
    )?;
    let result = client.ask(
        Question {
            question: args.question,
            allowed_answers: args.answers,
            context: args.context,
            link: args.link,
            from: args.from,
        },
        args.wait,
        args.timeout,
    )?;
    if args.json {
        emit_line(&serde_json::to_string(&result)?)?;
    } else {
        emit_line(&format!(
            "{}: {}{}; {}",
            result.id,
            result.status,
            result
                .answer
                .as_ref()
                .map_or(String::new(), |a| format!(" ({a})")),
            result.url
        ))?;
    }
    Ok(if result.timed_out { 2 } else { 0 })
}

fn core_io(context: &str) -> impl FnOnce(std::io::Error) -> flipdiff_core::Error {
    let context = context.to_owned();
    move |source| flipdiff_core::Error::Io { context, source }
}

fn emit_line(text: &str) -> Result<(), CliError> {
    crate::emit(&format!("{text}\n"))
}
