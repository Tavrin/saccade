#![cfg(feature = "compression")]
#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::quality::{self, Artifact, Candidate, Manifest, Stage};
#[test]
fn compression_metrics_match_official_nonidentical_references() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../saccade-core/tests/fixtures/compression-reference");
    let values: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("values.json")).unwrap()).unwrap();
    for (name, hash) in values["files_sha256"].as_object().unwrap() {
        assert_eq!(
            saccade_core::localized::digest(&std::fs::read(root.join(name)).unwrap()),
            hash.as_str().unwrap()
        );
    }
    let artifact = |name: &str| Artifact {
        path: name.into(),
        sha256: saccade_core::localized::digest(&std::fs::read(root.join(name)).unwrap()),
    };
    for pair in values["pairs"].as_array().unwrap() {
        let reference = pair["reference"].as_str().unwrap();
        let distorted = pair["distorted"].as_str().unwrap();
        let manifest = Manifest {
            schema: "saccade-quality-sweep.v1".into(),
            original: artifact(reference),
            reference: artifact(reference),
            minimum_score: -1000.0,
            maximum_bytes: 100000,
            viewing_conditions: "sRGB D65; 100%; 60cm; review pending".into(),
            candidates: vec![Candidate {
                id: "candidate".into(),
                stages: vec![Stage {
                    id: "delivery".into(),
                    encoder: "synthetic".into(),
                    quality: 100.0,
                    subsampling: "4:4:4".into(),
                    pixel_policy: "normalized_srgb_opaque".into(),
                    dimensions: [64, 64],
                    output: artifact(distorted),
                }],
            }],
        };
        let sweep = quality::sweep(&root, manifest).unwrap();
        assert_eq!(sweep.coverage, "complete", "{sweep:?}");
        let stage = serde_json::to_value(&sweep.candidates[0].stages[0]).unwrap();
        assert_eq!(stage["incremental_score"], stage["cumulative_score"]);
        assert_eq!(
            stage["incremental_butteraugli"],
            stage["cumulative_butteraugli"]
        );
        for (actual, expected, tolerance) in [
            (
                "cumulative_score",
                "ssimulacra2",
                "ssimulacra2_absolute_tolerance",
            ),
            (
                "cumulative_score",
                "cloudinary_ssimulacra2",
                "ssimulacra2_absolute_tolerance",
            ),
            (
                "cumulative_butteraugli",
                "butteraugli",
                "butteraugli_absolute_tolerance",
            ),
        ] {
            let actual = stage[actual].as_f64().unwrap();
            let expected = pair[expected].as_f64().unwrap();
            let tolerance = values[tolerance].as_f64().unwrap();
            println!(
                "reference {distorted} {expected} actual {actual} delta {} tolerance {tolerance}",
                actual - expected
            );
            assert!(
                (actual - expected).abs() <= tolerance,
                "{distorted} {actual} != reference {expected} within {tolerance}"
            );
            if distorted != reference && expected < 100.0 && expected > 30.0 {
                assert!(
                    (100.0 - expected).abs() > tolerance,
                    "constant 100 must fail"
                );
            }
        }
    }
}
