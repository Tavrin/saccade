//! `saccade export-regions`: crop the worst regions of a report, with coordinates.
use crate::agent::CliError;
use crate::local_cmd::{base_result, reference};
use image::GenericImageView;
use saccade_core::report::{Entry, Hotspot, Status};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

/// Schema of the export document.
const SCHEMA: &str = "saccade-region-export.v1";
const FILE: &str = "saccade-region-export.v1.json";

#[derive(clap::Args)]
pub(crate) struct Args {
    /// A saccade report JSON (saccade-report.v1 or its linked successor).
    report: PathBuf,
    /// Output directory for the crops and the coordinates document.
    #[arg(long, default_value = "regions-export")]
    out: PathBuf,
    /// How many regions to export, worst first (1 to 200).
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=200))]
    top: u32,
    /// Context pixels added around each hotspot box.
    #[arg(long, default_value_t = 8)]
    padding: u32,
    /// Print a JSON result.
    #[arg(long)]
    json: bool,
}

/// Worst-first rank of an entry: failing and local-change entries share the first tier
/// (so a tiny severe defect that still passes the deciding metric is not buried under
/// broad failures), ahead of errors, missing and new entries.
/// The HTML report orders its rows with the same key (`report.js`, `entryRank`).
fn entry_rank(entry: &Entry) -> f64 {
    if entry.pass_with_local_change && entry.status == Status::Pass {
        return 0.0;
    }
    match entry.status {
        Status::Fail => 0.0,
        Status::Error => 1.0,
        Status::Missing => 2.0,
        Status::New => 3.0,
        Status::Pass => 4.0,
    }
}

/// Largest single-pixel error an entry shows: the strongest hotspot, else the
/// frame-wide maximum.
fn peak(entry: &Entry) -> f64 {
    entry
        .hotspots
        .iter()
        .map(|h| h.max_flip)
        .chain(entry.metrics.as_ref().map(|m| m.max))
        .filter(|v| v.is_finite())
        .fold(0.0, f64::max)
}

fn relative(path: &str) -> Result<&Path, CliError> {
    let p = Path::new(path);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(CliError::new(
            "unsafe_path",
            "report image path escapes the report directory",
        ));
    }
    Ok(p)
}

fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect()
}

fn crop_rect(h: &Hotspot, pad: u32, width: u32, height: u32) -> [u32; 4] {
    let [x, y, w, hh] = h.rect_px;
    let x0 = x.saturating_sub(pad).min(width);
    let y0 = y.saturating_sub(pad).min(height);
    let x1 = x.saturating_add(w).saturating_add(pad).min(width);
    let y1 = y.saturating_add(hh).saturating_add(pad).min(height);
    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
}

pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let bytes = std::fs::read(&args.report)
        .map_err(|e| CliError::io(format!("reading report {}: {e}", args.report.display())))?;
    let report: saccade_core::Report =
        crate::parse_contract(&bytes, saccade_core::report::REPORT_SCHEMA)?;
    let linked: Value = serde_json::from_slice(&bytes)?;
    let dir = args
        .report
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    saccade_core::run::guard_output_dir(&args.out, &[dir], &[FILE])?;

    let mut entries: Vec<&Entry> = report.entries.iter().collect();
    entries.sort_by(|a, b| {
        entry_rank(a)
            .total_cmp(&entry_rank(b))
            .then(peak(b).total_cmp(&peak(a)))
            .then(b.value.unwrap_or(-1.0).total_cmp(&a.value.unwrap_or(-1.0)))
            .then(a.name.cmp(&b.name))
    });
    let mut chosen: Vec<(&Entry, &Hotspot)> = Vec::new();
    for entry in entries {
        let mut spots: Vec<&Hotspot> = entry.hotspots.iter().collect();
        spots.sort_by(|a, b| {
            b.max_flip
                .total_cmp(&a.max_flip)
                .then(b.share_of_total_error.total_cmp(&a.share_of_total_error))
        });
        chosen.extend(spots.into_iter().map(|h| (entry, h)));
        if chosen.len() >= args.top as usize {
            break;
        }
    }
    chosen.truncate(args.top as usize);

    if args.out.join(FILE).is_file() {
        let _ = std::fs::remove_dir_all(args.out.join("regions"));
    }
    std::fs::create_dir_all(args.out.join("regions"))
        .map_err(|e| CliError::io(format!("creating {}: {e}", args.out.display())))?;
    let mut regions = Vec::new();
    for (index, (entry, hotspot)) in chosen.iter().enumerate() {
        let rank = index + 1;
        let mut files = serde_json::Map::new();
        let mut crop = [0; 4];
        for (role, path) in [
            ("baseline", &entry.paths.baseline),
            ("capture", &entry.paths.capture),
            ("heatmap", &entry.paths.heatmap),
        ] {
            let Some(path) = path else { continue };
            let source = dir.join(relative(path)?);
            let image = image::open(&source)
                .map_err(|e| CliError::io(format!("reading {}: {e}", source.display())))?;
            let (width, height) = image.dimensions();
            let rect = crop_rect(hotspot, args.padding, width, height);
            if role == "capture" || crop == [0; 4] {
                crop = rect;
            }
            if rect[2] == 0 || rect[3] == 0 {
                continue;
            }
            let name = format!("regions/{rank:02}-{}-{role}.png", safe_name(&entry.name));
            image
                .crop_imm(rect[0], rect[1], rect[2], rect[3])
                .save(args.out.join(&name))
                .map_err(|e| CliError::io(format!("writing {name}: {e}")))?;
            files.insert(role.into(), json!(name));
        }
        regions.push(json!({
            "rank": rank,
            "entry": entry.name,
            "rect_px": hotspot.rect_px,
            "crop_rect_px": crop,
            "rect_frac": hotspot.rect_frac,
            "max_flip": hotspot.max_flip,
            "mean_flip": hotspot.mean_flip,
            "area_px": hotspot.area_px,
            "share_of_total_error": hotspot.share_of_total_error,
            "files": files,
        }));
    }
    let document = json!({
        "schema": SCHEMA,
        "source": {
            "report": crate::escape_control(&saccade_core::paths::record(
                &saccade_core::run::normalise_path(&args.report),
                &saccade_core::run::normalise_path(&args.out),
                false,
            )),
            "sha256": format!("{:x}", Sha256::digest(&bytes)),
            "report_id": linked.get("report_id").cloned().unwrap_or(Value::Null),
        },
        "order": "failing and local-change entries first, then by strongest single-pixel error; regions by max_flip",
        "padding_px": args.padding,
        "regions": regions,
        "note": "Exporting evidence records no decision and never writes a baseline.",
    });
    saccade_core::manifest::write_owned(&args.out.join(FILE), SCHEMA, &document)?;
    let mut result = base_result("export-regions");
    result["artifact"] = reference(&args.out.join(FILE))?;
    result["counts"] = json!({"regions": regions.len()});
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&result)?))?;
    } else {
        crate::emit(&format!(
            "wrote {} region crop set(s) to {}\n",
            regions.len(),
            crate::escape_control(&args.out.display().to_string())
        ))?;
    }
    Ok(0)
}
