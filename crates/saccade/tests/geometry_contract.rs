//! Feature-gated static mesh CLI contract.
#![cfg(feature = "geometry")]
#![allow(clippy::unwrap_used, missing_docs)]
use std::process::Command;
#[test]
fn geometry_and_exact_identity_have_separate_claims_and_exits() {
    let tmp = tempfile::tempdir().unwrap();
    let a = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
    std::fs::write(tmp.path().join("a.obj"), a).unwrap();
    std::fs::write(tmp.path().join("b.obj"), a.replace("v 0 0 0", "v 0 0 1")).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .current_dir(tmp.path())
            .args(args)
            .output()
            .unwrap()
    };
    let diff = run(&[
        "experiment",
        "geometry",
        "a.obj",
        "b.obj",
        "--unit",
        "m",
        "--samples",
        "16",
        "--json",
    ]);
    assert!(
        diff.status.success(),
        "{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert!(
        json["analysis"]["evidence"]["sampled_hausdorff"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(
        run(&[
            "prove",
            "mesh-identity",
            "a.obj",
            "b.obj",
            "--unit",
            "m",
            "--json"
        ])
        .status
        .code(),
        Some(1)
    );
    assert!(
        run(&["prove", "mesh-identity", "a.obj", "a.obj", "--unit", "m"])
            .status
            .success()
    );
}
