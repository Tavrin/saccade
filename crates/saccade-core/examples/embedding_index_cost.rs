//! Reproducible CPU-only synthetic segmented-index cost measurement; no model execution.
use saccade_core::general::{embedding, embedding_index};
use serde_json::json;
use std::{error::Error, path::Path, time::Instant};
fn model() -> Result<embedding::Model, Box<dyn Error>> {
    Ok(embedding::parse_model(&serde_json::to_vec(
        &json!({"schema":embedding::MODEL_SCHEMA,"family":"dinov2-small","artifact":{"role":"embedding","version":"a".repeat(40),"format":"onnx","url":format!("https://example.org/{}/model.onnx","a".repeat(40)),"bytes":1,"sha256":"a".repeat(64),"license":"Apache-2.0"},"input":"pixels","output":"embedding","size":[8,8],"mean":[0.,0.,0.],"std":[1.,1.,1.],"dimensions":384,"calibration":null}),
    )?)?)
}
fn vector(i: u32) -> Vec<f32> {
    let mut state = i.wrapping_add(1);
    (0..384)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f64 / u32::MAX as f64 - 0.5) as f32
        })
        .collect()
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: embedding_index_cost build|query DIRECTORY ROWS".into());
    }
    let dir = Path::new(&args[2]);
    let rows: usize = args[3].parse()?;
    let m = model()?;
    let id = saccade_core::localized::digest(&serde_json::to_vec(&m)?);
    let start = Instant::now();
    match args[1].as_str() {
        "build" => {
            let mut u = embedding_index::Update::begin(dir, m)?;
            for i in 0..rows {
                let label = format!("{i:07}.png");
                let hash = saccade_core::localized::digest(label.as_bytes());
                u.upsert(&id, &label, &hash, vector(i as u32))?;
            }
            let receipt = u.commit()?;
            println!("{receipt}");
        }
        "query" => {
            let index = embedding_index::Index::load(dir)?;
            if index.len() != rows {
                return Err("unexpected row count".into());
            }
            let hit = index.query(&id, &vector(42), 10)?;
            if hit["hits"][0]["row"]["path"] != "0000042.png" {
                return Err("exact self retrieval failed".into());
            }
            println!("{hit}");
        }
        _ => return Err("unknown operation".into()),
    }
    println!(
        "{}",
        json!({"operation":args[1],"rows":rows,"dimensions":384,"vector_bytes":rows as u64*384*4,"elapsed_seconds":start.elapsed().as_secs_f64(),"profile":"dev, unoptimized; synthetic vectors; includes hash verification"})
    );
    Ok(())
}
