//! Debounced filesystem comparison with native notifications and polling fallback.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::{Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::RunConfig;
use crate::{Error, Report, Result};

/// Watch settings. Reports are replaced using the normal safe-output rules.
#[derive(Clone)]
pub struct WatchOptions {
    /// Read-only baseline.
    pub baseline: PathBuf,
    /// Directory watched recursively.
    pub capture: PathBuf,
    /// Report destination outside both inputs.
    pub out: PathBuf,
    /// Comparison configuration.
    pub config: RunConfig,
    /// Quiet period after the last event.
    pub debounce: Duration,
}

/// A running watcher. Drop stops and joins it.
pub struct WatchHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// Keep the backend alive for the entire event loop.
enum Backend {
    Native(RecommendedWatcher),
    Poll(PollWatcher),
}
impl Backend {
    fn watch(&mut self, path: &Path) -> notify::Result<()> {
        match self {
            Self::Native(w) => w.watch(path, RecursiveMode::Recursive),
            Self::Poll(w) => w.watch(path, RecursiveMode::Recursive),
        }
    }
}

/// Compare once, without installing a filesystem watcher.
pub fn once(opts: &WatchOptions) -> Result<Report> {
    crate::run::run(&opts.baseline, &opts.capture, &opts.out, &opts.config)
}

/// Run until `stop` is set. An initial comparison always precedes change reruns.
/// A native backend failure switches to content-comparing polling.
pub fn run(
    opts: WatchOptions,
    stop: Arc<AtomicBool>,
    mut result: impl FnMut(Result<Report>),
) -> Result<()> {
    opts.config.validate()?;
    if !opts.capture.is_dir() {
        return Err(Error::Config("watch capture must be a directory".into()));
    }
    crate::run::guard_output_dir(
        &opts.out,
        &[&opts.baseline, &opts.capture],
        &[crate::report::REPORT_FILE_NAME, crate::run::RUN_SENTINEL],
    )?;
    let (tx, rx) = mpsc::channel();
    let handler_tx = tx.clone();
    let native = RecommendedWatcher::new(
        move |event| {
            let _ = handler_tx.send(event);
        },
        Config::default(),
    );
    let poll = || {
        let tx = tx.clone();
        PollWatcher::new(
            move |event| {
                let _ = tx.send(event);
            },
            Config::default()
                .with_poll_interval(Duration::from_millis(200))
                .with_compare_contents(true),
        )
        .map(Backend::Poll)
        .map_err(|e| Error::Config(format!("starting polling watcher: {e}")))
    };
    let mut backend = match native {
        Ok(w) => Backend::Native(w),
        Err(_) => poll()?,
    };
    if backend.watch(&opts.capture).is_err() {
        backend = poll()?;
        backend
            .watch(&opts.capture)
            .map_err(|e| Error::Config(format!("watching capture: {e}")))?;
    }
    result(once(&opts));
    let mut pending: Option<Instant> = None;
    while !stop.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(event)) if !matches!(event.kind, notify::EventKind::Access(_)) => {
                pending = Some(Instant::now())
            }
            Ok(Err(_)) => {
                backend = poll()?;
                backend
                    .watch(&opts.capture)
                    .map_err(|e| Error::Config(format!("polling capture: {e}")))?;
                pending = Some(Instant::now());
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if pending.is_some_and(|at| at.elapsed() >= opts.debounce) {
            pending = None;
            result(once(&opts));
        }
    }
    Ok(())
}

/// Start a watcher in a server-owned thread, reporting setup failures to its callback.
pub fn start(
    opts: WatchOptions,
    mut result: impl FnMut(Result<Report>) + Send + 'static,
) -> WatchHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        if let Err(e) = run(opts, thread_stop, &mut result) {
            result(Err(e));
        }
    });
    WatchHandle {
        stop,
        thread: Some(thread),
    }
}
