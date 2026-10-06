//! Generated inference acceptance with predeclared CER/WER and accent-removal controls.
use saccade_core::{
    general::{input, ocr, text},
    ui_review,
};
use serde_json::{Value, json};
use std::path::Path;
// Post-hoc coordinator scoring view; never delete or collapse characters.
fn fold_typography(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '’' | '‘' => '\'',
            '\u{202f}' | '\u{00a0}' | '\u{2009}' => ' ',
            '–' | '—' => '-',
            _ => c,
        })
        .collect()
}
fn folded_source(source: &ui_review::Source) -> ui_review::Source {
    let mut folded = source.clone();
    for node in &mut folded.nodes {
        node.text = fold_typography(&node.text);
    }
    folded
}
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
                println!(
                    "{} strict=FAIL CER=N/A WER=N/A folded=FAIL CER=N/A WER=N/A error={error}",
                    c["image"]
                );
                results.push(json!({"group":c["group"],"variant":c["variant"],"image":c["image"],
                    "font":c["font"],"expected_text":c["expected_text"],"pass":false,"gate_pass":false,
                    "typographic_equivalence":{"pass":false,"rates":null,"expected":null},
                    "accent_stripping_status":"UNRUN","accent_stripping_rejected":null,"error":error.to_string()}));
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
        let folded_required: Vec<_> = required.iter().map(|s| fold_typography(s)).collect();
        let folded = text::compare(
            &folded_source(&truth),
            &folded_source(&observed),
            &folded_required,
            0.,
            3.,
        )?;
        let folded_pass = folded.rates.cer.is_some_and(|v| v <= max_cer)
            && folded.rates.wer.is_some_and(|v| v <= max_wer)
            && folded.expected.iter().all(|e| e.present);
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
            // Preserve the rejection bar; accent-stripping no-ops are inapplicable.
            let rejected = changed
                && comparison.expected.iter().any(|e| !e.present)
                && comparison.rates.cer.is_some_and(|v| v > max_cer);
            let applicable = control["kind"] != "accent-stripping" || changed;
            let folded_control = text::compare(
                &folded_source(&truth),
                &folded_source(&stripped),
                &folded_required,
                0.,
                3.,
            )?;
            let folded_rejected = changed
                && folded_control.expected.iter().any(|e| !e.present)
                && folded_control.rates.cer.is_some_and(|v| v > max_cer);
            controls.push(
                json!({"kind":control["kind"],"text":control_text,"changed":changed,
                "applicable":applicable,
                "status":if !applicable { "N/A" } else if rejected { "PASS" } else { "FAIL" },
                "rejected":if applicable { Some(rejected) } else { None },
                "folded_rejected":if applicable { Some(folded_rejected) } else { None },
                "rates":comparison.rates,"expected":comparison.expected}),
            );
        }
        let rejected = !controls.is_empty()
            && controls.iter().all(|v| {
                v["applicable"] == false || (v["rejected"] == true && v["folded_rejected"] == true)
            });
        let accent_control = controls.iter().find(|v| v["kind"] == "accent-stripping");
        let accent_rejected = accent_control.map(|v| v["rejected"].clone());
        let accent_status = accent_control
            .and_then(|v| v["status"].as_str())
            .unwrap_or("N/A");
        if !pass || !folded_pass || !rejected {
            failed += 1;
        }
        println!(
            "{} strict={} CER={:.5} WER={:.5} folded={} CER={:.5} WER={:.5} accent-control={} controls={}",
            c["image"].as_str().ok_or("image missing")?,
            if pass { "PASS" } else { "FAIL" },
            comparison.rates.cer.unwrap_or(f64::NAN),
            comparison.rates.wer.unwrap_or(f64::NAN),
            if folded_pass { "PASS" } else { "FAIL" },
            folded.rates.cer.unwrap_or(f64::NAN),
            folded.rates.wer.unwrap_or(f64::NAN),
            accent_status,
            if rejected { "PASS" } else { "FAIL" }
        );
        results.push(json!({"group":c["group"],"variant":c["variant"],"image":c["image"],
            "font":c["font"],"expected_text":expected,"pass":pass,"gate_pass":pass && folded_pass && rejected,
            "typographic_equivalence":{"pass":folded_pass,"rates":folded.rates,"expected":folded.expected},
            "accent_stripping_status":accent_status,
            "accent_stripping_rejected":accent_rejected,"negative_controls":controls,
            "rates":comparison.rates,"expected":comparison.expected,"observed":observed.nodes,"producer":observed.producer}));
    }
    let receipt = json!({"schema":"saccade-ocr-accent-results.v1","typographic_equivalence":contracts["typographic_equivalence"],"review_status":contracts["review_status"],"coordinator_review_date":contracts["coordinator_review_date"],"contract_sha256":saccade_core::localized::digest(&input::bytes(&root.join("contracts.json"),2*1024*1024)?),"total":results.len(),"failed":failed,"results":results});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&receipt)?)?;
    println!("accent contracts: {} total, {failed} failed", results.len());
    if failed > 0 {
        return Err("accent acceptance failed".into());
    }
    Ok(())
}
