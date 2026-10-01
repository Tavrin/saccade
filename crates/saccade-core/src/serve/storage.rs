//! Bounded storage probes and local, validated inputs for background jobs.
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use super::State;
use super::browse::{PathError, hash128, is_image_name, is_plain_rel, resolve_under};

const MAX_PROBES: usize = 8;

pub(crate) struct Storage {
    active: Arc<AtomicUsize>,
    timeout: Duration,
    #[cfg(test)]
    delay: std::sync::atomic::AtomicU64,
}

impl Storage {
    pub fn new(timeout: Duration) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            timeout,
            #[cfg(test)]
            delay: std::sync::atomic::AtomicU64::new(0),
        }
    }

    #[cfg(test)]
    pub fn set_delay(&self, ms: u64) {
        self.delay.store(ms, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Timed-out probes retain their slot until the OS operation returns.
    /// Saturation fails immediately instead of spawning more hung threads.
    pub fn run<T: Send + 'static>(&self, op: impl FnOnce() -> T + Send + 'static) -> Result<T, ()> {
        // `try_update` requires Rust 1.95; retain the workspace's Rust 1.88 MSRV.
        let mut n = self.active.load(Ordering::Acquire);
        loop {
            if n >= MAX_PROBES {
                return Err(());
            }
            match self
                .active
                .compare_exchange_weak(n, n + 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(current) => n = current,
            }
        }
        #[cfg(test)]
        let delay = self.delay.load(Ordering::Relaxed);
        let active = self.active.clone();
        let (tx, rx) = sync_channel(1);
        let spawned = std::thread::Builder::new()
            .name("saccade-storage".into())
            .spawn(move || {
                struct Slot(Arc<AtomicUsize>);
                impl Drop for Slot {
                    fn drop(&mut self) {
                        self.0.fetch_sub(1, Ordering::AcqRel);
                    }
                }
                let _slot = Slot(active);
                #[cfg(test)]
                std::thread::sleep(Duration::from_millis(delay));
                let _ = tx.send(op());
            });
        if spawned.is_err() {
            self.active.fetch_sub(1, Ordering::AcqRel);
            return Err(());
        }
        rx.recv_timeout(self.timeout).map_err(|_| ())
    }
}

/// Absolute dashboard paths are checked lexically before any filesystem call.
pub(crate) fn absolute_rel(state: &State, path: &str) -> Result<String, PathError> {
    let abs = Path::new(path);
    if !abs.is_absolute()
        || path.contains('\0')
        || abs.components().any(|c| matches!(c, Component::ParentDir))
    {
        return Err(PathError::Invalid);
    }
    let root = state.root_of(abs).ok_or(PathError::Escapes)?;
    let sub = super::browse::rel_in(&root.path, abs);
    if !is_plain_rel(&sub) {
        return Err(PathError::Invalid);
    }
    let rel = if !state.multi() {
        sub
    } else if sub.is_empty() {
        root.name.clone()
    } else {
        format!("{}/{sub}", root.name)
    };
    resolve_under(state, &rel)?;
    Ok(rel)
}

/// Copy only validated image and metadata inputs to the local cache. Background
/// view/overview threads therefore never perform I/O against remote storage.
/// Each descendant is resolved through the same root policy as `/img`.
pub(crate) fn snapshot(state: &State, rel: &str) -> Result<PathBuf, PathError> {
    let source = resolve_under(state, rel)?;
    if !source.is_dir() {
        return Err(PathError::Missing);
    }
    let mut queue = vec![(String::new(), 0usize)];
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();
    let mut fingerprint = vec![rel.to_owned()];
    while let Some((sub, depth)) = queue.pop() {
        let dir_rel = if sub.is_empty() {
            rel.to_owned()
        } else if rel.is_empty() {
            sub.clone()
        } else {
            format!("{rel}/{sub}")
        };
        let dir = resolve_under(state, &dir_rel)?;
        let canon = crate::paths::canonicalize(&dir).map_err(|_| PathError::Missing)?;
        if !seen.insert(canon) {
            continue;
        }
        let rd = std::fs::read_dir(&dir).map_err(|_| PathError::Missing)?;
        for entry in rd {
            let entry = entry.map_err(|_| PathError::Missing)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if sub.is_empty() {
                name.clone()
            } else {
                format!("{sub}/{name}")
            };
            let child_rel = if rel.is_empty() {
                child.clone()
            } else {
                format!("{rel}/{child}")
            };
            // Ignore unrelated files, but validate directories and all input links.
            let ft = entry.file_type().map_err(|_| PathError::Missing)?;
            let relevant = is_image_name(&name)
                || name.ends_with(&state.view.meta.name)
                || name == state.view.perf.name;
            if !ft.is_dir() && !ft.is_symlink() && !relevant {
                continue;
            }
            let resolved = match resolve_under(state, &child_rel) {
                Ok(p) => p,
                Err(PathError::Escapes) if !relevant => continue,
                Err(e) => return Err(e),
            };
            let meta = std::fs::metadata(&resolved).map_err(|_| PathError::Missing)?;
            if meta.is_dir() {
                if depth >= 64 {
                    return Err(PathError::Invalid);
                }
                queue.push((child, depth + 1));
            } else if meta.is_file() && relevant {
                let canonical =
                    crate::paths::canonicalize(&resolved).map_err(|_| PathError::Missing)?;
                if is_image_name(&name) && !is_image_name(&canonical.to_string_lossy()) {
                    continue;
                }
                fingerprint.push(format!(
                    "{child}:{}:{:?}:{}",
                    meta.len(),
                    meta.modified().ok(),
                    canonical.display()
                ));
                if name == state.view.perf.name && meta.len() <= 4 * 1024 * 1024 {
                    let hash =
                        crate::run::sha256_file(&resolved).map_err(|_| PathError::Missing)?;
                    fingerprint.push(format!("{child}:perf-content:{hash}"));
                }
                files.push((child, resolved));
            }
        }
    }
    fingerprint.sort();
    let parts: Vec<&[u8]> = fingerprint.iter().map(|s| s.as_bytes()).collect();
    let dest = state.cache.join("sources").join(hash128(&parts));
    if dest.is_dir() {
        return Ok(dest);
    }
    let tmp = state
        .cache
        .join("sources")
        .join(format!(".tmp-{}", super::session::unique()));
    let copied = (|| {
        std::fs::create_dir_all(&tmp).map_err(|_| PathError::Invalid)?;
        for (name, source) in files {
            let to = tmp.join(name);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|_| PathError::Invalid)?;
            }
            std::fs::copy(source, to).map_err(|_| PathError::Missing)?;
        }
        // A competing identical snapshot may already have been published.
        if std::fs::rename(&tmp, &dest).is_err() && !dest.is_dir() {
            return Err(PathError::Invalid);
        }
        Ok(dest)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    copied
}
