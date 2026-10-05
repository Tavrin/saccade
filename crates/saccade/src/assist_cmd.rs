//! Feature-gated assist CLI and the shared CLI/MCP workflow implementation.
use crate::{agent::CliError, local_cmd};
use clap::{Args, ValueEnum};
use saccade_core::{
    assist::{
        self,
        catalog::{Catalog, Condition, Image, Region},
        execution::{self, CacheKey, Cached, Executor},
        geometry::Transform,
        mask_audit,
        schema::*,
        workflow::{self, Need, Route},
    },
    budget_ledger::{Ledger, MoneyScope},
    evidence::canonical::Digest,
    judge_provider::transport::{self, Authorization},
    root_policy::RootPolicy,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default, ValueEnum)]
pub(crate) enum Routing {
    Rules,
    #[default]
    Cascade,
    AllVision,
}
impl From<Routing> for Route {
    fn from(r: Routing) -> Self {
        match r {
            Routing::Rules => Self::Rules,
            Routing::Cascade => Self::Cascade,
            Routing::AllVision => Self::AllVision,
        }
    }
}
#[derive(Clone, Copy, Default, ValueEnum)]
pub(crate) enum VisibleKind {
    #[default]
    LabelVisible,
    BannerAbsent,
    NotClipped,
    NonOverlap,
}
#[derive(Args, Clone)]
pub(crate) struct Common {
    /// Required acknowledgement: this feature is unqualified experimental advice.
    #[arg(long)]
    pub experimental: bool,
    /// New empty directory for immutable sidecars and the advice report.
    #[arg(long)]
    pub out: PathBuf,
    /// Replay existing observations; never authorize providers.
    #[arg(long, conflicts_with = "run")]
    pub offline: bool,
    /// Recorded exact cache entries for offline fixture replay.
    #[arg(long, requires = "offline")]
    pub replay: Option<PathBuf>,
    /// Explicitly authorize evidence export under fixed user root policy.
    #[arg(long, conflicts_with = "offline")]
    pub run: bool,
    /// Deterministic rules, routed cascade or the full visual path.
    #[arg(long, value_enum, default_value = "cascade")]
    pub route: Routing,
    /// Real provider request cap; no retries or automatic top-up.
    #[arg(long,default_value_t=4,value_parser=clap::value_parser!(u64).range(1..=4))]
    pub budget_calls: u64,
    /// Finite per-entry USD ceiling, at most 0.15.
    #[arg(long, default_value_t = 0.15)]
    pub max_spend_usd: f64,
    /// Overall deadline, including both orders and support.
    #[arg(long,default_value_t=300,value_parser=clap::value_parser!(u64).range(1..=300))]
    pub deadline_secs: u64,
    /// Required immutable returned revision for dispatch/replay.
    #[arg(long)]
    pub gemini_revision: Option<String>,
    /// Required Jev returned revision; the pinned model ID is the initial binding.
    #[arg(long,default_value=JEV)]
    pub jev_revision: String,
    /// Do not reuse cache; required for independent qualification samples.
    #[arg(long)]
    pub bypass_cache: bool,
    /// Hash/dimension-bound Wave 3 source packets (at most one per image).
    #[arg(long)]
    pub source_evidence: Vec<PathBuf>,
    /// Producer states some requested capture scope was not captured.
    #[arg(long)]
    pub incomplete_capture: bool,
    /// Original pixels were blacked out before capture and are unavailable.
    #[arg(long)]
    pub pre_masked: bool,
}
#[derive(Args)]
pub(crate) struct ReportArgs {
    #[arg(long)]
    pub report: PathBuf,
    /// Select exactly one report entry; required when the report contains several.
    #[arg(long)]
    pub entry: Option<String>,
    /// Optional original individual-mask declarations, bound to exact report bytes.
    #[arg(long)]
    pub mask_manifest: Option<PathBuf>,
    #[command(flatten)]
    pub common: Common,
}
#[derive(Args)]
pub(crate) struct CheckArgs {
    /// Literal visible label, never an acting agent's success claim.
    pub condition: String,
    #[arg(long)]
    pub image: PathBuf,
    /// Original image pixels: X,Y,W,H.
    #[arg(long,value_parser=parse_box)]
    pub r#box: [u32; 4],
    /// Closed screenshot-only condition category.
    #[arg(long, value_enum, default_value = "label-visible")]
    pub kind: VisibleKind,
    /// Stable source node ID for geometric conditions.
    #[arg(long)]
    pub target: Option<String>,
    /// Containing panel or second source node ID.
    #[arg(long)]
    pub second_target: Option<String>,
    #[command(flatten)]
    pub common: Common,
}
pub(crate) fn parse_box(text: &str) -> Result<[u32; 4], String> {
    let values: Vec<u32> = text
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| "box needs four nonnegative integers")?;
    values.try_into().map_err(|_| "box needs X,Y,W,H".into())
}
impl From<assist::Error> for CliError {
    fn from(error: assist::Error) -> Self {
        let code = match error {
            assist::Error::Invalid(_) => "invalid_evidence",
            assist::Error::Policy(_) => "assist_policy_refused",
            assist::Error::Storage => "io",
            assist::Error::Provider => "assist_incomplete",
        };
        Self::new(code, error.to_string())
    }
}
fn check_path(path: &Path, roots: Option<&RootPolicy>) -> Result<PathBuf, CliError> {
    if let Some(roots) = roots {
        Ok(roots.read(path)?)
    } else {
        Ok(path.to_owned())
    }
}
fn read(path: &Path, roots: Option<&RootPolicy>, limit: usize) -> Result<Vec<u8>, CliError> {
    Ok(assist::read_bytes(&check_path(path, roots)?, limit)?)
}
struct Pixels {
    image: Image,
    png: Vec<u8>,
    rgba: Vec<u8>,
}
fn pixels(
    path: &Path,
    role: Role,
    common: &Common,
    roots: Option<&RootPolicy>,
    expected: Option<&str>,
) -> Result<Pixels, CliError> {
    let raw = read(path, roots, 16 * 1024 * 1024)?;
    let hash = Digest::of_bytes(&raw);
    if expected.is_some_and(|h| h != &hash.as_str()[7..]) {
        return Err(CliError::new(
            "stale_action",
            "screenshot bytes differ from report identity",
        ));
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&raw))
        .with_guessed_format()
        .map_err(|_| CliError::new("invalid_evidence", "image format unavailable"))?;
    if ![
        Some(image::ImageFormat::Png),
        Some(image::ImageFormat::Jpeg),
    ]
    .contains(&reader.format())
    {
        return Err(CliError::usage(
            "assist accepts bounded SDR PNG/JPEG captures",
        ));
    }
    let dimensions = image::ImageReader::new(std::io::Cursor::new(&raw))
        .with_guessed_format()
        .map_err(|_| CliError::usage("image format"))?
        .into_dimensions()
        .map_err(|_| CliError::usage("image dimensions"))?;
    if u64::from(dimensions.0) * u64::from(dimensions.1) > 4_194_304
        || dimensions.0 == 0
        || dimensions.1 == 0
    {
        return Err(CliError::usage("assist image pixel limit is 4194304"));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(dimensions.0);
    limits.max_image_height = Some(dimensions.1);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| CliError::new("invalid_evidence", "bounded image decode failed"))?
        .to_rgba8();
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(decoded.clone())
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|_| CliError::io("PNG encoding failed"))?;
    let png = encoded.into_inner();
    let dims = [dimensions.0, dimensions.1];
    Ok(Pixels {
        image: Image {
            role,
            sha256: hash,
            encoded_sha256: Digest::of_bytes(&png),
            dimensions: dims,
            capture_scope: [0, 0, dims[0], dims[1]],
            complete: !common.incomplete_capture,
            original_pixels: !common.pre_masked,
            transform: Transform {
                crop: [0, 0, dims[0], dims[1]],
                encoded: dims,
            },
        },
        png,
        rgba: decoded.into_raw(),
    })
}
fn add_source(
    catalog: &mut Catalog,
    common: &Common,
    roots: Option<&RootPolicy>,
) -> Result<(), CliError> {
    if common.source_evidence.len() > 2 {
        return Err(CliError::usage("at most two source packets"));
    }
    for path in &common.source_evidence {
        let source: saccade_core::ui_review::Source =
            assist::decode(&read(path, roots, 4 * 1024 * 1024)?)?;
        catalog
            .source_evidence_hashes
            .push(assist::digest(&source)?);
        catalog.source_evidence.push(source);
    }
    catalog.validate()?;
    Ok(())
}
fn base_catalog(images: Vec<Image>, regions: Vec<Region>) -> Catalog {
    Catalog {
        version: CATALOG_VERSION.into(),
        images,
        regions,
        exclusions: vec![],
        measurements: json!({}),
        source_evidence: vec![],
        source_evidence_hashes: vec![],
    }
}
/// Common CLI/MCP single-image operation.
pub(crate) fn check(
    args: CheckArgs,
    json_output: bool,
    roots: Option<&RootPolicy>,
    auth: Option<&Authorization>,
) -> Result<(Value, u8), CliError> {
    if args.common.run && roots.is_none() {
        let policy = configured_roots()?;
        return check(args, json_output, Some(&policy), auth);
    }
    let p = pixels(&args.image, Role::Single, &args.common, roots, None)?;
    assist::geometry::Geometry::Box(args.r#box.map(f64::from)).validate(p.image.dimensions)?;
    let mut catalog = base_catalog(
        vec![p.image],
        vec![Region {
            id: "scope".into(),
            image_role: Role::Single,
            rect: args.r#box,
        }],
    );
    // The visible condition is confined to the requested box; complete refers to it.
    catalog.images[0].capture_scope = args.r#box;
    add_source(&mut catalog, &args.common, roots)?;
    let condition = match args.kind {
        VisibleKind::LabelVisible => Condition::LabelVisible {
            label: args.condition,
        },
        VisibleKind::BannerAbsent => Condition::BannerAbsent {
            label: args.condition,
        },
        VisibleKind::NotClipped | VisibleKind::NonOverlap => {
            let first = args
                .target
                .ok_or_else(|| CliError::usage("geometric condition requires --target"))?;
            let second = args
                .second_target
                .ok_or_else(|| CliError::usage("geometric condition requires --second-target"))?;
            for id in [&first, &second] {
                let node = catalog
                    .source_evidence
                    .iter()
                    .flat_map(|s| &s.nodes)
                    .find(|n| &n.id == id)
                    .ok_or_else(|| {
                        CliError::new(
                            "invalid_evidence",
                            "known target absent from bound source packet",
                        )
                    })?;
                let [x, y, w, h] = node.bounds.ok_or_else(|| {
                    CliError::new("invalid_evidence", "known target geometry unavailable")
                })?;
                let rect = [
                    x.floor() as u32,
                    y.floor() as u32,
                    (x + w).ceil() as u32 - x.floor() as u32,
                    (y + h).ceil() as u32 - y.floor() as u32,
                ];
                catalog.regions.push(Region {
                    id: id.clone(),
                    image_role: Role::Single,
                    rect,
                });
            }
            catalog.validate()?;
            if matches!(args.kind, VisibleKind::NotClipped) {
                Condition::NotClipped {
                    target: first,
                    panel: second,
                }
            } else {
                Condition::NonOverlap { first, second }
            }
        }
    };
    let result = execute(
        catalog,
        Task::CheckUi,
        None,
        Some(condition),
        vec![(Role::Single, p.png)],
        None,
        args.common,
        roots,
        auth,
    )?;
    if json_output {
        local_cmd::print(&result.0, true)?;
    }
    Ok(result)
}
/// Common CLI/MCP report operation. Missing per-mask history remains explicit.
pub(crate) fn report(
    args: ReportArgs,
    task: Task,
    json_output: bool,
    roots: Option<&RootPolicy>,
    auth: Option<&Authorization>,
) -> Result<(Value, u8), CliError> {
    if args.common.run && roots.is_none() {
        let policy = configured_roots()?;
        return report(args, task, json_output, Some(&policy), auth);
    }
    let bytes = read(&args.report, roots, 32 * 1024 * 1024)?;
    let report: saccade_core::Report =
        crate::parse_contract(&bytes, saccade_core::report::REPORT_SCHEMA)?;
    let report_hash = Digest::of_bytes(&bytes);
    let entry = if let Some(name) = &args.entry {
        report.entries.iter().find(|e| &e.name == name)
    } else if report.entries.len() == 1 {
        report.entries.first()
    } else {
        None
    }
    .ok_or_else(|| CliError::usage("select one existing --entry"))?;
    if !matches!(
        entry.status,
        saccade_core::Status::Pass | saccade_core::Status::Fail
    ) || entry.capture_validity.status == saccade_core::meta::Validity::Invalid
        || report.capture_validity().status == saccade_core::meta::Validity::Invalid
    {
        return Err(CliError::new(
            "invalid_evidence",
            "invalid or incomplete capture cannot receive assist advice",
        ));
    }
    if entry.hdr.is_some() || entry.buffer.is_some() {
        return Err(CliError::usage(
            "assist needs original SDR screenshots; HDR/buffer display transforms are not qualified",
        ));
    }
    let before_hash = entry
        .baseline_sha256
        .as_deref()
        .ok_or_else(|| CliError::new("invalid_evidence", "missing original before hash"))?;
    let after_hash = entry
        .capture_sha256
        .as_deref()
        .ok_or_else(|| CliError::new("invalid_evidence", "missing original after hash"))?;
    let before_path = saccade_core::paths::resolve(
        entry
            .paths
            .baseline
            .as_deref()
            .ok_or_else(|| CliError::usage("before screenshot unavailable"))?,
        &args.report,
    );
    let after_path = saccade_core::paths::resolve(
        entry
            .paths
            .capture
            .as_deref()
            .ok_or_else(|| CliError::usage("after screenshot unavailable"))?,
        &args.report,
    );
    let before = pixels(
        &before_path,
        Role::Before,
        &args.common,
        roots,
        Some(before_hash),
    )?;
    let after = pixels(
        &after_path,
        Role::After,
        &args.common,
        roots,
        Some(after_hash),
    )?;
    if before.image.dimensions != after.image.dimensions {
        return Err(CliError::new(
            "invalid_evidence",
            "assist pair dimensions differ",
        ));
    }
    let dims = before.image.dimensions;
    let mut regions = vec![];
    for role in [Role::Before, Role::After] {
        regions.push(Region {
            id: format!("{role:?}:frame"),
            image_role: role,
            rect: [0, 0, dims[0], dims[1]],
        });
        for (n, h) in entry.hotspots.iter().take(32).enumerate() {
            regions.push(Region {
                id: format!("{role:?}:hotspot:{n}"),
                image_role: role,
                rect: h.rect_px,
            });
        }
    }
    let mut catalog = base_catalog(vec![before.image, after.image], regions);
    catalog.measurements = json!({"provenance":"deterministic_measurement","entry_status":entry.status,"metric":entry.metric_used,"value":entry.value,"threshold":entry.threshold,"pixel_exclusions":entry.pixel_exclusions});
    let mut audit = None;
    if let Some(path) = args.mask_manifest {
        let manifest: mask_audit::Manifest = assist::decode(&read(&path, roots, 8 * 1024 * 1024)?)?;
        if manifest.schema != "saccade-assist-masks.v1" || manifest.report_hash != report_hash {
            return Err(CliError::new(
                "invalid_evidence",
                "mask manifest report identity mismatch",
            ));
        }
        catalog.exclusions = manifest.masks;
        let measured = mask_audit::audit(&catalog, Some((&before.rgba, &after.rgba)))?;
        let expected = entry.pixel_exclusions.as_ref().ok_or_else(|| {
            CliError::new("invalid_evidence", "report resolved union unavailable")
        })?;
        if measured.union_hash.as_str()[7..] != expected.mask_sha256
            || measured.union_pixels != expected.pixels
        {
            return Err(CliError::new(
                "invalid_evidence",
                "declared individual masks differ from report union",
            ));
        }
        audit = Some(serde_json::to_value(measured)?);
    } else if task == Task::AuditMask {
        audit = Some(
            json!({"availability":"unavailable","reason":"historical report preserves union only; supply original hash-bound individual memberships","union":entry.pixel_exclusions}),
        );
    }
    add_source(&mut catalog, &args.common, roots)?;
    let verdict = report.combined_verdict.clone().unwrap_or_else(|| {
        if report.entries.iter().all(|e| {
            e.status == saccade_core::Status::Pass
                || (e.status == saccade_core::Status::New && !report.config.fail_on_new)
        }) {
            "pass"
        } else {
            "regression"
        }
        .into()
    });
    let result = execute(
        catalog,
        task,
        Some(report_hash),
        None,
        vec![(Role::Before, before.png), (Role::After, after.png)],
        Some((verdict, audit)),
        args.common,
        roots,
        auth,
    )?;
    if json_output {
        local_cmd::print(&result.0, true)?;
    }
    Ok(result)
}
fn execute(
    catalog: Catalog,
    task: Task,
    report_hash: Option<Digest>,
    condition: Option<Condition>,
    pngs: Vec<(Role, Vec<u8>)>,
    report: Option<(String, Option<Value>)>,
    common: Common,
    roots: Option<&RootPolicy>,
    startup_auth: Option<&Authorization>,
) -> Result<(Value, u8), CliError> {
    if !common.experimental {
        return Err(CliError::usage(
            "assist requires --experimental until qualification is recorded",
        ));
    }
    if !common.max_spend_usd.is_finite()
        || common.max_spend_usd <= 0.
        || common.max_spend_usd > 0.15
    {
        return Err(CliError::usage(
            "assist per-entry spend cap must be in (0, 0.15]",
        ));
    }
    if common.deadline_secs == 0
        || common.deadline_secs > 300
        || common.budget_calls == 0
        || common.budget_calls > 4
    {
        return Err(CliError::usage("assist deadline or request cap"));
    }
    let out = if let Some(roots) = roots {
        roots.write(&common.out)?
    } else {
        common.out.clone()
    };
    if out.exists()
        && std::fs::read_dir(&out)
            .map_err(|_| CliError::io("assist output directory unavailable"))?
            .next()
            .is_some()
    {
        return Err(CliError::new(
            "not_empty_out_dir",
            "assist output must be a new empty directory",
        ));
    }
    let identity = catalog.identity(task, report_hash, condition.as_ref())?;
    let verdict = report.as_ref().map(|r| r.0.clone());
    let need = workflow::evidence_need(&catalog, condition.as_ref(), task)?;
    let mut envelope = workflow::empty(
        catalog.clone(),
        identity.clone(),
        verdict,
        "Experimental advice is unqualified and cannot change the deterministic verdict.",
        false,
    );
    let audit_missing = task == Task::AuditMask && catalog.exclusions.is_empty();
    let visual = match need {
        Need::Unavailable(reason) => {
            envelope.limitations.push(reason.into());
            false
        }
        Need::Structured(outcome) if !matches!(common.route, Routing::AllVision) => {
            envelope.outcome = outcome;
            envelope.verification.order_consistency = "source_fact".into();
            envelope.limitations.push("Hash-bound producer geometry only; source assertions do not establish successful behavior.".into());
            false
        }
        _ if matches!(common.route, Routing::Rules) => {
            envelope.limitations.push("Deterministic rules require visual evidence; rules-only does not commit a model answer.".into());
            false
        }
        _ if audit_missing => {
            envelope.limitations.push("Individual mask provenance unavailable; union accounting cannot establish safe individual masks.".into());
            false
        }
        _ => true,
    };
    let config_path = saccade_core::judge_provider::Keys::default_dir().join("user.toml");
    let user = crate::review_cmd::load_user(&config_path)?;
    let api_config_hash = assist::digest(&user)?;
    let deadline = Instant::now() + Duration::from_secs(common.deadline_secs);
    let mut saved = Vec::<Cached>::new();
    let records = if let Some(file) = common.replay.as_deref() {
        assist::decode::<Vec<Cached>>(&read(file, roots, 8 * 1024 * 1024)?)?
    } else {
        vec![]
    };
    let mut request_artifacts = Vec::new();
    if visual {
        if let Some(revision) = common.gemini_revision.as_deref() {
            let mut prepared = vec![workflow::prepare(
                &catalog,
                identity.clone(),
                condition.as_ref(),
                &pngs,
                false,
                revision,
                api_config_hash.clone(),
            )?];
            if catalog.images.len() == 2 {
                prepared.push(workflow::prepare(
                    &catalog,
                    identity.clone(),
                    condition.as_ref(),
                    &pngs,
                    true,
                    revision,
                    api_config_hash.clone(),
                )?);
            }
            let mut answers = Vec::new();
            for request in &prepared {
                request_artifacts.push(json!({"cache_key":request.key,"payload":serde_json::from_slice::<Value>(&request.payload)?}));
                match acquire(
                    &request.key,
                    &request.payload,
                    &common,
                    &user,
                    roots,
                    startup_auth,
                    &records,
                    deadline,
                ) {
                    Ok(Some(record)) => {
                        envelope.provenance.push(record.provenance.clone());
                        match workflow::decode_answer(
                            &catalog,
                            request,
                            &record.response,
                            &record.provenance,
                        ) {
                            Ok(answer) => answers.push(answer),
                            Err(_) => {
                                envelope.verification.schema_valid = false;
                                envelope.verification.identity_valid = false;
                                envelope.verification.geometry_valid = false;
                                envelope.verification.citations_valid = false;
                                envelope.incomplete = true;
                                envelope.limitations.push("Response failed schema, identity, geometry or citation checks.".into());
                            }
                        }
                        saved.push(record);
                    }
                    Ok(None) => {
                        envelope.incomplete = true;
                        envelope.limitations.push(
                            "No authorized dispatch or exact offline observation for this order."
                                .into(),
                        );
                    }
                    Err(_) => {
                        envelope.incomplete = true;
                        envelope.limitations.push("Provider stage incomplete; reservations retained and local verdict unchanged.".into());
                    }
                }
            }
            if answers.len() == prepared.len()
                && (answers.len() == 1 || workflow::reconcile(&answers[0], &answers[1]))
            {
                envelope.outcome = answers[0].0;
                envelope.observations = answers[0].1.clone();
                envelope.verification.order_consistency = if answers.len() == 1 {
                    "single_image"
                } else {
                    "consistent"
                }
                .into();
                envelope.verification.depends_on_model_observation =
                    !envelope.observations.is_empty();
                if !envelope.observations.is_empty() {
                    let payload =
                        workflow::support_payload(&catalog, &identity, &envelope.observations)?;
                    let key = CacheKey {
                        evidence_hash: identity.request_hash.clone(),
                        payload_hash: Digest::of_bytes(&payload),
                        prompt_hash: Digest::of_bytes(execution::DATA_RULE.as_bytes()),
                        encoder_version: execution::ENCODER.into(),
                        provider: "jev".into(),
                        model: JEV.into(),
                        revision: common.jev_revision.clone(),
                        settings: json!({"choice":"assist.claim_support.v1"}),
                        api_config_hash,
                        order: "support".into(),
                    };
                    request_artifacts.push(json!({"cache_key":key,"payload":serde_json::from_slice::<Value>(&payload)?}));
                    match acquire(
                        &key,
                        &payload,
                        &common,
                        &user,
                        roots,
                        startup_auth,
                        &records,
                        deadline,
                    ) {
                        Ok(Some(record)) => {
                            envelope.provenance.push(record.provenance.clone());
                            envelope.verification.support =
                                workflow::support_answer(&record.response, &key.revision)
                                    .unwrap_or(Support::Insufficient);
                            saved.push(record);
                        }
                        _ => {
                            envelope.incomplete = true;
                            envelope
                                .limitations
                                .push("Dependent textual support is incomplete.".into());
                        }
                    }
                    if envelope.verification.support != Support::Supported {
                        envelope.outcome = Outcome::Unverifiable;
                        envelope.limitations.push("Unsupported or insufficient statements are withheld from committed advice.".into());
                    }
                }
            } else if answers.len() == prepared.len() {
                envelope
                    .limitations
                    .push("Contradictory blind orders: unresolved; observations withheld.".into());
            }
        } else {
            envelope.incomplete = true;
            envelope.limitations.push(
                "Pinned observed Gemini revision required to prepare/replay provider requests."
                    .into(),
            );
        }
    }
    if envelope.incomplete {
        envelope.outcome = Outcome::Unverifiable;
    }
    std::fs::create_dir_all(&out).map_err(|_| CliError::io("cannot create assist output"))?;
    assist::write(&out.join("saccade-assist.v1.json"), &envelope)?;
    assist::write(&out.join("requests.json"), &request_artifacts)?;
    assist::write(&out.join("observations.json"), &saved)?;
    if let Some((_, Some(audit))) = report {
        assist::write(&out.join("mask-audit.json"), &audit)?;
    }
    std::fs::write(out.join("index.html"), workflow::html(&envelope)?)
        .map_err(|_| CliError::io("cannot write assist report"))?;
    let mut value = local_cmd::base_result("review.assist");
    value["execution"] = json!(if envelope.incomplete {
        "incomplete"
    } else {
        "complete"
    });
    value["artifact"] = local_cmd::reference(&out.join("saccade-assist.v1.json"))?;
    value["data"] = json!({"task":task,"outcome":envelope.outcome,"experimental":true,"deterministic_verdict":envelope.deterministic_verdict,"depends_on_model_observation":envelope.verification.depends_on_model_observation,"order_consistency":envelope.verification.order_consistency,"support":envelope.verification.support});
    value["limits"] = json!(envelope.limitations);
    value["counts"] = json!({"observations":envelope.observations.len(),"provider_stages":envelope.provenance.len()});
    Ok((
        local_cmd::bounded(value, 4096)?,
        if envelope.incomplete { 4 } else { 0 },
    ))
}
fn acquire(
    key: &CacheKey,
    payload: &[u8],
    common: &Common,
    user: &transport::UserConfig,
    roots: Option<&RootPolicy>,
    startup_auth: Option<&Authorization>,
    records: &[Cached],
    deadline: Instant,
) -> Result<Option<Cached>, CliError> {
    let cache_dir = saccade_core::judge_provider::Keys::default_dir().join("assist-cache");
    if common.offline {
        if !records.is_empty() {
            let matches: Vec<_> = records.iter().filter(|r| r.key == *key).collect();
            if matches.len() != 1 {
                return Err(CliError::new(
                    "invalid_evidence",
                    "offline observation key missing or duplicated",
                ));
            }
            let mut record = matches[0].clone();
            if record.provenance.request_hash != key.payload_hash
                || record.provenance.response_hash != Digest::of_bytes(&record.response)
                || record.provenance.returned_revision != key.revision
                || record.provenance.requested_model != key.model
                || record.provenance.provider != key.provider
                || record.provenance.returned_model != key.model
                || record.provenance.prompt_hash != key.prompt_hash
                || record.provenance.sampling_settings != key.settings
                || record.provenance.order != key.order
                || record.provenance.encoder_version != key.encoder_version
            {
                return Err(CliError::new(
                    "invalid_evidence",
                    "offline observation provenance mismatch",
                ));
            }
            record.provenance.cache_status = "replay".into();
            return Ok(Some(record));
        }
        if common.bypass_cache {
            return Ok(None);
        }
        return Ok(execution::load_cache(
            &cache_dir,
            key,
            saccade_core::budget_ledger::now_ms(),
            7 * 24 * 3600 * 1000,
        )?);
    }
    if !common.run {
        return Ok(None);
    }
    let mut policy = if let Some(roots) = roots {
        roots.clone()
    } else {
        RootPolicy::new(
            &user
                .roots
                .iter()
                .map(|r| r.path.clone())
                .collect::<Vec<_>>(),
            user.out_root.as_deref(),
            false,
            &[],
        )?
    };
    user.apply(&mut policy)
        .map_err(|_| CliError::new("egress_denied", "invalid assist user root policy"))?;
    let sources = policy
        .roots
        .iter()
        .map(|r| saccade_core::paths::portable(&r.path))
        .collect::<Vec<_>>();
    let auth = if let Some(auth) = startup_auth {
        if !auth.enabled {
            return Err(CliError::new(
                "network_authorization_required",
                "MCP startup does not authorize providers",
            ));
        }
        let mut restricted = auth.clone();
        for scope in &mut restricted.scopes {
            scope.caps.total = scope.caps.total.min(common.budget_calls);
        }
        restricted
    } else {
        crate::review_cmd::authorization(
            true,
            common.budget_calls,
            &key.evidence_hash.as_str()[7..],
            user,
        )
    };
    if !common.bypass_cache {
        if let Some(record) = execution::load_cache(
            &cache_dir,
            key,
            saccade_core::budget_ledger::now_ms(),
            7 * 24 * 3600 * 1000,
        )? {
            return Ok(Some(record));
        }
    }
    let keys = execution::fixed_keys()?;
    let ledger = Ledger::new(
        &saccade_core::judge_provider::Keys::default_dir().join("attempts"),
        false,
    );
    let network = transport::Network;
    let transport = transport::Transport {
        user,
        roots: &policy,
        authorization: &auth,
        ledger: &ledger,
        keys: &keys,
        http: &network,
    };
    let scopes = vec![
        MoneyScope {
            id: format!("assist/entry/{}", key.evidence_hash.as_str()),
            cap_nano_usd: execution::nano_usd(common.max_spend_usd)?,
        },
        MoneyScope {
            id: format!("assist/pr/{}", key.evidence_hash.as_str()),
            cap_nano_usd: 300_000_000,
        },
        MoneyScope {
            id: format!(
                "assist/repository/day/{}",
                saccade_core::budget_ledger::now_ms() / 86_400_000
            ),
            cap_nano_usd: 5_000_000_000,
        },
    ];
    let executor = Executor {
        transport: &transport,
        ledger: &ledger,
        money_scopes: scopes,
        sources,
        deadline,
    };
    let completed = executor.call(key, payload)?;
    let record = Cached {
        key: key.clone(),
        response: completed.response,
        provenance: completed.provenance,
    };
    assist::write(
        &cache_dir.join(format!("{}.json", &assist::digest(key)?.as_str()[7..])),
        &record,
    )?;
    Ok(Some(record))
}
fn configured_roots() -> Result<RootPolicy, CliError> {
    let user = crate::review_cmd::load_user(
        &saccade_core::judge_provider::Keys::default_dir().join("user.toml"),
    )?;
    if user.roots.is_empty() {
        return Err(CliError::new(
            "egress_denied",
            "assist dispatch requires configured source roots and export permission",
        ));
    }
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
        .map_err(|_| CliError::new("egress_denied", "invalid assist root policy"))?;
    Ok(roots)
}
pub(crate) fn run_report(args: ReportArgs, task: Task, json_output: bool) -> Result<u8, CliError> {
    let (value, exit) = report(args, task, json_output, None, None)?;
    if !json_output {
        local_cmd::print(&value, false)?;
    }
    Ok(exit)
}
pub(crate) fn run_check(args: CheckArgs, json_output: bool) -> Result<u8, CliError> {
    let (value, exit) = check(args, json_output, None, None)?;
    if !json_output {
        local_cmd::print(&value, false)?;
    }
    Ok(exit)
}
