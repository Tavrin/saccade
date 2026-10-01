//! Portable provenance paths shared by reports and command output.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Resolves an existing path, including symlinks and Windows 8.3 short names.
/// Ordinary Windows paths use non-verbatim prefixes for display and comparison.
/// `dunce` retains a verbatim prefix only when simplifying would change the
/// path's meaning (for example, reserved names or paths beyond `MAX_PATH`).
/// Long-lived roots should be resolved once during startup and reused.
pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    dunce::canonicalize(native(path.as_ref()))
}

fn slash_separated(path: &str) -> String {
    path.replace('\\', "/")
}

/// Formats a recorded path with `/` separators and simplifies Windows verbatim
/// prefixes when doing so preserves the path's meaning. Filesystem containment
/// checks use the same spelling returned by [`canonicalize`].
pub fn portable(path: &Path) -> String {
    slash_separated(&dunce::simplified(path).to_string_lossy())
}

fn restore_verbatim(path: &str) -> Option<String> {
    path.starts_with("//?/").then(|| path.replace('/', "\\"))
}

/// Restores the native spelling of a slash-separated Windows verbatim path.
/// Such prefixes must be retained for paths that cannot be safely simplified
/// (long paths, UNC roots or reserved names). Other paths are borrowed unchanged.
pub fn native(path: &Path) -> Cow<'_, Path> {
    if cfg!(windows)
        && let Some(restored) = path.to_str().and_then(restore_verbatim)
    {
        Cow::Owned(PathBuf::from(restored))
    } else {
        Cow::Borrowed(path)
    }
}

/// Records `path` relative to `base`, unless absolute provenance is requested.
/// Different platform roots fall back to an absolute path.
pub fn record(path: &Path, base: &Path, absolute: bool) -> String {
    let path = crate::run::normalise_path(path);
    if absolute {
        return portable(&path);
    }
    let base = crate::run::normalise_path(base);
    record_relative(&path, &base)
}

fn record_relative(path: &Path, base: &Path) -> String {
    let target: Vec<_> = path.components().collect();
    let origin: Vec<_> = base.components().collect();
    if target.first() != origin.first() {
        return portable(path);
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
    portable(&relative)
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
    let p = native(Path::new(recorded));
    if p.is_absolute() {
        p.into_owned()
    } else {
        document.parent().unwrap_or(Path::new(".")).join(p.as_ref())
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
            if input.is_file()
                && let Some(parent) = input.parent()
            {
                paths.push(crate::run::normalise_path(parent));
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn windows_separators_are_portable_on_any_host() {
        for (native, expected) in [
            (
                r"..\baseline\nested\scene.png",
                "../baseline/nested/scene.png",
            ),
            (r"C:\captures\scene.png", "C:/captures/scene.png"),
            (r"\\server\share\scene.png", "//server/share/scene.png"),
            (
                "../baseline/nested/scene.png",
                "../baseline/nested/scene.png",
            ),
        ] {
            assert_eq!(slash_separated(native), expected);
        }
    }

    #[test]
    fn windows_verbatim_paths_restore_without_changing_their_components() {
        for native in [
            r"\\?\C:\captures\scene.png",
            r"\\?\UNC\server\share\scene.png",
            r"\\?\C:\captures\name.\scene.png",
        ] {
            assert_eq!(
                restore_verbatim(&slash_separated(native)).as_deref(),
                Some(native)
            );
        }
        assert!(restore_verbatim("C:/captures/scene.png").is_none());
        assert!(restore_verbatim("//server/share/scene.png").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_are_relative_only_when_their_prefixes_match() {
        for (path, base, expected) in [
            (r"C:\inputs\scene.png", r"C:\reports", "../inputs/scene.png"),
            (r"C:\inputs\scene.png", r"D:\reports", "C:/inputs/scene.png"),
            (
                r"\\server\share\inputs\scene.png",
                r"\\server\share\reports",
                "../inputs/scene.png",
            ),
            (
                r"\\server\share\inputs\scene.png",
                r"\\server\other\reports",
                "//server/share/inputs/scene.png",
            ),
        ] {
            let recorded = record_relative(Path::new(path), Path::new(base));
            assert_eq!(recorded, expected);
            assert_eq!(
                Path::new(&recorded).is_relative(),
                Path::new(path).components().next() == Path::new(base).components().next()
            );
            assert!(!recorded.contains('\\'));
        }
    }

    #[test]
    fn recorded_paths_are_relative_slash_separated_and_resolvable() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("inputs/nested/scene.png");
        std::fs::create_dir_all(input.parent().unwrap()).unwrap();
        std::fs::write(&input, b"image").unwrap();
        let output = tmp.path().join("reports/session");
        // The report directory need not exist yet; its spelling stays stable
        // after creation, including non-verbatim canonical paths on Windows.
        let recorded = record(&input, &output, false);
        assert_eq!(recorded, "../../inputs/nested/scene.png");
        std::fs::create_dir_all(&output).unwrap();
        assert_eq!(record(&input, &output, false), recorded);
        assert_eq!(record(&output, &output, false), ".");
        assert_eq!(
            canonicalize(resolve(&recorded, &output.join("report.json"))).unwrap(),
            canonicalize(&input).unwrap()
        );
        let absolute = record(&input, &output, true);
        assert!(!absolute.contains('\\'), "{absolute}");
        assert!(!absolute.starts_with("//?/"), "ordinary paths simplify");
        assert_eq!(
            native(Path::new(&absolute)).as_ref(),
            canonicalize(&input).unwrap()
        );
    }

    #[test]
    fn missing_path_suffixes_fold_without_escaping_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("missing/../reports/session");
        let expected = canonicalize(tmp.path()).unwrap().join("reports/session");
        assert_eq!(crate::run::normalise_path(&path), expected);
        assert_eq!(crate::explain::absolute(&path), expected);
    }

    // Unix symlinks need no special privileges. Windows runners may not have
    // symlink privileges; the portable path/identity tests above run everywhere.
    #[cfg(unix)]
    #[test]
    fn symlinked_temp_roots_preserve_provenance_and_containment() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        let alias = tmp.path().join("alias");
        std::fs::create_dir_all(real.join("inputs")).unwrap();
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        assert_eq!(
            record(&alias.join("inputs"), &real.join("reports/session"), false),
            "../../inputs"
        );
        let through_alias = crate::run::normalise_path(&alias.join("inputs/new/report"));
        assert!(through_alias.starts_with(canonicalize(&real).unwrap()));
        assert!(
            crate::run::guard_output_dir(
                &alias.join("inputs/new/report"),
                &[&real.join("inputs")],
                &[]
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_parent_traversal_is_resolved_before_missing_suffixes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(outside.join("child")).unwrap();
        std::os::unix::fs::symlink(outside.join("child"), root.join("link")).unwrap();
        let path = root.join("link/../missing/report");
        let resolved = crate::run::normalise_path(&path);
        assert_eq!(
            resolved,
            canonicalize(&outside).unwrap().join("missing/report")
        );
        assert!(!resolved.starts_with(canonicalize(&root).unwrap()));
        assert_eq!(crate::explain::absolute(&path), resolved);
    }
}
