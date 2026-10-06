//! Root-contained offline evidence mirrors on the existing measure tool.
use crate::agent::CliError;
use saccade_core::{evidence_quality::trial, root_policy::RootPolicy};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
enum Operation {
    #[serde(rename = "reference_compare")]
    Reference {
        render: PathBuf,
        reference: PathBuf,
        #[serde(default)]
        seeds: Vec<PathBuf>,
        variance: Option<PathBuf>,
        mask: Option<PathBuf>,
        policy: Option<PathBuf>,
    },
    #[serde(rename = "trial_register")]
    Register { plan: PathBuf, out: PathBuf },
    #[serde(rename = "trial_start")]
    Start { plan: PathBuf, out: PathBuf },
    #[serde(rename = "trial_vote")]
    Vote {
        plan: PathBuf,
        out: PathBuf,
        voter: String,
        item: String,
        answer: String,
    },
    #[serde(rename = "trial_import")]
    Import {
        plan: PathBuf,
        out: PathBuf,
        voter: String,
        judgments: PathBuf,
    },
}
pub(crate) fn handles(name: &str, op: &str) -> bool {
    name == "saccade_measure"
        && matches!(
            op,
            "reference_compare" | "trial_register" | "trial_start" | "trial_vote" | "trial_import"
        )
}
fn plan_inputs(policy: &RootPolicy, plan: &Path) -> Result<(), CliError> {
    let p = trial::read_plan(plan)?;
    let root = plan.parent().unwrap_or(Path::new("."));
    for pair in p.pairs {
        for name in [Some(pair.first), Some(pair.second), pair.mask]
            .into_iter()
            .flatten()
        {
            policy.read(&saccade_core::evidence_quality::relative(root, &name)?)?;
        }
    }
    Ok(())
}
pub(crate) fn call(
    policy: &RootPolicy,
    args: &serde_json::Map<String, Value>,
) -> Result<Value, CliError> {
    let op: Operation = serde_json::from_value(Value::Object(args.clone()))?;
    match op {
        Operation::Reference {
            render,
            reference,
            seeds,
            variance,
            mask,
            policy: p,
        } => {
            let args = crate::wave9_cmd::ReferenceArgs {
                render: policy.read(&render)?,
                reference: policy.read(&reference)?,
                seeds: seeds
                    .iter()
                    .map(|p| policy.read(p))
                    .collect::<Result<_, _>>()?,
                variance: variance.map(|p| policy.read(&p)).transpose()?,
                mask: mask.map(|p| policy.read(&p)).transpose()?,
                policy: p.map(|p| policy.read(&p)).transpose()?,
                out: None,
                json: true,
            };
            Ok(serde_json::to_value(crate::wave9_cmd::reference_value(
                &args,
            )?)?)
        }
        Operation::Register { plan, out } => {
            let plan = policy.read(&plan)?;
            plan_inputs(policy, &plan)?;
            Ok(serde_json::to_value(trial::register(
                &plan,
                &policy.write(&out)?,
            )?)?)
        }
        Operation::Start { plan, out } => {
            let plan = policy.read(&plan)?;
            plan_inputs(policy, &plan)?;
            let out = policy.write(&out)?;
            check_frozen(policy, &out)?;
            Ok(serde_json::to_value(trial::start(&plan, &out)?)?)
        }
        Operation::Vote {
            plan,
            out,
            voter,
            item,
            answer,
        } => {
            let plan = policy.read(&plan)?;
            plan_inputs(policy, &plan)?;
            let out = policy.write(&out)?;
            check_frozen(policy, &out)?;
            Ok(serde_json::to_value(trial::vote(
                &plan, &out, &voter, &item, &answer,
            )?)?)
        }
        Operation::Import {
            plan,
            out,
            voter,
            judgments,
        } => {
            let plan = policy.read(&plan)?;
            plan_inputs(policy, &plan)?;
            let out = policy.write(&out)?;
            check_frozen(policy, &out)?;
            let bytes = saccade_core::evidence_quality::read(&policy.read(&judgments)?, 1 << 20)?;
            Ok(serde_json::to_value(trial::import(
                &plan, &out, &voter, &bytes,
            )?)?)
        }
    }
}
fn check_frozen(policy: &RootPolicy, out: &Path) -> Result<(), CliError> {
    let bytes =
        saccade_core::evidence_quality::read(&policy.read(&out.join("frozen.json"))?, 1 << 20)?;
    let f: trial::Frozen = serde_json::from_slice(&bytes)?;
    for name in f.inputs.keys() {
        policy.read(&saccade_core::evidence_quality::relative(
            Path::new(&f.root),
            name,
        )?)?;
    }
    Ok(())
}
pub(crate) fn schemas() -> Vec<Value> {
    let mut result = vec![
        json!({"type":"object","additionalProperties":false,"required":["operation","render","reference"],"properties":{"operation":{"const":"reference_compare"},"render":{"type":"string"},"reference":{"type":"string"},"seeds":{"type":"array","maxItems":31,"items":{"type":"string"}},"variance":{"type":"string"},"mask":{"type":"string"},"policy":{"type":"string"}}}),
    ];
    for operation in [
        "trial_register",
        "trial_start",
        "trial_vote",
        "trial_import",
    ] {
        let mut properties = json!({"operation":{"const":operation},"plan":{"type":"string"},"out":{"type":"string"}});
        let mut required = vec!["operation", "plan", "out"];
        if operation == "trial_vote" {
            for key in ["voter", "item", "answer"] {
                properties[key] = json!({"type":"string"});
                required.push(key);
            }
            properties["answer"] = json!({"enum":["P1","P2","unsure"]});
        }
        if operation == "trial_import" {
            for key in ["voter", "judgments"] {
                properties[key] = json!({"type":"string"});
                required.push(key);
            }
        }
        result.push(json!({"type":"object","additionalProperties":false,"required":required,"properties":properties}));
    }
    result
}
