//! Optional ColorVideoVDP evaluation of numbered SDR frame directories.
use std::path::{Path, PathBuf};

use clap::Args;
use colorvideovdp::{Color, Cvvdp, DisplayModel, Image, Options};
use serde_json::{Value, json};

use crate::agent::CliError;

const FILE: &str = "saccade-temporal.v1.json";
const MAP_HOTSPOT: f32 = 0.2; // raw CVVDP map: per-pixel JOD below 8

#[derive(Args)]
pub(crate) struct TemporalArgs {
    /// Directory of numbered baseline PNG/JPEG frames.
    baseline_dir: PathBuf,
    /// Directory of numbered capture PNG/JPEG frames.
    capture_dir: PathBuf,
    /// Frame rate used by the temporal visibility model.
    #[arg(long)]
    fps: f32,
    /// Embedded ColorVideoVDP display model.
    #[arg(long, default_value = "standard_4k")]
    display: String,
    /// Relative-name glob for numbered frames.
    #[arg(long, default_value = "*")]
    pattern: String,
    /// Output directory for the sequence and temporal reports.
    #[arg(long, default_value = "temporal-report")]
    out: PathBuf,
    /// Optional minimum acceptable video quality in JOD units.
    #[arg(long)]
    min_jod: Option<f32>,
    /// Print a bounded JSON summary.
    #[arg(long)]
    json: bool,
}

fn cv_error(e: impl std::fmt::Display) -> CliError {
    CliError::new("config", format!("ColorVideoVDP: {e}"))
}

fn load_frame(path: &Path) -> Result<Image, CliError> {
    if !path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        s.eq_ignore_ascii_case("png")
            || s.eq_ignore_ascii_case("jpg")
            || s.eq_ignore_ascii_case("jpeg")
    }) {
        return Err(CliError::usage(format!(
            "temporal input must be PNG or JPEG (sRGB): {}",
            path.display()
        )));
    }
    let rgb = image::open(path)
        .map_err(|e| CliError::io(format!("{}: {e}", path.display())))?
        .to_rgb8();
    Image::new(
        rgb.width() as usize,
        rgb.height() as usize,
        3,
        rgb.into_raw()
            .into_iter()
            .map(|v| f32::from(v) / 255.0)
            .collect(),
    )
    .map_err(cv_error)
}

fn mean_signed(test: &Image, reference: &Image) -> f64 {
    let sum: f64 = test
        .data()
        .as_chunks::<3>()
        .0
        .iter()
        .zip(reference.data().as_chunks::<3>().0.iter())
        .map(|(t, r)| {
            0.2126 * f64::from(t[0] - r[0])
                + 0.7152 * f64::from(t[1] - r[1])
                + 0.0722 * f64::from(t[2] - r[2])
        })
        .sum();
    sum / (test.width() * test.height()) as f64
}

fn mse(a: &Image, b: &Image) -> f64 {
    a.data()
        .iter()
        .zip(b.data())
        .map(|(x, y)| {
            let d = f64::from(x - y);
            d * d
        })
        .sum::<f64>()
        / a.data().len() as f64
}

fn hotspots(map: &colorvideovdp::DistortionMap) -> Vec<Value> {
    let mut out = Vec::new();
    let plane = map.width * map.height;
    for frame in 0..map.frames {
        let pixels = &map.data[frame * plane..(frame + 1) * plane];
        out.extend(hotspots_in_frame(pixels, map.width, map.height, frame));
    }
    out
}

fn hotspots_in_frame(pixels: &[f32], width: usize, height: usize, frame: usize) -> Vec<Value> {
    let mut out = Vec::new();
    let mut visited = vec![false; pixels.len()];
    for seed in 0..pixels.len() {
        if visited[seed] || !is_hot(pixels[seed]) {
            continue;
        }
        visited[seed] = true;
        let mut queue = vec![seed];
        let mut next = 0;
        let (mut left, mut top, mut right, mut bottom) = (width, height, 0usize, 0usize);
        let mut max = 0.0f32;
        while next < queue.len() {
            let i = queue[next];
            next += 1;
            let (x, y) = (i % width, i / width);
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
            max = max.max(pixels[i]);
            for neighbor in [
                (x > 0).then_some(i.saturating_sub(1)),
                (x + 1 < width).then_some(i + 1),
                (y > 0).then_some(i.saturating_sub(width)),
                (y + 1 < height).then_some(i + width),
            ]
            .into_iter()
            .flatten()
            {
                if !visited[neighbor] && is_hot(pixels[neighbor]) {
                    visited[neighbor] = true;
                    queue.push(neighbor);
                }
            }
        }
        out.push(
            json!({"frame":frame,"rect_px":[left,top,right-left+1,bottom-top+1],
                "area_px":queue.len(),"max_raw_distortion":max}),
        );
    }
    out
}

fn is_hot(value: f32) -> bool {
    value.is_finite() && value > MAP_HOTSPOT
}

fn findings(test: &[Image], reference: &[Image]) -> Vec<Value> {
    let mut out = Vec::new();
    let residuals: Vec<_> = test
        .iter()
        .zip(reference)
        .map(|(t, r)| mean_signed(t, r))
        .collect();
    if residuals.len() >= 3
        && residuals.iter().all(|v| v.abs() >= 0.02)
        && residuals.windows(2).all(|p| p[0] * p[1] < 0.0)
    {
        out.push(json!({"kind":"flicker","frames":residuals.len(),
            "evidence":"alternating signed luminance residual at least 0.02 sRGB units"}));
    }
    for i in 1..test.len() {
        let current = mse(&test[i], &reference[i]);
        let previous = mse(&test[i], &reference[i - 1]);
        let motion = mse(&reference[i], &reference[i - 1]);
        if motion >= 0.001 && previous + 0.001 < current {
            out.push(json!({"kind":"ghosting","frame":i,"current_mse":current,
                "previous_mse":previous,"reference_motion_mse":motion,
                "evidence":"capture resembles the previous reference frame more than the current frame"}));
        }
    }
    out
}

pub(crate) fn run(args: TemporalArgs, absolute: bool) -> Result<u8, CliError> {
    if !args.fps.is_finite() || !(0.0..=16384.0).contains(&args.fps) || args.fps == 0.0 {
        return Err(CliError::usage("--fps must be finite and in (0, 16384]"));
    }
    if args
        .min_jod
        .is_some_and(|v| !v.is_finite() || !(0.0..=10.0).contains(&v))
    {
        return Err(CliError::usage(
            "--min-jod must be finite and between 0 and 10",
        ));
    }
    let cfg = saccade_core::config::RunConfig {
        record_absolute_paths: absolute,
        ..Default::default()
    };
    let sequence = saccade_core::sequence::run_sequence(
        &args.baseline_dir,
        &args.capture_dir,
        &args.out,
        &args.pattern,
        &cfg,
    )?;
    if sequence.frames.len() < 2
        || sequence.baseline_frames != sequence.capture_frames
        || sequence.frames.iter().any(|f| {
            !matches!(
                f.entry.status,
                saccade_core::report::Status::Pass | saccade_core::report::Status::Fail
            )
        })
    {
        return Err(CliError::usage(
            "temporal evaluation needs at least two complete, readable frame pairs; inspect the sequence report",
        ));
    }
    let mut reference = Vec::with_capacity(sequence.frames.len());
    let mut test = Vec::with_capacity(sequence.frames.len());
    for frame in &sequence.frames {
        let b = frame
            .baseline_name
            .as_deref()
            .ok_or_else(|| CliError::usage("missing baseline frame"))?;
        let c = frame
            .capture_name
            .as_deref()
            .ok_or_else(|| CliError::usage("missing capture frame"))?;
        reference.push(load_frame(&args.baseline_dir.join(b))?);
        test.push(load_frame(&args.capture_dir.join(c))?);
    }
    let display = DisplayModel::from_name(&args.display).map_err(cv_error)?;
    let metric =
        Cvvdp::new(display, Options::default().with_distortion_map(true)).map_err(cv_error)?;
    let video = metric
        .predict_video(&test, &reference, args.fps, Color::Srgb)
        .map_err(cv_error)?;
    let per_frame: Vec<f32> = test
        .iter()
        .zip(&reference)
        .map(|(t, r)| {
            metric
                .predict_image(t, r, Color::Srgb)
                .map(|p| p.jod)
                .map_err(cv_error)
        })
        .collect::<Result<_, _>>()?;
    let map = video
        .distortion_map
        .as_ref()
        .ok_or_else(|| cv_error("distortion map absent"))?;
    let hotspots = hotspots(map);
    let findings = findings(&test, &reference);
    let artifact = args.out.join(FILE);
    let full = json!({"schema":"saccade-temporal.v1","display_model":args.display,
        "fps":args.fps,"input_color":"sRGB","video_jod":video.jod,
        "min_jod":args.min_jod,"per_frame_jod":per_frame,
        "per_frame_jod_kind":"still_image",
        "per_frame_jod_note":"Each score uses predict_image independently; only video_jod includes temporal context.",
        "temporal_hotspots":hotspots,"hotspot_rule":"raw ColorVideoVDP distortion > 0.2 (per-pixel JOD < 8)",
        "findings":findings,"sequence_report":sequence.report_json,
        "limits":["Flicker and ghosting kinds are deterministic heuristics, not classifier outputs from ColorVideoVDP.",
        "Hotspot boxes use four-connected raw-map pixels; diagonal-only pixels are separate components."]});
    std::fs::write(&artifact, serde_json::to_vec_pretty(&full)?)
        .map_err(|e| CliError::io(format!("{}: {e}", artifact.display())))?;
    let fail_jod = args.min_jod.is_some_and(|min| video.jod < min);
    let summary = json!({"schema":"saccade-temporal.v1","operation":"temporal",
        "video_jod":video.jod,"min_jod":args.min_jod,"display_model":args.display,
        "fps":args.fps,"frames":per_frame.len(),
        "findings":findings.iter().take(5).collect::<Vec<_>>(),
        "temporal_hotspots":hotspots.iter().take(5).collect::<Vec<_>>(),
        "page":{"hotspots_omitted":hotspots.len().saturating_sub(5),
            "findings_omitted":findings.len().saturating_sub(5)},
        "artifact":saccade_core::paths::cwd(&artifact,absolute),
        "verdict":if sequence.is_regression() || fail_jod {"regression"} else {"pass"}});
    if args.json {
        let bytes = serde_json::to_vec(&summary)?;
        if bytes.len() > 4096 {
            return Err(CliError::new(
                "output_budget",
                "temporal summary exceeds 4096 bytes; inspect the artifact",
            ));
        }
        crate::emit(&format!("{}\n", String::from_utf8_lossy(&bytes)))?;
    } else {
        crate::emit(&format!(
            "temporal: {} JOD on {} frames ({}, {} fps)\nfindings: {}\nreport: {}\n",
            video.jod,
            per_frame.len(),
            args.display,
            args.fps,
            findings.len(),
            artifact.display()
        ))?;
    }
    Ok(u8::from(sequence.is_regression() || fail_jod))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_temporal_hotspots_have_separate_boxes() {
        let mut pixels = vec![0.0; 5 * 3];
        pixels[0] = 0.3;
        pixels[1] = 0.4;
        pixels[14] = 0.5;
        let boxes = hotspots_in_frame(&pixels, 5, 3, 2);
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0]["frame"], 2);
        assert_eq!(boxes[0]["rect_px"], json!([0, 0, 2, 1]));
        assert_eq!(boxes[0]["area_px"], 2);
        assert_eq!(boxes[1]["rect_px"], json!([4, 2, 1, 1]));
    }

    #[test]
    fn cvvdp_reports_jod_and_flicker_from_synthetic_frames() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (baseline, capture, out) = (
            dir.path().join("base"),
            dir.path().join("cap"),
            dir.path().join("out"),
        );
        std::fs::create_dir_all(&baseline).expect("base");
        std::fs::create_dir_all(&capture).expect("cap");
        for (i, value) in [180, 80, 180].into_iter().enumerate() {
            image::RgbImage::from_pixel(32, 32, image::Rgb([128, 128, 128]))
                .save(baseline.join(format!("frame_{i}.png")))
                .expect("save base");
            image::RgbImage::from_pixel(32, 32, image::Rgb([value, value, value]))
                .save(capture.join(format!("frame_{i}.png")))
                .expect("save cap");
        }
        let code = run(
            TemporalArgs {
                baseline_dir: baseline,
                capture_dir: capture,
                fps: 30.0,
                display: "standard_4k".into(),
                pattern: "*".into(),
                out: out.clone(),
                min_jod: None,
                json: false,
            },
            false,
        )
        .expect("temporal");
        assert_eq!(code, 1); // the per-frame FLIP comparison sees the changes
        let artifact: Value =
            serde_json::from_slice(&std::fs::read(out.join(FILE)).expect("artifact"))
                .expect("json");
        assert!(artifact["video_jod"].as_f64().is_some_and(|v| v < 10.0));
        assert_eq!(
            artifact["per_frame_jod"].as_array().expect("frames").len(),
            3
        );
        assert_eq!(artifact["per_frame_jod_kind"], "still_image");
        assert!(
            artifact["per_frame_jod_note"]
                .as_str()
                .unwrap_or("")
                .contains("predict_image")
        );
        assert_eq!(artifact["findings"][0]["kind"], "flicker");
        assert!(
            !artifact["temporal_hotspots"]
                .as_array()
                .expect("hotspots")
                .is_empty()
        );
    }

    #[test]
    fn lagging_frame_is_a_separate_ghosting_finding() {
        let mk = |v: f32| Image::new(4, 4, 3, vec![v; 4 * 4 * 3]).expect("image");
        let reference = vec![mk(0.0), mk(1.0), mk(0.0)];
        let test = vec![mk(0.0), mk(0.0), mk(1.0)];
        let items = findings(&test, &reference);
        assert!(
            items
                .iter()
                .any(|v| v["kind"] == "ghosting" && v["frame"] == 1)
        );
    }
}
