//! Rebuild the public twenty-item source-fidelity benchmark from showcase reports.
//! Usage: build_review_labels REPORTS_DIR OUTPUT_JSON
use saccade_core::judge::{EvidenceOptions, JudgeQuestion, report_items};
use saccade_core::labels::{Label, Labels, SCHEMA, identity};
use saccade_core::{Report, Result};
use serde_json::json;
use std::path::Path;

fn build(reports: &Path, out: &Path) -> Result<()> {
    let mut items = Vec::new();
    // Five texture encodings, three UI regressions, three covers, three ML changes,
    // one LOD pop and five upscaler losses, all under a source-fidelity rubric.
    let cases = [
        "texture-compression/rank/block-low",
        "texture-compression/rank/block-high",
        "texture-compression/rank/jpeg-q40",
        "texture-compression/rank/jpeg-q85",
        "texture-compression/rank/palette-256",
        "webapp-ui/compare",
        "cover-art/compare",
        "ml-image-model/compare",
        "lod-transition/sequence",
        "upscaler/rank/nearest",
        "upscaler/rank/bilinear",
        "upscaler/rank/bicubic",
        "upscaler/rank/lanczos",
        "upscaler/rank/sharpened-bicubic",
    ];
    for case in cases {
        let doc = reports.join(case).join("saccade-report.v1.json");
        let report: Report = serde_json::from_slice(&std::fs::read(&doc).map_err(|source| {
            saccade_core::Error::Io {
                context: "reading showcase report".into(),
                source,
            }
        })?)?;
        let entries: Vec<String> = report
            .entries
            .iter()
            .filter(|e| e.bit_identical != Some(true))
            .filter(|e| {
                if case.starts_with("webapp-ui") {
                    e.status == saccade_core::Status::Fail
                } else {
                    true
                }
            })
            .map(|e| e.name.clone())
            .collect();
        let intent = "Preserve all original content, placement, text and fine detail. Lossy encoding, removed detail, changed controls or crop zoom are regressions; prefer the faithful source. Ignore aesthetic preference.";
        let (questions, _) = report_items(
            &report,
            doc.parent().unwrap_or(Path::new(".")),
            JudgeQuestion::Preference,
            Some(intent),
            &entries,
            &EvidenceOptions::default(),
        )
        .map_err(|e| saccade_core::Error::Config(e.to_string()))?;
        for i in questions {
            let e = report
                .entries
                .iter()
                .find(|e| e.name == i.entry)
                .ok_or_else(|| saccade_core::Error::Config("missing fixture entry".into()))?;
            let images = [&e.paths.baseline, &e.paths.capture]
                .into_iter()
                .filter_map(|p| p.as_ref())
                .map(|p| {
                    saccade_core::paths::record(
                        &saccade_core::paths::resolve(p, &doc),
                        out.parent().unwrap_or(Path::new(".")),
                        false,
                    )
                })
                .collect();
            let mut label = Label {
                entry: format!("{case}/{}", i.entry),
                question: "preference".into(),
                answer: "a".into(),
                evidence_hash: String::new(),
                state: json!({"ab":i.states[0],"ba":i.states[1]}),
                intent: i.intent,
                images,
                sha256: vec![e.baseline_sha256.clone(), e.capture_sha256.clone()],
                hotspots: i.hotspots,
                report: None,
                provenance: vec![
                    json!({"human":"public-showcase-truths","source":"scripts/gen-showcases.py",
                    "criterion":"source fidelity","truth":"The reference preserves source pixels/content; the procedural candidate introduces a documented loss or regression."}),
                ],
            };
            // Fixture images are referenced in showcases, not temporary report copies.
            let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let case_root = case.split('/').next().unwrap_or("");
            let capture =
                if case.starts_with("texture-compression") || case.starts_with("upscaler/rank") {
                    format!("candidates/{}", case.split('/').nth(2).unwrap_or(""))
                } else {
                    "capture".into()
                };
            let original_name = if case.starts_with("lod-transition") {
                let frame = i
                    .entry
                    .strip_prefix("frame_")
                    .and_then(|s| s.strip_suffix(".png"))
                    .and_then(|s| s.parse::<u32>().ok())
                    .ok_or_else(|| {
                        saccade_core::Error::Config("unexpected sequence frame name".into())
                    })?;
                format!("frame_{frame:02}.png")
            } else {
                i.entry.clone()
            };
            label.images = ["baseline".to_owned(), capture]
                .iter()
                .map(|side| {
                    saccade_core::paths::record(
                        &project
                            .join("showcases")
                            .join(case_root)
                            .join(side)
                            .join(&original_name),
                        out.parent().unwrap_or(Path::new(".")),
                        false,
                    )
                })
                .collect();
            if label.hotspots.is_empty() {
                let size = i.states[0]["image_size"].as_array().ok_or_else(|| {
                    saccade_core::Error::Config("missing fixture dimensions".into())
                })?;
                let w = size[0].as_u64().unwrap_or(0) as u32;
                let h = size[1].as_u64().unwrap_or(0) as u32;
                label.hotspots.push(saccade_core::report::Hotspot {
                    pixel_runs: Vec::new(),
                    rect_px: [w / 4, h / 4, w / 2, h / 2],
                    rect_frac: [0.25, 0.25, 0.5, 0.5],
                    area_px: 0,
                    area_frac: 0.0,
                    mean_flip: 0.0,
                    max_flip: 0.0,
                    share_of_total_error: 0.0,
                    position: "manual detail crop (statistics not measured)".into(),
                });
            }
            label.evidence_hash = identity(&label);
            items.push(label);
        }
    }
    if items.len() != 20 {
        return Err(saccade_core::Error::Config(format!(
            "expected 20 public labels, found {}",
            items.len()
        )));
    }
    saccade_core::review::write_json(
        out,
        &serde_json::to_value(Labels {
            schema: SCHEMA.into(),
            items,
        })?,
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(saccade_core::Error::Config(
            "usage: build_review_labels REPORTS_DIR OUT_JSON".into(),
        ));
    }
    build(Path::new(&args[1]), Path::new(&args[2]))
}
