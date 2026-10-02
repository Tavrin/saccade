//! Local storage defaults and opaque identifiers, without a server dependency.

use std::io::Read;
use std::path::PathBuf;

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

/// `$XDG_CACHE_HOME/saccade` (or `~/.cache/saccade`).
pub fn default_cache_dir() -> PathBuf {
    env_dir("XDG_CACHE_HOME")
        .unwrap_or_else(|| home().join(".cache"))
        .join("saccade")
}

/// `$XDG_DATA_HOME/saccade/decisions` (or `~/.local/share/saccade/decisions`).
pub fn default_decisions_dir() -> PathBuf {
    env_dir("XDG_DATA_HOME")
        .unwrap_or_else(|| home().join(".local").join("share"))
        .join("saccade")
        .join("decisions")
}

/// 128 random bits as hex: `/dev/urandom` where available, else the std
/// hasher's per-process random keys mixed with the clock.
pub(crate) fn random_token() -> String {
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
