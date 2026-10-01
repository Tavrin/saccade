//! Tests of the run overview model.

use super::*;

fn png(dir: &Path, name: &str, rgb: [u8; 3]) {
    std::fs::create_dir_all(dir).unwrap();
    image::RgbImage::from_pixel(24, 24, image::Rgb(rgb))
        .save(dir.join(name))
        .unwrap();
}

fn input(dir: &Path, pairing: Pairing) -> RunInput {
    RunInput {
        dir: dir.to_path_buf(),
        label: dir.file_name().unwrap().to_string_lossy().into_owned(),
        display: dir.display().to_string(),
        pairing,
    }
}

#[test]
fn summary_counts_and_the_no_effect_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, edit, same) = (
        tmp.path().join("base"),
        tmp.path().join("edit"),
        tmp.path().join("same"),
    );
    for (n, c) in [
        ("a.png", [10, 10, 10]),
        ("b.png", [90, 90, 90]),
        ("c.png", [200, 0, 0]),
    ] {
        png(&base, n, c);
        png(&same, n, c);
    }
    png(&edit, "a.png", [10, 10, 10]);
    png(&edit, "b.png", [250, 250, 250]);
    png(&edit, "d.png", [0, 0, 255]);

    let m = overview(
        &input(&base, Pairing::Name),
        &[input(&edit, Pairing::Name), input(&same, Pairing::Name)],
        &RunsOptions::default(),
        None,
        &NoAssets,
    )
    .unwrap();
    assert_eq!(m.schema, RUNS_SCHEMA);
    let e = &m.runs[0];
    assert_eq!(
        (
            e.identical,
            e.changed,
            e.only_in_ref,
            e.only_in_run,
            e.errors
        ),
        (1, 1, 1, 1, 0)
    );
    assert_eq!(e.worst.as_ref().unwrap().name, "b.png");
    assert!(!e.no_visible_effect);
    assert!(
        e.summary
            .starts_with("1 identical · 1 changed (worst b.png "),
        "{}",
        e.summary
    );
    assert!(e.summary.ends_with("config differs: 0 keys"));
    assert!(
        m.runs[1].no_visible_effect,
        "all bit-identical and nothing missing"
    );
    // Rows: the reference's three images plus the run-only one.
    let names: Vec<&str> = m.images.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["a.png", "b.png", "c.png", "d.png"]);
    assert_eq!(m.images[2].cells[0].status, "only_in_ref");
    assert_eq!(m.images[3].cells[0].status, "only_in_run");
    assert_eq!(m.images[3].cells[1].status, "absent");
    assert!(m.progress.complete);
}

#[test]
fn pair_by_position_matches_unlike_names() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, other) = (tmp.path().join("base"), tmp.path().join("other"));
    png(&base, "a.png", [10, 10, 10]);
    png(&base, "b.png", [90, 40, 40]);
    png(&other, "x.png", [10, 10, 10]);
    png(&other, "y.png", [90, 40, 40]);
    let run = |p| {
        overview(
            &input(&base, Pairing::Name),
            &[input(&other, p)],
            &RunsOptions::default(),
            None,
            &NoAssets,
        )
        .unwrap()
    };

    let by_name = &run(Pairing::Name).runs[0].clone();
    assert!(by_name.mismatch && by_name.name_matches == 0);
    assert_eq!(
        (by_name.identical, by_name.only_in_ref, by_name.only_in_run),
        (0, 2, 2)
    );
    assert_eq!(
        by_name.run_images.len(),
        2,
        "a mismatched run lists its images for manual pairing"
    );

    let by_pos = run(Pairing::Position);
    let r = &by_pos.runs[0];
    assert_eq!(
        (
            r.pairing.as_str(),
            r.identical,
            r.only_in_ref,
            r.only_in_run
        ),
        ("position", 2, 0, 0)
    );
    assert_eq!(by_pos.images[0].cells[0].run_name.as_deref(), Some("x.png"));

    let manual = run(Pairing::parse("manual:0-1,1-0").unwrap());
    assert_eq!(
        manual.runs[0].identical, 0,
        "a crossed pairing compares unlike images"
    );
    assert_eq!(manual.runs[0].changed, 2);
}
