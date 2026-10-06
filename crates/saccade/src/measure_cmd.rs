//! Mask metrics, box annotation interchange and frame-map checks.
use crate::agent::CliError;
use saccade_core::{boxes, frame_map, mask_metrics};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const READ_LIMIT: u64 = 64 << 20;

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, CliError> {
    Ok(serde_json::from_slice(
        &saccade_core::evidence_quality::read(path, READ_LIMIT)?,
    )?)
}

fn write_json(out: &Path, name: &str, value: &impl serde::Serialize) -> Result<PathBuf, CliError> {
    let file = out.join(name);
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    std::fs::write(&file, text).map_err(|e| CliError::io(e.to_string()))?;
    Ok(file)
}

fn parse_numbers<const N: usize>(text: &str, what: &str) -> Result<[u32; N], CliError> {
    let parts: Vec<u32> = text
        .split(',')
        .map(|s| s.trim().parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| CliError::usage(format!("{what} takes {N} comma-separated whole numbers")))?;
    parts
        .try_into()
        .map_err(|_| CliError::usage(format!("{what} takes {N} comma-separated whole numbers")))
}

#[derive(clap::Args)]
pub(crate) struct MaskMetricsArgs {
    /// Predicted label image (single-channel native labels, packed RGB or integer EXR).
    pub predicted: PathBuf,
    /// Reference label image of the same size.
    pub reference: PathBuf,
    /// Class as NAME=PREDICATE (id=, range=, above=, mask); repeatable. Default: one `foreground` class, label not zero.
    #[arg(long = "class", conflicts_with = "each_label")]
    pub classes: Vec<String>,
    /// Score every distinct non-void label as its own class (at most 256).
    #[arg(long)]
    pub each_label: bool,
    /// Reference labels to exclude everywhere, as NAME=PREDICATE.
    #[arg(long)]
    pub void: Option<String>,
    /// Boundary match tolerance in pixels (0-64, Euclidean, inclusive).
    #[arg(long, default_value_t = 2.0)]
    pub boundary_px: f64,
    /// Write saccade-mask-metrics.v1.json into this new or empty directory.
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

pub(crate) fn mask_metrics(args: MaskMetricsArgs) -> Result<u8, CliError> {
    let classes = if args.each_label {
        mask_metrics::ClassSelection::EachLabel
    } else if args.classes.is_empty() {
        mask_metrics::ClassSelection::Foreground
    } else {
        mask_metrics::ClassSelection::Named(
            args.classes
                .iter()
                .map(|s| mask_metrics::ClassSpec::parse(s))
                .collect::<Result<_, _>>()?,
        )
    };
    let policy = mask_metrics::Policy {
        classes,
        void: args
            .void
            .as_deref()
            .map(mask_metrics::ClassSpec::parse)
            .transpose()?,
        boundary_tolerance_px: args.boundary_px,
    };
    let report = mask_metrics::evaluate(
        &mask_metrics::read_labels(&args.predicted)?,
        &mask_metrics::read_labels(&args.reference)?,
        &policy,
    )?;
    if let Some(out) = &args.out {
        crate::general_cmd::prepare_out(out, &[&args.predicted, &args.reference])?;
        write_json(out, &format!("{}.json", mask_metrics::SCHEMA), &report)?;
    }
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&report)?))?;
    } else {
        let f = |v: Option<f64>| v.map_or("n/a".into(), |v| format!("{v:.4}"));
        let mut text = format!(
            "{} over {} pixels ({} void); macro IoU {}, Dice {}, boundary F {}\n",
            report.summary.state,
            report.evaluated_pixels,
            report.void_pixels,
            f(report.summary.macro_iou),
            f(report.summary.macro_dice),
            f(report.summary.macro_boundary_f)
        );
        for c in &report.classes {
            text.push_str(&format!(
                "  {}: {} IoU {} Dice {}\n",
                c.name,
                c.status,
                f(c.iou),
                f(c.dice)
            ));
        }
        if !report.summary.missed_classes.is_empty() {
            text.push_str(&format!("missed: {:?}\n", report.summary.missed_classes));
        }
        crate::emit(&text)?;
    }
    Ok(0)
}

#[derive(clap::Args)]
pub(crate) struct BoxesArgs {
    #[command(subcommand)]
    operation: BoxesOperation,
}

#[derive(clap::Subcommand)]
enum BoxesOperation {
    /// Export a saccade-boxes.v1 document as COCO JSON or YOLO text.
    Export {
        /// saccade-boxes.v1 document.
        doc: PathBuf,
        #[arg(long, value_parser = ["coco", "yolo"])]
        format: String,
        /// New or empty output directory.
        #[arg(long)]
        out: PathBuf,
        /// Clip boxes that leave the image (counted) instead of refusing them.
        #[arg(long)]
        clip: bool,
        #[arg(long)]
        json: bool,
    },
    /// Import COCO JSON or YOLO text into a saccade-boxes.v1 document.
    Import {
        input: PathBuf,
        #[arg(long, value_parser = ["coco", "yolo"])]
        format: String,
        #[arg(long)]
        out: PathBuf,
        /// YOLO only: image width in pixels.
        #[arg(long)]
        width: Option<u32>,
        /// YOLO only: image height in pixels.
        #[arg(long)]
        height: Option<u32>,
        /// YOLO only: image file name to record.
        #[arg(long)]
        image_file: Option<String>,
        /// YOLO only: classes.txt, one name per line.
        #[arg(long)]
        classes: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Re-express boxes for a cropped or resized copy of the image.
    Transform {
        doc: PathBuf,
        /// Crop window X,Y,W,H in source pixels.
        #[arg(long, conflicts_with = "resize", required_unless_present = "resize")]
        crop: Option<String>,
        /// Resized image size W,H (each axis scaled independently).
        #[arg(long)]
        resize: Option<String>,
        /// File name of the derived image to record.
        #[arg(long)]
        image_file: Option<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

fn receipt(
    out: &Path,
    operation: &str,
    format: &str,
    count: usize,
    adj: &boxes::Adjustments,
    files: &[PathBuf],
    json: bool,
) -> Result<u8, CliError> {
    let mut listed = Vec::new();
    for f in files {
        listed.push(json!({
            "path": f.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
            "sha256": saccade_core::run::sha256_file(f)?,
        }));
    }
    let value = json!({
        "schema": boxes::RESULT_SCHEMA, "operation": operation, "format": format,
        "boxes": count, "adjustments": adj, "files": listed,
    });
    write_json(out, &format!("{}.json", boxes::RESULT_SCHEMA), &value)?;
    if json {
        crate::emit(&format!("{value}\n"))?;
    } else {
        crate::emit(&format!(
            "{operation} {format}: {count} boxes ({} clipped, {} dropped); wrote {}\n",
            adj.clipped,
            adj.dropped,
            out.display()
        ))?;
    }
    Ok(0)
}

pub(crate) fn boxes(args: BoxesArgs) -> Result<u8, CliError> {
    match args.operation {
        BoxesOperation::Export {
            doc,
            format,
            out,
            clip,
            json,
        } => {
            let (fitted, adj) = read_json::<boxes::BoxDoc>(&doc)?.fit(clip)?;
            crate::general_cmd::prepare_out(&out, &[&doc])?;
            let mut files = Vec::new();
            if format == "coco" {
                files.push(write_json(
                    &out,
                    "annotations.coco.json",
                    &boxes::to_coco(&fitted)?,
                )?);
            } else {
                let (labels, classes) = boxes::to_yolo(&fitted)?;
                let stem = Path::new(&fitted.image.file)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| CliError::usage("image file name has no stem"))?;
                let labels_path = out.join(format!("{stem}.txt"));
                let classes_path = out.join("classes.txt");
                std::fs::write(&labels_path, labels).map_err(|e| CliError::io(e.to_string()))?;
                std::fs::write(&classes_path, classes).map_err(|e| CliError::io(e.to_string()))?;
                files.extend([labels_path, classes_path]);
            }
            receipt(
                &out,
                "export",
                &format,
                fitted.boxes.len(),
                &adj,
                &files,
                json,
            )
        }
        BoxesOperation::Import {
            input,
            format,
            out,
            width,
            height,
            image_file,
            classes,
            json,
        } => {
            let doc = if format == "coco" {
                if width.is_some() || height.is_some() || classes.is_some() || image_file.is_some()
                {
                    return Err(CliError::usage(
                        "COCO carries its own image and classes; drop --width/--height/--image-file/--classes",
                    ));
                }
                boxes::from_coco(&read_json::<Value>(&input)?)?
            } else {
                let (Some(w), Some(h), Some(file), Some(cls)) =
                    (width, height, image_file, classes)
                else {
                    return Err(CliError::usage(
                        "YOLO import needs --width, --height, --image-file and --classes: the format does not carry them",
                    ));
                };
                let names: Vec<String> =
                    String::from_utf8_lossy(&saccade_core::evidence_quality::read(&cls, 1 << 20)?)
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .map(str::to_string)
                        .collect();
                let text = String::from_utf8_lossy(&saccade_core::evidence_quality::read(
                    &input, READ_LIMIT,
                )?)
                .into_owned();
                boxes::from_yolo(
                    &text,
                    names,
                    boxes::ImageRef {
                        file,
                        width: w,
                        height: h,
                        sha256: None,
                        derived_from_sha256: None,
                        derivation: None,
                    },
                )?
            };
            let inputs = [input.as_path()];
            crate::general_cmd::prepare_out(&out, &inputs)?;
            let file = write_json(&out, &format!("{}.json", boxes::SCHEMA), &doc)?;
            receipt(
                &out,
                "import",
                &format,
                doc.boxes.len(),
                &boxes::Adjustments::default(),
                &[file],
                json,
            )
        }
        BoxesOperation::Transform {
            doc,
            crop,
            resize,
            image_file,
            out,
            json,
        } => {
            let source = read_json::<boxes::BoxDoc>(&doc)?;
            let (result, adj) = if let Some(c) = crop {
                let [x, y, w, h] = parse_numbers::<4>(&c, "--crop")?;
                boxes::crop(&source, x, y, w, h, image_file.as_deref())?
            } else {
                let [w, h] = parse_numbers::<2>(resize.as_deref().unwrap_or_default(), "--resize")?;
                (
                    boxes::resize(&source, w, h, image_file.as_deref())?,
                    boxes::Adjustments::default(),
                )
            };
            crate::general_cmd::prepare_out(&out, &[&doc])?;
            let file = write_json(&out, &format!("{}.json", boxes::SCHEMA), &result)?;
            receipt(
                &out,
                "transform",
                "boxes",
                result.boxes.len(),
                &adj,
                &[file],
                json,
            )
        }
    }
}

#[derive(clap::Args)]
pub(crate) struct FrameMapArgs {
    #[command(subcommand)]
    operation: FrameMapOperation,
}

#[derive(clap::Subcommand)]
enum FrameMapOperation {
    /// Report gaps, constant or variable rate, file integrity and settling in the map's own timestamps.
    Check {
        /// saccade-frame-map.v1 document.
        map: PathBuf,
        /// Directory the map's relative paths resolve against (default: the map's directory).
        #[arg(long)]
        root: Option<PathBuf>,
        /// Do not look at frame files; check the index and timestamps only.
        #[arg(long, conflicts_with = "root")]
        skip_files: bool,
        /// Relative spread of the per-frame step, in percent, still called constant.
        #[arg(long, default_value_t = 1.0)]
        rate_tolerance_pct: f64,
        /// saccade-settling.v1 report for the same frames, to restate settling in map time.
        #[arg(long)]
        settling: Option<PathBuf>,
        /// Write saccade-frame-map-check.v1.json into this new or empty directory.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn frame_map(args: FrameMapArgs) -> Result<u8, CliError> {
    let FrameMapOperation::Check {
        map,
        root,
        skip_files,
        rate_tolerance_pct,
        settling,
        out,
        json,
    } = args.operation;
    let parsed: frame_map::FrameMap = read_json(&map)?;
    let default_root = map.parent().map(|p| {
        if p.as_os_str().is_empty() {
            Path::new(".")
        } else {
            p
        }
    });
    let root = if skip_files {
        None
    } else {
        root.as_deref().or(default_root)
    };
    let report = settling.as_deref().map(read_json::<Value>).transpose()?;
    let check = frame_map::check(
        &parsed,
        root,
        frame_map::CheckPolicy { rate_tolerance_pct },
        report.as_ref(),
    )?;
    if let Some(out) = &out {
        crate::general_cmd::prepare_out(out, &[&map])?;
        write_json(out, &format!("{}.json", frame_map::CHECK_SCHEMA), &check)?;
    }
    if json {
        crate::emit(&format!("{}\n", serde_json::to_string(&check)?))?;
    } else {
        let mut text = format!(
            "{}: {} frames, {} missing; index-aligned {}, uniform-time {}\n",
            check.state,
            check.frames,
            check.missing_total,
            check.usable_for.index_aligned,
            check.usable_for.uniform_time
        );
        for r in &check.reasons {
            text.push_str(&format!("  - {r}\n"));
        }
        crate::emit(&text)?;
    }
    Ok(u8::from(!matches!(
        check.state.as_str(),
        "constant_frame_rate" | "single_frame"
    )))
}
