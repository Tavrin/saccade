//! Generated inference acceptance with predeclared CER/WER and accent-removal controls.
use saccade_core::{
    general::{input, ocr, text},
    ui_review,
};
use serde_json::{Value, json};
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: ocr_accents FIXTURE_DIR MODEL_CACHE RECEIPT.json".into());
    }
    let root = Path::new(&args[1]);
    let contracts: Value = serde_json::from_slice(&input::bytes(
        &root.join("contracts.json"),
        2 * 1024 * 1024,
    )?)?;
    if contracts["thresholds_declared_before_inference"] != true {
        return Err("contracts must precede inference".into());
    }
    let mut engine = ocr::Engine::load(&ocr::default_contract()?, Path::new(&args[2]), false)?;
    let mut results = Vec::new();
    let mut failed = 0;
    for c in contracts["fixtures"]
        .as_array()
        .ok_or("contracts missing")?
    {
        let bytes = input::bytes(
            &root.join(c["image"].as_str().ok_or("image missing")?),
            input::MAX_BYTES,
        )?;
        if saccade_core::localized::digest(&bytes) != c["image_sha256"] {
            return Err("fixture hash mismatch".into());
        }
        let observed = engine.recognize(&bytes)?;
        let expected = c["expected_text"].as_str().ok_or("expected text missing")?;
        let mut truth = observed.clone();
        truth.kind = "paddle_ocr".into();
        truth.nodes = vec![ui_review::Node {
            id: "truth".into(),
            text: expected.into(),
            bounds: Some([
                0.,
                0.,
                f64::from(truth.dimensions[0]),
                f64::from(truth.dimensions[1]),
            ]),
            ocr_confidence: Some(100.),
            role: String::new(),
            reading_order: None,
            keyboard_order: None,
            disclosure: false,
        }];
        let accents: Vec<String> = expected
            .split_whitespace()
            .filter(|w| !w.is_ascii())
            .map(str::to_owned)
            .collect();
        let comparison = text::compare(&truth, &observed, &accents, 0., 3.)?;
        let pass = comparison
            .rates
            .cer
            .is_some_and(|v| v <= c["max_cer"].as_f64().unwrap_or(-1.))
            && comparison
                .rates
                .wer
                .is_some_and(|v| v <= c["max_wer"].as_f64().unwrap_or(-1.))
            && comparison.expected.iter().all(|e| e.present);
        // Deliberately replace accents with plain Latin; exact expectations must fail independently of OCR success.
        let mut stripped = observed.clone();
        for n in &mut stripped.nodes {
            n.text = n
                .text
                .chars()
                .map(|c| match c {
                    'é' | 'è' | 'ê' => "e".into(),
                    'à' | 'á' => "a".into(),
                    'ç' => "c".into(),
                    'ô' | 'ö' | 'ó' => "o".into(),
                    'ù' | 'ü' => "u".into(),
                    'ñ' => "n".into(),
                    'ß' => "ss".into(),
                    'œ' => "oe".into(),
                    'í' => "i".into(),
                    v => v.to_string(),
                })
                .collect();
        }
        let control = text::compare(&truth, &stripped, &accents, 0., 3.)?;
        let rejected = control.expected.iter().any(|e| !e.present)
            && control
                .rates
                .cer
                .is_some_and(|v| v > c["max_cer"].as_f64().unwrap_or(1.));
        if !pass || !rejected {
            failed += 1;
        }
        results.push(json!({"image":c["image"],"pass":pass,"accent_stripping_rejected":rejected,"rates":comparison.rates,"expected":comparison.expected,"observed":observed.nodes,"producer":observed.producer}));
    }
    let receipt = json!({"schema":"saccade-ocr-accent-results.v1","review_status":"generated, pending coordinator review","contract_sha256":saccade_core::localized::digest(&input::bytes(&root.join("contracts.json"),2*1024*1024)?),"total":results.len(),"failed":failed,"results":results});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&receipt)?)?;
    println!("accent contracts: {} total, {failed} failed", results.len());
    if failed > 0 {
        return Err("accent acceptance failed".into());
    }
    Ok(())
}
