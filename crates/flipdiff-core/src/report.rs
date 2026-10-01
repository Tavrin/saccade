//! The `flipdiff-report.v1` model: the single contract between the comparison
//! run (CLI) and every presentation of its result (HTML report, Markdown
//! summary, the GitHub Action).
//!
//! Presentation code reads only this model, never the images' pixels, so a
//! report directory is self-describing: `flipdiff-report.v1.json` plus the
//! images it references by path relative to that directory.

use serde::{Deserialize, Serialize};

/// Schema identifier written to [`Report::schema`].
pub const REPORT_SCHEMA: &str = "flipdiff-report.v1";

/// File name of the report JSON inside a report directory.
pub const REPORT_FILE_NAME: &str = "flipdiff-report.v1.json";

/// One comparison run over a baseline directory and a capture directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Always [`REPORT_SCHEMA`].
    pub schema: String,
    /// Version of the tool that produced the report (`CARGO_PKG_VERSION`).
    pub tool_version: String,
    /// Seconds since the Unix epoch when the run finished.
    pub generated_at_unix: u64,
    /// Effective run-wide settings (per-entry overrides live on each entry).
    pub config: ReportConfig,
    /// Counts per status; always consistent with `entries`.
    pub totals: Totals,
    /// One entry per image name seen in either directory, sorted by `name`.
    pub entries: Vec<Entry>,
}

/// Run-wide settings recorded for reproducibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Default pass threshold applied when no override matches.
    pub default_threshold: f64,
    /// Default metric applied when no override matches.
    pub default_metric: Metric,
    /// FLIP observer setting (pixels per degree of visual angle).
    pub pixels_per_degree: f32,
    /// Whether a `new` entry fails the run.
    pub fail_on_new: bool,
}

/// Per-status counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    /// Number of entries.
    pub total: usize,
    /// Entries whose metric is at or below their threshold.
    pub pass: usize,
    /// Entries whose metric exceeds their threshold.
    pub fail: usize,
    /// Capture present, baseline absent.
    pub new: usize,
    /// Baseline present, capture absent.
    pub missing: usize,
    /// Could not be compared (decode failure, dimension mismatch).
    pub error: usize,
}

/// Outcome of one image name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// `value <= threshold`.
    Pass,
    /// `value > threshold`.
    Fail,
    /// Capture present, baseline absent.
    New,
    /// Baseline present, capture absent.
    Missing,
    /// Could not be compared; see [`Entry::error`].
    Error,
}

/// Which statistic of the FLIP error map decides pass/fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    /// Mean FLIP error over all pixels.
    Mean,
    /// 95th-percentile FLIP error.
    P95,
    /// Maximum FLIP error.
    Max,
}

/// One image name's comparison result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Path relative to the baseline/capture roots, `/`-separated.
    pub name: String,
    /// Outcome.
    pub status: Status,
    /// Metric that decided `status` for this entry (after overrides).
    pub metric_used: Metric,
    /// Threshold that decided `status` for this entry (after overrides).
    pub threshold: f64,
    /// Value of `metric_used`; `None` unless status is pass or fail.
    pub value: Option<f64>,
    /// Full FLIP statistics; `None` unless status is pass or fail.
    pub metrics: Option<Metrics>,
    /// Structural checks on the capture; `None` when there is no decodable capture.
    pub properties: Option<Properties>,
    /// Image paths relative to the report directory.
    pub paths: EntryPaths,
    /// Human-readable reason; `Some` exactly when status is error.
    pub error: Option<String>,
}

/// Serde adapter for metric values: a non-finite number is written as `null`
/// and `null` is read back as NaN, so a report always round-trips.
mod finite_or_null {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f64(*v)
        } else {
            s.serialize_none()
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
    }
}

/// FLIP error-map statistics. All error values are in `[0, 1]`; a non-finite
/// value (which a valid run does not produce) is serialized as `null` and read
/// back as NaN.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Mean error.
    #[serde(with = "finite_or_null")]
    pub mean: f64,
    /// Maximum error.
    #[serde(with = "finite_or_null")]
    pub max: f64,
    /// Median error.
    #[serde(with = "finite_or_null")]
    pub p50: f64,
    /// 95th-percentile error.
    #[serde(with = "finite_or_null")]
    pub p95: f64,
    /// 99th-percentile error.
    #[serde(with = "finite_or_null")]
    pub p99: f64,
    /// Fraction of pixels with error > 0.1.
    #[serde(with = "finite_or_null")]
    pub frac_above_0_1: f64,
    /// Fraction of pixels with error > 0.5.
    #[serde(with = "finite_or_null")]
    pub frac_above_0_5: f64,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Structural sanity checks on a single image (catches black frames, blowouts).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Properties {
    /// Every pixel is (0,0,0).
    pub is_all_black: bool,
    /// Every pixel is (255,255,255).
    pub is_all_white: bool,
    /// Mean Rec. 709 luminance in `[0, 1]`.
    pub mean_luminance: f32,
    /// Minimum Rec. 709 luminance.
    pub min_luminance: f32,
    /// Maximum Rec. 709 luminance.
    pub max_luminance: f32,
}

/// Paths (relative to the report directory, `/`-separated) of the images the
/// report copies or generates. `None` when that image does not exist.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryPaths {
    /// Copy of the baseline image.
    pub baseline: Option<String>,
    /// Copy of the capture image.
    pub capture: Option<String>,
    /// FLIP error heatmap (magma colormap).
    pub heatmap: Option<String>,
}

impl Report {
    /// Whether this run should exit non-zero: any fail, error or missing entry,
    /// or any new entry when `config.fail_on_new`.
    pub fn is_regression(&self) -> bool {
        let t = &self.totals;
        t.fail > 0 || t.error > 0 || t.missing > 0 || (self.config.fail_on_new && t.new > 0)
    }
}
