//! Mask metrics, box interchange, frame maps and the performance-sidecar kit through the CLI.
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn run(root: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    let value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    (out.status.code(), value)
}

fn conforms(id: &str, value: &Value) {
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get(id).unwrap()).unwrap();
    let errors: Vec<String> = jsonschema::validator_for(&schema)
        .unwrap()
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{id}: {errors:?}");
}

fn fill(w: u32, h: u32, f: impl Fn(u32, u32) -> u32) -> Vec<u32> {
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| f(x, y))
        .collect()
}

/// The same scene in two encodings: single-channel 8-bit classes, and packed-RGB object IDs.
/// Reference has a 16x16 block of the first label, an 8x8 block of the second, and a void
/// band two columns wide. The prediction moves the first block 2 px right and omits the second.
fn write_pairs(root: &Path) {
    let layout = |predicted: bool, first: u32, second: u32, void: u32| {
        fill(32, 32, move |x, y| {
            let block =
                |x0: u32, y0: u32, s: u32| (x0..x0 + s).contains(&x) && (y0..y0 + s).contains(&y);
            if !predicted && x < 2 {
                void
            } else if (!predicted && block(4, 4, 16)) || (predicted && block(6, 4, 16)) {
                first
            } else if !predicted && block(22, 22, 8) {
                second
            } else {
                0
            }
        })
    };
    for (name, first, second, packed) in [("gray", 1, 2, false), ("packed", 500, 70_000, true)] {
        for (kind, predicted) in [("pred", true), ("ref", false)] {
            let labels = layout(predicted, first, second, 255);
            let path = root.join(format!("{name}-{kind}.png"));
            if packed {
                let mut img = image::RgbImage::new(32, 32);
                for (i, v) in labels.iter().enumerate() {
                    img.put_pixel(
                        i as u32 % 32,
                        i as u32 / 32,
                        image::Rgb([(v >> 16) as u8, (v >> 8) as u8, *v as u8]),
                    );
                }
                img.save(path).unwrap();
            } else {
                let mut img = image::GrayImage::new(32, 32);
                for (i, v) in labels.iter().enumerate() {
                    img.put_pixel(i as u32 % 32, i as u32 / 32, image::Luma([*v as u8]));
                }
                img.save(path).unwrap();
            }
        }
    }
}

#[test]
fn one_command_scores_a_class_raster_and_an_id_buffer_identically() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_pairs(root);
    let score = |name: &str, boundary: &str| {
        let (code, v) = run(
            root,
            &[
                "mask-metrics",
                &format!("{name}-pred.png"),
                &format!("{name}-ref.png"),
                "--each-label",
                "--void",
                "ignore=id=255",
                "--boundary-px",
                boundary,
                "--json",
            ],
        );
        assert_eq!(code, Some(0));
        conforms("saccade-mask-metrics.v1", &v);
        v
    };
    let class = |v: &Value, name: &str| {
        v["classes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()
            .clone()
    };
    let (gray, packed) = (score("gray", "2"), score("packed", "2"));
    for (v, first, second) in [(&gray, "id=1", "id=2"), (&packed, "id=500", "id=70000")] {
        // 224 shared pixels of a 288-pixel union; void columns are not counted anywhere.
        assert_eq!(
            (v["void_pixels"].as_u64(), v["evaluated_pixels"].as_u64()),
            (Some(64), Some(960))
        );
        let c = class(v, first);
        assert_eq!(
            (c["intersection"].as_u64(), c["union"].as_u64()),
            (Some(224), Some(288))
        );
        assert!((c["iou"].as_f64().unwrap() - 224.0 / 288.0).abs() < 1e-12);
        assert_eq!(c["boundary"]["f_score"], 1.0);
        let m = class(v, second);
        assert_eq!(
            (m["status"].as_str(), m["iou"].as_f64()),
            (Some("missed"), Some(0.0))
        );
        assert_eq!(v["summary"]["missed_classes"], json!([second]));
    }
    assert_eq!(
        class(&gray, "id=1")["dice"],
        class(&packed, "id=500")["dice"],
        "encoding must not change the score"
    );
    // A 2-px shift is outside a 1-px boundary tolerance: still a partial match, never a pass.
    let tight = class(&score("gray", "1"), "id=1")["boundary"]["f_score"]
        .as_f64()
        .unwrap();
    assert!(0.0 < tight && tight < 1.0, "{tight}");
    // Differently sized images are refused, never resampled.
    image::GrayImage::new(8, 8)
        .save(root.join("small.png"))
        .unwrap();
    let (code, _) = run(
        root,
        &["mask-metrics", "small.png", "gray-ref.png", "--json"],
    );
    assert_ne!(code, Some(0));
}

#[test]
fn box_export_import_round_trip_and_out_of_bounds_refusal() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let doc = json!({"schema":"saccade-boxes.v1","coordinates":{"unit":"pixel","origin":"top_left","form":"xywh"},
        "image":{"file":"scene.png","width":200,"height":100},"classes":["a","b"],
        "boxes":[{"class":"a","x":10.0,"y":20.0,"w":50.0,"h":30.0},{"class":"b","x":150.0,"y":60.0,"w":60.0,"h":40.0}]});
    std::fs::write(root.join("doc.json"), doc.to_string()).unwrap();
    // The second box leaves the image: refused by default, clipped and counted on request.
    let (code, _) = run(
        root,
        &[
            "boxes", "export", "doc.json", "--format", "coco", "--out", "refused", "--json",
        ],
    );
    assert_ne!(code, Some(0));
    assert!(!root.join("refused/annotations.coco.json").exists());
    let (code, r) = run(
        root,
        &[
            "boxes", "export", "doc.json", "--format", "yolo", "--clip", "--out", "yolo", "--json",
        ],
    );
    assert_eq!(code, Some(0));
    conforms("saccade-boxes-result.v1", &r);
    assert_eq!(r["adjustments"], json!({"clipped":1,"dropped":0}));
    let (code, back) = run(
        root,
        &[
            "boxes",
            "import",
            "yolo/scene.txt",
            "--format",
            "yolo",
            "--width",
            "200",
            "--height",
            "100",
            "--image-file",
            "scene.png",
            "--classes",
            "yolo/classes.txt",
            "--out",
            "yolo-back",
            "--json",
        ],
    );
    assert_eq!(code, Some(0));
    assert_eq!(back["boxes"], 2);
    let imported: Value = serde_json::from_slice(
        &std::fs::read(root.join("yolo-back/saccade-boxes.v1.json")).unwrap(),
    )
    .unwrap();
    conforms("saccade-boxes.v1", &imported);
    assert!((imported["boxes"][1]["w"].as_f64().unwrap() - 50.0).abs() < 1e-9);
    // Crop, then COCO, then back: the derived coordinates survive.
    let (code, _) = run(
        root,
        &[
            "boxes",
            "transform",
            "yolo-back/saccade-boxes.v1.json",
            "--crop",
            "40,10,100,60",
            "--out",
            "cropped",
            "--json",
        ],
    );
    assert_eq!(code, Some(0));
    let (code, _) = run(
        root,
        &[
            "boxes",
            "export",
            "cropped/saccade-boxes.v1.json",
            "--format",
            "coco",
            "--out",
            "coco",
            "--json",
        ],
    );
    assert_eq!(code, Some(0));
    let (code, _) = run(
        root,
        &[
            "boxes",
            "import",
            "coco/annotations.coco.json",
            "--format",
            "coco",
            "--out",
            "coco-back",
            "--json",
        ],
    );
    assert_eq!(code, Some(0));
    let cropped: Value = serde_json::from_slice(
        &std::fs::read(root.join("coco-back/saccade-boxes.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cropped["image"]["width"], 100);
    assert_eq!(cropped["boxes"].as_array().unwrap().len(), 1);
    assert_eq!(cropped["boxes"][0]["x"], 0.0);
}

#[test]
fn frame_map_names_gaps_variable_rate_and_never_settled() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let map = |times: &[(u64, f64)]| json!({"schema":"saccade-frame-map.v1","frames":times.iter().map(|(i, t)| json!({"index":i,"timestamp_s":t,"file":format!("f{i}.png")})).collect::<Vec<_>>()});
    for (name, frames, expected, code) in [
        (
            "steady",
            vec![(0, 0.0), (1, 0.5), (2, 1.0)],
            "constant_frame_rate",
            0,
        ),
        (
            "variable",
            vec![(0, 0.0), (1, 0.5), (2, 1.5)],
            "variable_frame_rate",
            1,
        ),
        (
            "gap",
            vec![(0, 0.0), (1, 0.5), (3, 1.5)],
            "missing_frames",
            1,
        ),
    ] {
        std::fs::write(root.join(format!("{name}.json")), map(&frames).to_string()).unwrap();
        let (c, v) = run(
            root,
            &[
                "frame-map",
                "check",
                &format!("{name}.json"),
                "--skip-files",
                "--json",
            ],
        );
        assert_eq!(
            (c, v["state"].as_str()),
            (Some(code), Some(expected)),
            "{name}"
        );
        conforms("saccade-frame-map-check.v1", &v);
    }
    let settling = json!({"policy":{"change_frame":1,"fps":2.0},"global_error":[0.9,0.8,0.7],"settle_frame":null});
    std::fs::write(root.join("settling.json"), settling.to_string()).unwrap();
    let (_, v) = run(
        root,
        &[
            "frame-map",
            "check",
            "steady.json",
            "--skip-files",
            "--settling",
            "settling.json",
            "--json",
        ],
    );
    assert_eq!(v["settle"]["state"], "never_settled");
    assert_eq!(v["settle"]["elapsed_s"], Value::Null);
    conforms("saccade-frame-map-check.v1", &v);
}

#[test]
fn performance_kit_produces_the_declared_outcomes() {
    let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/perf-kit");
    let tmp = tempfile::tempdir().unwrap();
    for (name, verdict, code) in [
        ("unchanged", "equivalent", 0),
        ("slower", "slower", 1),
        ("faster", "faster", 0),
    ] {
        let session = kit.join(format!("session-{name}.json"));
        let out = tmp.path().join(name);
        let (c, v) = run(
            tmp.path(),
            &[
                "timing",
                "ab",
                session.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--json",
            ],
        );
        assert_eq!(
            (c, v["verdict"].as_str()),
            (Some(code), Some(verdict)),
            "{name}"
        );
    }
    let sidecar = |name: &str| {
        let path = kit.join(name);
        run(
            tmp.path(),
            &["perf", "validate", path.to_str().unwrap(), "--json"],
        )
        .1["valid"]
            .clone()
    };
    assert_eq!(sidecar("sidecar-valid.json"), true);
    assert_eq!(sidecar("sidecar-invalid.json"), false);
}

#[test]
fn output_manifests_cover_mask_boxes_and_frame_map_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_pairs(root);
    let doc = json!({"schema":"saccade-boxes.v1","coordinates":{"unit":"pixel","origin":"top_left","form":"xywh"},
        "image":{"file":"scene.png","width":32,"height":32},"classes":["a"],
        "boxes":[{"class":"a","x":1.0,"y":1.0,"w":5.0,"h":5.0}]});
    std::fs::write(root.join("doc.json"), doc.to_string()).unwrap();
    let map = json!({"schema":"saccade-frame-map.v1","frames":[
        {"index":0,"file":"gray-ref.png","timestamp_s":0.0}]});
    std::fs::write(root.join("map.json"), map.to_string()).unwrap();
    for (args, directory, artifact) in [
        (
            vec![
                "mask-metrics",
                "gray-pred.png",
                "gray-ref.png",
                "--out",
                "mask",
                "--json",
            ],
            "mask",
            "saccade-mask-metrics.v1.json",
        ),
        (
            vec![
                "boxes", "export", "doc.json", "--format", "coco", "--out", "boxes", "--json",
            ],
            "boxes",
            "saccade-boxes-result.v1.json",
        ),
        (
            vec![
                "frame-map",
                "check",
                "map.json",
                "--out",
                "frames",
                "--json",
            ],
            "frames",
            "saccade-frame-map-check.v1.json",
        ),
    ] {
        let (code, _) = run(root, &args);
        assert_eq!(code, Some(0), "{args:?}");
        let out = root.join(directory);
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(out.join(saccade_core::manifest::MANIFEST_FILE)).unwrap(),
        )
        .unwrap();
        conforms(saccade_core::manifest::MANIFEST_SCHEMA, &manifest);
        assert!(
            manifest["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["path"] == artifact),
            "{manifest}"
        );
        assert!(saccade_core::manifest::verify(&out).unwrap().is_empty());
        std::fs::write(out.join(artifact), b"changed").unwrap();
        assert!(
            saccade_core::manifest::verify(&out)
                .unwrap()
                .iter()
                .any(|v| v["code"] == saccade_core::manifest::CODE_STALE_LINK)
        );
    }
}
