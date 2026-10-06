//! Event-relative visual trajectories, settling and pre-change residual projection.
use crate::{Error, Result};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
/// Trajectory contract.
pub const SCHEMA: &str = "saccade-settling.v1";
/// Declared analysis settings, using normalized RGB error.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Zero-based frame at which the external change was requested.
    pub change_frame: usize,
    /// Frame rate for conversion to seconds.
    pub fps: f64,
    /// Tile edge in pixels.
    pub tile_size: u32,
    /// Mean absolute normalized RGB threshold.
    pub threshold: f64,
    /// Required consecutive frames under threshold.
    pub consecutive: usize,
    /// Final frames averaged when no reference sequence is supplied.
    pub final_frames: usize,
}
/// One spatial tile trajectory.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct Tile {
    /// Pixel rectangle x, y, width, height.
    pub rect: [u32; 4],
    /// Mean RGB absolute error against the reference for every frame.
    pub error: Vec<f64>,
    /// Projection coefficient onto pre-change minus reference content, per frame.
    pub ghosting: Vec<Option<f64>>,
    /// First frame with K consecutive under-threshold observations, or null.
    pub settle_frame: Option<usize>,
    /// Elapsed time from declared change to settle, seconds.
    pub time_to_settle_seconds: Option<f64>,
    /// First observed deviation from the initial pre-change frame.
    pub onset_frame: Option<usize>,
    /// Signed onset lag in frames relative to declared change.
    pub lag_frames: Option<i64>,
    /// Mean residual coefficient in the declared final window.
    pub persistent_ghosting: Option<f64>,
}
/// Full report, including worst tiles (descending final error).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Version discriminator.
    pub schema: String,
    /// Recorded effective policy.
    pub policy: Policy,
    /// Reference source mode.
    pub reference: String,
    /// Per-tile trajectories.
    pub tiles: Vec<Tile>,
    /// Maximum tile error at every frame.
    pub global_error: Vec<f64>,
    /// First K-frame interval in which every tile settles.
    pub settle_frame: Option<usize>,
    /// Elapsed time from change to global settling.
    pub time_to_settle_seconds: Option<f64>,
    /// First onset among all tiles.
    pub onset_frame: Option<usize>,
    /// Onset lag in frames.
    pub lag_frames: Option<i64>,
    /// Up to sixteen worst tile indices.
    pub worst_tiles: Vec<usize>,
    /// Diagnostic interpretation limits.
    pub limits: Vec<String>,
}
/// Discover numeric-suffix image frames with bounded count and unique indices.
#[cfg(feature = "graphics")]
pub fn frame_paths(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let files = crate::run::collect_images(dir)?;
    if files.files.len() > 512 {
        return Err(Error::Config("settling supports at most 512 frames".into()));
    }
    let mut paths = Vec::new();
    for path in files.files.values() {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let digits = stem
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        let index = digits
            .parse::<u64>()
            .map_err(|_| Error::Config("frame names must end in a numeric index".into()))?;
        paths.push((index, path.clone()));
    }
    paths.sort_by_key(|p| p.0);
    if paths.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err(Error::Config("duplicate frame index".into()));
    }
    Ok(paths.into_iter().map(|(_, p)| p).collect())
}
fn settle(errors: &[f64], p: &Policy) -> Option<usize> {
    (p.change_frame..=errors.len().saturating_sub(p.consecutive)).find(|i| {
        errors[*i..*i + p.consecutive]
            .iter()
            .all(|e| *e <= p.threshold)
    })
}
fn samples(image: &RgbaImage, rect: [u32; 4]) -> Vec<f64> {
    let mut out = Vec::new();
    for y in rect[1]..rect[1] + rect[3] {
        for x in rect[0]..rect[0] + rect[2] {
            out.extend(
                image.get_pixel(x, y).0[..3]
                    .iter()
                    .map(|v| *v as f64 / 255.),
            );
        }
    }
    out
}
/// Analyze equal-sized frames, with an optional per-frame reference sequence.
pub fn analyze(frames: &[RgbaImage], reference: Option<&[RgbaImage]>, p: Policy) -> Result<Report> {
    if frames.len() < 3
        || frames.len() > 512
        || p.change_frame == 0
        || p.change_frame >= frames.len()
        || !p.fps.is_finite()
        || p.fps <= 0.
        || p.tile_size == 0
        || !p.threshold.is_finite()
        || !(0. ..=1.).contains(&p.threshold)
        || p.consecutive == 0
        || p.consecutive > frames.len() - p.change_frame
        || p.final_frames == 0
        || p.final_frames > frames.len() - p.change_frame
    {
        return Err(Error::Config(
            "settling: invalid bounded sequence or policy".into(),
        ));
    }
    let dims = frames[0].dimensions();
    if dims.0 == 0
        || dims.1 == 0
        || frames.iter().any(|f| f.dimensions() != dims)
        || reference
            .is_some_and(|r| r.len() != frames.len() || r.iter().any(|f| f.dimensions() != dims))
    {
        return Err(Error::Config(
            "settling: frames and reference must have equal nonempty dimensions".into(),
        ));
    }
    if dims.0.div_ceil(p.tile_size) as u64 * dims.1.div_ceil(p.tile_size) as u64 > 4096 {
        return Err(Error::Config("settling: more than 4096 tiles".into()));
    }
    let mut tiles = Vec::new();
    let mut global_error = vec![0_f64; frames.len()];
    for y in (0..dims.1).step_by(p.tile_size as usize) {
        for x in (0..dims.0).step_by(p.tile_size as usize) {
            let rect = [
                x,
                y,
                p.tile_size.min(dims.0 - x),
                p.tile_size.min(dims.1 - y),
            ];
            let values = frames.iter().map(|f| samples(f, rect)).collect::<Vec<_>>();
            let n = values[0].len();
            let mut pre = vec![0.; n];
            for row in &values[..p.change_frame] {
                for (a, b) in pre.iter_mut().zip(row) {
                    *a += b / p.change_frame as f64;
                }
            }
            let mut final_mean = vec![0.; n];
            for row in &values[frames.len() - p.final_frames..] {
                for (a, b) in final_mean.iter_mut().zip(row) {
                    *a += b / p.final_frames as f64;
                }
            }
            let refs = reference.map(|r| r.iter().map(|f| samples(f, rect)).collect::<Vec<_>>());
            let mut error = Vec::new();
            let mut ghosting = Vec::new();
            let mut onset = None;
            for (i, row) in values.iter().enumerate() {
                let target = refs.as_ref().map_or(&final_mean, |r| &r[i]);
                let e = row
                    .iter()
                    .zip(target)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f64>()
                    / n as f64;
                error.push(e);
                global_error[i] = global_error[i].max(e);
                if onset.is_none()
                    && i >= 1
                    && row
                        .iter()
                        .zip(&values[0])
                        .map(|(a, b)| (a - b).abs())
                        .sum::<f64>()
                        / n as f64
                        > p.threshold
                {
                    onset = Some(i);
                }
                let energy = pre
                    .iter()
                    .zip(target)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>();
                let residual = row
                    .iter()
                    .zip(target)
                    .zip(&pre)
                    .map(|((a, b), c)| (a - b) * (c - b))
                    .sum::<f64>();
                ghosting.push(if energy > 1e-12 {
                    Some(residual / energy)
                } else {
                    None
                });
            }
            let settled = settle(&error, &p);
            let last = ghosting[frames.len() - p.final_frames..]
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            tiles.push(Tile {
                rect,
                error,
                ghosting,
                settle_frame: settled,
                time_to_settle_seconds: settled.map(|i| (i - p.change_frame) as f64 / p.fps),
                onset_frame: onset,
                lag_frames: onset.map(|i| i as i64 - p.change_frame as i64),
                persistent_ghosting: if last.is_empty() {
                    None
                } else {
                    Some(last.iter().sum::<f64>() / last.len() as f64)
                },
            });
        }
    }
    let settled = settle(&global_error, &p);
    let onset = tiles.iter().filter_map(|t| t.onset_frame).min();
    let mut worst_tiles = (0..tiles.len()).collect::<Vec<_>>();
    worst_tiles.sort_by(|a, b| {
        tiles[*b].error[frames.len() - p.final_frames..]
            .iter()
            .sum::<f64>()
            .total_cmp(
                &tiles[*a].error[frames.len() - p.final_frames..]
                    .iter()
                    .sum::<f64>(),
            )
            .then(a.cmp(b))
    });
    worst_tiles.truncate(16);
    Ok(Report{schema:SCHEMA.into(),time_to_settle_seconds:settled.map(|i|(i-p.change_frame) as f64/p.fps),settle_frame:settled,onset_frame:onset,lag_frames:onset.map(|i|i as i64-p.change_frame as i64),policy:p,reference:if reference.is_some(){"provided_sequence"}else{"final_window_mean"}.into(),tiles,global_error,worst_tiles,limits:vec!["Unregistered RGB diagnostics require a fixed viewpoint and exposure; onset uses the declared error threshold.".into(),"K consecutive observations establish only the recorded window, not unobserved future stability.".into(),"Final-window references can conceal persistent ghosts; use an independent reference to measure residuals.".into(),"Ghosting is least-squares correlation/projection, not a causal attribution; zero pre/reference contrast is unavailable.".into()]})
}
impl Report {
    /// Portable HTML strip chart, with no scripts or external assets.
    pub fn html(&self) -> String {
        let mut out = String::from(
            "<!doctype html><meta charset=utf-8><title>Visual settling</title><h1>Visual settling</h1><p>Normalized RGB error; red is above threshold. Tile labels are pixel rectangles.</p>",
        );
        out.push_str(&format!(
            "<p>Change frame {}; global settle {:?}; lag {:?} frames</p><table>",
            self.policy.change_frame, self.settle_frame, self.lag_frames
        ));
        for i in &self.worst_tiles {
            let t = &self.tiles[*i];
            out.push_str(&format!("<tr><th>{:?}</th><td><svg width=800 height=40 viewBox=\"0 0 800 40\" role=img aria-label=\"Tile error trajectory\">",t.rect));
            for (j, e) in t.error.iter().enumerate() {
                let w = 800. / t.error.len() as f64;
                out.push_str(&format!("<rect x=\"{}\" y=\"0\" width=\"{}\" height=\"40\" fill=\"{}\" opacity=\"{}\"><title>frame {j}: {e:.6}</title></rect>",j as f64*w,w,if *e>self.policy.threshold{"red"}else{"green"},(0.2+*e).min(1.)));
            }
            out.push_str("</svg></td></tr>");
        }
        out.push_str("</table>");
        out
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use image::Rgba;
    fn image(v: u8) -> RgbaImage {
        RgbaImage::from_pixel(8, 8, Rgba([v, v, v, 255]))
    }
    fn policy() -> Policy {
        Policy {
            change_frame: 2,
            fps: 10.,
            tile_size: 4,
            threshold: 0.02,
            consecutive: 2,
            final_frames: 2,
        }
    }
    #[test]
    fn known_specimen_fade_and_lag() {
        let frames = [255, 255, 255, 180, 80, 0, 0, 0].map(image);
        let r = analyze(&frames, None, policy()).unwrap();
        assert_eq!(r.settle_frame, Some(5));
        assert_eq!(r.lag_frames, Some(1));
        assert_eq!(r.time_to_settle_seconds, Some(0.3));
        assert_eq!(r.tiles.len(), 4);
    }
    #[test]
    fn onset_before_the_declared_event_keeps_negative_lag() {
        let frames = [255, 255, 0, 0, 0, 0, 0, 0].map(image);
        let mut p = policy();
        p.change_frame = 4;
        let r = analyze(&frames, None, p).unwrap();
        assert_eq!(r.onset_frame, Some(2));
        assert_eq!(r.lag_frames, Some(-2));
    }
    #[test]
    fn persistent_prechange_residual_requires_independent_reference() {
        let frames = [255, 255, 64, 64, 64, 64].map(image);
        let refs = [0, 0, 0, 0, 0, 0].map(image);
        let r = analyze(&frames, Some(&refs), policy()).unwrap();
        assert!(
            r.tiles
                .iter()
                .all(|t| (t.persistent_ghosting.unwrap() - 64. / 255.).abs() < 1e-9)
        );
        assert_eq!(r.settle_frame, None);
        let misleading = analyze(&frames, None, policy()).unwrap();
        assert_eq!(misleading.settle_frame, Some(2));
    }
}
