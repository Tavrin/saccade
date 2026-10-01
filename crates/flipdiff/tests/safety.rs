//! Safety and CI-correctness rules of `compare`, `view` and `approve`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;
use std::process::{Command, Output};

use image::{Rgb, RgbImage};

fn save(dir: &Path, name: &str, v: u8) {
    std::fs::create_dir_all(dir).expect("mkdir");
    RgbImage::from_pixel(64, 64, Rgb([v, v, v]))
        .save(dir.join(name))
        .expect("save");
}

fn flipdiff(cwd: &Path, args: &[&str], paths: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flipdiff"))
        .current_dir(cwd)
        .args(args)
        .args(paths)
        .output()
        .expect("spawn")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn an_unrelated_output_directory_is_never_cleared() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("precious"),
    );
    save(&base, "a.png", 100);
    save(&cap, "a.png", 100);
    std::fs::create_dir_all(out.join("images/logo")).unwrap();
    std::fs::write(out.join("images/logo/a.txt"), "keep me").unwrap();

    let run = flipdiff(
        tmp.path(),
        &["compare"],
        &[&base, &cap, Path::new("--out"), &out],
    );
    assert_eq!(run.status.code(), Some(2), "{}", text(&run));
    assert!(
        text(&run).contains("not a previous flipdiff output"),
        "{}",
        text(&run)
    );
    let view = flipdiff(
        tmp.path(),
        &["view"],
        &[&base, &cap, Path::new("--out"), &out],
    );
    assert_eq!(view.status.code(), Some(2), "{}", text(&view));
    assert_eq!(
        std::fs::read_to_string(out.join("images/logo/a.txt")).unwrap(),
        "keep me"
    );

    // A directory that already holds a flipdiff report is replaced as before.
    let again = tmp.path().join("report");
    for _ in 0..2 {
        let ok = flipdiff(
            tmp.path(),
            &["compare"],
            &[&base, &cap, Path::new("--out"), &again],
        );
        assert_eq!(ok.status.code(), Some(0), "{}", text(&ok));
    }
}

#[test]
fn an_output_directory_inside_an_input_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = (tmp.path().join("base"), tmp.path().join("cap"));
    save(&base, "a.png", 100);
    save(&cap, "a.png", 100);
    for inside in [base.join("report"), cap.join("deep/report")] {
        let o = flipdiff(
            tmp.path(),
            &["compare"],
            &[&base, &cap, Path::new("--out"), &inside],
        );
        assert_eq!(o.status.code(), Some(2), "{}", text(&o));
        assert!(
            text(&o).contains("inside an input directory"),
            "{}",
            text(&o)
        );
        assert!(!inside.exists(), "nothing is created inside an input");
    }
    let o = flipdiff(
        tmp.path(),
        &["view"],
        &[&base, &cap, Path::new("--out"), &base.join("view")],
    );
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
}

#[test]
fn comparing_nothing_is_not_a_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("out"),
    );
    std::fs::create_dir_all(&base).unwrap();
    save(&cap, "a.png", 100);
    let o = flipdiff(
        tmp.path(),
        &["compare"],
        &[&base, &cap, Path::new("--out"), &out],
    );
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("nothing compared"), "{}", text(&o));

    let allowed = flipdiff(
        tmp.path(),
        &["compare", "--allow-empty"],
        &[&base, &cap, Path::new("--out"), &out],
    );
    assert_eq!(allowed.status.code(), Some(0), "{}", text(&allowed));
}

#[test]
fn a_local_hotspot_fails_only_when_hotspot_fail_is_set() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("out"),
    );
    save(&base, "a.png", 20);
    save(&cap, "a.png", 20);
    // A small white square on a dark frame: the mean barely moves.
    let mut img = RgbImage::from_pixel(64, 64, Rgb([20, 20, 20]));
    for y in 28..34 {
        for x in 28..34 {
            img.put_pixel(x, y, Rgb([255, 255, 255]));
        }
    }
    img.save(cap.join("a.png")).unwrap();
    let args = ["compare", "--threshold", "0.05"];

    let plain = flipdiff(tmp.path(), &args, &[&base, &cap, Path::new("--out"), &out]);
    assert_eq!(plain.status.code(), Some(0), "{}", text(&plain));
    assert!(
        text(&plain).contains("pass, but local hotspot"),
        "{}",
        text(&plain)
    );

    let config = tmp.path().join("flipdiff.toml");
    std::fs::write(&config, "hotspot_fail = 0.5\n").unwrap();
    let strict = flipdiff(
        tmp.path(),
        &["compare", "--threshold", "0.05", "--config"],
        &[&config, &base, &cap, Path::new("--out"), &out],
    );
    assert_eq!(strict.status.code(), Some(1), "{}", text(&strict));
}

#[test]
fn approve_refuses_captures_that_changed_after_review() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("out"),
    );
    save(&base, "a.png", 100);
    save(&cap, "a.png", 200);
    let report = out.join("flipdiff-report.v1.json");
    let run = flipdiff(
        tmp.path(),
        &["compare"],
        &[&base, &cap, Path::new("--out"), &out],
    );
    assert_eq!(run.status.code(), Some(1), "{}", text(&run));

    // Another capture lands in place after the report was reviewed.
    save(&cap, "a.png", 250);
    let approve = |extra: &[&str], baseline: &Path| {
        let mut args = vec!["approve"];
        args.extend(extra);
        flipdiff(
            tmp.path(),
            &args,
            &[&cap, baseline, Path::new("--all-failing"), &report],
        )
    };
    let refused = approve(&[], &base);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(
        text(&refused).contains("changed since it was reviewed"),
        "{}",
        text(&refused)
    );
    let px = |p: &Path| {
        image::open(p.join("a.png"))
            .unwrap()
            .to_rgb8()
            .get_pixel(0, 0)[0]
    };
    assert_eq!(px(&base), 100, "the baseline is untouched");

    // The wrong baseline directory is refused too, even with unchanged captures.
    let other = tmp.path().join("other");
    save(&other, "a.png", 1);
    let wrong = approve(&[], &other);
    assert_eq!(wrong.status.code(), Some(2), "{}", text(&wrong));
    assert!(text(&wrong).contains("not "), "{}", text(&wrong));

    let forced = approve(&["--force"], &base);
    assert_eq!(forced.status.code(), Some(0), "{}", text(&forced));
    assert_eq!(px(&base), 250);
}
