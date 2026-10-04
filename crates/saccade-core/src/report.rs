//! The `saccade-report.v1` model: the single contract between the comparison
//! run (CLI) and every presentation of its result (HTML report, Markdown
//! summary, the GitHub Action).
//!
//! Presentation code reads only this model, never the images' pixels, so a
//! report directory is self-describing: `saccade-report.v1.json` plus the
//! images it references by path relative to that directory.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Schema identifier written to [`Report::schema`].
pub const REPORT_SCHEMA: &str = "saccade-report.v1";

/// File name of the report JSON inside a report directory.
pub const REPORT_FILE_NAME: &str = "saccade-report.v1.json";

/// One comparison run over a baseline directory and a capture directory.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Always [`REPORT_SCHEMA`].
    pub schema: String,
    /// Version of the tool that produced the report (`CARGO_PKG_VERSION`).
    pub tool_version: String,
    /// Seconds since the Unix epoch when the run finished.
    pub generated_at_unix: u64,
    /// Baseline input relative to the report directory (absolute only by opt-in);
    /// `approve` checks it against the directory it is given.
    #[serde(default)]
    pub baseline_dir: Option<String>,
    /// Capture input relative to the report directory (absolute only by opt-in).
    #[serde(default)]
    pub capture_dir: Option<String>,
    /// Effective run-wide settings (per-entry overrides live on each entry).
    pub config: ReportConfig,
    /// Counts per status; always consistent with `entries`.
    pub totals: Totals,
    /// One entry per image name seen in either directory, sorted by `name`.
    pub entries: Vec<Entry>,
    /// Run-wide attribution, present when both captures carry performance evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perf_diff: Option<crate::perf::PerfDiff>,
    /// Malformed performance inputs (also represented by error entries).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub perf_errors: Vec<crate::perf::PerfError>,
    /// Image and performance evidence on one line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combined_verdict: Option<String>,
}

/// Run-wide settings recorded for reproducibility.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportConfig {
    /// Selected name globs; empty selects every name in the supplied inputs.
    #[serde(default)]
    pub entries: Vec<String>,
    /// Explicit exclusions from the supplied scope.
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Default pass threshold applied when no override matches.
    pub default_threshold: f64,
    /// Default metric applied when no override matches.
    pub default_metric: Metric,
    /// FLIP observer setting (pixels per degree of visual angle).
    pub pixels_per_degree: f32,
    /// Whether a `new` entry fails the run.
    pub fail_on_new: bool,
    /// What the run is for; changes defaults and presentation, not the model.
    #[serde(default)]
    pub mode: Mode,
    /// Display names for the two sides (e.g. "parent" / "candidate").
    #[serde(default)]
    pub labels: Labels,
    /// How metadata sidecars were read and enforced.
    #[serde(default)]
    pub meta: MetaSettings,
    /// Historical empty-run opt-in, retained for reading; new empty runs fail.
    #[serde(default)]
    pub allow_empty: bool,
    /// Whether a capture with NaN or infinite samples fails its entry.
    #[serde(default = "default_true")]
    pub fail_on_nonfinite: bool,
    /// Peak-error value at which a local hotspot fails an entry; `None` is off.
    #[serde(default)]
    pub hotspot_fail: Option<f64>,
    /// A passing metric is annotated when a hotspot meets these limits.
    #[serde(default = "default_local_max")]
    pub hotspot_local_max: f64,
    /// Minimum hotspot area in pixels for a local-change finding.
    #[serde(default = "default_local_pixels")]
    pub hotspot_local_min_pixels: u32,
}

fn default_local_max() -> f64 {
    0.5
}
fn default_local_pixels() -> u32 {
    16
}

fn default_true() -> bool {
    true
}

/// Metadata-sidecar settings recorded for reproducibility.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetaSettings {
    /// Project capture contract; empty means no proof profile was supplied.
    #[serde(default)]
    pub required_keys: Vec<String>,
    /// Structured declarations, including their reasons and expected values.
    #[serde(default)]
    pub changes: Vec<crate::meta::DeclaredChange>,
    /// Sidecar file name (the per-image sidecar is `<stem>.<name>`).
    pub name: String,
    /// Whether an undeclared differing key fails the entry.
    pub required: bool,
    /// Keys whose difference is declared (expected) and therefore allowed.
    pub declared: Vec<String>,
    /// Effective ignore globs (built-in defaults plus `--meta-ignore`).
    pub ignored: Vec<String>,
}

impl Default for MetaSettings {
    fn default() -> Self {
        Self {
            required_keys: Vec::new(),
            changes: Vec::new(),
            name: crate::meta::DEFAULT_META_NAME.to_owned(),
            required: false,
            declared: Vec::new(),
            ignored: Vec::new(),
        }
    }
}

/// One metadata key whose value differs between the two sides.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaDiff {
    /// Sidecar key.
    pub key: String,
    /// Value on the baseline side, or `<absent>`.
    pub baseline: String,
    /// Value on the capture side, or `<absent>`.
    pub capture: String,
}

/// Purpose of a comparison run.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Captures checked against approved baselines (`saccade compare`).
    #[default]
    Regression,
    /// A candidate build checked for sameness against its parent
    /// (`saccade identity`): strict defaults, bit-identity reported.
    Identity,
}

/// Display names of the two compared sides.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Labels {
    /// Name of the reference side (FLIP reference). Default `"baseline"`.
    pub baseline: String,
    /// Name of the test side. Default `"capture"`.
    pub capture: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            baseline: "baseline".to_owned(),
            capture: "capture".to_owned(),
        }
    }
}

/// Result of one named region of interest of an entry.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionResult {
    /// Region name from the config.
    pub name: String,
    /// Rectangle in pixels after resolving the config fractions: `[x, y, w, h]`.
    pub rect_px: [u32; 4],
    /// `Pass`/`Fail` when the region has a threshold; `None` when informational.
    pub status: Option<Status>,
    /// Metric that decides `status`.
    pub metric_used: Metric,
    /// Threshold, when the region has one.
    pub threshold: Option<f64>,
    /// Value of `metric_used` over the region (masked pixels excluded); a
    /// fully masked region has no pixels and is `null` (read back as NaN).
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub value: f64,
    /// FLIP statistics over the region; `width`/`height` are the region's.
    pub metrics: Metrics,
}

/// HDR comparison settings actually used for an entry (HDR-FLIP).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrInfo {
    /// Tone mapper applied at each exposure (`"aces"`, `"hable"`, `"reinhard"`).
    pub tonemapper: String,
    /// First exposure, in stops.
    pub start_exposure: f32,
    /// Last exposure, in stops.
    pub stop_exposure: f32,
    /// Number of exposures evaluated.
    pub num_exposures: u32,
    /// Whether the exposure range was computed from the baseline (true) or given.
    pub auto_range: bool,
}

/// Per-status counts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    /// Mean FLIP error over all pixels.
    Mean,
    /// 95th-percentile FLIP error.
    P95,
    /// 99th-percentile FLIP error.
    P99,
    /// Maximum FLIP error.
    Max,
}

/// One image name's comparison result.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Per-region results (config `[[region]]`); a failing region fails the entry.
    #[serde(default)]
    pub regions: Vec<RegionResult>,
    /// Fraction of pixels excluded by masks; `None` when no mask applied.
    #[serde(default)]
    pub masked_fraction: Option<f64>,
    /// Whether the decoded pixels are exactly equal; `None` unless compared.
    #[serde(default)]
    pub bit_identical: Option<bool>,
    /// File-byte equality, separate from decoded-sample equality.
    #[serde(default)]
    pub file_bytes_identical: Option<bool>,
    /// Capture validity, independent of sample equality and perceptual error.
    #[serde(default)]
    pub capture_validity: crate::meta::CaptureValidity,
    /// Binary, source and capture identities from capture metadata or cost cards.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capture_provenance: BTreeMap<String, String>,
    /// Declared keys present on both sides whose values did not change.
    #[serde(default)]
    pub meta_declared_unchanged: Vec<String>,
    /// HDR-FLIP settings when the pair was compared as HDR.
    #[serde(default)]
    pub hdr: Option<HdrInfo>,
    /// Sidecar keys that differ between the sides (ignored keys excluded).
    #[serde(default)]
    pub meta_diff: Vec<MetaDiff>,
    /// Sidecar keys that differ but were ignored (timestamps, durations...),
    /// listed so an ignore glob that hides a real change stays visible.
    #[serde(default)]
    pub meta_ignored_diff: Vec<MetaDiff>,
    /// Structural checks on the baseline; `None` when there is no decodable baseline.
    #[serde(default)]
    pub baseline_properties: Option<Properties>,
    /// Non-fatal observations: an all-black or all-white image, non-finite
    /// samples, a local hotspot that failed an otherwise passing entry.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// SHA-256 (hex) of the baseline file as compared.
    #[serde(default)]
    pub baseline_sha256: Option<String>,
    /// SHA-256 (hex) of the capture file as compared.
    #[serde(default)]
    pub capture_sha256: Option<String>,
    /// Where the FLIP error is concentrated, largest first (see
    /// [`crate::hotspots`]); empty when none exceed the threshold, when the
    /// pair was not compared or when hotspots are disabled.
    #[serde(default)]
    pub hotspots: Vec<Hotspot>,
    /// Every unmasked pixel above the hotspot threshold, as row-major runs.
    /// Unlike `hotspots`, this detection mask has no display cap or share filter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_pixel_runs: Vec<[u32; 2]>,
    /// Per-hotspot error attributed to object/material IDs supplied with the capture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_attribution: Vec<crate::object_ids::Attribution>,
    /// Additive status: the deciding metric passes but a severe local change exists.
    #[serde(default)]
    pub pass_with_local_change: bool,
    /// Numerical comparison for non-colour buffers; FLIP metrics stay absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer: Option<crate::buffer::BufferResult>,
    /// Why the images differ: cause decomposition, sub-pixel shift, signed
    /// difference, non-finite map, timing pairs and a plain-English
    /// description (see [`crate::diagnostics`]). `None` when diagnostics are
    /// disabled or the pair was not compared as images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<crate::diagnostics::Diagnostics>,
}

/// One concentration of FLIP error on the frame, in pixels of the compared
/// images. Masked pixels never count.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hotspot {
    /// Exact row-major runs of pixels in this component: [start, length].
    /// Empty only for reports produced before exact-mask support.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pixel_runs: Vec<[u32; 2]>,
    /// Bounding box `[x, y, w, h]` in pixels.
    pub rect_px: [u32; 4],
    /// The same box as fractions of the frame, `[x, y, w, h]`.
    pub rect_frac: [f64; 4],
    /// Number of pixels above the hotspot threshold that make up the hotspot.
    pub area_px: u64,
    /// `area_px` as a fraction of the frame.
    pub area_frac: f64,
    /// Mean FLIP error over the unmasked pixels inside `rect_px`.
    pub mean_flip: f64,
    /// Largest FLIP error inside `rect_px`.
    pub max_flip: f64,
    /// Summed error of the hotspot's pixels over the summed error of every
    /// unmasked pixel of the frame, in `[0, 1]`.
    pub share_of_total_error: f64,
    /// Coarse 3x3 position of the box centre: `top-left`, `top-center`,
    /// `top-right`, `middle-left`, `center`, `middle-right`, `bottom-left`,
    /// `bottom-center`, `bottom-right`.
    pub position: String,
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Mean error.
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub mean: f64,
    /// Maximum error.
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub max: f64,
    /// Median error.
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub p50: f64,
    /// 95th-percentile error.
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub p95: f64,
    /// 99th-percentile error.
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub p99: f64,
    /// Fraction of pixels with error > 0.1.
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub frac_above_0_1: f64,
    /// Fraction of pixels with error > 0.5.
    #[serde(with = "finite_or_null")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<f64>"))]
    pub frac_above_0_5: f64,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Structural sanity checks on a single image (catches black frames, blowouts).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// Samples that are NaN (HDR images only; always 0 for 8-bit images).
    #[serde(default)]
    pub nan_count: u64,
    /// Samples that are infinite (HDR images only).
    #[serde(default)]
    pub inf_count: u64,
    /// Finite samples below zero (HDR images only).
    #[serde(default)]
    pub negative_count: u64,
}

/// Paths (relative to the report directory, `/`-separated) of the images the
/// report copies or generates. `None` when that image does not exist.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryPaths {
    /// Copy of the baseline image.
    pub baseline: Option<String>,
    /// Copy of the capture image.
    pub capture: Option<String>,
    /// FLIP error heatmap (magma colormap).
    pub heatmap: Option<String>,
    /// Signed luminance difference (blue darker, orange brighter), when
    /// diagnostics ran on a non-identical pair.
    #[serde(default)]
    pub signed_diff: Option<String>,
    /// Mask of NaN (magenta), infinite (yellow) and negative (cyan) samples of
    /// an HDR capture, when it has any.
    #[serde(default)]
    pub nonfinite_mask: Option<String>,
}

impl Entry {
    /// Describe the strongest recorded local-change finding.
    pub fn local_hotspot_note(&self) -> Option<String> {
        if !self.pass_with_local_change {
            return None;
        }
        Some(format!(
            "pass with local change: severe component in the full error map (max FLIP {:.2}); inspect the heatmap",
            self.metrics.as_ref().map_or(0.0, |m| m.max)
        ))
    }
}

impl Report {
    /// Number of pairs actually compared: entries with FLIP metrics (a pair
    /// that errored only on its sidecars still counts).
    pub fn compared(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| {
                e.metrics.is_some()
                    || e.value.is_some()
                    || (self.config.mode == Mode::Identity && e.bit_identical.is_some())
            })
            .count()
    }

    /// Whether the run compared nothing: an empty scope cannot establish evidence.
    pub fn is_empty_run(&self) -> bool {
        self.compared() == 0
    }

    /// Whether this run should exit non-zero: any fail, error or missing entry,
    /// any new entry when required, or an empty scope. Identity also requires
    /// exact samples and complete pairing.
    pub fn is_regression(&self) -> bool {
        let t = &self.totals;
        t.fail > 0
            || t.error > 0
            || t.missing > 0
            || ((self.config.fail_on_new || self.config.mode == Mode::Identity) && t.new > 0)
            || self.is_empty_run()
            || (self.config.mode == Mode::Identity && self.sample_equality() != Some(true))
    }

    /// Exact sample equality across every selected entry; incomplete evidence is unknown.
    pub fn sample_equality(&self) -> Option<bool> {
        if self.entries.iter().any(|e| e.bit_identical == Some(false)) {
            Some(false)
        } else if !self.entries.is_empty()
            && self.entries.iter().all(|e| e.bit_identical == Some(true))
        {
            Some(true)
        } else {
            None
        }
    }

    /// Aggregate capture validity; invalid evidence wins over missing context.
    pub fn capture_validity(&self) -> crate::meta::CaptureValidity {
        use crate::meta::{CaptureValidity, Validity};
        let status = if self
            .entries
            .iter()
            .any(|e| e.capture_validity.status == Validity::Invalid)
        {
            Validity::Invalid
        } else if self.entries.is_empty()
            || self
                .entries
                .iter()
                .any(|e| e.capture_validity.status == Validity::Unknown)
        {
            Validity::Unknown
        } else {
            Validity::Valid
        };
        let mut reasons: Vec<String> = self
            .entries
            .iter()
            .flat_map(|e| {
                e.capture_validity
                    .reasons
                    .iter()
                    .map(move |r| format!("{}: {r}", e.name))
            })
            .collect();
        if self.entries.is_empty() {
            reasons.push("selected scope is empty".into());
        }
        CaptureValidity { status, reasons }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn fully_masked_region_value_round_trips() {
        let m = Metrics {
            mean: f64::NAN,
            max: f64::NAN,
            p50: f64::NAN,
            p95: f64::NAN,
            p99: f64::NAN,
            frac_above_0_1: f64::NAN,
            frac_above_0_5: f64::NAN,
            width: 1,
            height: 1,
        };
        let r = RegionResult {
            name: "hud".into(),
            rect_px: [0, 0, 1, 1],
            status: None,
            metric_used: Metric::Mean,
            threshold: None,
            value: f64::NAN,
            metrics: m,
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let back: RegionResult = serde_json::from_str(&json).expect("deserialize");
        assert!(back.value.is_nan());
    }
}
