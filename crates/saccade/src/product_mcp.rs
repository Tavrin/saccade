//! Product tool adapter with startup-only network and notification authority.
use super::{Server, ToolOutput, ToolResult, arg_str, reject_unknown, require_str};
use crate::{
    agent::CliError, design_cmd as design, imgtune_cmd as tune, product_io as io,
    sweep_cmd as sweep,
};
use serde_json::{Map, Value, json};
use std::path::Path;

impl Server {
    pub(super) fn product_tool(&self, args: &Map<String, Value>) -> ToolResult {
        reject_unknown(
            args,
            &[
                "operation",
                "artifact",
                "out",
                "captures",
                "pull",
                "config",
                "before_origin",
                "after_origin",
                "seed",
                "samples",
                "viewports",
                "accept",
                "fixture_dir",
                "cache",
                "scale",
                "align",
                "resample",
                "baseline",
                "history_store",
                "template",
                "report_link",
            ],
        )?;
        let op = require_str(args, "operation")?;
        let file = |key: &str| -> Result<std::path::PathBuf, CliError> {
            self.existing_file(key, &require_str(args, key)?)
        };
        let output = || -> Result<std::path::PathBuf, CliError> {
            self.resolve("out", &require_str(args, "out")?)
        };
        let network = || {
            if self.product_network {
                Ok(())
            } else {
                Err(CliError::new(
                    "network_not_authorized",
                    "product HTTP requires human startup --allow-product-network",
                ))
            }
        };
        let config = || -> Result<saccade_core::config::RunConfig, CliError> {
            if let Some(path) = arg_str(args, "config")? {
                let path = self.existing_file("config", &path)?;
                let config = crate::load_config(Some(&path))?;
                self.validate_config(&config)?;
                Ok(config)
            } else {
                Ok(saccade_core::config::RunConfig::default())
            }
        };
        let value = match op.as_str() {
            "sweep_plan" => {
                let artifact = file("artifact")?;
                let urls = String::from_utf8(io::read(&artifact)?)
                    .map_err(|_| CliError::usage("URL list must be UTF-8"))?
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .map(str::to_owned)
                    .collect();
                let viewports: Vec<[u32; 2]> = args
                    .get("viewports")
                    .map(|v| serde_json::from_value(v.clone()))
                    .transpose()?
                    .unwrap_or_else(|| vec![[1280, 720]]);
                let manifest = sweep::plan(
                    urls,
                    &require_str(args, "before_origin")?,
                    &require_str(args, "after_origin")?,
                    args.get("samples").and_then(Value::as_u64).unwrap_or(3) as usize,
                    args.get("seed").and_then(Value::as_u64).unwrap_or(42),
                    &viewports,
                )?;
                io::write(&output()?, &manifest)?;
                serde_json::to_value(manifest)?
            }
            "sweep_compare" => {
                let artifact = file("artifact")?;
                let captures = file("captures")?;
                self.document_inputs(&captures)?;
                let last = if arg_str(args, "baseline")?.as_deref() == Some("last-good") {
                    Some(self.product_last_good(args)?)
                } else {
                    None
                };
                let align = arg_str(args, "align")?;
                let resample = arg_str(args, "resample")?;
                if align.is_some() || resample.is_some() {
                    if args.contains_key("config") || align.is_none() {
                        return Err(CliError::usage(
                            "registration requires align and refuses comparison config",
                        ));
                    }
                    use clap::ValueEnum;
                    let options = crate::general_cmd::CompareArgs {
                        align: Some(
                            crate::general_cmd::Align::from_str(
                                align.as_deref().unwrap_or(""),
                                false,
                            )
                            .map_err(CliError::usage)?,
                        ),
                        resample: resample
                            .as_deref()
                            .map(|s| {
                                crate::general_cmd::Resample::from_str(s, false)
                                    .map_err(CliError::usage)
                            })
                            .transpose()?,
                        ..Default::default()
                    };
                    sweep::compare_with_registration(
                        &artifact,
                        &captures,
                        &output()?,
                        &config()?,
                        last.as_ref().map(|t| t.path()),
                        Some(options),
                    )?
                } else {
                    sweep::compare(
                        &artifact,
                        &captures,
                        &output()?,
                        &config()?,
                        last.as_ref().map(|t| t.path()),
                    )?
                }
            }
            "last_good_compare" => {
                let captures = self.existing_dir("captures", &require_str(args, "captures")?)?;
                let last = self.product_last_good(args)?;
                let out = output()?;
                let report = saccade_core::run::run(last.path(), &captures, &out, &config()?)?;
                crate::agent::result_value(
                    &report,
                    &out.join(saccade_core::report::REPORT_FILE_NAME),
                    5,
                    false,
                )
            }
            "imgtune_audit" => {
                network()?;
                let artifact = file("artifact")?;
                let urls = String::from_utf8(io::read(&artifact)?)
                    .map_err(|_| CliError::usage("URL list must be UTF-8"))?
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let accepts: Vec<String> = serde_json::from_value(
                    args.get("accept")
                        .cloned()
                        .ok_or_else(|| CliError::usage("Accept headers required"))?,
                )?;
                let value = tune::audit(&urls, &accepts)?;
                io::write(&output()?, &value)?;
                value
            }
            "imgtune_search" => {
                let artifact = file("artifact")?;
                let input: tune::Search = io::json(&artifact)?;
                let root = artifact.parent().unwrap_or(Path::new("."));
                if matches!(input.adapter, tune::Adapter::UrlTemplate { .. }) {
                    network()?;
                }
                for image in &input.images {
                    for path in [&image.source, &image.current] {
                        if path.starts_with("http://") || path.starts_with("https://") {
                            network()?;
                        } else {
                            self.policy.read(&root.join(path)).map_err(|_| {
                                CliError::new(
                                    "unsafe_path",
                                    "image tuning source outside registered roots",
                                )
                            })?;
                        }
                    }
                }
                let value = tune::search(input, root)?;
                io::write(&output()?, &value)?;
                value
            }
            "design_pull" => {
                let artifact = file("artifact")?;
                let map = design::mapping(&artifact)?;
                let out = output()?;
                let cache = self.resolve("out", &require_str(args, "cache")?)?;
                let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
                let pull = if let Some(root) = arg_str(args, "fixture_dir")? {
                    let root = self.existing_dir("fixture_dir", &root)?;
                    for frame in &map.frames {
                        for kind in ["file", "styles", "variables", "images"] {
                            self.policy
                                .read(&root.join(&frame.file_key).join(format!("{kind}.json")))
                                .map_err(|_| {
                                    CliError::new(
                                        "unsafe_path",
                                        "design fixture outside registered roots",
                                    )
                                })?;
                        }
                        self.policy
                            .read(&root.join("images").join(format!(
                                "{}.png",
                                design::frame_id(&frame.file_key, &frame.node_id)
                            )))
                            .map_err(|_| {
                                CliError::new(
                                    "unsafe_path",
                                    "design image fixture outside registered roots",
                                )
                            })?;
                    }
                    design::pull(
                        &map,
                        &out,
                        &cache,
                        scale,
                        &design::Fixtures { root },
                        "recorded_fixture",
                    )?
                } else {
                    network()?;
                    design::pull_live(&map, &out, &cache, scale)?
                };
                serde_json::to_value(pull)?
            }
            "design_compare" => {
                let artifact = file("artifact")?;
                let pull = file("pull")?;
                let captures = file("captures")?;
                self.document_inputs(&pull)?;
                self.document_inputs(&captures)?;
                let align = arg_str(args, "align")?.unwrap_or_else(|| "translation".into());
                if !matches!(align.as_str(), "translation" | "none") {
                    return Err(CliError::usage(
                        "design alignment must be none or translation",
                    ));
                }
                design::compare(&artifact, &pull, &captures, &output()?, &config()?, &align)?
            }
            "notify" => {
                if !self.product_notifications {
                    return Err(CliError::new(
                        "notification_not_authorized",
                        "webhooks require explicit human startup --allow-webhook-notifications",
                    ));
                }
                let artifact = file("artifact")?;
                let report: Value = io::json(&artifact)?;
                let template = match arg_str(args, "template")?.as_deref().unwrap_or("generic") {
                    "generic" => crate::notifier_cmd::Template::Generic,
                    "slack" => crate::notifier_cmd::Template::Slack,
                    "teams" => crate::notifier_cmd::Template::Teams,
                    _ => return Err(CliError::usage("unknown webhook template")),
                };
                crate::notifier_cmd::send_configured(
                    &crate::notifier_cmd::summary(
                        &report,
                        &artifact,
                        arg_str(args, "report_link")?.as_deref(),
                    )?,
                    template,
                )?
            }
            _ => return Err(CliError::usage("unknown product operation")),
        };
        // Full evidence stays in artifacts. Returned summaries stay within MCP's
        // normal bounded envelope; provider text never becomes an executable action.
        let mut result = crate::local_cmd::base_result(&op);
        result["measurement"] = json!(match value["verdict"].as_str() {
            Some("pass" | "complete") => "pass",
            Some("regression" | "incomplete") => "regression",
            _ => "unknown",
        });
        result["data"] = if serde_json::to_vec(&value)?.len() <= 6000 {
            value.clone()
        } else {
            json!({"schema":value["schema"],"verdict":value["verdict"],"full_evidence":"written artifact","items":value["items"].as_array().map(Vec::len),"frames":value["frames"].as_array().map(Vec::len),"capture_failures":value["capture_failures"].as_array().map(Vec::len)})
        };
        if let Some(out) = arg_str(args, "out")? {
            let out = self.resolve("out", &out)?;
            let artifact = match op.as_str() {
                "sweep_compare" => out.join("saccade-sweep-report.v1.json"),
                "design_pull" => out.join("pull.json"),
                "design_compare" => out.join("saccade-design-report.v1.json"),
                "last_good_compare" => out.join(saccade_core::report::REPORT_FILE_NAME),
                _ => out,
            };
            result["artifact"] = crate::local_cmd::reference(&artifact)?;
        }
        let result = crate::local_cmd::bounded(result, 8192)?;
        Ok(ToolOutput {
            structured: result,
            text: format!(
                "Product operation {op}; inspect the written artifact for complete evidence."
            ),
            images: vec![],
        })
    }
    fn product_last_good(&self, args: &Map<String, Value>) -> Result<tempfile::TempDir, CliError> {
        let store = self.existing_dir("history_store", &require_str(args, "history_store")?)?;
        crate::last_good::resolve_checked(&store, |path| {
            self.policy.read(path).map(|_| ()).map_err(|_| {
                CliError::new("unsafe_path", "last-good capture outside registered roots")
            })
        })
    }
}
pub(super) fn schema() -> Value {
    let mut tool = json!({"name":"saccade_products","description":"Wave 5 planning, tuning, design and last-good. Inputs remain inside registered roots, outputs require out-root; HTTP and notifications require independent human startup authorization.","inputSchema":{"type":"object","properties":{"operation":{"enum":["sweep_plan","sweep_compare","last_good_compare","imgtune_audit","imgtune_search","design_pull","design_compare","notify"]},"artifact":{"type":"string"},"out":{"type":"string"},"captures":{"type":"string"},"pull":{"type":"string"},"config":{"type":"string"},"before_origin":{"type":"string"},"after_origin":{"type":"string"},"seed":{"type":"integer","minimum":0},"samples":{"type":"integer","minimum":1,"maximum":100},"viewports":{"type":"array","items":{"type":"array","items":{"type":"integer"},"minItems":2,"maxItems":2}},"accept":{"type":"array","items":{"type":"string"}},"fixture_dir":{"type":"string"},"cache":{"type":"string"},"scale":{"type":"number","minimum":0.01,"maximum":4},"align":{"enum":["none","translation","similarity","affine","homography","auto"]},"resample":{"enum":["reference","common"]},"baseline":{"const":"last-good"},"history_store":{"type":"string"},"template":{"enum":["generic","slack","teams"]},"report_link":{"type":"string"}},"required":["operation"],"additionalProperties":false},"outputSchema":{"type":"object","properties":{"schema":{"const":"saccade-result.v2"}},"required":["schema"]},"annotations":{"destructiveHint":false,"openWorldHint":true}});
    let base = tool["inputSchema"].clone();
    let variants: Vec<_> = base["properties"]["operation"]["enum"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|operation| {
            let mut variant = base.clone();
            variant["properties"]["operation"] = json!({"type":"string","const":operation});
            variant
        })
        .collect();
    tool["inputSchema"] = json!({"type":"object","oneOf":variants});
    tool
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn tool_arguments_cannot_authorize_product_http_or_notifications() {
        let root = tempfile::tempdir().expect("root");
        let server = Server::new(&[root.path().to_path_buf()], None, false, &[]).expect("server");
        for (operation, code) in [
            ("imgtune_audit", "network_not_authorized"),
            ("notify", "notification_not_authorized"),
        ] {
            let args = json!({"operation":operation});
            let error = server
                .product_tool(args.as_object().expect("object"))
                .err()
                .expect("denied");
            assert_eq!(error.code, code);
        }
        let args = json!({"operation":"notify","allow_webhook_notifications":true});
        assert_eq!(
            server
                .product_tool(args.as_object().expect("object"))
                .err()
                .expect("unknown argument")
                .code,
            "usage"
        );
    }
}
