//! External timing, event trajectories and concise native-mask policies.
use crate::agent::CliError;
#[cfg(feature = "graphics")]
use saccade_core::settling;
use saccade_core::timing;
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct TimingArgs {
    #[command(subcommand)]
    operation: TimingOperation,
}
#[derive(clap::Subcommand)]
enum TimingOperation {
    /// Analyze external paired timings; no commands are executed.
    Ab(TimingAbArgs),
}
#[derive(clap::Args)]
pub(crate) struct TimingAbArgs {
    /// Session manifest or paired CSV.
    pub input: PathBuf,
    /// session or csv; referenced runs support hyperfine, perf and JSON paths.
    #[arg(long, default_value = "session")]
    pub format: String,
    #[arg(long, default_value = "timing-ab")]
    pub out: PathBuf,
    #[arg(long)]
    pub json: bool,
    /// Override practical band for CSV imports (manifest policies otherwise retained).
    #[arg(long)]
    pub band_pct: Option<f64>,
}
pub(crate) fn timing(args: TimingArgs) -> Result<u8, CliError> {
    match args.operation {
        TimingOperation::Ab(a) => timing_ab(a),
    }
}
pub(crate) fn timing_ab(args: TimingAbArgs) -> Result<u8, CliError> {
    let (mut session, sources) = timing::load(&args.input, &args.format)?;
    if let Some(b) = args.band_pct {
        session.band_pct = b;
    }
    let root = args.input.parent().unwrap_or(Path::new("."));
    let report = timing::analyze(session, sources, root)?;
    crate::general_cmd::prepare_out(&args.out, &[&args.input])?;
    let file = args.out.join(format!("{}.json", timing::REPORT_SCHEMA));
    let linked = saccade_core::report_links::write(&file, &report)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&linked)?))?;
    } else {
        crate::emit(&format!(
            "{} within band ±{}%; CI {:?}; A/A {:?}%; diagnostic-only {}; reasons {:?}\nwrote {}\n",
            report.verdict,
            report.band_pct,
            report.interval_pct,
            report.noise_floor_pct,
            report.diagnostic_only,
            report.reasons,
            file.display()
        ))?;
    }
    Ok(if report.verdict == "slower" { 1 } else { 0 })
}
#[derive(clap::Args)]
pub(crate) struct SettleArgs {
    /// Directory of numbered image frames, sorted by numeric suffix.
    pub frames: PathBuf,
    #[arg(long, conflicts_with = "event")]
    pub change_frame: Option<usize>,
    /// Bounded JSON event marker containing change_frame.
    #[arg(long, required_unless_present = "change_frame")]
    pub event: Option<PathBuf>,
    #[arg(long)]
    pub reference: Option<PathBuf>,
    #[arg(long, default_value_t = 30.)]
    pub fps: f64,
    #[arg(long, default_value_t = 32)]
    pub tile_size: u32,
    #[arg(long, default_value_t = 0.02)]
    pub threshold: f64,
    #[arg(long, default_value_t = 3)]
    pub consecutive: usize,
    #[arg(long, default_value_t = 3)]
    pub final_frames: usize,
    #[arg(long, default_value = "settling")]
    pub out: PathBuf,
    #[arg(long)]
    pub json: bool,
}
#[cfg(feature = "graphics")]
pub(crate) fn frames(dir: &Path) -> Result<Vec<image::RgbaImage>, CliError> {
    let paths = settling::frame_paths(dir)?;
    let mut images = Vec::new();
    let mut total = 0_u64;
    for path in &paths {
        let bytes = saccade_core::evidence_quality::read(path, 64 << 20)?;
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| CliError::io(e.to_string()))?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(64 << 20);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|e| CliError::usage(e.to_string()))?
            .to_rgba8();
        total = total.saturating_add(image.as_raw().len() as u64);
        if total > 256 << 20 {
            return Err(CliError::usage(
                "settling sequence exceeds 256 MiB decoded pixels",
            ));
        }
        images.push(image);
    }
    Ok(images)
}
#[cfg(feature = "graphics")]
pub(crate) fn settling(args: SettleArgs) -> Result<u8, CliError> {
    let change = if let Some(i) = args.change_frame {
        i
    } else {
        let event = args
            .event
            .as_ref()
            .ok_or_else(|| CliError::usage("declare change frame or event"))?;
        let v: serde_json::Value =
            serde_json::from_slice(&saccade_core::evidence_quality::read(event, 1 << 20)?)?;
        v["change_frame"]
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
            .ok_or_else(|| CliError::usage("event needs integer change_frame"))?
    };
    let frames = frames(&args.frames)?;
    let refs = args.reference.as_deref().map(self::frames).transpose()?;
    let report = settling::analyze(
        &frames,
        refs.as_deref(),
        settling::Policy {
            change_frame: change,
            fps: args.fps,
            tile_size: args.tile_size,
            threshold: args.threshold,
            consecutive: args.consecutive,
            final_frames: args.final_frames,
        },
    )?;
    let mut inputs = vec![args.frames.as_path()];
    if let Some(r) = &args.reference {
        inputs.push(r);
    }
    if let Some(e) = &args.event {
        inputs.push(e);
    }
    crate::general_cmd::prepare_out(&args.out, &inputs)?;
    let mut value = serde_json::to_value(&report)?;
    // Exact image-byte identities bind the trajectory to source observations.
    let mut sources = std::collections::BTreeMap::new();
    for dir in std::iter::once(&args.frames).chain(args.reference.iter()) {
        for p in &settling::frame_paths(dir)? {
            sources.insert(p.display().to_string(), saccade_core::run::sha256_file(p)?);
        }
    }
    if let Some(e) = &args.event {
        sources.insert(e.display().to_string(), saccade_core::run::sha256_file(e)?);
    }
    value["sources"] = serde_json::json!(sources);
    let linked = saccade_core::report_links::write(
        &args.out.join(format!("{}.json", settling::SCHEMA)),
        &value,
    )?;
    std::fs::write(args.out.join("index.html"), report.html())
        .map_err(|e| CliError::io(e.to_string()))?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&linked)?))?;
    } else {
        crate::emit(&format!(
            "settle {:?}; lag {:?} frames; wrote {}\n",
            report.settle_frame,
            report.lag_frames,
            args.out.display()
        ))?;
    }
    Ok(0)
}
pub(crate) fn predicate(
    spec: &str,
) -> Result<(String, saccade_core::evidence_quality::layers::Predicate), CliError> {
    Ok(saccade_core::evidence_quality::layers::parse_layer_spec(
        spec,
    )?)
}
pub(crate) fn apply_masks(
    cfg: &mut saccade_core::config::RunConfig,
    mask: Option<&str>,
    required: &[String],
) -> Result<(), CliError> {
    use saccade_core::evidence_quality::{
        effect::{RequiredEffect, Selection, Sides},
        layers::{LayerScope, ScopeMode},
    };
    if let Some(spec) = mask {
        let (layer, predicate) = predicate(spec)?;
        cfg.layers.get_or_insert_with(Default::default).scope = Some(LayerScope {
            layer,
            predicate,
            mode: ScopeMode::Union,
        });
        cfg.mask_mode = saccade_core::compare::MaskMode::Neutralize;
    }
    if required.len() > 128 {
        return Err(CliError::usage("at most 128 required effects"));
    }
    for spec in required {
        let parsed = saccade_core::mask_spec::parse_effect_spec(spec)?;
        let selection = match parsed.selection {
            saccade_core::mask_spec::EffectSelection::Mask { image } => Selection::Mask { image },
            saccade_core::mask_spec::EffectSelection::Layer { name, predicate } => {
                Selection::NamedLayer {
                    name,
                    predicate,
                    manifest: cfg.layers.as_ref().and_then(|p| p.manifest.clone()),
                    dump: cfg.layers.as_ref().and_then(|p| p.dump.clone()),
                }
            }
        };
        let p = RequiredEffect {
            name: parsed.name,
            glob: "**".into(),
            selection,
            min_pixels: parsed.min_pixels,
            min_fraction: 0.,
            sides: Sides::Both,
        };
        p.validate()?;
        cfg.required_effect.push(p);
    }
    Ok(())
}
pub(crate) fn export(index: &Path, format: &str, out: Option<&Path>) -> Result<u8, CliError> {
    let rows = saccade_core::report_links::export(index)?;
    let text = match format {
        "json" => serde_json::to_string_pretty(&rows)?,
        "jsonl" => rows
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
        _ => return Err(CliError::usage("index export format must be json or jsonl")),
    };
    if let Some(out) = out {
        crate::schema_cmd::write_new(out, format!("{text}\n").as_bytes())?;
    } else {
        crate::emit(&format!("{text}\n"))?;
    }
    Ok(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use clap::Parser;
    use serde_json::json;
    #[test]
    fn one_flag_policies_expand_and_empty_specimens_fail_occupancy() {
        let mut cfg = saccade_core::config::RunConfig::default();
        apply_masks(
            &mut cfg,
            Some("sample=id=12,13"),
            &["sample=id=12:8".into()],
        )
        .unwrap();
        assert_eq!(cfg.required_effect[0].min_pixels, 8);
        assert_eq!(cfg.mask_mode, saccade_core::compare::MaskMode::Neutralize);
        assert!(predicate("sample=id=").is_err());
        assert!(predicate("sample=above=NaN").is_err());
        assert!(apply_masks(&mut cfg, None, &["sample=mask:0".into()]).is_err());
        for args in [
            vec![
                "saccade",
                "timing",
                "ab",
                "pairs.csv",
                "--format",
                "csv",
                "--json",
            ],
            vec!["saccade", "index", "export", "--format", "json"],
            vec![
                "saccade",
                "compare",
                "a",
                "b",
                "--mask-layer",
                "sample=label=cell*",
                "--mask-from-dump",
                "objects.json",
                "--require-effect",
                "sample=id=12:8",
                "--source-ref",
                "urn:specimen:12",
                "--json",
            ],
        ] {
            assert!(crate::Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
        }
        let _ = json!({"generic":true});
    }
    #[test]
    #[cfg(feature = "graphics")]
    fn generated_dump_labels_and_effect_are_nonvacuous_end_to_end() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        for dir in [&a, &b] {
            std::fs::create_dir(dir).unwrap();
            image::RgbaImage::from_pixel(32, 32, image::Rgba([90, 90, 90, 255]))
                .save(dir.join("frame.png"))
                .unwrap();
            std::fs::write(
                dir.join("objects.json"),
                serde_json::to_vec(&json!([{"id":12,"name":"cell-nucleus","boxes":[[0,0,8,8]]}]))
                    .unwrap(),
            )
            .unwrap();
        }
        let mut cfg = saccade_core::config::RunConfig::default();
        let flags = crate::wave10_cmd::CompareArgs {
            mask_dump: Some("objects.json".into()),
            mask_layer: Some("derived_instances=label=cell*".into()),
            require_effect: vec!["derived_instances=id=12:64".into()],
            ..Default::default()
        };
        flags.apply(&mut cfg).unwrap();
        let report = saccade_core::run::run(&a, &b, &tmp.path().join("out"), &cfg).unwrap();
        assert_eq!(report.totals.pass, 1);
        assert!(
            report.entries[0]
                .required_effects
                .iter()
                .all(|e| e.failures.is_empty())
        );
        cfg.required_effect[0].min_pixels = 65;
        let failed = saccade_core::run::run(&a, &b, &tmp.path().join("fail"), &cfg).unwrap();
        assert!(failed.is_regression());
    }
}
