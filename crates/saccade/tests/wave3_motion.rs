#![allow(clippy::unwrap_used, missing_docs)]
use std::process::Command;
#[test]
fn motion_front_door_reports_raw_verdict_and_rejects_stale_vectors() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a.png");
    let b = d.path().join("b.png");
    let out = d.path().join("report.json");
    let image = image::RgbaImage::from_fn(32, 32, |x, y| {
        let v = (x.wrapping_mul(31) ^ y.wrapping_mul(57)) as u8;
        image::Rgba([v, v, v, 255])
    });
    image.save(&a).unwrap();
    image.save(&b).unwrap();
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args([
                "review",
                "motion",
                a.to_str().unwrap(),
                b.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--json",
            ])
            .args(extra)
            .output()
            .unwrap()
    };
    let result = run(&[]);
    if !cfg!(feature = "dense-motion") {
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stdout).contains("feature_unavailable"));
        assert!(!out.exists());
        return;
    }
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: saccade_core::dense_motion::Report =
        serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(report.raw_flip.mean, 0.0);
    assert!(!report.raw_regression);
    assert!(report.renderer.is_none());
    let vector = d.path().join("vectors.json");
    let sidecar = d.path().join("sidecar.json");
    let buffer = saccade_core::dense_motion::Buffer {
        schema: "saccade-vector-buffer.v1".into(),
        vectors: vec![[0.0; 2]; 1024],
        valid: vec![true; 1024],
    };
    std::fs::write(&vector, serde_json::to_vec(&buffer).unwrap()).unwrap();
    let s = saccade_core::dense_motion::Sidecar {
        schema: "saccade-motion-vectors.v1".into(),
        dimensions: [32, 32],
        reference_sha256: saccade_core::localized::digest(&std::fs::read(&a).unwrap()),
        candidate_sha256: saccade_core::localized::digest(&std::fs::read(&b).unwrap()),
        vectors_sha256: "0".repeat(64),
        units: saccade_core::dense_motion::Units::Pixels,
        origin: saccade_core::dense_motion::Origin::TopLeft,
        direction: saccade_core::dense_motion::Direction::ReferenceToCandidate,
        reference_jitter_px: [0.0; 2],
        candidate_jitter_px: [0.0; 2],
        includes_jitter: false,
        frame_interval_ms: 16.0,
        producer: serde_json::json!({"fixture":"synthetic"}),
    };
    std::fs::write(&sidecar, serde_json::to_vec(&s).unwrap()).unwrap();
    let stale = run(&[
        "--vectors",
        vector.to_str().unwrap(),
        "--sidecar",
        sidecar.to_str().unwrap(),
    ]);
    assert_eq!(stale.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&stale.stdout).contains("identity"));
}
