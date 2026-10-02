//! Additive root-confined MCP pre-check tools; no AI/network MCP option.

use crate::agent::CliError;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"name":"saccade_safety","description":"Photosensitivity PRE-CHECK only. Not certification; does not replace Harding FPA/platform required testing or formal compliance. Deterministic numbered frames, or optional ffmpeg video.","annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false},"inputSchema":{"type":"object","additionalProperties":false,"properties":{"input":{"type":"string","minLength":1},"out_dir":{"type":"string","minLength":1},"fps":{"type":"number","exclusiveMinimum":0},"display":{"type":"string"},"standard":{"enum":["itu-bt1702","wcag"]},"junit":{"type":"string","minLength":1}},"required":["input","out_dir"]},"outputSchema":serde_json::from_str::<Value>(include_str!("../../../schemas/saccade-safety.v1.schema.json")).unwrap_or_else(|_|json!({"type":"object"}))}),
        json!({"name":"saccade_a11y","description":"Accessibility PRE-CHECK only: Machado colour-vision simulations, candidate information loss and contrast of confirmed config regions. Not certification or formal compliance. No network.","annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false},"inputSchema":{"type":"object","additionalProperties":false,"properties":{"input":{"type":"string","minLength":1},"out_dir":{"type":"string","minLength":1},"config":{"type":"string","minLength":1},"junit":{"type":"string","minLength":1}},"required":["input","out_dir"]},"outputSchema":serde_json::from_str::<Value>(include_str!("../../../schemas/saccade-a11y.v1.schema.json")).unwrap_or_else(|_|json!({"type":"object"}))}),
    ]
}
fn string(args: &Map<String, Value>, key: &str) -> Result<Option<String>, CliError> {
    match args.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        _ => Err(CliError::usage(format!("{key} must be a nonempty string"))),
    }
}
fn required(args: &Map<String, Value>, key: &str) -> Result<String, CliError> {
    string(args, key)?.ok_or_else(|| CliError::usage(format!("missing {key}")))
}

pub(crate) fn call(
    name: &str,
    args: &Map<String, Value>,
    resolve: &impl Fn(&str, &str) -> Result<PathBuf, CliError>,
) -> Option<Result<(Value, String), CliError>> {
    if !matches!(name, "saccade_safety" | "saccade_a11y") {
        return None;
    }
    Some((|| {
        let allowed: &[&str] = if name == "saccade_safety" {
            &["input", "out_dir", "fps", "display", "standard", "junit"]
        } else {
            &["input", "out_dir", "config", "junit"]
        };
        if let Some(key) = args.keys().find(|k| !allowed.contains(&k.as_str())) {
            return Err(CliError::usage(format!("unknown argument {key}")));
        }
        let input = resolve("input", &required(args, "input")?)?;
        let out = resolve("out_dir", &required(args, "out_dir")?)?;
        let junit = string(args, "junit")?
            .map(|p| resolve("junit", &p))
            .transpose()?;
        if name == "saccade_safety" {
            let fps = args
                .get("fps")
                .map(|v| {
                    v.as_f64()
                        .ok_or_else(|| CliError::usage("fps must be a number"))
                })
                .transpose()?;
            let display = string(args, "display")?
                .as_deref()
                .map(saccade_core::safety::Display::parse)
                .transpose()?
                .unwrap_or_default();
            let standard = saccade_core::safety::Standard::parse(
                string(args, "standard")?.as_deref().unwrap_or("itu-bt1702"),
            )?;
            let report = saccade_core::safety::run(
                &input,
                &out,
                &saccade_core::safety::Options {
                    fps,
                    display,
                    standard,
                },
            )?;
            if let Some(path) = &junit {
                saccade_core::safety::output::junit(
                    path,
                    "saccade safety pre-check",
                    &report.junit_cases(),
                    &[&input],
                )?;
            }
            Ok((serde_json::to_value(&report)?, report.text()))
        } else {
            let config = string(args, "config")?
                .map(|p| resolve("config", &p))
                .transpose()?;
            let report = saccade_core::a11y::run(
                &input,
                &out,
                &saccade_core::a11y::Options {
                    config: config.clone(),
                    ..Default::default()
                },
            )?;
            if let Some(path) = &junit {
                let mut inputs = vec![input.as_path()];
                if let Some(c) = &config {
                    inputs.push(c.as_path());
                }
                saccade_core::safety::output::junit(
                    path,
                    "saccade accessibility pre-check",
                    &report.junit_cases(),
                    &inputs,
                )?;
            }
            Ok((serde_json::to_value(&report)?, report.text()))
        }
    })())
}
