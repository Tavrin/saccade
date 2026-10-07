//! Generated split, route, containment and recall contracts.
#![allow(clippy::unwrap_used, missing_docs)]
use image::{Rgba, RgbaImage};
use saccade_core::general::split_review::{self as sr, EmbeddingRoute, Entry, Injection, Manifest};
use tempfile::TempDir;

fn texture(mut state: u64) -> RgbaImage {
    RgbaImage::from_fn(192, 160, |x, y| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let v = (state % 256) as u8;
        Rgba([v, ((x * 17 + y * 23) % 256) as u8, 255 - v, 255])
    })
}
fn manifest(entries: &[(&str, &str)]) -> Manifest {
    Manifest {
        schema: sr::MANIFEST_SCHEMA.into(),
        entries: entries
            .iter()
            .map(|(path, split)| Entry {
                path: (*path).into(),
                split: (*split).into(),
            })
            .collect(),
        injected_pairs: vec![],
    }
}
fn save(dir: &TempDir, name: &str, im: &RgbaImage) {
    im.save(dir.path().join(name)).unwrap();
}
fn injection(a: &str, b: &str, transform: &str) -> Injection {
    Injection {
        a: a.into(),
        b: b.into(),
        transform: transform.into(),
    }
}

#[test]
fn generated_crop_and_exact_cross_split_recall_keep_evidence_and_misses() {
    let dir = TempDir::new().unwrap();
    let image = texture(938475);
    save(&dir, "source.png", &image);
    save(&dir, "copy.png", &image);
    save(
        &dir,
        "crop.png",
        &image::imageops::crop_imm(&image, 20, 12, 144, 128).to_image(),
    );
    save(
        &dir,
        "other.png",
        &RgbaImage::from_pixel(192, 160, Rgba([10, 100, 250, 255])),
    );
    let mut m = manifest(&[
        ("source.png", "train"),
        ("copy.png", "validation"),
        ("crop.png", "test"),
        ("other.png", "test"),
    ]);
    m.injected_pairs = vec![
        injection("copy.png", "source.png", "exact"),
        injection("source.png", "crop.png", "cropped"),
        injection("source.png", "other.png", "deliberate_miss"),
    ];
    let r = sr::review(&m, dir.path(), 0, 0.95, None).unwrap();
    assert_eq!(r["injected_recall"]["total"], 3);
    assert_eq!(r["injected_recall"]["found"], 2);
    assert_eq!(
        r["injected_recall"]["missed_pairs"][0]["transform"],
        "deliberate_miss"
    );
    let pairs = r["cross_split_pairs"].as_array().unwrap();
    assert!(pairs.iter().all(|p| p["a_split"] != p["b_split"]));
    let crop = pairs
        .iter()
        .find(|p| p["a"] == "source.png" && p["b"] == "crop.png")
        .unwrap();
    assert!(
        crop["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["route"] == "geometric" && e["inliers"].as_u64().unwrap() >= 6)
    );
    let linked = saccade_core::report_links::decorate(&r).unwrap();
    assert_eq!(linked["schema"], "saccade-split-review.v2");
    assert_eq!(saccade_core::report_links::legacy_view(&linked), r);
}
#[test]
fn bursts_remain_within_partition_and_absent_truth_has_null_recall() {
    let dir = TempDir::new().unwrap();
    let image = texture(938475);
    for name in ["a.png", "b.png", "c.png"] {
        save(&dir, name, &image);
    }
    let m = manifest(&[
        ("a.png", "session"),
        ("b.png", "session"),
        ("c.png", "session"),
    ]);
    let r = sr::review(&m, dir.path(), 6, 0.95, None).unwrap();
    assert_eq!(r["groups"].as_array().unwrap().len(), 1);
    assert_eq!(r["groups"][0]["members"].as_array().unwrap().len(), 3);
    assert_eq!(r["within_split_pairs"].as_array().unwrap().len(), 3);
    assert_eq!(r["cross_split_pairs"], serde_json::json!([]));
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("not proof of no leakage")
    );
    assert!(r["injected_recall"]["recall"].is_null());
    assert_eq!(r["routes"][2]["status"], "not_enabled");
}
#[test]
fn membership_and_truth_reject_ambiguity_and_bounds() {
    let dir = TempDir::new().unwrap();
    save(&dir, "a.png", &texture(17));
    save(&dir, "b.png", &texture(83));
    let base = manifest(&[("a.png", "train"), ("b.png", "test")]);
    for entries in [
        vec![("a.png", "train"), ("a.png", "test")],
        vec![("../outside.png", "train")],
        vec![("a.png", "")],
        vec![("a.png", "train"); 129],
        vec![],
    ] {
        assert!(manifest(&entries).resolve(dir.path()).is_err());
    }
    for pairs in [
        vec![injection("a.png", "missing.png", "crop")],
        vec![injection("a.png", "a.png", "exact")],
        vec![
            injection("a.png", "b.png", "crop"),
            injection("b.png", "a.png", "crop"),
        ],
    ] {
        let mut m = base.clone();
        m.injected_pairs = pairs;
        assert!(m.resolve(dir.path()).is_err());
    }
    assert!(sr::review(&base, dir.path(), 65, 0.95, None).is_err());
    assert!(sr::review(&base, dir.path(), 6, f64::NAN, None).is_err());
    std::fs::write(dir.path().join("b.png"), b"corrupt").unwrap();
    assert!(sr::review(&base, dir.path(), 6, 0.95, None).is_err());
}
#[cfg(unix)]
#[test]
fn canonical_aliases_and_symlink_escapes_are_rejected() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    save(&dir, "a.png", &texture(17));
    save(&outside, "outside.png", &texture(83));
    std::os::unix::fs::symlink(dir.path().join("a.png"), dir.path().join("alias.png")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("outside.png"),
        dir.path().join("escape.png"),
    )
    .unwrap();
    assert!(
        manifest(&[("a.png", "train"), ("alias.png", "test")])
            .resolve(dir.path())
            .is_err()
    );
    assert!(
        manifest(&[("escape.png", "test")])
            .resolve(dir.path())
            .is_err()
    );
}
struct MockRoute {
    id: String,
    calls: usize,
}
impl EmbeddingRoute for MockRoute {
    fn identity(&self) -> &str {
        &self.id
    }
    fn embed(&mut self, _: &RgbaImage) -> saccade_core::Result<Vec<f32>> {
        self.calls += 1;
        Ok(vec![1., 0.])
    }
}
#[test]
fn supplied_embedding_route_records_identity_and_numeric_evidence() {
    let dir = TempDir::new().unwrap();
    save(&dir, "a.png", &texture(17));
    save(&dir, "b.png", &texture(83));
    let mut route = MockRoute {
        id: "a".repeat(64),
        calls: 0,
    };
    let r = sr::review(
        &manifest(&[("a.png", "train"), ("b.png", "test")]),
        dir.path(),
        0,
        0.99,
        Some(&mut route),
    )
    .unwrap();
    assert_eq!(route.calls, 2);
    assert_eq!(r["routes"][2]["status"], "used");
    let e = r["cross_split_pairs"][0]["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["route"] == "embedding")
        .unwrap();
    assert_eq!(e["model_id"], route.id);
    assert_eq!(e["cosine"], 1.);
    route.id = "unbound".into();
    assert!(
        sr::review(
            &manifest(&[("a.png", "train")]),
            dir.path(),
            0,
            0.99,
            Some(&mut route)
        )
        .is_err()
    );
}
#[test]
fn csv_quotes_user_supplied_names_and_split_labels() {
    let r = serde_json::json!({"cross_split_pairs":[{"a":"a,\"x.png","b":"b.png","a_split":"train\nfold","b_split":"test","evidence":[{"route":"hash"}]}]});
    let csv = sr::csv(&r).unwrap();
    assert!(csv.contains("\"a,\"\"x.png\""));
    assert!(csv.contains("\"train\nfold\""));
    assert!(csv.contains("\"\"route\"\""));
}
