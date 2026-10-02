//! MCP bindings for S6. Shell capture is deliberately absent.
use crate::agent::CliError;
#[cfg(feature = "workbench")]
use saccade_core::inbox::Question;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

#[cfg(any(feature = "graphics", feature = "workbench"))]
mod local_tools {
    use super::*;
    fn str_arg(args: &Map<String, Value>, key: &str) -> Result<Option<String>, CliError> {
        match args.get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            _ => Err(CliError::usage(format!("{key} must be a string"))),
        }
    }
    fn required(args: &Map<String, Value>, key: &str) -> Result<String, CliError> {
        str_arg(args, key)?.ok_or_else(|| CliError::usage(format!("missing {key}")))
    }
    #[cfg(feature = "workbench")]
    fn bool_arg(args: &Map<String, Value>, key: &str) -> Result<bool, CliError> {
        match args.get(key) {
            None => Ok(false),
            Some(Value::Bool(b)) => Ok(*b),
            _ => Err(CliError::usage(format!("{key} must be boolean"))),
        }
    }
    fn strings(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, CliError> {
        let value = args
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| CliError::usage(format!("{key} must be an array of strings")))?;
        value
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| CliError::usage(format!("{key} must contain strings")))
            })
            .collect()
    }
    fn unknown(args: &Map<String, Value>, keys: &[&str]) -> Result<(), CliError> {
        if let Some(key) = args.keys().find(|k| !keys.contains(&k.as_str())) {
            return Err(CliError::usage(format!("unknown argument {key}")));
        }
        Ok(())
    }
    #[cfg(feature = "workbench")]
    fn cache(
        args: &Map<String, Value>,
        resolve: &impl Fn(&str, &str) -> Result<PathBuf, CliError>,
    ) -> Result<PathBuf, CliError> {
        str_arg(args, "cache_dir")?.map_or_else(
            || Ok(saccade_core::local::default_cache_dir()),
            |p| resolve("cache_dir", &p),
        )
    }

    pub(crate) fn call(
        name: &str,
        args: &Map<String, Value>,
        resolve: &impl Fn(&str, &str) -> Result<PathBuf, CliError>,
    ) -> Option<Result<(Value, String), CliError>> {
        match name {
            #[cfg(feature = "graphics")]
            "saccade_bisect" => Some((|| {
                unknown(
                    args,
                    &["runs", "good", "out_dir", "threshold", "metric", "entries"],
                )?;
                let runs = strings(args, "runs")?
                    .iter()
                    .map(|p| resolve("runs", p))
                    .collect::<Result<Vec<_>, _>>()?;
                let good = str_arg(args, "good")?
                    .map(|p| resolve("good", &p))
                    .transpose()?;
                let out = resolve("out_dir", &required(args, "out_dir")?)?;
                let threshold = match args.get("threshold") {
                    None => None,
                    Some(v) => Some(
                        v.as_f64()
                            .ok_or_else(|| CliError::usage("threshold must be a number"))?,
                    ),
                };
                let opts = crate::s6::options(
                    threshold,
                    str_arg(args, "metric")?.as_deref(),
                    str_arg(args, "entries")?,
                )?;
                let result = saccade_core::bisect::runs(&runs, good.as_deref(), &out, &opts)?;
                Ok((serde_json::to_value(&result)?, result.text()))
            })()),
            #[cfg(feature = "workbench")]
            "saccade_ask_human" => Some((|| {
                unknown(
                    args,
                    &[
                        "serve",
                        "cache_dir",
                        "question",
                        "allowed_answers",
                        "context",
                        "link",
                        "from",
                        "wait",
                        "timeout",
                    ],
                )?;
                let client =
                    crate::s6::InboxClient::new(&required(args, "serve")?, &cache(args, resolve)?)?;
                let question = Question {
                    question: required(args, "question")?,
                    allowed_answers: strings(args, "allowed_answers")?,
                    context: str_arg(args, "context")?,
                    link: str_arg(args, "link")?,
                    from: str_arg(args, "from")?,
                };
                let timeout = match args.get("timeout") {
                    None => 600,
                    Some(v) => v
                        .as_u64()
                        .ok_or_else(|| CliError::usage("timeout must be an unsigned integer"))?,
                };
                let result = client.ask(question, bool_arg(args, "wait")?, timeout)?;
                Ok((
                    serde_json::to_value(&result)?,
                    format!("{}: {}; {}", result.id, result.status, result.url),
                ))
            })()),
            #[cfg(feature = "workbench")]
            "saccade_inbox_get" => Some((|| {
                unknown(args, &["serve", "cache_dir", "id"])?;
                let client =
                    crate::s6::InboxClient::new(&required(args, "serve")?, &cache(args, resolve)?)?;
                let item = client.get(&required(args, "id")?)?;
                Ok((
                    serde_json::to_value(&item)?,
                    format!("{}: {}", item.id, item.status),
                ))
            })()),
            _ => None,
        }
    }
}
#[cfg(any(feature = "graphics", feature = "workbench"))]
pub(crate) use local_tools::call;
#[cfg(not(any(feature = "graphics", feature = "workbench")))]
pub(crate) fn call(
    _name: &str,
    _args: &Map<String, Value>,
    _resolve: &impl Fn(&str, &str) -> Result<PathBuf, CliError>,
) -> Option<Result<(Value, String), CliError>> {
    None
}

fn output(name: &str) -> Value {
    let source = match name {
        "bisect" => include_str!("../../../schemas/saccade-bisect.v1.schema.json"),
        "inbox" => include_str!("../../../schemas/saccade-inbox-item.v1.schema.json"),
        "ask" => include_str!("../../../schemas/saccade-ask-result.v1.schema.json"),
        _ => include_str!("../../../schemas/saccade-result.v1.schema.json"),
    };
    serde_json::from_str(source).unwrap_or_else(|_| json!({"type":"object"}))
}

pub(crate) fn schemas() -> Vec<Value> {
    let string = json!({"type":"string","minLength":1});
    let array = json!({"type":"array","items":{"type":"string","minLength":1},"minItems":1});
    let tool = |name, description, properties, required, output, read_only| json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"outputSchema":output,"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"idempotentHint":read_only,"openWorldHint":false}});
    let mut tools = vec![
        tool(
            "saccade_bisect",
            "Find the first diverging existing run (ordered oldest to newest). Identity by default. Capture-command mode is CLI-only because it executes user shell code.",
            json!({"runs":array,"good":string,"out_dir":string,"threshold":{"type":"number","minimum":0,"maximum":1},"metric":{"enum":["mean","p95","p99","max"]},"entries":string}),
            json!(["runs", "out_dir"]),
            output("bisect"),
            false,
        ),
        tool(
            "saccade_ask_human",
            "Post a closed-answer question to local serve; optionally wait up to timeout seconds (default 600). Never approves or changes a baseline. Requires that serve's private discovery cache.",
            json!({"serve":string,"cache_dir":string,"question":string,"allowed_answers":array,"context":string,"link":string,"from":string,"wait":{"type":"boolean"},"timeout":{"type":"integer","minimum":0}}),
            json!(["serve", "question", "allowed_answers"]),
            output("ask"),
            false,
        ),
        tool(
            "saccade_inbox_get",
            "Read a local human inbox item and its answer.",
            json!({"serve":string,"cache_dir":string,"id":string}),
            json!(["serve", "id"]),
            output("inbox"),
            true,
        ),
    ];
    tools.retain(|t| crate::operation_available(t["name"].as_str().unwrap_or("")));
    tools
}
