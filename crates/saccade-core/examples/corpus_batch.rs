//! Offline collection only. Paid submission/counting requires the authorized assist executor.
use saccade_core::judge_provider::batch::collect;
use serde_json::Value;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 || args[1] != "--collect" {
        return Err(
            "live batch operations disabled; usage: corpus_batch --collect OPERATION REQUESTS"
                .into(),
        );
    }
    let operation: Value = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    let requests: Value = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    let rows: Vec<_> = collect(&operation, requests.as_array().ok_or("invalid requests")?)?
        .into_iter()
        .map(|(id, response)| serde_json::json!({"job_id":id,"response":response}))
        .collect();
    println!("{}", serde_json::to_string(&rows)?);
    Ok(())
}
