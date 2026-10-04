//! Private R12q Batch API boundary. Never accepts a key or endpoint argument.
use saccade_core::judge_provider::Keys;
use saccade_core::judge_provider::batch::{BatchNetwork, GeminiBatch, collect};
use serde_json::Value;
use std::path::Path;

fn read(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(Path::new(path))?)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !(args.len() == 4 || (args.len() == 3 && args[1] == "--get")) {
        return Err("usage: corpus_batch --count MODEL REQUEST | --submit MODEL BATCH | --get NAME | --collect OPERATION REQUESTS".into());
    }
    let keys = Keys::new(None);
    let http = BatchNetwork;
    let batch = GeminiBatch {
        keys: &keys,
        http: &http,
    };
    let value = match args[1].as_str() {
        "--count" => {
            serde_json::json!({"totalTokens": batch.count_tokens(&args[2], &read(&args[3])?)?})
        }
        "--submit" => batch.submit(&args[2], &std::fs::read(&args[3])?)?,
        "--get" => batch.get(&args[2])?,
        "--collect" => {
            let operation = read(&args[2])?;
            let requests = read(&args[3])?;
            let requests = requests.as_array().ok_or("invalid requests")?;
            Value::Array(
                collect(&operation, requests)?
                    .into_iter()
                    .map(|(id, response)| serde_json::json!({"job_id":id,"response":response}))
                    .collect(),
            )
        }
        _ => return Err("invalid corpus_batch command".into()),
    };
    println!("{}", serde_json::to_string(&value)?);
    Ok(())
}
