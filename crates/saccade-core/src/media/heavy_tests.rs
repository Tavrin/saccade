//! Explicitly gated installed model execution, no network/provider calls.
#![allow(clippy::unwrap_used)]
#[test]
#[ignore = "heavy: models"]
fn installed_media_sections_reuse_sessions() {
    use super::*;
    let registry = std::env::var_os("SACCADE_W8_REGISTRY")
        .map(PathBuf::from)
        .unwrap();
    let cache = std::env::var_os("SACCADE_W8_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap();
    let analyzer = Analyzer::with_registry(
        Profile::CpuFull,
        cache,
        false,
        models::Registry::load(&registry).unwrap(),
    )
    .unwrap();
    let image = image::RgbaImage::from_pixel(96, 64, image::Rgba([40, 80, 120, 255]));
    let mut encoded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let bytes = encoded.into_inner();
    let options = Options::default();
    let first = analyzer.analyze_bytes(&bytes, &options).unwrap();
    assert_eq!(first.focal.data["faces"]["status"], "ok");
    assert_eq!(first.text.status, Status::Ok);
    assert_eq!(first.embeddings.status, Status::Ok);
    let strict = Options {
        strict: true,
        ..Default::default()
    };
    let strict_record = analyzer.analyze_bytes(&bytes, &strict).unwrap();
    assert_eq!(strict_record.text.status, Status::Ok);
    assert_eq!(strict_record.embeddings.status, Status::Ok);
    let sessions = analyzer.sessions.lock().unwrap();
    assert!(sessions.faces.is_some() && sessions.ocr.is_some() && sessions.embeddings.is_some());
    drop(sessions);
    let second = analyzer.analyze_bytes(&bytes, &options).unwrap();
    assert_eq!(first.embeddings.data, second.embeddings.data);
    assert_eq!(first.text.data, second.text.data);
    assert!(
        first
            .embeddings
            .provenance
            .pins
            .iter()
            .all(|p| models::valid_hash(p))
    );
}

#[test]
#[ignore = "heavy: text-image"]
fn installed_joint_text_index_roundtrip() {
    use super::*;
    let registry = PathBuf::from(std::env::var_os("SACCADE_W8_JOINT_REGISTRY").unwrap());
    let cache = PathBuf::from(std::env::var_os("SACCADE_W8_MODEL_DIR").unwrap());
    let analyzer = Analyzer::with_registry(
        Profile::CpuFull,
        cache,
        false,
        models::Registry::load(&registry).unwrap(),
    )
    .unwrap();
    let a = analyzer.embed_text("a red square").unwrap();
    let b = analyzer.embed_text("a blue circle").unwrap();
    assert_eq!(a.len(), 768);
    assert!(crate::general::embedding::cosine(&a, &b).unwrap() < 0.999);
    assert_eq!(a, analyzer.embed_text("a red square").unwrap());
    let mut index = search::Index::new(analyzer.embedding_model().unwrap()).unwrap();
    for (name, color) in [
        ("generated-red.png", [255, 0, 0, 255]),
        ("generated-blue.png", [0, 0, 255, 255]),
    ] {
        let mut image = image::RgbaImage::from_pixel(224, 224, image::Rgba([255; 4]));
        for y in 48..176 {
            for x in 48..176 {
                image.put_pixel(x, y, image::Rgba(color));
            }
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        index.add_image(&analyzer, name, bytes.get_ref()).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    index.save(&dir.path().join("index")).unwrap();
    let loaded = search::Index::load(&dir.path().join("index")).unwrap();
    let hit = loaded.query_text(&analyzer, "a red square", 1).unwrap();
    assert_eq!(hit["query_kind"], "text");
    assert_eq!(hit["hits"][0]["row"]["path"], "generated-red.png");
    assert!(hit["hits"][0]["cosine"].as_f64().unwrap().is_finite());
    assert_eq!(hit["calibration"], "uncalibrated");
    let blue = loaded.query_text(&analyzer, "a blue square", 1).unwrap();
    assert_eq!(blue["hits"][0]["row"]["path"], "generated-blue.png");
    let mut changed = models::Registry::load(&PathBuf::from(
        std::env::var_os("SACCADE_W8_JOINT_REGISTRY").unwrap(),
    ))
    .unwrap();
    changed.contracts.get_mut("embedding").unwrap()["text"]["tokenizer"]["sha256"] =
        serde_json::Value::String("a".repeat(64));
    let other = Analyzer::with_registry(
        Profile::CpuFull,
        PathBuf::from(std::env::var_os("SACCADE_W8_MODEL_DIR").unwrap()),
        false,
        changed,
    )
    .unwrap();
    assert_eq!(
        loaded
            .query_text(&other, "a red square", 1)
            .unwrap_err()
            .code,
        "index_mismatch"
    );
}
