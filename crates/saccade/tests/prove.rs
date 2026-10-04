#![allow(clippy::unwrap_used, missing_docs)]

use std::process::Command;

#[test]
fn prove_routes_identity_and_performance_without_removing_old_commands() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base");
    let arm = temp.path().join("arm");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&arm).unwrap();
    let image = image::RgbImage::from_pixel(16, 16, image::Rgb([80, 80, 80]));
    image.save(base.join("frame.png")).unwrap();
    image.save(arm.join("frame.png")).unwrap();
    let bin = env!("CARGO_BIN_EXE_saccade");
    let help = Command::new(bin).arg("--help").output().unwrap();
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("compare") && text.contains("prove") && text.contains("review"));
    assert!(text.contains("Advanced:"));
    let proof = Command::new(bin)
        .args(["prove", "identity"])
        .arg(&base)
        .arg(&arm)
        .args([
            "--out",
            "proof",
            "--json",
            "--ppd",
            "50",
            "--labels",
            "old,new",
            "--junit",
            "proof.xml",
        ])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        proof.status.success(),
        "{}",
        String::from_utf8_lossy(&proof.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(temp.path().join("proof/saccade-report.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["config"]["mode"], "identity");
    assert_eq!(report["config"]["pixels_per_degree"], 50.0);
    assert_eq!(report["config"]["labels"]["baseline"], "old");
    assert!(temp.path().join("proof.xml").is_file());
    let legacy = Command::new(bin)
        .arg("identity")
        .arg(&base)
        .arg(&arm)
        .args(["--out", "legacy"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(legacy.status.success());
    #[cfg(feature = "graphics")]
    {
        let perf = Command::new(bin)
            .args(["prove", "performance"])
            .arg(&base)
            .arg(&arm)
            .args(["--out", "performance", "--json"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            perf.status.success(),
            "{}",
            String::from_utf8_lossy(&perf.stderr)
        );
        assert!(
            temp.path()
                .join("performance/saccade-ablate.v1.json")
                .is_file()
        );
    }
}
