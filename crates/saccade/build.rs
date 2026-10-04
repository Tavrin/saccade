//! Captures build identity for the CLI without requiring Git in crate tarballs.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
        .map(|s| s.trim().to_owned())
}

fn checkout_root(manifest: &Path) -> Option<PathBuf> {
    let root = manifest.parent()?.parent()?;
    if !root.join(".git").exists() {
        return None;
    }
    let top = git(root, &["rev-parse", "--show-toplevel"])?;
    (Path::new(&top) == root).then(|| root.to_owned())
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let mut full = None;
    let mut short = None;
    let mut dirty = None;

    if let Some(root) = checkout_root(&manifest) {
        // Worktrees store HEAD in a per-worktree gitdir and branch refs in the
        // common gitdir. Ask Git for both paths rather than assuming .git is a dir.
        println!("cargo:rerun-if-changed={}", root.join(".git").display());
        for name in ["HEAD", "packed-refs"] {
            if let Some(path) = git(&root, &["rev-parse", "--git-path", name]) {
                println!("cargo:rerun-if-changed={path}");
            }
        }
        if let Some(reference) = git(&root, &["symbolic-ref", "HEAD"])
            && let Some(path) = git(&root, &["rev-parse", "--git-path", &reference])
        {
            println!("cargo:rerun-if-changed={path}");
        }
        // The status snapshot is part of the build identity. Recheck it when
        // worktree files change, including newly added source files.
        println!("cargo:rerun-if-changed={}", root.display());
        full = git(&root, &["rev-parse", "HEAD"]);
        short = git(&root, &["rev-parse", "--short=12", "HEAD"]);
        dirty = git(
            &root,
            &["status", "--porcelain", "--untracked-files=normal"],
        )
        .map(|status| !status.is_empty());
    }

    let rustc = env::var_os("RUSTC")
        .and_then(|path| Command::new(path).arg("--version").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned());
    let suffix = short.as_ref().map_or_else(String::new, |commit| {
        format!(
            "+g{commit}{}",
            if dirty == Some(true) { ".dirty" } else { "" }
        )
    });
    let version = format!("{}{}", env!("CARGO_PKG_VERSION"), suffix);
    println!(
        "cargo:rustc-env=SACCADE_GIT_COMMIT={}",
        full.unwrap_or_default()
    );
    println!(
        "cargo:rustc-env=SACCADE_GIT_COMMIT_SHORT={}",
        short.unwrap_or_default()
    );
    println!(
        "cargo:rustc-env=SACCADE_GIT_DIRTY={}",
        dirty.map_or("", |value| if value { "true" } else { "false" })
    );
    println!("cargo:rustc-env=SACCADE_BUILD_PROFILE={profile}");
    println!("cargo:rustc-env=SACCADE_RUSTC_VERSION={rustc}");
    println!("cargo:rustc-env=SACCADE_DISPLAY_VERSION={version}");
}
