#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;
use std::process::Command;

use image::{Rgb, RgbImage};

fn save(dir: &Path, name: &str, v: u8) {
    std::fs::create_dir_all(dir).expect("mkdir");
    RgbImage::from_pixel(8, 8, Rgb([v, v, v]))
        .save(dir.join(name))
        .expect("save");
}

fn decisions(sets: &[(&str, &str)]) -> String {
    let sets: Vec<String> = sets
        .iter()
        .map(|(n, d)| format!(r#"{{"name":{n:?},"decision":{d:?}}}"#))
        .collect();
    format!(
        r#"{{"schema":"flipdiff-decisions.v1","seed":1,"labels":["a","b"],"sets":[{}]}}"#,
        sets.join(",")
    )
}

fn approve(cap: &Path, base: &Path, file: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_flipdiff"))
        .args(["approve"])
        .args([cap, base])
        .arg("--decisions")
        .arg(file)
        .output()
        .expect("spawn")
}

#[test]
fn approve_decisions_copies_only_accepted_and_rejects_unsafe_names() {
    let tmp = tempfile::tempdir().unwrap();
    let (cap, base) = (tmp.path().join("cap"), tmp.path().join("base"));
    save(&cap, "a.png", 200);
    save(&cap, "b.png", 200);
    save(&cap, "c.png", 200);
    save(&base, "a.png", 10);
    save(&base, "b.png", 10);
    let file = tmp.path().join("d.json");

    std::fs::write(
        &file,
        decisions(&[
            ("a.png", "accept"),
            ("b.png", "reject"),
            ("c.png", "needs-work"),
        ]),
    )
    .unwrap();
    let out = approve(&cap, &base, &file);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let px = |p: &str| image::open(base.join(p)).unwrap().to_rgb8().get_pixel(0, 0)[0];
    assert_eq!(px("a.png"), 200, "accepted entry is promoted");
    assert_eq!(px("b.png"), 10, "rejected entry keeps its baseline");
    assert!(
        !base.join("c.png").exists(),
        "needs-work entry is not promoted"
    );

    std::fs::write(&file, decisions(&[("../escape.png", "accept")])).unwrap();
    let out = approve(&cap, &base, &file);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unsafe image name"));
    assert!(!tmp.path().join("escape.png").exists());
}

#[cfg(unix)]
#[test]
fn approve_refuses_to_write_through_a_symlink() {
    let tmp = tempfile::tempdir().unwrap();
    let (cap, base, elsewhere) = (
        tmp.path().join("cap"),
        tmp.path().join("base"),
        tmp.path().join("elsewhere"),
    );
    save(&cap.join("sub"), "a.png", 200);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, base.join("sub")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_flipdiff"))
        .arg("approve")
        .args([&cap, &base])
        .arg("sub/a.png")
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("symlink"));
    assert!(!elsewhere.join("a.png").exists());
}

#[test]
fn prune_missing_deletes_only_baselines_without_a_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let (cap, base, out) = (
        tmp.path().join("cap"),
        tmp.path().join("base"),
        tmp.path().join("out"),
    );
    save(&cap, "kept.png", 10);
    save(&base, "kept.png", 10);
    save(&base, "gone.png", 10);
    let compare = Command::new(env!("CARGO_BIN_EXE_flipdiff"))
        .current_dir(tmp.path())
        .arg("compare")
        .args([&base, &cap])
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(
        compare.status.code(),
        Some(1),
        "missing entry is a regression"
    );
    let approve = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_flipdiff"))
            .arg("approve")
            .args([&cap, &base])
            .arg("--all-failing")
            .arg(out.join("flipdiff-report.v1.json"))
            .args(extra)
            .output()
            .unwrap()
    };
    assert_eq!(approve(&[]).status.code(), Some(0));
    assert!(
        base.join("gone.png").exists(),
        "no deletion without the flag"
    );
    let o = approve(&["--prune-missing"]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert!(String::from_utf8_lossy(&o.stdout).contains("removed"));
    assert!(!base.join("gone.png").exists());
    assert!(base.join("kept.png").exists());
}
