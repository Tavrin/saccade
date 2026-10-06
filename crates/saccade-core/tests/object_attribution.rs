//! Synthetic end-to-end object ID sidecar comparison.
#![allow(clippy::expect_used, missing_docs)]

use image::{Rgb, RgbImage};
use saccade_core::config::RunConfig;
use saccade_core::report::Status;

#[test]
fn comparison_publishes_object_error_shares_in_json_and_html() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("base");
    let cap = dir.path().join("cap");
    let out = dir.path().join("report");
    std::fs::create_dir_all(&base).expect("base");
    std::fs::create_dir_all(&cap).expect("cap");
    let reference = RgbImage::from_pixel(32, 32, Rgb([40, 40, 40]));
    let mut test = reference.clone();
    for y in 8..24 {
        for x in 8..24 {
            test.put_pixel(x, y, Rgb([240, 240, 240]));
        }
    }
    reference.save(base.join("scene.png")).expect("baseline");
    test.save(cap.join("scene.png")).expect("capture");
    RgbImage::from_pixel(32, 32, Rgb([0, 0, 1]))
        .save(cap.join("scene.object-id.png"))
        .expect("IDs");
    std::fs::write(
        cap.join("scene.object-id.json"),
        r#"{"schema":"saccade-object-ids.v1","kind":"object","ids":{"1":"rock_lichen"}}"#,
    )
    .expect("legend");
    let report = saccade_core::run::run(&base, &cap, &out, &RunConfig::default()).expect("compare");
    let entry = &report.entries[0];
    assert_eq!(entry.status, Status::Fail);
    assert!(!entry.hotspots.is_empty());
    let attribution = &entry.object_attribution[0];
    assert_eq!(attribution.hotspots[0].contributions[0].name, "rock_lichen");
    assert_eq!(attribution.hotspots[0].contributions[0].error_share, 1.0);
    assert!(out.join(&attribution.image_path).is_file());
    let saved: saccade_core::Report = serde_json::from_slice(
        &std::fs::read(out.join(saccade_core::report::REPORT_FILE_NAME)).expect("report"),
    )
    .expect("report JSON");
    assert_eq!(
        saved.entries[0].object_attribution,
        entry.object_attribution
    );
    let html = std::fs::read_to_string(out.join("index.html")).expect("HTML");
    assert!(html.contains("Object and material attribution"));
    assert!(html.contains("rock_lichen"));
}
