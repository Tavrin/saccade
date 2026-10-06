//! Bounded external video decoding and deterministic histogram change-point keyframes.
use super::{
    Analyzer, MediaError, Options, Record, Result, Section, Status, enforce_strict, provenance,
    skipped,
};
use crate::{
    general::{hashing, input},
    wave7::models,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
/// Keyframe report discriminator.
pub const SCHEMA: &str = "saccade-keyframes.v1";
/// Video sampling/shot controls; conservative limits keep decoder work bounded.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VideoOptions {
    /// Requested samples per second, 0.1..10 (decoder may lower to keep <=300 samples).
    pub sample_fps: f64,
    /// SSE change-point penalty, 0.001..10; content-dependent, uncalibrated.
    pub shot_penalty: f64,
    /// Near-duplicate pHash/dHash Hamming threshold (0..16), followed by histogram verification.
    pub duplicate_hamming: u32,
}
impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            sample_fps: 1.,
            shot_penalty: 0.15,
            duplicate_hamming: 4,
        }
    }
}
/// Representative frame bound to its actual encoded file and normalized timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe {
    /// Original zero-based sample number.
    pub sample: usize,
    /// Normalized presentation time in seconds (sampling grid, not audio time).
    pub timestamp_seconds: f64,
    /// File containing the selected PNG/raster.
    pub path: PathBuf,
    /// Exact encoded file digest.
    pub sha256: String,
}
/// One detected shot, including duplicate-remapped representative.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shot {
    /// Inclusive sample start.
    pub start_sample: usize,
    /// Exclusive sample end.
    pub end_sample: usize,
    /// Start time.
    pub start_seconds: f64,
    /// End time, duration-bounded.
    pub end_seconds: f64,
    /// Index into unique keyframes; repeated shots can share a representative.
    pub keyframe: usize,
}
/// Complete shot/keyframe receipt; no source images are overwritten.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframes {
    /// SCHEMA.
    pub schema: String,
    /// Declared/probed duration.
    pub duration_seconds: f64,
    /// Probed video duration or derived directory sampling grid.
    pub duration_basis: String,
    /// Effective decoding samples per second.
    pub sample_fps: f64,
    /// Frames sampled (fast shots between samples may be missed).
    pub samples: usize,
    /// Detected shots.
    pub shots: Vec<Shot>,
    /// Unique representative frames.
    pub keyframes: Vec<Keyframe>,
    /// Decoder/sample algorithm and limitations.
    pub provenance: Value,
}
struct Frame {
    path: PathBuf,
    hash: String,
    hist: [f64; 24],
    perceptual: hashing::Hashes,
}
fn histogram(pixels: &image::RgbaImage) -> [f64; 24] {
    let small = image::imageops::resize(
        &crate::compare::flatten_over(pixels, 255),
        64,
        64,
        image::imageops::FilterType::Triangle,
    );
    let mut bins = [0.; 24];
    for p in small.pixels() {
        for c in 0..3 {
            bins[c * 8 + usize::from(p[c] / 32)] += 1. / (4096. * 3.);
        }
    }
    bins
}
fn hist_distance(a: &[f64; 24], b: &[f64; 24]) -> f64 {
    a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum()
}
fn select(
    paths: &[PathBuf],
    fps: f64,
    duration: f64,
    basis: &str,
    opts: &VideoOptions,
) -> Result<Keyframes> {
    if paths.is_empty() || paths.len() > 300 {
        return Err(MediaError::new(
            "invalid_video_input",
            "declare 1..300 sampled frames",
        ));
    }
    let mut frames = Vec::new();
    for path in paths {
        let bytes = input::bytes(path, input::MAX_BYTES)?;
        let pixels = input::decode(&bytes)?;
        frames.push(Frame {
            path: path.clone(),
            hash: models::digest(&bytes),
            hist: histogram(&pixels),
            perceptual: hashing::hash(&pixels),
        });
    }
    // Same deterministic penalized segmentation recurrence as the existing onset machinery.
    // Histogram SSE costs replace timing MAD costs; no timing qualification is implied.
    let n = frames.len();
    let mut cost = vec![vec![0.; n + 1]; n];
    for (start, cost_row) in cost.iter_mut().enumerate() {
        let mut sums = [0.; 24];
        let mut squared = 0.;
        for end in start + 1..=n {
            for (i, v) in frames[end - 1].hist.iter().enumerate() {
                sums[i] += v;
                squared += v * v;
            }
            cost_row[end] =
                (squared - sums.iter().map(|v| v * v).sum::<f64>() / (end - start) as f64).max(0.);
        }
    }
    let (ends, _) = crate::onset::segment(&cost, 1, opts.shot_penalty);
    let mut representatives: Vec<usize> = Vec::new();
    let mut shots = Vec::new();
    let mut start = 0;
    for end in ends {
        let selected = (start + end - 1) / 2;
        let candidate = &frames[selected];
        let duplicate = representatives.iter().position(|&i| {
            let p = &frames[i];
            p.hash == candidate.hash
                || ((p.perceptual.phash ^ candidate.perceptual.phash).count_ones()
                    <= opts.duplicate_hamming
                    && (p.perceptual.dhash ^ candidate.perceptual.dhash).count_ones()
                        <= opts.duplicate_hamming
                    && hist_distance(&p.hist, &candidate.hist) <= 0.02)
        });
        let keyframe = match duplicate {
            Some(i) => i,
            None => {
                if representatives.len() >= 64 {
                    return Err(MediaError::new(
                        "invalid_video_input",
                        "more than 64 unique keyframes; increase shot penalty",
                    ));
                }
                representatives.push(selected);
                representatives.len() - 1
            }
        };
        shots.push(Shot {
            start_sample: start,
            end_sample: end,
            start_seconds: start as f64 / fps,
            end_seconds: (end as f64 / fps).min(duration),
            keyframe,
        });
        start = end;
    }
    let keyframes = representatives
        .into_iter()
        .map(|i| Keyframe {
            sample: i,
            timestamp_seconds: i as f64 / fps,
            path: frames[i].path.clone(),
            sha256: frames[i].hash.clone(),
        })
        .collect();
    Ok(Keyframes {
        schema: SCHEMA.into(),
        duration_seconds: duration,
        duration_basis: basis.into(),
        sample_fps: fps,
        samples: n,
        shots,
        keyframes,
        provenance: json!({"algorithm":"RGB-histogram-SSE-onset-DP/1","shot_penalty":opts.shot_penalty,"duplicate_hamming":opts.duplicate_hamming,"duplicates":"pHash+dHash candidate, histogram verification","timestamp_basis":"normalized fps sampling grid; source timestamps not inferred from filenames","sampled_frames_sha256":models::digest(&serde_json::to_vec(&frames.iter().map(|f|&f.hash).collect::<Vec<_>>())?),"limitations":["fast shots between samples may be missed","histogram-only shot boundaries are content-dependent and uncalibrated","same-colour shots can be merged; per-keyframe analysis sees decoded sample resolution"]}),
    })
}
fn validate(opts: &VideoOptions) -> Result<()> {
    if !opts.sample_fps.is_finite()
        || !(0.1..=10.).contains(&opts.sample_fps)
        || !opts.shot_penalty.is_finite()
        || !(0.001..=10.).contains(&opts.shot_penalty)
        || opts.duplicate_hamming > 16
    {
        return Err(MediaError::new(
            "invalid_media_options",
            "invalid video fps, penalty or hash threshold",
        ));
    }
    Ok(())
}
/// Analyse a directory of pre-extracted frames (sorted filenames, explicit sampling rate).
/// Duration is the declared sample grid length, not an independently probed video duration.
pub fn from_directory(dir: &Path, opts: &VideoOptions) -> Result<Keyframes> {
    validate(opts)?;
    let paths = input::files(dir, 300)?;
    select(
        &paths,
        opts.sample_fps,
        paths.len() as f64 / opts.sample_fps,
        "pre-extracted directory sampling grid",
        opts,
    )
}
fn external(mut command: Command) -> Result<Vec<u8>> {
    let out = tempfile::tempfile().map_err(|e| MediaError::new("io_error", e.to_string()))?;
    let err = tempfile::tempfile().map_err(|e| MediaError::new("io_error", e.to_string()))?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            out.try_clone()
                .map_err(|e| MediaError::new("io_error", e.to_string()))?,
        ))
        .stderr(Stdio::from(
            err.try_clone()
                .map_err(|e| MediaError::new("io_error", e.to_string()))?,
        ));
    let mut child = command.spawn().map_err(|_| {
        MediaError::new(
            "video_decode_unavailable",
            format!("external ffmpeg/ffprobe unavailable; fix: {}; or supply pre-extracted frame directory",crate::optional::decoder_fix()),
        )
    })?;
    let start = Instant::now();
    let result = loop {
        let state = child
            .try_wait()
            .map_err(|e| MediaError::new("video_decode_failed", e.to_string()))?;
        if let Some(status) = state {
            break if status.success() {
                Ok(())
            } else {
                Err(MediaError::new(
                    "video_decode_failed",
                    "external decoder failed; no stderr or input URL logged",
                ))
            };
        }
        if start.elapsed() > Duration::from_secs(120)
            || out.metadata().map(|m| m.len()).unwrap_or(u64::MAX) > 1024 * 1024
            || err.metadata().map(|m| m.len()).unwrap_or(u64::MAX) > 1024 * 1024
        {
            let _ = child.kill();
            let _ = child.wait();
            break Err(MediaError::new(
                "video_decode_failed",
                "external decoder deadline/output limit",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    result?;
    use std::io::{Read, Seek, SeekFrom};
    let mut out = out;
    out.seek(SeekFrom::Start(0))
        .map_err(|e| MediaError::new("io_error", e.to_string()))?;
    let mut bytes = Vec::new();
    out.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| MediaError::new("io_error", e.to_string()))?;
    if bytes.len() > 1024 * 1024 {
        return Err(MediaError::new(
            "video_decode_failed",
            "decoder output limit",
        ));
    }
    Ok(bytes)
}
fn decoded(video: &Path, dir: &Path, opts: &VideoOptions) -> Result<Keyframes> {
    validate(opts)?;
    crate::optional::require_binaries(&["ffmpeg", "ffprobe"])
        .map_err(|e| MediaError::new("video_decode_unavailable", e.to_string()))?;
    let path = std::fs::canonicalize(video)
        .map_err(|_| MediaError::new("invalid_video_input", "video path unavailable"))?;
    let meta = std::fs::metadata(&path)
        .map_err(|_| MediaError::new("invalid_video_input", "video metadata unavailable"))?;
    if !meta.is_file() || meta.len() > 4 * 1024 * 1024 * 1024 {
        return Err(MediaError::new(
            "invalid_video_input",
            "video must be a regular file <=4 GiB",
        ));
    }
    let source_hash = input::sha256(&path, 4 * 1024 * 1024 * 1024)?;
    let mut probe = Command::new("ffprobe");
    probe
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=width,height",
            "-of",
            "json",
        ])
        .arg(&path);
    let p: Value = serde_json::from_slice(&external(probe)?)?;
    let duration = p["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| MediaError::new("invalid_video_input", "video duration unavailable"))?;
    if !duration.is_finite() || !(0.01..=600.).contains(&duration) {
        return Err(MediaError::new(
            "invalid_video_input",
            "video duration must be <=600 seconds",
        ));
    }
    let fps = opts.sample_fps.min(299. / duration);
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-nostdin",
        "-v",
        "error",
        "-threads",
        "4",
        "-protocol_whitelist",
        "file,pipe",
        "-i",
    ])
    .arg(&path)
    .args(["-an", "-sn", "-dn", "-vf"])
    .arg(format!(
        "fps={fps:.9},scale=w='min(640,iw)':h='min(640,ih)':force_original_aspect_ratio=decrease"
    ))
    .args(["-frames:v", "300", "-threads", "4", "-n"])
    .arg(dir.join("%06d.png"));
    external(cmd)?;
    let paths = input::files(dir, 300)?;
    let mut report = select(&paths, fps, duration, "ffprobe format duration", opts)?;
    if input::sha256(&path, 4 * 1024 * 1024 * 1024)? != source_hash {
        return Err(MediaError::new(
            "video_decode_failed",
            "video changed during external decoding",
        ));
    }
    report.provenance["decoder"] = json!({"binary":"external ffmpeg + ffprobe on PATH","linked":false,"resolution_limit":[640,640],"source_sha256":source_hash,"source_hash_policy":"before/after content checks; original path passed to external decoder","streams":p["streams"],"licence_reasoning":"external user-installed executable, neither linked nor distributed by Saccade; its build licence belongs to its distributor"});
    Ok(report)
}
/// Extract representatives to a new directory. Existing files/directories are never overwritten.
/// Directory inputs need no ffmpeg; video decoding is an explicitly heavy operation.
pub fn keyframes(source: &Path, out: &Path, opts: &VideoOptions) -> Result<Keyframes> {
    validate(opts)?;
    if out.exists() {
        return Err(MediaError::new(
            "io_error",
            "keyframe output must be a new directory",
        ));
    }
    let temp = tempfile::tempdir().map_err(|e| MediaError::new("io_error", e.to_string()))?;
    let mut report = if source.is_dir() {
        from_directory(source, opts)?
    } else {
        decoded(source, temp.path(), opts)?
    };
    std::fs::create_dir(out).map_err(|e| MediaError::new("io_error", e.to_string()))?;
    for (i, frame) in report.keyframes.iter_mut().enumerate() {
        let bytes = input::bytes(&frame.path, input::MAX_BYTES)?;
        if models::digest(&bytes) != frame.sha256 {
            return Err(MediaError::new(
                "video_decode_failed",
                "selected keyframe changed before export",
            ));
        }
        let pixels = input::decode(&bytes)?;
        let path = out.join(format!("{i:04}.png"));
        let f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| MediaError::new("io_error", e.to_string()))?;
        pixels
            .write_to(&mut std::io::BufWriter::new(f), image::ImageFormat::Png)
            .map_err(|e| MediaError::new("io_error", e.to_string()))?;
        frame.path = path;
        frame.sha256 = input::sha256(&frame.path, input::MAX_BYTES)?;
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join(format!("{SCHEMA}.json")))
        .map_err(|e| MediaError::new("io_error", e.to_string()))?;
    let linked = crate::report_links::decorate(&serde_json::to_value(&report)?)
        .map_err(|e| MediaError::new("report_links", e.to_string()))?;
    serde_json::to_writer_pretty(file, &linked)?;
    crate::report_links::index(&out.join(format!("{SCHEMA}.json")), &linked)
        .map_err(|e| MediaError::new("report_links", e.to_string()))?;
    Ok(report)
}
impl Analyzer {
    /// Analyze a video or extracted frame directory; section analysis delegates to analyze_bytes.
    pub fn analyze_video(
        &self,
        source: &Path,
        options: &Options,
        video_options: &VideoOptions,
    ) -> Result<Record> {
        let temp = tempfile::tempdir().map_err(|e| MediaError::new("io_error", e.to_string()))?;
        let start = Instant::now();
        let mut shots = if source.is_dir() {
            from_directory(source, video_options)?
        } else {
            decoded(source, temp.path(), video_options)?
        };
        let mut records = Vec::new();
        let mut base = None;
        for frame in &shots.keyframes {
            let bytes = input::bytes(&frame.path, input::MAX_BYTES)?;
            if models::digest(&bytes) != frame.sha256 {
                return Err(MediaError::new(
                    "video_decode_failed",
                    "keyframe content changed",
                ));
            }
            let record = self.analyze_bytes(&bytes, options)?;
            if base.is_none() {
                base = Some(record.clone());
            }
            records.push(json!({"timestamp_seconds":frame.timestamp_seconds,"record":record}));
        }
        let mut r =
            base.ok_or_else(|| MediaError::new("video_decode_failed", "no decoded keyframes"))?;
        let (hash, dimensions) = if source.is_dir() {
            (
                models::digest(&serde_json::to_vec(
                    &json!({"sampled_frames_sha256":shots.provenance["sampled_frames_sha256"],"sample_fps":shots.sample_fps}),
                )?),
                Value::Null,
            )
        } else {
            (
                input::sha256(source, 4 * 1024 * 1024 * 1024)?,
                shots.provenance["decoder"]["streams"].clone(),
            )
        };
        r.identity.data = json!({"sha256":hash,"kind":"video","dimensions":dimensions,"duration_seconds":shots.duration_seconds,"directory_hash_basis":"ordered sampled encoded hashes plus declared sample rate"});
        for s in [
            &mut r.metadata,
            &mut r.quality,
            &mut r.focal,
            &mut r.text,
            &mut r.fingerprints,
            &mut r.embeddings,
            &mut r.description,
        ] {
            *s = skipped("per-keyframe-analysis", "see video.keyframe_records");
        }
        // Ephemeral paths are not persisted as if available after the temporary decode directory expires.
        for f in &mut shots.keyframes {
            f.path = PathBuf::from(format!("keyframe:{}", f.sample));
        }
        r.video = Section {
            status: Status::Ok,
            provenance: provenance("RGB-histogram-SSE-onset-DP"),
            timing_ms: start.elapsed().as_secs_f64() * 1000.,
            data: json!({"summary":shots,"keyframe_records":records}),
            reason: None,
            error_code: None,
        };
        enforce_strict(r, options.strict)
    }
}
/// Extension hint only; decoding still verifies the actual file through ffprobe/ffmpeg.
pub fn is_video(path: &Path) -> bool {
    path.is_dir()
        || path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v"
            )
        })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::media::Profile;
    #[test]
    fn generated_shots_deduplicate_without_merging_distinct_flat_colours() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("input");
        std::fs::create_dir(&source).unwrap();
        for (i, colour) in [
            [255, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 255, 255],
            [255, 0, 0, 255],
            [255, 0, 0, 255],
        ]
        .iter()
        .enumerate()
        {
            image::RgbaImage::from_pixel(40, 30, image::Rgba(*colour))
                .save(source.join(format!("{i:03}.png")))
                .unwrap();
        }
        let opts = VideoOptions {
            shot_penalty: 0.05,
            ..Default::default()
        };
        let r = from_directory(&source, &opts).unwrap();
        assert_eq!(r.shots.len(), 3);
        assert_eq!(r.keyframes.len(), 2);
        assert_eq!(r.shots[0].keyframe, r.shots[2].keyframe);
        let out = d.path().join("out");
        let r = keyframes(&source, &out, &opts).unwrap();
        assert_eq!(r.keyframes.len(), 2);
        assert!(keyframes(&source, &out, &opts).is_err());
        let a = Analyzer::new(Profile::CpuLite, "cache".into(), false).unwrap();
        let record = a
            .analyze_video(d.path(), &Options::default(), &opts)
            .unwrap();
        assert_eq!(record.video.status, Status::Ok);
        assert!(
            record.video.data["keyframe_records"]
                .as_array()
                .unwrap()
                .len()
                >= 2
        );
    }
    #[test]
    #[ignore = "heavy: ffmpeg"]
    fn ffmpeg_generated_video_has_duration_shots_and_timestamps() {
        let d = tempfile::tempdir().unwrap();
        let video = d.path().join("video.mp4");
        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=64x48:d=2",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=64x48:d=2",
            "-filter_complex",
            "[0:v][1:v]concat=n=2:v=1:a=0",
            "-threads",
            "4",
            "-y",
        ])
        .arg(&video);
        external(cmd).unwrap();
        let out = d.path().join("frames");
        let r = keyframes(
            &video,
            &out,
            &VideoOptions {
                shot_penalty: 0.01,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.duration_seconds > 3.9);
        assert_eq!(r.shots.len(), 2);
        assert_eq!(r.keyframes.len(), 2);
        assert!(r.keyframes[1].timestamp_seconds >= 2.);
        let a = Analyzer::new(Profile::CpuLite, "cache".into(), false).unwrap();
        assert_eq!(
            a.analyze_video(
                &video,
                &Options::default(),
                &VideoOptions {
                    shot_penalty: 0.01,
                    ..Default::default()
                }
            )
            .unwrap()
            .video
            .status,
            Status::Ok
        );
    }
}
