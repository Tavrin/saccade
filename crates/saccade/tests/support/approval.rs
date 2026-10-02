#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn draft(report: &Path, plan: &Path, names: &[&str], prune: bool) -> PathBuf {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_saccade"));
    cmd.args(["approve", "--report"])
        .arg(report)
        .args(["--dry-run", "--out"])
        .arg(plan);
    if names.is_empty() {
        cmd.arg("--all-failing");
    } else {
        for name in names {
            cmd.arg("--entry").arg(name);
        }
    }
    if prune {
        cmd.arg("--prune-missing");
    }
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(!plan.join("receipt.json").exists());
    plan.join("decision.json")
}
