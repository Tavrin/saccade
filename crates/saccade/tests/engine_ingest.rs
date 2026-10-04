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

#[test]
fn unreal_manifest_rejects_parent_absolute_and_symlink_escape() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("results");
    std::fs::create_dir(&root).unwrap();
    let outside = tmp.path().join("outside.png");
    image(&outside, 40);
    image(&root.join("inside.png"), 220);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("link.png")).unwrap();
    let paths = vec![
        "../outside.png".to_string(),
        outside.to_string_lossy().into_owned(),
    ];
    #[cfg(unix)]
    let paths = paths
        .into_iter()
        .chain(["link.png".into()])
        .collect::<Vec<_>>();
    for (index, escape) in paths.iter().enumerate() {
        let manifest = root.join("comparison.json");
        std::fs::write(&manifest, serde_json::json!({"ReportApprovedFilePath":escape,"ReportIncomingFilePath":"inside.png"}).to_string()).unwrap();
        let out = tmp.path().join(format!("out-{index}"));
        let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(["ingest", "unreal"])
            .arg(&manifest)
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap();
        assert_ne!(result.status.code(), Some(0), "{result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("ingest path"),
            "{result:?}"
        );
        assert!(!out.exists());
    }
}

#[cfg(unix)]
#[test]
fn directory_layouts_reject_symlinked_images() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside.png");
    image(&outside, 40);
    let blender = tmp.path().join("blender");
    std::fs::create_dir_all(blender.join("lighting/ref")).unwrap();
    std::os::unix::fs::symlink(&outside, blender.join("lighting/ref/lamp.png")).unwrap();
    let bevy_ref = tmp.path().join("bevy-ref");
    let bevy_cap = tmp.path().join("bevy-cap");
    std::fs::create_dir(&bevy_ref).unwrap();
    std::fs::create_dir(&bevy_cap).unwrap();
    std::os::unix::fs::symlink(&outside, bevy_ref.join("screenshot-0.png")).unwrap();
    image(&bevy_cap.join("screenshot-0.png"), 220);
    let unity = tmp.path().join("unity/Assets");
    std::fs::create_dir_all(unity.join("ReferenceImages")).unwrap();
    std::fs::create_dir_all(unity.join("ActualImages")).unwrap();
    std::os::unix::fs::symlink(&outside, unity.join("ReferenceImages/lamp.png")).unwrap();
    image(&unity.join("ActualImages/lamp.png"), 220);
    for (format, inputs) in [
        ("blender", vec![blender]),
        ("bevy", vec![bevy_ref, bevy_cap]),
        ("unity", vec![unity]),
    ] {
        let out = tmp.path().join(format!("{format}-out"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_saccade"));
        command
            .arg("ingest")
            .arg(format)
            .args(inputs)
            .arg("--out")
            .arg(&out);
        let result = command.output().unwrap();
        assert_ne!(result.status.code(), Some(0), "{format}: {result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("symlink"),
            "{format}: {result:?}"
        );
        assert!(!out.exists());
    }
}
