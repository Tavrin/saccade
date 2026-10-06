//! Output discovery, linking and worst-first region export at the executable boundary.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use image::{Rgb, RgbImage};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str], dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn")
}

fn json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Many harmless differences, one broad mild failure and one tiny severe defect.
fn compare_fixture(tmp: &Path) -> std::path::PathBuf {
    let (base, cap) = (tmp.join("base"), tmp.join("cap"));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&cap).unwrap();
    for i in 0..4 {
        let name = format!("harmless-{i}.png");
        RgbImage::from_pixel(64, 64, Rgb([128; 3]))
            .save(base.join(&name))
            .unwrap();
        RgbImage::from_pixel(64, 64, Rgb([129; 3]))
            .save(cap.join(&name))
            .unwrap();
    }
    RgbImage::from_pixel(64, 64, Rgb([100; 3]))
        .save(base.join("broad.png"))
        .unwrap();
    RgbImage::from_pixel(64, 64, Rgb([150; 3]))
        .save(cap.join("broad.png"))
        .unwrap();
    RgbImage::from_pixel(64, 64, Rgb([128; 3]))
        .save(base.join("defect.png"))
        .unwrap();
    let mut defect = RgbImage::from_pixel(64, 64, Rgb([128; 3]));
    for x in 30..34 {
        for y in 30..34 {
            defect.put_pixel(x, y, Rgb([255, 0, 0]));
        }
    }
    defect.save(cap.join("defect.png")).unwrap();
    let out = tmp.join("report");
    run(
        &[
            "compare",
            base.to_str().unwrap(),
            cap.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        tmp,
    );
    out
}

#[test]
fn manifest_links_a_report_and_fails_loudly_when_it_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let out = compare_fixture(tmp.path());
    let build = run(
        &["manifest", "build", out.to_str().unwrap(), "--json"],
        tmp.path(),
    );
    assert!(build.status.success(), "{build:?}");
    let manifest = json(&out.join("saccade-manifest.json"));
    assert_eq!(manifest["schema"], "saccade-manifest.v1");
    assert_eq!(manifest["approval"]["state"], "none");
    let id = manifest["reports"][0]["report_id"].as_str().unwrap();
    let link = tmp.path().join("link.json");
    let made = run(
        &[
            "manifest",
            "link",
            out.to_str().unwrap(),
            "--report-id",
            id,
            "--out",
            link.to_str().unwrap(),
        ],
        tmp.path(),
    );
    assert!(made.status.success(), "{made:?}");
    let ok = run(&["manifest", "verify", link.to_str().unwrap()], tmp.path());
    assert!(ok.status.success(), "{ok:?}");
    let kind = run(&["manifest", "classify", out.to_str().unwrap()], tmp.path());
    assert!(String::from_utf8_lossy(&kind.stdout).contains("report_directory"));

    std::fs::write(out.join("saccade-report.v1.json"), b"{}").unwrap();
    let stale = run(
        &["manifest", "verify", link.to_str().unwrap(), "--json"],
        tmp.path(),
    );
    assert!(!stale.status.success());
    assert!(
        String::from_utf8_lossy(&stale.stdout).contains("stale_link"),
        "{stale:?}"
    );
}

#[test]
fn export_regions_puts_the_tiny_severe_defect_first_with_coordinates() {
    let tmp = tempfile::tempdir().unwrap();
    let out = compare_fixture(tmp.path());
    let export = tmp.path().join("export");
    let done = run(
        &[
            "export-regions",
            out.join("saccade-report.v1.json").to_str().unwrap(),
            "--out",
            export.to_str().unwrap(),
            "--top",
            "3",
        ],
        tmp.path(),
    );
    assert!(done.status.success(), "{done:?}");
    let document = json(&export.join("saccade-region-export.v1.json"));
    let first = &document["regions"][0];
    assert_eq!(first["entry"], "defect.png");
    let [x, y, w, h] = first["rect_px"]
        .as_array()
        .unwrap()
        .clone()
        .try_into()
        .unwrap();
    assert!(x.as_u64().unwrap() <= 33 && y.as_u64().unwrap() <= 33);
    assert!(w.as_u64().unwrap() >= 4 && h.as_u64().unwrap() >= 4);
    assert!(
        export
            .join(first["files"]["capture"].as_str().unwrap())
            .is_file()
    );
    // A second run regenerates its own output; a foreign directory is refused.
    assert!(
        run(
            &[
                "export-regions",
                out.join("saccade-report.v1.json").to_str().unwrap(),
                "--out",
                export.to_str().unwrap()
            ],
            tmp.path()
        )
        .status
        .success()
    );
    let foreign = tmp.path().join("foreign");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("keep.txt"), b"x").unwrap();
    let refused = run(
        &[
            "export-regions",
            out.join("saccade-report.v1.json").to_str().unwrap(),
            "--out",
            foreign.to_str().unwrap(),
        ],
        tmp.path(),
    );
    assert!(!refused.status.success());
    assert!(foreign.join("keep.txt").is_file());
}
