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
