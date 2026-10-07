//! One closed answer schema for Gemini, OpenRouter and offline stage-2 plans.
use super::{Result, decode};
use serde_json::{Value, json};

/// Shared JSON Schema; semantic, citation and geometry checks remain local.
pub fn answer_schema() -> Result<Value> {
    decode(include_bytes!("answer.schema.json"))
}

/// Exact strict OpenRouter format, pinned before authorization or accounting.
pub fn openrouter_format() -> Result<Value> {
    Ok(json!({"type":"json_schema","json_schema":{
        "name":"saccade_assist_answer","strict":true,"schema":answer_schema()?
    }}))
}
