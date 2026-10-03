//! `saccade serve`: a local web app for browsing capture archives and
//! comparing runs with the `saccade view` pipeline.
//!
//! Security model (hard requirements, see the module tests):
//! - the server binds `127.0.0.1` only and has no option to change that;
//! - every request's `Host` must be `127.0.0.1:<port>` or `localhost:<port>`
//!   (DNS-rebinding defence); every POST also needs a matching `Origin` and the
//!   per-process random token in `X-Saccade-Token`;
//! - client paths are relative to the archive root, canonicalised, and must stay
//!   below it (no `..`, no absolute paths, no escaping symlinks); only image
//!   files are served from the archive. With several roots each is a top-level
//!   entry named after its directory; `follow_symlinks_within_roots` lets a
//!   symlink that resolves inside *any* root be browsed. Explicit symlink
//!   targets allow capture directories on external storage; lexical containment
//!   under a served root is checked before resolution. Storage probes time out
//!   and at most eight may run; background comparisons use validated local copies;
//! - the archive root is read-only: sessions and thumbnails go to the cache
//!   directory, decisions to the decisions directory, uploads to the cache.

mod api;
mod browse;
mod overview;
mod session;
mod storage;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::run::io_err;
use crate::view::ViewOptions;

/// Default upload cap per file (64 MB).
pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 64 * 1024 * 1024;

const WORKERS: usize = 8;

/// Options of [`start`].
#[derive(Debug, Clone)]
pub struct ServeOptions {
    /// The archive root: browsed read-only.
    pub root: PathBuf,
    /// More archive roots; with any, every root is a top-level entry named
    /// after its directory.
    pub extra_roots: Vec<PathBuf>,
    /// Let symlinks that resolve inside any root be browsed and served (a
    /// symlink to anywhere else stays refused).
    pub follow_symlinks_within_roots: bool,
    /// Extra directories that symlinks reached under a root may resolve into.
    /// Unavailable targets warn and are skipped at startup.
    pub symlink_targets: Vec<PathBuf>,
    /// Deadline for storage requests; default 3000 milliseconds.
    pub fs_timeout_ms: u64,
    #[cfg(test)]
    pub(crate) probe_delay_ms: u64,
    #[cfg(test)]
    pub(crate) review_ttl_ms: u64,
    #[cfg(test)]
    pub(crate) probe_timeout_ms: Option<u64>,
    /// TCP port on `127.0.0.1`; 0 picks a free one.
    pub port: u16,
    /// Cache directory (sessions, thumbnails, uploads).
    pub cache_dir: PathBuf,
    /// Directory the decision files are written to.
    pub decisions_dir: PathBuf,
    /// Template for the options of every view session (sidecar name, HDR
    /// settings, regions, FLIP pixels per degree).
    pub view: ViewOptions,
    /// Upload cap per file, in bytes.
    pub max_upload_bytes: u64,
}

impl ServeOptions {
    /// Options for `root` with the platform default cache and decisions
    /// directories and an ephemeral port.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            extra_roots: Vec::new(),
            follow_symlinks_within_roots: false,
            symlink_targets: Vec::new(),
            fs_timeout_ms: 3000,
            #[cfg(test)]
            probe_delay_ms: 0,
            #[cfg(test)]
            review_ttl_ms: 600_000,
            #[cfg(test)]
            probe_timeout_ms: None,
            port: 0,
            cache_dir: default_cache_dir(),
            decisions_dir: default_decisions_dir(),
            view: ViewOptions::default(),
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
        }
    }
}

pub use crate::local::{default_cache_dir, default_decisions_dir};

use crate::root_policy::{Egress, Root, RootPolicy};

/// Shared server state.
pub(crate) struct State {
    pub roots: Vec<Root>,
    pub policy: RootPolicy,
    pub storage: storage::Storage,
    pub cache: PathBuf,
    pub decisions: PathBuf,
    pub token: String,
    pub port: u16,
    pub view: ViewOptions,
    pub max_upload: u64,
    pub sessions: Mutex<HashMap<String, session::SessionState>>,
    pub overviews: Mutex<HashMap<String, Arc<overview::Job>>>,
    pub inbox_lock: Mutex<()>,
    pub reviews: Mutex<HashMap<String, api::review_api::ReviewSession>>,
    pub review_ttl: Duration,
}

impl State {
    /// More than one root: paths carry the root's name as first segment.
    pub fn multi(&self) -> bool {
        self.roots.len() > 1
    }

    /// The innermost root containing the canonical path `canon`.
    pub fn root_of(&self, canon: &Path) -> Option<&Root> {
        self.roots
            .iter()
            .filter(|r| canon.starts_with(&r.path))
            .max_by_key(|r| r.path.components().count())
    }

    /// Whether a canonical path reached from `home` may be served: it stays
    /// inside `home`, or (with `--follow-symlinks-within-roots`) inside any root.
    pub fn allows(&self, home: &Path, canon: &Path) -> bool {
        self.policy.allows(home, canon)
    }
}

/// A running server; dropping it stops the workers.
pub struct ServeHandle {
    port: u16,
    token: String,
    cache: PathBuf,
    decisions: PathBuf,
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl ServeHandle {
    /// The port the server listens on (`127.0.0.1`).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The per-process token embedded in served pages.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// The canonical cache directory selected at startup.
    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    /// The canonical decisions directory selected at startup.
    pub fn decisions_dir(&self) -> &Path {
        &self.decisions
    }

    /// Blocks until the server is stopped (never, for the CLI).
    pub fn wait(mut self) {
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for ServeHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// True when `inner` is `outer` or below it (both canonical).
fn is_within(inner: &Path, outer: &Path) -> bool {
    inner.starts_with(outer)
}

/// Starts the server on `127.0.0.1` and returns once it is listening.
pub fn start(opts: ServeOptions) -> Result<ServeHandle> {
    opts.view.perf.resolved_floor()?;
    if opts.fs_timeout_ms == 0 {
        return Err(Error::Config("fs_timeout_ms must be positive".into()));
    }
    let storage = storage::Storage::new(Duration::from_millis(opts.fs_timeout_ms));
    let mut roots: Vec<Root> = Vec::new();
    for given in std::iter::once(&opts.root).chain(&opts.extra_roots) {
        let probe = given.clone();
        let path = storage
            .run(move || {
                let target = crate::paths::canonicalize(&probe)?;
                if !target.is_dir() {
                    return Err(std::io::Error::other("not a directory"));
                }
                Ok(target)
            })
            .map_err(|_| Error::Config(format!("storage not reachable: {}", given.display())))?
            .map_err(io_err(format!(
                "resolving archive root {}",
                given.display()
            )))?;
        if roots.iter().any(|r| r.path == path) {
            return Err(Error::Config(format!(
                "{} is given as a root twice",
                path.display()
            )));
        }
        let base = path
            .file_name()
            .map_or_else(|| "root".to_owned(), |n| n.to_string_lossy().into_owned());
        let mut name = base.clone();
        let mut n = 2;
        while roots.iter().any(|r| r.name == name) {
            name = format!("{base}-{n}");
            n += 1;
        }
        roots.push(Root {
            name,
            path,
            egress: Egress::Deny,
        });
    }
    let mut symlink_targets = Vec::new();
    for given in &opts.symlink_targets {
        let probe = given.clone();
        match storage.run(move || {
            let p = crate::paths::canonicalize(&probe)?;
            if !p.is_dir() {
                return Err(std::io::Error::other("not a directory"));
            }
            Ok(p)
        }) {
            Ok(Ok(target)) => symlink_targets.push(target),
            Ok(Err(error)) => eprintln!(
                "warning: skipping --symlink-target {}: {error}",
                given.display()
            ),
            Err(()) => eprintln!(
                "warning: skipping --symlink-target {}: storage not reachable within the probe limit",
                given.display()
            ),
        }
    }
    let policy = RootPolicy::from_normalized(
        roots.clone(),
        None,
        opts.follow_symlinks_within_roots,
        symlink_targets,
    )?;
    let mut dirs = Vec::new();
    for (what, dir) in [
        ("cache", &opts.cache_dir),
        ("decisions", &opts.decisions_dir),
    ] {
        std::fs::create_dir_all(dir).map_err(io_err(format!(
            "creating the {what} directory {}",
            dir.display()
        )))?;
        let canon = crate::paths::canonicalize(dir)
            .map_err(io_err(format!("resolving {}", dir.display())))?;
        if roots.iter().any(|r| is_within(&canon, &r.path)) || !policy.allows_output(&canon) {
            return Err(Error::Config(format!(
                "the {what} directory {} is inside an archive root; serve never writes under a root",
                canon.display()
            )));
        }
        dirs.push(canon);
    }
    let (cache, decisions) = (dirs[0].clone(), dirs[1].clone());
    for sub in ["sessions", "thumbs", "uploads", "pairs", "runs", "sources"] {
        let p = cache.join(sub);
        std::fs::create_dir_all(&p).map_err(io_err(format!("creating {}", p.display())))?;
    }
    opts.view.meta.checker()?;

    let server = tiny_http::Server::http(("127.0.0.1", opts.port))
        .map_err(|e| Error::Config(format!("cannot listen on 127.0.0.1:{}: {e}", opts.port)))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or_else(|| Error::Config("server has no IP address".into()))?;
    let token = random_token();
    let inbox = decisions.join("inbox");
    std::fs::create_dir_all(&inbox).map_err(io_err("creating inbox directory".into()))?;
    let inbox_canon =
        crate::paths::canonicalize(&inbox).map_err(io_err("resolving inbox directory".into()))?;
    if inbox_canon != inbox || roots.iter().any(|r| inbox_canon.starts_with(&r.path)) {
        return Err(Error::Config(
            "inbox must be a real directory outside archive roots".into(),
        ));
    }
    crate::inbox::write_discovery(&cache, port, token.clone())?;
    #[cfg(test)]
    let storage = {
        let mut storage = storage;
        storage.set_delay(opts.probe_delay_ms);
        if let Some(ms) = opts.probe_timeout_ms {
            storage.set_timeout(Duration::from_millis(ms));
        }
        storage
    };
    let state = Arc::new(State {
        roots,
        policy,
        storage,
        cache,
        decisions,
        token: token.clone(),
        port,
        view: opts.view,
        max_upload: opts.max_upload_bytes,
        sessions: Mutex::new(HashMap::new()),
        overviews: Mutex::new(HashMap::new()),
        inbox_lock: Mutex::new(()),
        reviews: Mutex::new(HashMap::new()),
        #[cfg(test)]
        review_ttl: Duration::from_millis(opts.review_ttl_ms),
        #[cfg(not(test))]
        review_ttl: Duration::from_secs(600),
    });
    let server = Arc::new(server);
    let stop = Arc::new(AtomicBool::new(false));
    let threads = (0..WORKERS)
        .map(|_| {
            let (server, state, stop) = (server.clone(), state.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match server.recv_timeout(Duration::from_millis(200)) {
                        Ok(Some(req)) => api::handle(&state, req),
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            })
        })
        .collect();
    Ok(ServeHandle {
        port,
        token,
        cache: state.cache.clone(),
        decisions: state.decisions.clone(),
        stop,
        threads,
    })
}

pub(crate) use crate::local::random_token;

/// Constant-time-ish equality for the token check.
pub(crate) fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
