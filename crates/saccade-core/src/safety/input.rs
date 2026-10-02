//! Numbered-frame reuse and optional external ffmpeg decoding.

use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) struct Input {
    pub frames: Vec<(String, u64, PathBuf)>,
    pub fps: f64,
    pub warnings: Vec<String>,
    _temporary: Option<tempfile::TempDir>,
}

fn rate(s: &str) -> Option<f64> {
    let v = match s.split_once('/') {
        Some((a, b)) => a.parse::<f64>().ok()? / b.parse::<f64>().ok()?,
        None => s.parse().ok()?,
    };
    (v.is_finite() && v > 0.0).then_some(v)
}

pub(crate) fn load(path: &Path, fps: Option<f64>) -> Result<Input> {
    if fps.is_some_and(|v| !v.is_finite() || v <= 0.0) {
        return Err(Error::Config("fps must be finite and positive".into()));
    }
    // Use the canonical helper on every OS, then the existing collector's
    // duplicate/numbering/symlink checks. Never interpret paths in a shell.
    let root = crate::paths::canonicalize(path)
        .map_err(crate::run::io_err("opening safety input".into()))?;
    let mut warnings = Vec::new();
    let (directory, temporary, metadata_fps) = if root.is_dir() {
        let meta = root.join("saccade-meta.json");
        let metadata_fps = if meta.is_file() {
            let text = std::fs::read_to_string(&meta)
                .map_err(crate::run::io_err("reading frame metadata".into()))?;
            let v: serde_json::Value = serde_json::from_str(&text)?;
            v.get("fps")
                .map(|value| {
                    value.as_f64().ok_or_else(|| {
                        Error::Config(
                            "metadata fps must be a positive number, not a string/null".into(),
                        )
                    })
                })
                .transpose()?
        } else {
            None
        };
        (root, None, metadata_fps)
    } else {
        if !root.extension().and_then(|s| s.to_str()).is_some_and(|e| {
            ["mp4", "mov", "mkv"]
                .iter()
                .any(|v| e.eq_ignore_ascii_case(v))
        }) {
            return Err(Error::Config(
                "safety input must be a numbered frame directory or mp4/mov/mkv video".into(),
            ));
        }
        let installed = Command::new("ffmpeg")
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| Error::Config("video decoding requires optional external ffmpeg on PATH; install it or supply numbered PNG frames".into()))?;
        if !installed.success() {
            return Err(Error::Config(
                "external ffmpeg on PATH did not start successfully".into(),
            ));
        }
        let metadata_fps = if fps.is_none() {
            let probe = Command::new("ffprobe").args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=avg_frame_rate,r_frame_rate", "-of", "json"]).arg(&root).stdin(Stdio::null()).output()
                .map_err(|_| Error::Config("video FPS requires ffprobe on PATH or explicit --fps; decoding requires ffmpeg on PATH".into()))?;
            if !probe.status.success() {
                return Err(Error::Config(
                    "ffprobe could not read video FPS; provide --fps".into(),
                ));
            }
            let v: serde_json::Value = serde_json::from_slice(&probe.stdout)?;
            let stream = &v["streams"][0];
            let avg = stream["avg_frame_rate"].as_str().and_then(rate);
            let nominal = stream["r_frame_rate"].as_str().and_then(rate);
            if let (Some(a), Some(n)) = (avg, nominal)
                && (a - n).abs() > a * 0.01
            {
                return Err(Error::Config("variable-frame-rate video: export constant-rate frames with known --fps before this pre-check".into()));
            }
            Some(avg.ok_or_else(|| Error::Config("video has no usable FPS; provide --fps".into()))?)
        } else {
            None
        };
        let tmp = tempfile::tempdir().map_err(crate::run::io_err(
            "creating decoded-frame directory".into(),
        ))?;
        let decoded = Command::new("ffmpeg").args(["-nostdin", "-v", "error", "-i"]).arg(&root)
            .args(["-map", "0:v:0", "-vsync", "0", "-start_number", "0"])
            .arg(tmp.path().join("frame_%09d.png")).stdin(Stdio::null()).output()
            .map_err(|_| Error::Config("video decoding requires optional external ffmpeg on PATH; install it or supply numbered PNG frames".into()))?;
        if !decoded.status.success() {
            return Err(Error::Config(format!(
                "ffmpeg decoding failed: {}",
                String::from_utf8_lossy(&decoded.stderr)
            )));
        }
        warnings.push("Video interpreted as constant frame rate; exported SDR sRGB frames. Verify colour transfer and timing before formal testing.".into());
        (tmp.path().to_path_buf(), Some(tmp), metadata_fps)
    };
    let fps = fps.or(metadata_fps).unwrap_or_else(|| {
        warnings.push("No FPS metadata: assuming 60 fps. Supply --fps to match the source.".into());
        60.0
    });
    if !fps.is_finite() || fps <= 0.0 {
        return Err(Error::Config(
            "metadata fps must be finite and positive".into(),
        ));
    }
    let frames = crate::sequence::numbered_frames(&directory)?;
    if frames.len() < 2 {
        return Err(Error::Config(
            "safety needs at least two numbered frames".into(),
        ));
    }
    if !(frames.len() as f64 / fps).is_finite() {
        return Err(Error::Config(
            "fps produces non-finite sequence timestamps".into(),
        ));
    }
    if frames
        .windows(2)
        .any(|p| p[1].1.checked_sub(p[0].1) != Some(1))
    {
        return Err(Error::Config(
            "sequence has missing frame numbers; uninterrupted timing is required".into(),
        ));
    }
    Ok(Input {
        frames,
        fps,
        warnings,
        _temporary: temporary,
    })
}
