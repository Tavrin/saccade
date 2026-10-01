//! Portable provenance paths shared by reports and command output.

use std::path::{Path, PathBuf};

/// Records `path` relative to `base`, unless absolute provenance is requested.
/// Different platform roots fall back to an absolute path.
pub fn record(path: &Path, base: &Path, absolute: bool) -> String {
    let path = crate::run::normalise_path(path);
    if absolute {
        return path.display().to_string();
    }
    let base = crate::run::normalise_path(base);
    let target: Vec<_> = path.components().collect();
    let origin: Vec<_> = base.components().collect();
    if target.first() != origin.first() {
        return path.display().to_string();
    }
    let common = target
        .iter()
        .zip(&origin)
        .take_while(|(a, b)| a == b)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..origin.len() {
        relative.push("..");
    }
    for c in target.iter().skip(common) {
        relative.push(c.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    relative.display().to_string()
}

/// Records a path relative to the current working directory.
pub fn cwd(path: &Path, absolute: bool) -> String {
    record(
        path,
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        absolute,
    )
}

/// Resolves a recorded path against the document that contains it.
pub fn resolve(recorded: &str, document: &Path) -> PathBuf {
    let p = Path::new(recorded);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        document.parent().unwrap_or(Path::new(".")).join(p)
    }
}

/// Whether a name matches any selection glob (or the selection is empty).
/// Callers validate globs before using this predicate.
pub fn matches_entries(globs: &[String], name: &str) -> bool {
    globs.is_empty()
        || globs
            .iter()
            .any(|g| crate::config::compile_glob(g).is_ok_and(|m| m.is_match(name)))
}

/// Rebases decisions when saved away from their source viewer or report.
pub fn rebase_decisions(
    d: &mut crate::view::Decisions,
    source: &Path,
    destination: &Path,
    absolute: bool,
) {
    let base = destination.parent().unwrap_or(Path::new("."));
    for dir in &mut d.dirs {
        *dir = record(&resolve(dir, source), base, absolute);
    }
    for set in &mut d.sets {
        if let Some(dir) = &mut set.chosen_dir {
            *dir = record(&resolve(dir, source), base, absolute);
        }
    }
}

/// Replaces source path text in entry errors and warnings with relative paths.
pub fn redact_entry(entry: &mut crate::Entry, base: &Path, inputs: &[&Path]) {
    let redact = |s: &mut String| {
        for input in inputs {
            let mut paths = vec![input.to_path_buf(), crate::run::normalise_path(input)];
            if input.is_file() {
                if let Some(parent) = input.parent() {
                    paths.push(crate::run::normalise_path(parent));
                }
            }
            for p in paths {
                if p.is_absolute() {
                    *s = s.replace(&p.display().to_string(), &record(&p, base, false));
                }
            }
        }
    };
    if let Some(e) = &mut entry.error {
        redact(e);
    }
    for w in &mut entry.warnings {
        redact(w);
    }
}
