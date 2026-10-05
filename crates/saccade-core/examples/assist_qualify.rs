//! Opt-in constructed-epoch provider runner. Ordinary tests never call this example.
use saccade_core::{
    assist::{
        self,
        catalog::{Catalog, Condition, Image, Mask, Region},
        execution::{self, CacheKey, Executor},
        geometry::Transform,
        schema::*,
        workflow::{self, Need},
    },
    budget_ledger::{Caps, Ledger, MoneyScope, Scope},
    evidence::canonical::Digest,
    judge_provider::transport::{self, Authorization},
    root_policy::RootPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct Models {
    gemini: String,
    gemini_revision: String,
    jev: String,
    jev_revision: String,
}
#[derive(Clone, Deserialize)]
struct Case {
    case_id: Digest,
    root_id: Digest,
    family: String,
    split: String,
    workload: String,
    task: Task,
    before: String,
    after: String,
    before_hash: Digest,
    after_hash: Digest,
    dimensions: [u32; 2],
    target: [u32; 4],
    label: String,
    original_pixels: bool,
    complete: bool,
    exclusions: Vec<Mask>,
    source: Option<String>,
    condition: Option<Condition>,
    counterfactual: Option<Value>,
}
#[derive(Deserialize)]
struct Manifest {
    schema: String,
    manifest_hash: Digest,
    oracle_hash: Digest,
    models: Models,
    cases: Vec<Case>,
}
struct Context<'a> {
    user: &'a transport::UserConfig,
    roots: &'a RootPolicy,
    ledger: &'a Ledger,
    keys: &'a saccade_core::judge_provider::Keys,
    epoch: Digest,
    cap: u64,
    api: Digest,
}
struct Pack {
    catalog: Catalog,
    identity: Identity,
    pngs: Vec<(Role, Vec<u8>)>,
    sources: Vec<String>,
}
fn invalid(message: &'static str) -> assist::Error {
    assist::Error::Invalid(message)
}
fn bound_read(
    root: &Path,
    relative: &str,
    policy: &RootPolicy,
    expected: Option<&Digest>,
) -> assist::Result<(Vec<u8>, String)> {
    let path = root.join(relative);
    let path = policy
        .read(&path)
        .map_err(|_| invalid("qualification source outside registered roots"))?;
    if !path.starts_with(root) {
        return Err(invalid("fixture path escaped frozen corpus"));
    }
    let bytes = assist::read_bytes(&path, 16 * 1024 * 1024)?;
    if expected.is_some_and(|hash| Digest::of_bytes(&bytes) != *hash) {
        return Err(invalid("frozen fixture hash drift"));
    }
    Ok((bytes, saccade_core::paths::portable(&path)))
}
fn pack(case: &Case, root: &Path, context: &Context<'_>) -> assist::Result<Pack> {
    let roles = if case.task == Task::CheckUi {
        vec![Role::Single]
    } else {
        vec![Role::Before, Role::After]
    };
    let mut images = Vec::new();
    let mut pngs = Vec::new();
    let mut sources = Vec::new();
    let mut regions = Vec::new();
    for role in roles {
        let (path, hash) = if role == Role::Before {
            (&case.before, &case.before_hash)
        } else {
            (&case.after, &case.after_hash)
        };
        let (png, source) = bound_read(root, path, context.roots, Some(hash))?;
        let dimensions = image::ImageReader::new(std::io::Cursor::new(&png))
            .with_guessed_format()
            .map_err(|_| invalid("fixture PNG"))?
            .into_dimensions()
            .map_err(|_| invalid("fixture dimensions"))?;
        if [dimensions.0, dimensions.1] != case.dimensions {
            return Err(invalid("fixture dimensions changed"));
        }
        images.push(Image {
            role,
            sha256: hash.clone(),
            encoded_sha256: Digest::of_bytes(&png),
            dimensions: case.dimensions,
            capture_scope: [0, 0, dimensions.0, dimensions.1],
            complete: case.complete,
            original_pixels: case.original_pixels,
            transform: Transform {
                crop: [0, 0, dimensions.0, dimensions.1],
                encoded: case.dimensions,
            },
        });
        regions.push(Region {
            id: format!("{role:?}:frame"),
            image_role: role,
            rect: [0, 0, dimensions.0, dimensions.1],
        });
        regions.push(Region {
            id: format!("{role:?}:target"),
            image_role: role,
            rect: case.target,
        });
        pngs.push((role, png));
        sources.push(source);
    }
    let mut catalog = Catalog {
        version: CATALOG_VERSION.into(),
        images,
        regions,
        exclusions: case.exclusions.clone(),
        measurements: json!({}),
        source_evidence: vec![],
        source_evidence_hashes: vec![],
    };
    if let Some(path) = &case.source {
        let (bytes, source) = bound_read(root, path, context.roots, None)?;
        let packet: saccade_core::ui_review::Source = assist::decode(&bytes)?;
        for node in &packet.nodes {
            if let Some(bounds) = node.bounds {
                if bounds.iter().any(|v| {
                    !v.is_finite() || *v < 0. || v.fract() != 0. || *v > f64::from(u32::MAX)
                }) {
                    return Err(invalid("constructed source geometry"));
                }
                catalog.regions.push(Region {
                    id: node.id.clone(),
                    image_role: Role::Single,
                    rect: bounds.map(|v| v as u32),
                });
            }
        }
        catalog
            .source_evidence_hashes
            .push(assist::digest(&packet)?);
        catalog.source_evidence.push(packet);
        sources.push(source);
    }
    let identity = catalog.identity(case.task, None, case.condition.as_ref())?;
    Ok(Pack {
        catalog,
        identity,
        pngs,
        sources,
    })
}
fn call(
    context: &Context<'_>,
    pack: &Pack,
    arm: &str,
    key: &CacheKey,
    payload: &[u8],
    deadline: Instant,
) -> assist::Result<execution::Completed> {
    let auth = Authorization {
        enabled: true,
        scopes: vec![Scope {
            id: format!(
                "constructed/{}/{}/{}",
                context.epoch.as_str(),
                arm,
                pack.identity.request_hash.as_str()
            ),
            caps: Caps {
                total: 8,
                providers: BTreeMap::from([("gemini".into(), 4), ("jev".into(), 2)]),
            },
        }],
    };
    let network = transport::Network;
    let transport = transport::Transport {
        user: context.user,
        roots: context.roots,
        authorization: &auth,
        ledger: context.ledger,
        keys: context.keys,
        http: &network,
    };
    let money = vec![
        MoneyScope {
            id: format!("constructed/epoch/{}", context.epoch.as_str()),
            cap_nano_usd: context.cap,
        },
        MoneyScope {
            id: format!(
                "constructed/entry/{}/{}/{}",
                context.epoch.as_str(),
                arm,
                pack.identity.request_hash.as_str()
            ),
            cap_nano_usd: 150_000_000,
        },
    ];
    let executor = Executor {
        transport: &transport,
        ledger: context.ledger,
        money_scopes: money,
        sources: pack.sources.clone(),
        deadline,
    };
    executor.call(key, payload)
}
#[derive(Serialize)]
struct Row {
    case_id: Digest,
    arm: String,
    outcome: Outcome,
    complete: bool,
    observations: Vec<Observation>,
    provenance: Vec<Provenance>,
    mechanical_valid: bool,
    order_disagreement: bool,
    contradiction_withheld: bool,
    used_vision: bool,
    queue_delay_ms: u64,
    retries: u32,
    counterfactual_checked: bool,
    source_only: bool,
    elapsed_ms: u64,
    failure: Option<String>,
    counterfactual: Option<Box<Row>>,
}
fn row(case: &Case, arm: &str) -> Row {
    Row {
        case_id: case.case_id.clone(),
        arm: arm.into(),
        outcome: Outcome::Unverifiable,
        complete: false,
        observations: vec![],
        provenance: vec![],
        mechanical_valid: true,
        order_disagreement: false,
        contradiction_withheld: true,
        used_vision: false,
        queue_delay_ms: 0,
        retries: 0,
        counterfactual_checked: case.counterfactual.is_none(),
        source_only: false,
        elapsed_ms: 0,
        failure: None,
        counterfactual: None,
    }
}
fn support(
    context: &Context<'_>,
    pack: &Pack,
    arm: &str,
    observations: &[Observation],
    revision: &str,
    deadline: Instant,
) -> assist::Result<(Option<Support>, Provenance)> {
    let payload = workflow::support_payload(&pack.catalog, &pack.identity, observations)?;
    let key = CacheKey {
        evidence_hash: pack.identity.request_hash.clone(),
        payload_hash: Digest::of_bytes(&payload),
        prompt_hash: Digest::of_bytes(execution::DATA_RULE.as_bytes()),
        encoder_version: execution::ENCODER.into(),
        provider: "jev".into(),
        model: JEV.into(),
        revision: revision.into(),
        settings: json!({"choice":"assist.claim_support.v1"}),
        api_config_hash: context.api.clone(),
        order: "support".into(),
    };
    let completed = call(context, pack, arm, &key, &payload, deadline)?;
    let answer = workflow::support_answer(&completed.response, revision).ok();
    Ok((answer, completed.provenance))
}
fn run_case(
    case: &Case,
    arm: &str,
    pack: &Pack,
    context: &Context<'_>,
    models: &Models,
    oracle: &Value,
    corpus: &Path,
) -> Row {
    let clock = Instant::now();
    let deadline = clock + Duration::from_secs(300);
    let mut result = row(case, arm);
    let execution = (|| -> assist::Result<()> {
        let need = workflow::evidence_need(&pack.catalog, case.condition.as_ref(), case.task)?;
        if let Need::Unavailable(_) = need {
            result.complete = true;
            return Ok(());
        }
        if arm == "rules" || arm == "cascade" {
            if let Need::Structured(outcome) = need {
                result.outcome = outcome;
                result.complete = true;
                result.source_only = true;
                return Ok(());
            }
            if arm == "rules" {
                result.complete = true;
                return Ok(());
            }
        }
        if arm == "oracle_jev" {
            // Diagnostic ceiling only: oracle-derived textual observations are
            // segregated from every vision/routing arm and never carry mutation labels.
            let statements = oracle[case.case_id.as_str()]["diagnostic_statements"]
                .as_array()
                .ok_or(invalid("diagnostic oracle statements"))?;
            let role = pack.catalog.images[pack.catalog.images.len() - 1].role;
            let reference = pack
                .catalog
                .regions
                .iter()
                .find(|r| r.image_role == role)
                .ok_or(invalid("diagnostic region"))?;
            for (index, statement) in statements.iter().enumerate() {
                result.observations.push(Observation {
                    observation_id: format!("diagnostic-{index}"),
                    image_role: role,
                    kind: Kind::Presence,
                    statement: statement.as_str().ok_or(invalid("diagnostic text"))?.into(),
                    geometry: assist::geometry::Geometry::Box(case.target.map(f64::from)),
                    visibility: Visibility::Visible,
                    evidence_refs: vec![reference.id.clone()],
                    uncertainty: 0.,
                });
            }
            let (answer, receipt) = support(
                context,
                pack,
                arm,
                &result.observations,
                &models.jev_revision,
                deadline,
            )?;
            result.provenance.push(receipt);
            let answer = answer.ok_or(invalid("invalid Jev support response"))?;
            result.outcome = if answer == Support::Supported {
                assist::decode::<Outcome>(
                    &serde_json::to_vec(&oracle[case.case_id.as_str()]["expected_outcome"])
                        .map_err(|_| invalid("diagnostic outcome"))?,
                )?
            } else {
                Outcome::Unverifiable
            };
            result.complete = true;
            return Ok(());
        }
        let mut outputs = Vec::new();
        let two = arm != "single_gemini" && pack.catalog.images.len() == 2;
        result.used_vision = true;
        for reverse in if two { vec![false, true] } else { vec![false] } {
            let request = workflow::prepare(
                &pack.catalog,
                pack.identity.clone(),
                case.condition.as_ref(),
                &pack.pngs,
                reverse,
                &models.gemini_revision,
                context.api.clone(),
            )?;
            let completed = call(context, pack, arm, &request.key, &request.payload, deadline)?;
            result.provenance.push(completed.provenance.clone());
            outputs.push(workflow::decode_answer(
                &pack.catalog,
                &request,
                &completed.response,
                &completed.provenance,
            )?);
        }
        if two && !workflow::reconcile(&outputs[0], &outputs[1]) {
            result.order_disagreement = true;
            result.complete = true;
            return Ok(());
        }
        result.outcome = outputs[0].0;
        result.observations = outputs[0].1.clone();
        if ["two_gemini_jev", "cascade"].contains(&arm) && !result.observations.is_empty() {
            let (answer, receipt) = support(
                context,
                pack,
                arm,
                &result.observations,
                &models.jev_revision,
                deadline,
            )?;
            result.provenance.push(receipt);
            let answer = answer.ok_or(invalid("invalid Jev support response"))?;
            if answer != Support::Supported {
                result.outcome = Outcome::Unverifiable;
            }
        }
        result.complete = true;
        // Necessary-evidence descendants run through the same full path below;
        // they remain attached to this root rather than increasing denominators.
        result.counterfactual_checked = case.counterfactual.is_none();
        Ok(())
    })();
    if let Err(error) = execution {
        result.complete = false;
        result.outcome = Outcome::Unverifiable;
        result.failure = Some(error.to_string());
        if matches!(error, assist::Error::Invalid(_)) {
            result.mechanical_valid = false;
        }
    }
    if result.complete && case.counterfactual.is_some() && !["rules", "oracle_jev"].contains(&arm) {
        let counter = (|| -> assist::Result<Row> {
            let counter = case
                .counterfactual
                .as_ref()
                .ok_or(invalid("counterfactual missing"))?;
            if counter["root_id"].as_str() != Some(case.root_id.as_str())
                || counter["split"].as_str() != Some(case.split.as_str())
            {
                return Err(invalid("counterfactual root/split drift"));
            }
            let mut child = case.clone();
            child.after = counter["path"]
                .as_str()
                .ok_or(invalid("counterfactual path"))?
                .into();
            child.after_hash = assist::decode(
                &serde_json::to_vec(&counter["hash"])
                    .map_err(|_| invalid("counterfactual hash"))?,
            )?;
            child.counterfactual = None;
            child.source = None;
            let child_pack = crate::pack(&child, corpus, context)?;
            Ok(run_case(
                &child,
                arm,
                &child_pack,
                context,
                models,
                oracle,
                corpus,
            ))
        })();
        match counter {
            Ok(child) => {
                result.counterfactual_checked = child.complete;
                result.counterfactual = Some(Box::new(child));
            }
            Err(_) => {
                result.counterfactual_checked = false;
                result.complete = false;
                result.outcome = Outcome::Unverifiable;
                result.failure = Some("counterfactual full path incomplete".into());
            }
        }
    }
    result.elapsed_ms = clock.elapsed().as_millis() as u64;
    result
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut options = BTreeMap::new();
    while let Some(name) = args.next() {
        let value = args.next().ok_or("qualification options need values")?;
        if options.insert(name, value).is_some() {
            return Err("duplicate qualification option".into());
        }
    }
    if options
        .keys()
        .any(|k| !["--manifest", "--out", "--max-spend-usd", "--run"].contains(&k.as_str()))
        || options.get("--run").map(String::as_str) != Some("true")
    {
        return Err("opt-in runner requires --run true and a hard --max-spend-usd cap".into());
    }
    let manifest_path = PathBuf::from(options.get("--manifest").ok_or("manifest required")?);
    let output = PathBuf::from(options.get("--out").ok_or("output required")?);
    let cap = execution::nano_usd(
        options
            .get("--max-spend-usd")
            .ok_or("hard spend cap required")?
            .parse()?,
    )?;
    let manifest_bytes = assist::read_bytes(&manifest_path, 32 * 1024 * 1024)?;
    let manifest: Manifest = assist::decode(&manifest_bytes)?;
    let mut manifest_value: Value = assist::decode(&manifest_bytes)?;
    manifest_value
        .as_object_mut()
        .ok_or("manifest must be an object")?
        .remove("manifest_hash")
        .ok_or("manifest identity required")?;
    if assist::digest(&manifest_value)? != manifest.manifest_hash {
        return Err("frozen manifest identity drift".into());
    }
    if manifest.schema != CONSTRUCTED_SCHEMA
        || manifest.models.gemini != GEMINI
        || manifest.models.jev != JEV
    {
        return Err("constructed epoch/model mismatch".into());
    }
    let corpus =
        saccade_core::paths::canonicalize(manifest_path.parent().ok_or("corpus parent required")?)?;
    let oracle_bytes = assist::read_bytes(&corpus.join("oracle.json"), 32 * 1024 * 1024)?;
    let oracle_document: Value = assist::decode(&oracle_bytes)?;
    // Python verifies the manifest and independent render witnesses immediately
    // before this opt-in runner. Check the private oracle's canonical identity again.
    if assist::digest(&oracle_document)? != manifest.oracle_hash {
        return Err("oracle identity drift".into());
    }
    if oracle_document["schema"] != ORACLE_SCHEMA {
        return Err("oracle schema mismatch".into());
    }
    let oracle = oracle_document["cases"].clone();
    let user = transport::UserConfig::load(
        &saccade_core::judge_provider::Keys::default_dir().join("user.toml"),
    )
    .map_err(|_| "qualification user policy unavailable")?;
    let mut roots = RootPolicy::new(
        &user
            .roots
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>(),
        user.out_root.as_deref(),
        false,
        &[],
    )?;
    user.apply(&mut roots)
        .map_err(|_| "qualification root policy")?;
    roots.read(&manifest_path)?;
    let output = roots.write(&output)?;
    if output.exists() {
        return Err("qualification results must be new; no silent retry/relabel".into());
    }
    let keys = execution::fixed_keys()?;
    let ledger = Ledger::new(
        &saccade_core::judge_provider::Keys::default_dir().join("attempts"),
        true,
    );
    let context = Context {
        user: &user,
        roots: &roots,
        ledger: &ledger,
        keys: &keys,
        epoch: manifest.manifest_hash.clone(),
        cap,
        api: assist::digest(&user)?,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    for case in manifest.cases.iter().filter(|c| c.split == "heldout") {
        // Family/root metadata is kept local and never enters model payloads.
        if case.root_id != case.case_id
            || case.family.is_empty()
            || case.workload.is_empty()
            || case.label.is_empty()
        {
            return Err("constructed root identity".into());
        }
        let pack = pack(case, &corpus, &context)?;
        for arm in [
            "rules",
            "oracle_jev",
            "single_gemini",
            "two_gemini",
            "two_gemini_jev",
            "cascade",
        ] {
            let result = run_case(
                case,
                arm,
                &pack,
                &context,
                &manifest.models,
                &oracle,
                &corpus,
            );
            serde_json::to_writer(&mut file, &result)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
    }
    Ok(())
}
