//! `flipdiff serve`: a local web app for browsing capture archives and
//! comparing runs with the `flipdiff view` pipeline.
//!
//! Security model (hard requirements, see the module tests):
//! - the server binds `127.0.0.1` only and has no option to change that;
//! - every request's `Host` must be `127.0.0.1:<port>` or `localhost:<port>`
//!   (DNS-rebinding defence); every POST also needs a matching `Origin` and the
//!   per-process random token in `X-Flipdiff-Token`;
//! - client paths are relative to the archive root, canonicalised, and must stay
//!   below it (no `..`, no absolute paths, no escaping symlinks); only image
//!   files are served from the archive;
//! - the archive root is read-only: sessions and thumbnails go to the cache
//!   directory, decisions to the decisions directory, uploads to the cache.

mod api;
mod browse;
mod session;

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::meta::MetaChecker;
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
            port: 0,
            cache_dir: default_cache_dir(),
            decisions_dir: default_decisions_dir(),
            view: ViewOptions::default(),
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// `$XDG_CACHE_HOME/flipdiff` (or `~/.cache/flipdiff`).
pub fn default_cache_dir() -> PathBuf {
    env_dir("XDG_CACHE_HOME")
        .unwrap_or_else(|| home().join(".cache"))
        .join("flipdiff")
}

/// `$XDG_DATA_HOME/flipdiff/decisions` (or `~/.local/share/flipdiff/decisions`).
pub fn default_decisions_dir() -> PathBuf {
    env_dir("XDG_DATA_HOME")
        .unwrap_or_else(|| home().join(".local").join("share"))
        .join("flipdiff")
        .join("decisions")
}

/// Shared server state.
pub(crate) struct State {
    pub root: PathBuf,
    pub cache: PathBuf,
    pub decisions: PathBuf,
    pub token: String,
    pub port: u16,
    pub view: ViewOptions,
    pub meta: MetaChecker,
    pub max_upload: u64,
    pub sessions: Mutex<HashMap<String, session::SessionState>>,
}

/// A running server; dropping it stops the workers.
pub struct ServeHandle {
    port: u16,
    token: String,
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
    let root = opts.root.canonicalize().map_err(io_err(format!(
        "resolving archive root {}",
        opts.root.display()
    )))?;
    if !root.is_dir() {
        return Err(Error::Config(format!(
            "{} is not a directory",
            root.display()
        )));
    }
    let mut dirs = Vec::new();
    for (what, dir) in [
        ("cache", &opts.cache_dir),
        ("decisions", &opts.decisions_dir),
    ] {
        std::fs::create_dir_all(dir).map_err(io_err(format!(
            "creating the {what} directory {}",
            dir.display()
        )))?;
        let canon = dir
            .canonicalize()
            .map_err(io_err(format!("resolving {}", dir.display())))?;
        if is_within(&canon, &root) {
            return Err(Error::Config(format!(
                "the {what} directory {} is inside the archive root; serve never writes under the root",
                canon.display()
            )));
        }
        dirs.push(canon);
    }
    let (cache, decisions) = (dirs[0].clone(), dirs[1].clone());
    for sub in ["sessions", "thumbs", "uploads", "pairs"] {
        let p = cache.join(sub);
        std::fs::create_dir_all(&p).map_err(io_err(format!("creating {}", p.display())))?;
    }
    let meta = opts.view.meta.checker()?;

    let server = tiny_http::Server::http(("127.0.0.1", opts.port))
        .map_err(|e| Error::Config(format!("cannot listen on 127.0.0.1:{}: {e}", opts.port)))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or_else(|| Error::Config("server has no IP address".into()))?;
    let token = random_token();
    let state = Arc::new(State {
        root,
        cache,
        decisions,
        token: token.clone(),
        port,
        view: opts.view,
        meta,
        max_upload: opts.max_upload_bytes,
        sessions: Mutex::new(HashMap::new()),
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
        stop,
        threads,
    })
}

/// 128 random bits as hex: `/dev/urandom` where available, else the std
/// hasher's per-process random keys mixed with the clock.
fn random_token() -> String {
    let mut buf = [0u8; 16];
    let from_os = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_ok();
    if !from_os {
        use std::hash::{BuildHasher, Hasher};
        for chunk in buf.chunks_mut(8) {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos()),
            );
            let bytes = h.finish().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

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
