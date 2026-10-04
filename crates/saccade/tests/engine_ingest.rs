#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use std::path::Path;
use std::process::Command;

fn image(path: &Path, shade: u8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    RgbImage::from_pixel(32, 32, Rgb([shade; 3]))
        .save(path)
        .unwrap();
}

fn ingest(format: &str, inputs: &[&Path], out: &Path) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_saccade"));
    command.arg("ingest").arg(format);
    command.args(inputs).arg("--out").arg(out).arg("--json");
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{format}: {output:?}");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["totals"]["fail"], 1, "{format}: {result}");
    assert!(out.join("report/saccade-report.v1.json").is_file());
    assert!(out.join("ingest-mapping.json").is_file());
}

#[test]
fn documented_engine_layouts_pair_synthetic_images() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let blender = root.join("blender");
    image(&blender.join("lighting/ref/lamp.png"), 40);
    image(&blender.join("lighting/lamp.png"), 220);
    ingest("blender", &[&blender], &root.join("blender-out"));

    let bevy_ref = root.join("bevy-reference");
    let bevy_run = root.join("bevy-run");
    image(&bevy_ref.join("screenshot-0.png"), 40);
    image(&bevy_run.join("screenshot-0.png"), 220);
    ingest("bevy", &[&bevy_ref, &bevy_run], &root.join("bevy-out"));

    let unity = root.join("unity/Assets");
    image(
        &unity.join("ReferenceImages/Linear/LinuxEditor/lamp.png"),
        40,
    );
    image(
        &unity.join("ActualImages/Linear/LinuxEditor/Vulkan/lamp.png"),
        220,
    );
    ingest("unity", &[&unity], &root.join("unity-out"));

    let unreal = root.join("unreal");
    image(&unreal.join("approved.png"), 40);
    image(&unreal.join("incoming.png"), 220);
    let results = unreal.join("comparison.json");
    std::fs::write(&results, r#"[{"ReportApprovedFilePath":"approved.png","ReportIncomingFilePath":"incoming.png","ReportComparisonFilePath":"","ScreenshotPath":"Lamp","SourcePlatform":"Linux","SourceRHI":"Vulkan"}]"#).unwrap();
    ingest("unreal", &[&results], &root.join("unreal-out"));
}
