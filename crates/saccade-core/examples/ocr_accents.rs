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
        let observed = match engine.recognize(&bytes) {
            Ok(observed) => observed,
            Err(error) => {
                failed += 1;
                println!("{} OCR=FAIL error={error}", c["image"]);
                results.push(json!({"group":c["group"],"variant":c["variant"],"image":c["image"],
                    "font":c["font"],"expected_text":c["expected_text"],"pass":false,"gate_pass":false,
                    "accent_stripping_rejected":false,"error":error.to_string()}));
                continue;
            }
        };
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
        let required: Vec<String> = c["required_exact_strings"]
            .as_array()
            .ok_or("exact strings missing")?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or("invalid exact string"))
            .collect::<Result<_, _>>()?;
        let comparison = text::compare(&truth, &observed, &required, 0., 3.)?;
        let max_cer = c["max_cer"].as_f64().ok_or("CER threshold missing")?;
        let max_wer = c["max_wer"].as_f64().ok_or("WER threshold missing")?;
        let pass = comparison.rates.cer.is_some_and(|v| v <= max_cer)
            && comparison.rates.wer.is_some_and(|v| v <= max_wer)
            && comparison.expected.iter().all(|e| e.present);
        let mut controls = Vec::new();
        for control in c["negative_controls"]
            .as_array()
            .ok_or("controls missing")?
        {
            let control_text = control["text"].as_str().ok_or("control text missing")?;
            let changed = control_text != expected;
            let mut stripped = truth.clone();
            stripped.nodes[0].text = control_text.into();
            let comparison = text::compare(&truth, &stripped, &required, 0., 3.)?;
            // Preserve the original accent-control bar; no-op controls cannot pass.
            let rejected = changed
                && comparison.expected.iter().any(|e| !e.present)
                && comparison.rates.cer.is_some_and(|v| v > max_cer);
            controls.push(
                json!({"kind":control["kind"],"text":control_text,"changed":changed,
                "rejected":rejected,"rates":comparison.rates,"expected":comparison.expected}),
            );
        }
        let rejected = !controls.is_empty() && controls.iter().all(|v| v["rejected"] == true);
        let accent_rejected = controls
            .iter()
            .any(|v| v["kind"] == "accent-stripping" && v["rejected"] == true);
        if !pass || !rejected {
            failed += 1;
        }
        println!(
            "{} OCR={} controls={}",
            c["image"].as_str().ok_or("image missing")?,
            if pass { "PASS" } else { "FAIL" },
            if rejected { "PASS" } else { "FAIL" }
        );
        results.push(json!({"group":c["group"],"variant":c["variant"],"image":c["image"],
            "font":c["font"],"expected_text":expected,"pass":pass,"gate_pass":pass && rejected,
            "accent_stripping_rejected":accent_rejected,"negative_controls":controls,
            "rates":comparison.rates,"expected":comparison.expected,"observed":observed.nodes,"producer":observed.producer}));
    }
    let receipt = json!({"schema":"saccade-ocr-accent-results.v1","review_status":contracts["review_status"],"coordinator_review_date":contracts["coordinator_review_date"],"contract_sha256":saccade_core::localized::digest(&input::bytes(&root.join("contracts.json"),2*1024*1024)?),"total":results.len(),"failed":failed,"results":results});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&receipt)?)?;
    println!("accent contracts: {} total, {failed} failed", results.len());
    if failed > 0 {
        return Err("accent acceptance failed".into());
    }
    Ok(())
}
