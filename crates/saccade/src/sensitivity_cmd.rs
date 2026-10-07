//! Gate sensitivity transport, sharing the authoritative compare runner.
use crate::agent::CliError;
use saccade_core::{
    config::RunConfig, general::input, localized::digest, report::Status, sensitivity as sen,
};
use std::path::{Path, PathBuf};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Baseline directory (bounded 8-bit raster images); never modified.
    baseline: PathBuf,
    /// Frozen saccade-sensitivity-catalogue.v1 injection catalogue.
    #[arg(long)]
    catalogue: PathBuf,
    /// Configured compare policy, including overrides and hotspot_fail cluster guard.
    #[arg(long)]
    config: PathBuf,
    /// Optional previous policy; evaluate the identical frozen controls under both policies.
    #[arg(long)]
    before_config: Option<PathBuf>,
    /// Empty output directory outside baseline, catalogue, patches and policy inputs.
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
fn io(e: std::io::Error) -> CliError {
    CliError::io(e.to_string())
}
fn save(image: &image::RgbaImage, path: &Path) -> Result<String, CliError> {
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| CliError::new("config", "image path needs parent"))?,
    )
    .map_err(io)?;
    image
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(|e| CliError::io(e.to_string()))?;
    Ok(input::sha256(path, input::MAX_BYTES)?)
}
fn policy(path: &Path) -> Result<(RunConfig, Vec<u8>), CliError> {
    let bytes = input::bytes(path, 1 << 20)?;
    let mut config = RunConfig::from_toml_str(
        std::str::from_utf8(&bytes).map_err(|_| CliError::new("config", "policy must be UTF-8"))?,
    )?;
    config.config_dir = Some(path.parent().unwrap_or(Path::new(".")).to_path_buf());
    config.validate()?;
    // These require acquisition sidecars or non-raster evidence. Never silently drop them.
    if config.mode != saccade_core::report::Mode::Regression
        || !config.buffers.is_empty()
        || config.meta.required
        || config.meta.require_valid_arms
        || !config.required_effect.is_empty()
        || config.layers.is_some()
        || config.field_ids.is_some()
        || !config.meta.required_keys.is_empty()
        || !config.field.noise_from.is_empty()
        || config.perf.noise.is_some()
    {
        return Err(CliError::new(
            "config",
            "sensitivity requires an image compare gate; sidecar/identity/buffer/effect gates are unsupported",
        ));
    }
    Ok((config, bytes))
}
fn evaluate(
    config: &RunConfig,
    name: &str,
    images: [&Path; 2],
    root: &Path,
    prefix: &str,
    gate: &str,
    changed: u64,
) -> Result<sen::Outcome, CliError> {
    let mut outcome = sen::Outcome {
        gate: gate.into(),
        state: "unavailable".into(),
        reports: vec![],
        reasons: vec![],
    };
    let Some(config) = sen::config_for_entry(config, name)? else {
        outcome.state = "excluded".into();
        return Ok(outcome);
    };
    if changed == 0 {
        outcome.state = "ineffective".into();
        return Ok(outcome);
    }
    for (kind, candidate) in [("control", images[0]), ("candidate", images[1])] {
        let dir = format!("{prefix}/{gate}-{kind}");
        let report = match saccade_core::run::run(images[0], candidate, &root.join(&dir), &config) {
            Ok(report) => report,
            Err(error) => {
                outcome.reasons.push(error.to_string());
                return Ok(outcome);
            }
        };
        outcome
            .reports
            .push(format!("{dir}/saccade-report.v1.json"));
        let Some(entry) = report.entries.first().filter(|_| report.entries.len() == 1) else {
            outcome
                .reasons
                .push("gate produced no single image verdict".into());
            return Ok(outcome);
        };
        if kind == "control" && entry.status != Status::Pass {
            outcome
                .reasons
                .push("unmodified control does not pass the gate".into());
            return Ok(outcome);
        }
        if kind == "candidate" {
            outcome.state = match entry.status {
                Status::Pass => "missed",
                Status::Fail => "detected",
                _ => "unavailable",
            }
            .into();
            if let Some(e) = &entry.error {
                outcome.reasons.push(e.clone());
            }
        }
    }
    Ok(outcome)
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    saccade_core::report_links::require_index_path(&args.out.join("reports/index.jsonl"))?;
    let encoded = input::bytes(&args.catalogue, 1 << 20)?;
    let catalogue: sen::Catalogue = serde_json::from_slice(&encoded)?;
    catalogue.validate()?;
    let mut policies = vec![("configured", policy(&args.config)?)];
    if let Some(path) = &args.before_config {
        policies.push(("before", policy(path)?));
    }
    let files = sen::baseline_files(&args.baseline)?;
    if files.is_empty() {
        return Err(CliError::new(
            "config",
            "sensitivity baseline set must not be empty",
        ));
    }
    for path in &files {
        let name = path
            .strip_prefix(&args.baseline)
            .map_err(|_| CliError::new("config", "baseline name escaped root"))?
            .to_str()
            .ok_or_else(|| CliError::new("config", "baseline names must be UTF-8"))?;
        if name.contains('\\') || name.chars().any(char::is_control) {
            return Err(CliError::new(
                "config",
                "baseline names must not contain backslashes or control characters",
            ));
        }
    }
    let strengths: usize = catalogue
        .injections
        .iter()
        .map(|i| i.magnitudes.len())
        .sum();
    if files.len() * strengths > 2048 {
        return Err(CliError::new(
            "config",
            "sensitivity trial limit exceeded (2048)",
        ));
    }
    let mut protected = vec![
        args.baseline.clone(),
        args.catalogue.clone(),
        args.config.clone(),
    ];
    protected.extend(args.before_config.iter().cloned());
    let mut glyphs = Vec::new();
    let base = args.catalogue.parent().unwrap_or(Path::new("."));
    for injection in &catalogue.injections {
        if let sen::Defect::GlyphEdit {
            before,
            after,
            before_sha256,
            after_sha256,
            ..
        } = &injection.defect
        {
            let mut pair = Vec::new();
            for (p, hash) in [(before, before_sha256), (after, after_sha256)] {
                let path = base.join(p);
                let bytes = input::bytes(&path, input::MAX_BYTES)?;
                if digest(&bytes) != hash.to_ascii_lowercase() {
                    return Err(CliError::new("config", "glyph patch SHA-256 mismatch"));
                }
                protected.push(path);
                pair.push(input::decode(&bytes)?);
            }
            glyphs.push(Some([pair.remove(0), pair.remove(0)]));
        } else {
            glyphs.push(None);
        }
    }
    // Preflight decoding and all geometry before creating output. Bound total processed pixels.
    let mut pixel_work = 0u64;
    for path in &files {
        let source = input::load(path)?;
        pixel_work += u64::from(source.width()) * u64::from(source.height()) * strengths as u64;
        if pixel_work > 256 * 1024 * 1024 {
            return Err(CliError::new(
                "config",
                "sensitivity pixel-work limit exceeded; reduce the baseline set or catalogue",
            ));
        }
        for (i, injection) in catalogue.injections.iter().enumerate() {
            sen::inject(
                &source,
                injection,
                injection.magnitudes[0],
                glyphs[i].as_ref(),
            )?;
        }
    }
    // Reject both descendant and ancestor outputs: no experiment can contain its inputs.
    let norm = saccade_core::run::normalise_path(&args.out);
    for path in &protected {
        let p = saccade_core::run::normalise_path(path);
        if p.starts_with(&norm) || norm.starts_with(&p) {
            return Err(CliError::new(
                "config",
                "sensitivity output overlaps an input",
            ));
        }
    }
    crate::general_cmd::prepare_out(
        &args.out,
        &protected.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
    )?;
    std::fs::write(args.out.join("catalogue.json"), &encoded).map_err(io)?;
    for (index, (_, (_, bytes))) in policies.iter().enumerate() {
        std::fs::write(args.out.join(format!("policy-{index}.toml")), bytes).map_err(io)?;
    }
    let mut trials = Vec::new();
    for (n, path) in files.iter().enumerate() {
        let bytes = input::bytes(path, input::MAX_BYTES)?;
        let source = input::decode(&bytes)?;
        let name = path
            .strip_prefix(&args.baseline)
            .map_err(|_| CliError::new("config", "baseline name escaped root"))?
            .to_str()
            .ok_or_else(|| CliError::new("config", "baseline names must be UTF-8"))?
            .replace('\\', "/");
        let snapshot = args.out.join("sources").join(&name);
        std::fs::create_dir_all(
            snapshot
                .parent()
                .ok_or_else(|| CliError::new("config", "snapshot needs parent"))?,
        )
        .map_err(io)?;
        std::fs::write(&snapshot, &bytes).map_err(io)?;
        for (i, injection) in catalogue.injections.iter().enumerate() {
            for (m, &magnitude) in injection.magnitudes.iter().enumerate() {
                let prefix = format!("trials/{n:03}-{i:02}-{m:02}");
                let paths = [
                    format!("{prefix}/baseline/image.png"),
                    format!("{prefix}/capture/image.png"),
                ];
                let (control, candidate, changed) =
                    sen::inject(&source, injection, magnitude, glyphs[i].as_ref())?;
                let hashes = [
                    save(&control, &args.out.join(&paths[0]))?,
                    save(&candidate, &args.out.join(&paths[1]))?,
                ];
                let mut outcomes = Vec::new();
                for (gate, (config, _)) in &policies {
                    outcomes.push(evaluate(
                        config,
                        &name,
                        [&args.out.join(&paths[0]), &args.out.join(&paths[1])],
                        &args.out,
                        &prefix,
                        gate,
                        changed,
                    )?);
                }
                trials.push(sen::Trial {
                    entry: name.clone(),
                    injection: injection.id.clone(),
                    magnitude,
                    source_sha256: digest(&bytes),
                    images: paths,
                    image_sha256: hashes,
                    changed_pixels: changed,
                    outcomes,
                });
            }
        }
    }
    let (summaries, minima) = sen::summarize(&catalogue, &trials);
    let incomplete = summaries
        .iter()
        .any(|r| r.counts.unavailable > 0 || r.counts.ineffective > 0 || r.counts.excluded > 0);
    let misses = summaries
        .iter()
        .any(|r| r.gate == "configured" && r.counts.missed > 0);
    let report = sen::Report {
        schema: sen::SCHEMA.into(),
        state: if incomplete { "insufficient_evidence" } else { "complete" }.into(),
        limitations: vec![
            "Sensitivity on injected defects is not field recall.".into(),
            "Smallest detected magnitude means at least one eligible detection at a tested value; it is not an all-images guarantee or a monotonic threshold.".into(),
            "Glyph operations add pinned intact/edited patches to control/candidate copies; they do not measure recognition of existing text.".into(),
            "Missing-element regions and background colours are caller-declared; no automatic element inference.".into(),
        ],
        catalogue,
        catalogue_sha256: digest(&encoded),
        config_sha256: policies.iter().map(|(_, (_, bytes))| digest(bytes)).collect(),
        trials,
        summaries,
        minima,
    };
    let value = saccade_core::report_links::decorate(&serde_json::to_value(&report)?)?;
    let path = args.out.join("saccade-sensitivity.v2.json");
    crate::general_cmd::write_new(&path, &value)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
    } else {
        crate::emit(&format!(
            "sensitivity: {}; {} trials; evidence: {}\nSensitivity on injected defects is not field recall.\n",
            report.state,
            report.trials.len(),
            path.display()
        ))?;
        for r in &report.summaries {
            crate::emit(&format!(
                "{} {} magnitude={} detected={} missed={} miss_rate={:?}\n",
                r.gate, r.class, r.magnitude, r.counts.detected, r.counts.missed, r.miss_rate
            ))?;
        }
    }
    Ok(if incomplete { 4 } else { u8::from(misses) })
}
