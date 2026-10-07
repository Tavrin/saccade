//! Assist extends the existing review tool; startup authority cannot come from arguments.
use super::{
    Server, ToolOutput, ToolResult, arg_bool, arg_f64, arg_str, arg_strings, reject_unknown,
    require_str,
};
use crate::{
    agent::CliError,
    assist_cmd::{self, Common, Routing, VisibleKind},
};
use saccade_core::assist::schema::Task;
use serde_json::{Map, Value, json};

impl Server {
    fn video_judge(&self, args: &Map<String, Value>) -> ToolResult {
        reject_unknown(
            args,
            &[
                "operation",
                "rubric",
                "frame_map",
                "image",
                "reference",
                "reference_frame_map",
                "view_id",
                "contact_sheet",
                "repeats",
                "model",
                "revision",
                "fps",
                "max_edge",
                "max_spend_usd",
                "out",
                "experimental",
            ],
        )?;
        let rubric = self.existing_file("rubric", &require_str(args, "rubric")?)?;
        let maps = arg_strings(args, "frame_map")?
            .iter()
            .map(|p| self.existing_file("frame_map", p))
            .collect::<Result<Vec<_>, _>>()?;
        let images = arg_strings(args, "image")?
            .iter()
            .map(|p| self.existing_file("image", p))
            .collect::<Result<Vec<_>, _>>()?;
        let reference = arg_str(args, "reference")?
            .map(|p| self.existing_file("reference", &p))
            .transpose()?;
        if !(1..=2).contains(&(maps.len() + images.len())) {
            return Err(CliError::usage("video needs one or two frame maps"));
        }
        let edge = arg_f64(args, "max_edge")?.unwrap_or(256.0);
        if !edge.is_finite() || edge.fract() != 0.0 || !(1.0..=2048.0).contains(&edge) {
            return Err(CliError::usage("video max edge"));
        }
        let repeats = arg_f64(args, "repeats")?.unwrap_or(1.0);
        if !repeats.is_finite() || repeats.fract() != 0.0 || !(1.0..=32.0).contains(&repeats) {
            return Err(CliError::usage("video repeats must be an integer in 1..32"));
        }
        let out = self.resolve("out", &require_str(args, "out")?)?;
        let value = crate::video_judge_cmd::execute(crate::video_judge_cmd::Args {
            rubric,
            frame_map: maps,
            image: images,
            reference,
            reference_frame_map: arg_str(args, "reference_frame_map")?
                .map(|p| self.existing_file("reference_frame_map", &p))
                .transpose()?,
            view_id: arg_strings(args, "view_id")?,
            contact_sheet: arg_bool(args, "contact_sheet")?.unwrap_or(false),
            repeats: repeats as u32,
            model: arg_strings(args, "model")?,
            revision: arg_strings(args, "revision")?,
            fps: arg_f64(args, "fps")?.unwrap_or(1.0),
            max_edge: edge as u32,
            max_spend_usd: arg_str(args, "max_spend_usd")?.unwrap_or_else(|| "2".into()),
            out,
            experimental: arg_bool(args, "experimental")?.unwrap_or(false),
        })?;
        Ok(ToolOutput {
            structured: value,
            text: "Prepared advisory video requests offline; no provider dispatch.".into(),
            images: Vec::new(),
        })
    }
    pub(super) fn assist_review(&self, args: &Map<String, Value>) -> ToolResult {
        if arg_str(args, "operation")?.as_deref() == Some("video-judge") {
            return self.video_judge(args);
        }
        reject_unknown(
            args,
            &[
                "operation",
                "artifact",
                "out",
                "entry",
                "experimental",
                "run",
                "offline",
                "replay",
                "route",
                "jev_routing",
                "budget_calls",
                "max_spend_usd",
                "deadline_secs",
                "gemini_revision",
                "jev_revision",
                "bypass_cache",
                "source_evidence",
                "incomplete_capture",
                "pre_masked",
                "condition",
                "box",
                "kind",
                "mask_manifest",
                "target",
                "second_target",
                "response",
            ],
        )?;
        let operation = require_str(args, "operation")?;
        if let Some(action) = operation.strip_prefix("batch-") {
            return self.assist_batch(action, args);
        }
        let file = self.existing_file("artifact", &require_str(args, "artifact")?)?;
        if let Some(reference) = args.get("artifact").and_then(Value::as_object) {
            let actual = saccade_core::evidence::canonical::Digest::of_bytes(
                &saccade_core::assist::read_bytes(&file, 32 * 1024 * 1024)?,
            )
            .as_str()
            .to_owned();
            if reference["sha256"].as_str() != Some(&actual) {
                return Err(CliError::new(
                    "stale_action",
                    "assist artifact digest changed",
                ));
            }
        }
        let run = arg_bool(args, "run")?.unwrap_or(false);
        let user = crate::review_cmd::load_user(&crate::review_cmd::user_file(
            self.providers.user_config.as_deref(),
        ))?;
        let startup_budget = self.providers.budget_calls.unwrap_or(0);
        let auth = crate::review_cmd::authorization(
            self.providers.allow_provider_calls,
            startup_budget,
            &self.run_id,
            &user,
        );
        if run {
            auth.check().map_err(|_| {
                CliError::new(
                    "network_authorization_required",
                    "assist requires human MCP startup authorization",
                )
            })?;
        }
        let number = |name: &str, default: u64| -> Result<u64, CliError> {
            match args.get(name) {
                None => Ok(default),
                Some(v) => v
                    .as_u64()
                    .ok_or_else(|| CliError::usage(format!("{name} must be an integer"))),
            }
        };
        let budget = number("budget_calls", 6.min(startup_budget.max(1)))?;
        if run && budget > startup_budget {
            return Err(CliError::usage(
                "tool request budget exceeds startup authorization",
            ));
        }
        let route = match arg_str(args, "route")?.as_deref().unwrap_or("cascade") {
            "rules" => Routing::Rules,
            "cascade" => Routing::Cascade,
            "all-vision" => Routing::AllVision,
            _ => return Err(CliError::usage("invalid assist route")),
        };
        let source_evidence = arg_strings(args, "source_evidence")?
            .into_iter()
            .map(|p| self.existing_file("source_evidence", &p))
            .collect::<Result<Vec<_>, _>>()?;
        let replay = arg_str(args, "replay")?
            .map(|p| self.existing_file("replay", &p))
            .transpose()?;
        let offline = arg_bool(args, "offline")?.unwrap_or(false);
        if run && offline || replay.is_some() && !offline {
            return Err(CliError::usage(
                "replay requires offline; offline conflicts with run",
            ));
        }
        let common = Common {
            #[cfg(feature = "vision-providers")]
            vision_provider: None,
            #[cfg(feature = "vision-providers")]
            vision_response: None,
            user_policy_file: self.providers.user_config.clone(),
            experimental: arg_bool(args, "experimental")?.unwrap_or(false),
            out: self.resolve("out", &require_str(args, "out")?)?,
            offline,
            replay,
            run,
            route,
            jev_routing: arg_bool(args, "jev_routing")?.unwrap_or(false),
            budget_calls: budget,
            max_spend_usd: arg_f64(args, "max_spend_usd")?.unwrap_or(0.15),
            deadline_secs: number("deadline_secs", 300)?,
            gemini_revision: arg_str(args, "gemini_revision")?,
            jev_revision: arg_str(args, "jev_revision")?
                .unwrap_or_else(|| saccade_core::assist::schema::JEV.into()),
            bypass_cache: arg_bool(args, "bypass_cache")?.unwrap_or(false),
            source_evidence,
            incomplete_capture: arg_bool(args, "incomplete_capture")?.unwrap_or(false),
            pre_masked: arg_bool(args, "pre_masked")?.unwrap_or(false),
        };
        let (value, _exit) = match operation.as_str() {
            "explain" | "audit-mask" => {
                if args.contains_key("condition")
                    || args.contains_key("box")
                    || args.contains_key("kind")
                    || args.contains_key("target")
                    || args.contains_key("second_target")
                {
                    return Err(CliError::usage(
                        "report assist does not take a single-image condition",
                    ));
                }
                let masks = arg_str(args, "mask_manifest")?
                    .map(|p| self.existing_file("mask_manifest", &p))
                    .transpose()?;
                assist_cmd::report(
                    assist_cmd::ReportArgs {
                        report: file,
                        entry: arg_str(args, "entry")?,
                        mask_manifest: masks,
                        common,
                    },
                    if operation == "explain" {
                        Task::Explain
                    } else {
                        Task::AuditMask
                    },
                    false,
                    Some(&self.policy),
                    Some(&auth),
                )?
            }
            "check-ui" => {
                if args.contains_key("entry") || args.contains_key("mask_manifest") {
                    return Err(CliError::usage(
                        "single-image assist does not take report entry or masks",
                    ));
                }
                let values = args
                    .get("box")
                    .and_then(Value::as_array)
                    .ok_or_else(|| CliError::usage("check-ui requires box [X,Y,W,H]"))?;
                let box_: Vec<u32> = values
                    .iter()
                    .map(|v| {
                        v.as_u64()
                            .and_then(|v| u32::try_from(v).ok())
                            .ok_or_else(|| CliError::usage("invalid box coordinate"))
                    })
                    .collect::<Result<_, _>>()?;
                let box_: [u32; 4] = box_
                    .try_into()
                    .map_err(|_| CliError::usage("box needs four coordinates"))?;
                let kind = match arg_str(args, "kind")?.as_deref().unwrap_or("label-visible") {
                    "label-visible" => VisibleKind::LabelVisible,
                    "banner-absent" => VisibleKind::BannerAbsent,
                    "not-clipped" => VisibleKind::NotClipped,
                    "non-overlap" => VisibleKind::NonOverlap,
                    _ => return Err(CliError::usage("unsupported visible condition")),
                };
                assist_cmd::check(
                    assist_cmd::CheckArgs {
                        condition: require_str(args, "condition")?,
                        image: file,
                        r#box: box_,
                        kind,
                        target: arg_str(args, "target")?,
                        second_target: arg_str(args, "second_target")?,
                        grounding: Default::default(),
                        common,
                    },
                    false,
                    Some(&self.policy),
                    Some(&auth),
                )?
            }
            _ => return Err(CliError::usage("unknown assist operation")),
        };
        Ok(ToolOutput {text:"Experimental AI advice. Read outcome, execution, limitations and unchanged deterministic verdict.".into(),structured:value,images:vec![]})
    }
}
impl Server {
    fn assist_batch(&self, operation: &str, args: &Map<String, Value>) -> ToolResult {
        reject_unknown(
            args,
            &[
                "operation",
                "artifact",
                "out",
                "experimental",
                "run",
                "response",
                "budget_calls",
                "deadline_secs",
            ],
        )?;
        if !["submit", "status", "collect"].contains(&operation) {
            return Err(CliError::usage("unknown Batch operation"));
        }
        let plan = self.existing_file("artifact", &require_str(args, "artifact")?)?;
        if let Some(reference) = args.get("artifact").and_then(Value::as_object) {
            let hash = saccade_core::evidence::canonical::Digest::of_bytes(
                &saccade_core::assist::read_bytes(&plan, 32 * 1024 * 1024)?,
            );
            if reference["sha256"].as_str() != Some(hash.as_str()) {
                return Err(CliError::new("stale_action", "Batch plan digest changed"));
            }
        }
        let user = crate::review_cmd::load_user(&crate::review_cmd::user_file(
            self.providers.user_config.as_deref(),
        ))?;
        let auth = crate::review_cmd::authorization(
            self.providers.allow_provider_calls,
            self.providers.budget_calls.unwrap_or(0),
            &self.run_id,
            &user,
        );
        let integer = |name: &str, default: u64| -> Result<u64, CliError> {
            args.get(name).map_or(Ok(default), |v| {
                v.as_u64()
                    .ok_or_else(|| CliError::usage("Batch integer control"))
            })
        };
        let (value, _exit) = crate::assist_batch_cmd::execute(
            operation,
            crate::assist_batch_cmd::BatchCommon {
                allow_spend_above_25_usd: false,
                plan,
                job: self.resolve("out", &require_str(args, "out")?)?,
                experimental: arg_bool(args, "experimental")?.unwrap_or(false),
                run: arg_bool(args, "run")?.unwrap_or(false),
                response: arg_str(args, "response")?
                    .map(|p| self.existing_file("response", &p))
                    .transpose()?,
                budget_calls: integer(
                    "budget_calls",
                    self.providers.budget_calls.unwrap_or(8).min(128),
                )?,
                deadline_secs: integer("deadline_secs", 300)?,
            },
            self.providers.user_config.as_deref(),
            Some(&self.policy),
            Some(&auth),
        )?;
        Ok(ToolOutput {
            text: "Asynchronous experimental Batch receipt; no approval authority.".into(),
            structured: value,
            images: vec![],
        })
    }
}
pub(super) fn schemas() -> Vec<Value> {
    let mut variants = vec![
        json!({"type":"object","additionalProperties":false,"required":["operation","rubric","model","revision","out","experimental"],
        "anyOf":[{"required":["frame_map"],"properties":{"frame_map":{"minItems":1}}},{"required":["image"],"properties":{"image":{"minItems":1}}}],
        "not":{"required":["reference","reference_frame_map"]},"properties":{
        "operation":{"const":"video-judge"},"rubric":{"type":"string"},"frame_map":{"type":"array","maxItems":2,"items":{"type":"string"}},
        "image":{"type":"array","maxItems":2,"items":{"type":"string"}},
        "reference":{"type":"string"},"reference_frame_map":{"type":"string"},
        "view_id":{"type":"array","maxItems":2,"items":{"type":"string","minLength":1,"maxLength":128}},"contact_sheet":{"type":"boolean"},"repeats":{"type":"integer","minimum":1,"maximum":32},
        "model":{"type":"array","minItems":1,"items":{"type":"string"}},"revision":{"type":"array","minItems":1,"items":{"type":"string"}},
        "fps":{"type":"number","exclusiveMinimum":0,"maximum":120},"max_edge":{"type":"integer","minimum":1,"maximum":2048},
        "max_spend_usd":{"type":"string"},"out":{"type":"string"},"experimental":{"const":true}}}),
    ];
    for op in ["explain", "audit-mask", "check-ui"] {
        let mut props = json!({"operation":{"const":op},"artifact":{"oneOf":[{"type":"string"},{"type":"object","properties":{"path":{"type":"string"},"sha256":{"type":"string","pattern":"^sha256:[0-9a-f]{64}$"}},"required":["path","sha256"],"additionalProperties":false}]},"out":{"type":"string"},"experimental":{"type":"boolean","const":true},"run":{"type":"boolean","default":false},"offline":{"type":"boolean","default":false},"replay":{"type":"string"},"route":{"enum":["rules","cascade","all-vision"]},"jev_routing":{"type":"boolean","default":false},"budget_calls":{"type":"integer","minimum":1,"maximum":8},"max_spend_usd":{"type":"number","exclusiveMinimum":0,"maximum":0.15},"deadline_secs":{"type":"integer","minimum":1,"maximum":300},"gemini_revision":{"type":"string"},"jev_revision":{"type":"string"},"bypass_cache":{"type":"boolean"},"source_evidence":{"type":"array","items":{"type":"string"},"maxItems":2},"incomplete_capture":{"type":"boolean"},"pre_masked":{"type":"boolean"}});
        let mut required = vec!["operation", "artifact", "out", "experimental"];
        if op == "check-ui" {
            props["condition"] = json!({"type":"string","maxLength":512});
            props["box"] = json!({"type":"array","items":{"type":"integer","minimum":0},"minItems":4,"maxItems":4});
            props["kind"] =
                json!({"enum":["label-visible","banner-absent","not-clipped","non-overlap"]});
            props["target"] = json!({"type":"string"});
            props["second_target"] = json!({"type":"string"});
            required.extend(["condition", "box"]);
        } else {
            props["entry"] = json!({"type":"string"});
            props["mask_manifest"] = json!({"type":"string"});
        }
        variants.push(json!({"type":"object","properties":props,"required":required,"additionalProperties":false}));
    }
    for op in ["batch-submit", "batch-status", "batch-collect"] {
        variants.push(json!({"type":"object","properties":{"operation":{"const":op},"artifact":{"oneOf":[{"type":"string"},{"type":"object","properties":{"path":{"type":"string"},"sha256":{"type":"string","pattern":"^sha256:[0-9a-f]{64}$"}},"required":["path","sha256"],"additionalProperties":false}]},"out":{"type":"string"},"experimental":{"type":"boolean","const":true},"run":{"type":"boolean","default":false},"response":{"type":"string"},"budget_calls":{"type":"integer","minimum":1,"maximum":128},"deadline_secs":{"type":"integer","minimum":1,"maximum":300}},"required":["operation","artifact","out","experimental"],"additionalProperties":false}));
    }
    variants
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn judge_parity_mcp_schema_accepts_still_reference_and_refuses_empty_or_ambiguous_inputs() {
        let schema = schemas().remove(0);
        let validator = jsonschema::validator_for(&schema).unwrap();
        let mut args = json!({"operation":"video-judge","rubric":"rubric.json","image":["candidate.png"],
            "model":["model"],"revision":["revision"],"out":"plan","experimental":true,
            "reference":"reference.png","view_id":["view"],"contact_sheet":false});
        assert!(validator.is_valid(&args));
        args["frame_map"] = json!([]);
        assert!(validator.is_valid(&args));
        args["image"] = json!([]);
        assert!(!validator.is_valid(&args));
        args["frame_map"] = json!(["map.json"]);
        assert!(validator.is_valid(&args));
        args["reference_frame_map"] = json!("reference-map.json");
        assert!(!validator.is_valid(&args));
    }
}
