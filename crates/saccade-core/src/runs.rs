//! Run overview: compare whole runs (directories of images) against a
//! reference run and summarise the result as a matrix.
//!
//! [`plan`] pairs the images of every run with the reference's (by relative
//! name, by sorted position, or by an explicit mapping), [`compute_pair`]
//! measures one pair (bit-identity first, then FLIP; results are cached on
//! disk by file identity), and [`assemble`] builds the `saccade-runs.v1`
//! model that the overview page, `GET /api/runs`, `saccade runs` and the MCP
//! tool `saccade_compare_runs` all share. [`render_page`] and [`write_static`]
//! produce the self-contained HTML page.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::compare::{CompareOptions, compare_rgba, native_samples_identical};
use crate::error::{Error, Result};
use crate::hdr::{HdrConfig, Tonemapper};
use crate::meta::{ABSENT, MetaOptions};
use crate::report::Metrics;
use crate::run::{collect_images, decode, io_err, sha256_file};

/// Schema identifier of the overview model.
pub const RUNS_SCHEMA: &str = "saccade-runs.v1";

/// File the static overview writes first: the marker that makes its output
/// directory recognisably saccade's own.
pub const RUNS_MARKER_FILE: &str = "saccade-runs.v1.json";

/// Fake image name used to load a run's directory-level sidecar.
const RUN_PROBE: &str = ".saccade-run";
/// Longest edge of a matrix thumbnail, in pixels.
pub const THUMB_EDGE: u32 = 320;
/// Longest edge of a contact-sheet preview, in pixels.
pub const PREVIEW_EDGE: u32 = 640;
/// Longest edge of a cached mini heatmap, in pixels.
const HEAT_EDGE: u32 = 240;
/// Most runs an overview compares against the reference.
pub const MAX_RUNS: usize = 5;

const PAGE: &str = include_str!("../assets/runs.html");
const SERVE_CSS: &str = include_str!("../assets/serve.css");
const RUNS_CSS: &str = include_str!("../assets/runs.css");
const RUNS_JS: &str = include_str!("../assets/runs.js");

/// How the images of a run are matched with the reference's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Pairing {
    /// Same relative path.
    #[default]
    Name,
    /// The n-th image of the run (sorted by name) with the n-th of the reference.
    Position,
    /// Explicit `(reference index, run index)` pairs into the sorted name lists.
    Manual(Vec<(usize, usize)>),
}

impl Pairing {
    /// Parses `name`, `position` or `manual:<ref>-<run>,<ref>-<run>...`.
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        match s {
            "" | "name" => Ok(Self::Name),
            "position" => Ok(Self::Position),
            _ => {
                let list = s
                    .strip_prefix("manual:")
                    .ok_or_else(|| format!("unknown pairing {s:?}"))?;
                let mut out = Vec::new();
                for part in list.split(',').filter(|p| !p.is_empty()) {
                    let (a, b) = part
                        .split_once('-')
                        .ok_or_else(|| format!("bad pair {part:?}, expected <ref>-<run>"))?;
                    let idx = |t: &str| {
                        t.parse::<usize>()
                            .map_err(|_| format!("bad index in pair {part:?}"))
                    };
                    out.push((idx(a)?, idx(b)?));
                }
                Ok(Self::Manual(out))
            }
        }
    }

    /// The name used in the model.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Position => "position",
            Self::Manual(_) => "manual",
        }
    }
}

/// One run to compare.
#[derive(Debug, Clone)]
pub struct RunInput {
    /// The directory of images.
    pub dir: PathBuf,
    /// Short name shown in the matrix header.
    pub label: String,
    /// How the shown path reads (an archive-relative path under `serve`).
    pub display: String,
    /// How its images are matched with the reference's.
    pub pairing: Pairing,
}

/// Settings of an overview.
#[derive(Debug, Clone)]
pub struct RunsOptions {
    /// FLIP pixels per degree.
    pub pixels_per_degree: f32,
    /// HDR-FLIP settings.
    pub hdr: HdrConfig,
    /// Sidecar settings (run-level config differences).
    pub meta: MetaOptions,
}

impl Default for RunsOptions {
    fn default() -> Self {
        Self {
            pixels_per_degree: CompareOptions::default().pixels_per_degree,
            hdr: HdrConfig::default(),
            meta: MetaOptions::default(),
        }
    }
}

impl RunsOptions {
    fn compare(&self) -> CompareOptions {
        CompareOptions {
            pixels_per_degree: self.pixels_per_degree,
            hdr: self.hdr,
        }
    }
}

/// Labels for `dirs`: the directory names, made unique with `-2`, `-3`.
pub fn unique_labels(dirs: &[PathBuf]) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for (i, d) in dirs.iter().enumerate() {
        let base = d.file_name().map_or_else(
            || format!("run{}", i + 1),
            |n| n.to_string_lossy().into_owned(),
        );
        let mut label = base.clone();
        let mut n = 2;
        while labels.contains(&label) {
            label = format!("{base}-{n}");
            n += 1;
        }
        labels.push(label);
    }
    labels
}

/// One image pair to measure.
#[derive(Debug, Clone)]
pub struct PairTask {
    /// Index of the run (not counting the reference).
    pub run: usize,
    /// Name of the image in the reference run (the matrix row).
    pub ref_name: String,
    /// Name of the image in the compared run.
    pub run_name: String,
    /// Reference image file.
    pub ref_path: PathBuf,
    /// Compared image file.
    pub run_path: PathBuf,
}

/// The images of one run, matched with the reference's.
#[derive(Debug)]
pub struct RunPlan {
    input: RunInput,
    files: BTreeMap<String, PathBuf>,
    /// `(reference name, run name)`.
    pairs: Vec<(String, String)>,
    only_ref: Vec<String>,
    only_run: Vec<String>,
    name_matches: usize,
    task_base: usize,
    meta: std::result::Result<Option<crate::meta::Meta>, String>,
}

/// Everything an overview compares, resolved on disk.
#[derive(Debug)]
pub struct Plan {
    ref_input: RunInput,
    ref_files: BTreeMap<String, PathBuf>,
    ref_meta: std::result::Result<Option<crate::meta::Meta>, String>,
    runs: Vec<RunPlan>,
    opts: RunsOptions,
}

fn sorted_names(files: &BTreeMap<String, PathBuf>) -> Vec<String> {
    files.keys().cloned().collect()
}

/// Resolves the images of the reference and of every run and pairs them.
///
/// Errors when a directory cannot be read, a run list is empty or has more
/// than [`MAX_RUNS`] entries, or labels collide.
pub fn plan(reference: &RunInput, runs: &[RunInput], opts: &RunsOptions) -> Result<Plan> {
    if runs.is_empty() || runs.len() > MAX_RUNS {
        return Err(Error::Config(format!(
            "an overview compares 1 to {MAX_RUNS} runs against the reference, got {}",
            runs.len()
        )));
    }
    let mut seen = BTreeSet::new();
    for l in std::iter::once(&reference.label).chain(runs.iter().map(|r| &r.label)) {
        if l.is_empty() || !seen.insert(l.clone()) {
            return Err(Error::Config(format!(
                "run labels must be non-empty and unique (got {l:?})"
            )));
        }
    }
    let checker = opts.meta.checker()?;
    let ref_files = collect_images(&reference.dir)?.files;
    let ref_names = sorted_names(&ref_files);
    let ref_meta = checker.load(&reference.dir, RUN_PROBE);
    let mut out = Vec::with_capacity(runs.len());
    let mut task_base = 0;
    for input in runs {
        let files = collect_images(&input.dir)?.files;
        let run_names = sorted_names(&files);
        let name_matches = ref_names.iter().filter(|n| files.contains_key(*n)).count();
        let index_pairs: Vec<(usize, usize)> = match &input.pairing {
            Pairing::Name => ref_names
                .iter()
                .enumerate()
                .filter_map(|(i, n)| run_names.iter().position(|r| r == n).map(|j| (i, j)))
                .collect(),
            Pairing::Position => (0..ref_names.len().min(run_names.len()))
                .map(|i| (i, i))
                .collect(),
            Pairing::Manual(list) => {
                let (mut used_r, mut used_c) = (BTreeSet::new(), BTreeSet::new());
                list.iter()
                    .filter(|(i, j)| {
                        *i < ref_names.len()
                            && *j < run_names.len()
                            && used_r.insert(*i)
                            && used_c.insert(*j)
                    })
                    .copied()
                    .collect()
            }
        };
        let paired_ref: BTreeSet<usize> = index_pairs.iter().map(|p| p.0).collect();
        let paired_run: BTreeSet<usize> = index_pairs.iter().map(|p| p.1).collect();
        let pairs: Vec<(String, String)> = index_pairs
            .iter()
            .map(|&(i, j)| (ref_names[i].clone(), run_names[j].clone()))
            .collect();
        let only_ref = ref_names
            .iter()
            .enumerate()
            .filter(|(i, _)| !paired_ref.contains(i))
            .map(|(_, n)| n.clone())
            .collect();
        let only_run = run_names
            .iter()
            .enumerate()
            .filter(|(j, _)| !paired_run.contains(j))
            .map(|(_, n)| n.clone())
            .collect();
        let n = pairs.len();
        out.push(RunPlan {
            meta: checker.load(&input.dir, RUN_PROBE),
            input: input.clone(),
            files,
            pairs,
            only_ref,
            only_run,
            name_matches,
            task_base,
        });
        task_base += n;
    }
    Ok(Plan {
        ref_input: reference.clone(),
        ref_files,
        ref_meta,
        runs: out,
        opts: opts.clone(),
    })
}

impl Plan {
    /// Every pair to measure, in a stable order; [`assemble`] takes results
    /// aligned with it.
    pub fn tasks(&self) -> Vec<PairTask> {
        let mut out = Vec::new();
        for (k, run) in self.runs.iter().enumerate() {
            for (rn, cn) in &run.pairs {
                if let (Some(rp), Some(cp)) = (self.ref_files.get(rn), run.files.get(cn)) {
                    out.push(PairTask {
                        run: k,
                        ref_name: rn.clone(),
                        run_name: cn.clone(),
                        ref_path: rp.clone(),
                        run_path: cp.clone(),
                    });
                }
            }
        }
        out
    }

    /// The pairs of run `k` as `(reference name, reference file, run file)`.
    pub fn paired_files(&self, k: usize) -> Vec<(String, PathBuf, PathBuf)> {
        let Some(run) = self.runs.get(k) else {
            return Vec::new();
        };
        run.pairs
            .iter()
            .filter_map(|(rn, cn)| {
                Some((
                    rn.clone(),
                    self.ref_files.get(rn)?.clone(),
                    run.files.get(cn)?.clone(),
                ))
            })
            .collect()
    }

    /// A digest of everything the result depends on: image files (name, size,
    /// modification time), pairing and measurement settings.
    pub fn fingerprint(&self) -> String {
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let stat = |p: &Path| {
            std::fs::metadata(p).map_or_else(
                |_| String::new(),
                |m| format!("{}:{:?}", m.len(), m.modified().ok()),
            )
        };
        for t in self.tasks() {
            parts.push(
                format!(
                    "{}|{}|{}|{}|{}",
                    t.run,
                    t.ref_path.display(),
                    stat(&t.ref_path),
                    t.run_path.display(),
                    stat(&t.run_path)
                )
                .into_bytes(),
            );
        }
        for run in &self.runs {
            parts.push(
                format!("{}|{:?}|{:?}", run.input.label, run.only_ref, run.only_run).into_bytes(),
            );
        }
        parts.push(
            format!(
                "{}|{:?}|{}",
                self.opts.pixels_per_degree,
                self.opts.hdr,
                env!("CARGO_PKG_VERSION")
            )
            .into_bytes(),
        );
        digest(&parts.iter().map(Vec::as_slice).collect::<Vec<_>>())
    }
}

/// 32 hex characters of SHA-256 over the parts (a cache key).
pub(crate) fn digest(parts: &[&[u8]]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p);
    }
    h.finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// How one pair compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairStatus {
    /// The decoded samples are bit-identical.
    Identical,
    /// They differ; `metrics` holds the FLIP statistics.
    Changed,
    /// The pair could not be compared (size mismatch, undecodable image).
    Error,
}

/// The measured result of one pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairResult {
    /// Identical, changed or error.
    pub status: PairStatus,
    /// FLIP statistics of a changed pair.
    pub metrics: Option<Metrics>,
    /// Cache key of the mini heatmap (`<key>.heat.png` in the cache directory).
    pub heat: Option<String>,
    /// Why the pair could not be compared.
    pub error: Option<String>,
}

fn error_result(e: impl ToString) -> PairResult {
    PairResult {
        status: PairStatus::Error,
        metrics: None,
        heat: None,
        error: Some(e.to_string()),
    }
}

fn pair_key(t: &PairTask, opts: &RunsOptions) -> String {
    let stat = |p: &Path| {
        std::fs::metadata(p).map_or_else(
            |_| String::new(),
            |m| format!("{}:{:?}", m.len(), m.modified().ok()),
        )
    };
    digest(&[
        t.ref_path.to_string_lossy().as_bytes(),
        stat(&t.ref_path).as_bytes(),
        t.run_path.to_string_lossy().as_bytes(),
        stat(&t.run_path).as_bytes(),
        format!(
            "{}|{:?}|{}",
            opts.pixels_per_degree,
            opts.hdr,
            env!("CARGO_PKG_VERSION")
        )
        .as_bytes(),
    ])
}

fn measure(t: &PairTask, opts: &RunsOptions, key: &str, cache: Option<&Path>) -> PairResult {
    let same_bytes = matches!(
        (sha256_file(&t.ref_path), sha256_file(&t.run_path)),
        (Ok(a), Ok(b)) if a == b
    );
    if same_bytes || native_samples_identical(&t.ref_path, &t.run_path) {
        return PairResult {
            status: PairStatus::Identical,
            metrics: None,
            heat: None,
            error: None,
        };
    }
    let copts = opts.compare();
    let cmp = match (
        crate::hdr::is_hdr_path(&t.ref_path),
        crate::hdr::is_hdr_path(&t.run_path),
    ) {
        (true, true) => {
            match (
                crate::hdr::decode_hdr(&t.run_path),
                crate::hdr::decode_hdr(&t.ref_path),
            ) {
                (Ok(c), Ok(r)) => crate::hdr::compare_hdr(&c, &r, &copts).map(|(c, _)| c),
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        (false, false) => match (decode(&t.run_path), decode(&t.ref_path)) {
            (Ok(c), Ok(r)) => compare_rgba(&c, &r, &copts),
            (Err(e), _) | (_, Err(e)) => Err(e),
        },
        _ => Err(Error::HdrMismatch(
            "one of the images is HDR and the other is LDR".into(),
        )),
    };
    let cmp = match cmp {
        Ok(c) => c,
        Err(e) => return error_result(e),
    };
    let mut heat = None;
    if let Some(dir) = cache {
        let rgb = cmp.heatmap_rgb();
        let (w, h) = rgb.dimensions();
        let scale = (f64::from(HEAT_EDGE) / f64::from(w.max(h).max(1))).min(1.0);
        let small = image::imageops::thumbnail(
            &rgb,
            ((f64::from(w) * scale).round() as u32).max(1),
            ((f64::from(h) * scale).round() as u32).max(1),
        );
        let dest = dir.join(format!("{key}.heat.png"));
        if small
            .save_with_format(&dest, image::ImageFormat::Png)
            .is_ok()
        {
            heat = Some(key.to_owned());
        }
    }
    PairResult {
        status: PairStatus::Changed,
        metrics: Some(cmp.metrics),
        heat,
        error: None,
    }
}

/// Measures one pair, reading and writing the on-disk cache `cache` (a
/// directory; `None` measures without caching and without a heatmap).
pub fn compute_pair(t: &PairTask, opts: &RunsOptions, cache: Option<&Path>) -> PairResult {
    let key = pair_key(t, opts);
    if let Some(dir) = cache {
        let file = dir.join(format!("{key}.json"));
        if let Some(r) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|j| serde_json::from_str::<PairResult>(&j).ok())
        {
            let heat_ok = r
                .heat
                .as_ref()
                .is_none_or(|h| dir.join(format!("{h}.heat.png")).is_file());
            if heat_ok {
                return r;
            }
        }
    }
    let result = measure(t, opts, &key, cache);
    if let (Some(dir), true) = (cache, result.status != PairStatus::Error) {
        let tmp = dir.join(format!(".{key}.{}.tmp", std::process::id()));
        let file = dir.join(format!("{key}.json"));
        if serde_json::to_string(&result)
            .ok()
            .is_some_and(|j| std::fs::write(&tmp, j).is_ok())
        {
            let _ = std::fs::rename(&tmp, &file);
        }
    }
    result
}

/// Measures every pair of `plan` in parallel, calling `sink(index, result)`
/// as each finishes (indices follow [`Plan::tasks`]).
pub fn compute_all(plan: &Plan, cache: Option<&Path>, sink: &(dyn Fn(usize, PairResult) + Sync)) {
    if let Some(dir) = cache {
        let _ = std::fs::create_dir_all(dir);
    }
    let tasks = plan.tasks();
    tasks.par_iter().enumerate().for_each(|(i, t)| {
        sink(i, compute_pair(t, &plan.opts, cache));
    });
}

/// URLs of the images an overview shows. The server maps them to its image
/// routes; the static page writes the files.
pub trait AssetUrls {
    /// URL of a thumbnail of the image at `path` whose longest edge is `edge`.
    fn image(&self, path: &Path, edge: u32) -> Option<String>;
    /// URL of the mini heatmap with cache key `key`.
    fn heat(&self, key: &str) -> Option<String>;
}

/// No images: the model carries values only (CLI JSON, MCP).
pub struct NoAssets;

impl AssetUrls for NoAssets {
    fn image(&self, _: &Path, _: u32) -> Option<String> {
        None
    }
    fn heat(&self, _: &str) -> Option<String> {
        None
    }
}

/// An image of a run, with thumbnails.
#[derive(Debug, Clone, Serialize)]
pub struct ImageRef {
    /// Relative image name.
    pub name: String,
    /// Small thumbnail URL.
    pub thumb: Option<String>,
    /// Larger preview URL (contact sheet).
    pub preview: Option<String>,
}

/// The reference run.
#[derive(Debug, Clone, Serialize)]
pub struct RefRun {
    /// Label.
    pub label: String,
    /// Shown path.
    pub path: String,
    /// Its images, sorted by name (a manual pairing indexes into this list).
    pub images: Vec<ImageRef>,
}

/// The worst changed image of a run.
#[derive(Debug, Clone, Serialize)]
pub struct Worst {
    /// Image name (the matrix row).
    pub name: String,
    /// Its mean FLIP.
    pub mean: f64,
}

/// One compared run's summary against the reference.
#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    /// Label.
    pub label: String,
    /// Shown path.
    pub path: String,
    /// `name`, `position` or `manual`.
    pub pairing: String,
    /// Number of images in the run.
    pub images: usize,
    /// Bit-identical to the reference.
    pub identical: usize,
    /// Differing from the reference.
    pub changed: usize,
    /// Pairs that could not be compared.
    pub errors: usize,
    /// Pairs not measured yet.
    pub pending: usize,
    /// Reference images with no counterpart in the run.
    pub only_in_ref: usize,
    /// Run images with no counterpart in the reference.
    pub only_in_run: usize,
    /// Reference names the run also has (whatever the pairing).
    pub name_matches: usize,
    /// Few or no file names match: the run probably needs another pairing.
    pub mismatch: bool,
    /// Largest mean FLIP among the changed images.
    pub worst: Option<Worst>,
    /// Every image is bit-identical to the reference's and none is missing or
    /// extra: the run changed nothing.
    pub no_visible_effect: bool,
    /// Run-level sidecar keys that differ from the reference's.
    pub config_differs: Vec<String>,
    /// Why a run-level sidecar could not be read.
    pub config_error: Option<String>,
    /// The run's own images (only when `mismatch`, for manual pairing).
    pub run_images: Vec<ImageRef>,
    /// One-line summary.
    pub summary: String,
}

/// A run-level sidecar key that differs between runs.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigDiff {
    /// Sidecar key.
    pub key: String,
    /// One value per run, reference first (the reference's value when the run
    /// does not differ).
    pub values: Vec<String>,
}

/// One compared image in the matrix.
#[derive(Debug, Clone, Serialize)]
pub struct Cell {
    /// `identical`, `changed`, `error`, `pending`, `only_in_ref` or `only_in_run`.
    pub status: &'static str,
    /// The run's image name when it is not the row's (position/manual pairing).
    pub run_name: Option<String>,
    /// Thumbnail of the run's image.
    pub thumb: Option<String>,
    /// Preview of the run's image.
    pub preview: Option<String>,
    /// FLIP statistics of a changed pair.
    pub metrics: Option<Metrics>,
    /// Mini heatmap URL.
    pub heat: Option<String>,
    /// Why the pair could not be compared.
    pub error: Option<String>,
}

/// One matrix row: an image and its cell in every run.
#[derive(Debug, Clone, Serialize)]
pub struct ImageRow {
    /// Image name.
    pub name: String,
    /// The reference's image, absent for an image only some run has.
    pub ref_image: Option<ImageRef>,
    /// Largest mean FLIP of the row.
    pub worst: Option<f64>,
    /// One cell per run, in run order.
    pub cells: Vec<Cell>,
}

/// Measurement progress.
#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    /// Pairs measured.
    pub done: usize,
    /// Pairs to measure.
    pub total: usize,
    /// `done == total`.
    pub complete: bool,
}

/// The `saccade-runs.v1` overview.
#[derive(Debug, Clone, Serialize)]
pub struct RunsModel {
    /// Always [`RUNS_SCHEMA`].
    pub schema: String,
    /// `saccade` version.
    pub tool_version: String,
    /// The reference run.
    #[serde(rename = "ref")]
    pub reference: RefRun,
    /// The compared runs.
    pub runs: Vec<RunSummary>,
    /// Run-level config differences, shown once at the top.
    pub config_differences: Vec<ConfigDiff>,
    /// The matrix rows, sorted by name.
    pub images: Vec<ImageRow>,
    /// How much is measured.
    pub progress: Progress,
}

/// The one-line summary of a run, as the page shows it.
pub fn summary_line(r: &RunSummary) -> String {
    let mut parts = vec![format!("{} identical", r.identical)];
    let mut changed = format!("{} changed", r.changed);
    if let Some(w) = &r.worst {
        changed.push_str(&format!(" (worst {} {:.3})", w.name, w.mean));
    }
    parts.push(changed);
    parts.push(format!("{} only-in-ref", r.only_in_ref));
    parts.push(format!("{} only-in-run", r.only_in_run));
    if r.errors > 0 {
        parts.push(format!("{} errors", r.errors));
    }
    if r.pending > 0 {
        parts.push(format!("{} pending", r.pending));
    }
    parts.push(format!("config differs: {} keys", r.config_differs.len()));
    parts.join(" · ")
}

/// Builds the model from the plan and the results measured so far
/// (`results[i]` belongs to `plan.tasks()[i]`; `None` is pending).
pub fn assemble(plan: &Plan, results: &[Option<PairResult>], urls: &dyn AssetUrls) -> RunsModel {
    let image_ref = |name: &str, path: &Path| ImageRef {
        name: name.to_owned(),
        thumb: urls.image(path, THUMB_EDGE),
        preview: urls.image(path, PREVIEW_EDGE),
    };
    let ref_names = sorted_names(&plan.ref_files);
    let ref_images: Vec<ImageRef> = ref_names
        .iter()
        .map(|n| image_ref(n, &plan.ref_files[n]))
        .collect();
    let ref_set: BTreeSet<&String> = ref_names.iter().collect();

    // Matrix rows: reference names first, then images only some run has.
    let mut rows: BTreeMap<String, ImageRow> = ref_names
        .iter()
        .zip(&ref_images)
        .map(|(n, im)| {
            (
                n.clone(),
                ImageRow {
                    name: n.clone(),
                    ref_image: Some(im.clone()),
                    worst: None,
                    cells: Vec::new(),
                },
            )
        })
        .collect();
    let mut extra_rows: BTreeMap<String, ImageRow> = BTreeMap::new();
    let blank = |status: &'static str| Cell {
        status,
        run_name: None,
        thumb: None,
        preview: None,
        metrics: None,
        heat: None,
        error: None,
    };
    let mut cells: Vec<BTreeMap<String, Cell>> = Vec::new();
    let mut extra_cells: Vec<BTreeMap<String, Cell>> = Vec::new();
    let mut summaries = Vec::new();
    let done = results.iter().flatten().count();
    let total = results.len();

    for run in &plan.runs {
        let mut mine: BTreeMap<String, Cell> = BTreeMap::new();
        let mut s = RunSummary {
            label: run.input.label.clone(),
            path: run.input.display.clone(),
            pairing: run.input.pairing.name().to_owned(),
            images: run.files.len(),
            identical: 0,
            changed: 0,
            errors: 0,
            pending: 0,
            only_in_ref: run.only_ref.len(),
            only_in_run: run.only_run.len(),
            name_matches: run.name_matches,
            mismatch: false,
            worst: None,
            no_visible_effect: false,
            config_differs: Vec::new(),
            config_error: None,
            run_images: Vec::new(),
            summary: String::new(),
        };
        for (i, (rn, cn)) in run.pairs.iter().enumerate() {
            let Some(cp) = run.files.get(cn) else {
                continue;
            };
            let mut cell = blank("pending");
            cell.run_name = (cn != rn).then(|| cn.clone());
            cell.thumb = urls.image(cp, THUMB_EDGE);
            cell.preview = urls.image(cp, PREVIEW_EDGE);
            match results.get(run.task_base + i).and_then(Option::as_ref) {
                None => s.pending += 1,
                Some(r) => {
                    cell.error.clone_from(&r.error);
                    match r.status {
                        PairStatus::Identical => {
                            cell.status = "identical";
                            s.identical += 1;
                        }
                        PairStatus::Error => {
                            cell.status = "error";
                            s.errors += 1;
                        }
                        PairStatus::Changed => {
                            cell.status = "changed";
                            s.changed += 1;
                            cell.metrics = r.metrics;
                            cell.heat = r.heat.as_deref().and_then(|h| urls.heat(h));
                            if let Some(m) = r.metrics.filter(|m| m.mean.is_finite()) {
                                if s.worst.as_ref().is_none_or(|w| m.mean > w.mean) {
                                    s.worst = Some(Worst {
                                        name: rn.clone(),
                                        mean: m.mean,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            mine.insert(rn.clone(), cell);
        }
        for rn in &run.only_ref {
            mine.insert(rn.clone(), blank("only_in_ref"));
        }
        let mut extras: BTreeMap<String, Cell> = BTreeMap::new();
        for cn in &run.only_run {
            let mut cell = blank("only_in_run");
            if let Some(cp) = run.files.get(cn) {
                cell.thumb = urls.image(cp, THUMB_EDGE);
                cell.preview = urls.image(cp, PREVIEW_EDGE);
            }
            let key = if ref_set.contains(cn) {
                format!("{cn} (unpaired)")
            } else {
                cn.clone()
            };
            extras.insert(key, cell);
        }
        let smaller = run.files.len().min(plan.ref_files.len());
        s.mismatch = smaller > 0 && run.name_matches * 2 < smaller;
        if s.mismatch {
            s.run_images = sorted_names(&run.files)
                .iter()
                .map(|n| image_ref(n, &run.files[n]))
                .collect();
        }
        s.no_visible_effect = s.pending == 0
            && s.errors == 0
            && s.changed == 0
            && s.identical > 0
            && s.only_in_ref == 0
            && s.only_in_run == 0;
        cells.push(mine);
        extra_cells.push(extras);
        summaries.push(s);
    }

    // Run-level configuration: the keys that differ from the reference's.
    let checker = plan.opts.meta.checker().ok();
    let mut key_order: BTreeSet<String> = BTreeSet::new();
    let mut per_run: Vec<BTreeMap<String, (String, String)>> = Vec::new();
    for (run, s) in plan.runs.iter().zip(summaries.iter_mut()) {
        let mut diffs = BTreeMap::new();
        match (&plan.ref_meta, &run.meta, &checker) {
            (Ok(rm), Ok(cm), Some(c)) => {
                for d in c.diff(rm.as_ref(), cm.as_ref()) {
                    key_order.insert(d.key.clone());
                    s.config_differs.push(d.key.clone());
                    diffs.insert(d.key, (d.baseline, d.capture));
                }
            }
            (Err(e), _, _) | (_, Err(e), _) => s.config_error = Some(e.clone()),
            _ => {}
        }
        per_run.push(diffs);
    }
    let config_differences: Vec<ConfigDiff> = key_order
        .into_iter()
        .map(|key| {
            let base = per_run
                .iter()
                .find_map(|m| m.get(&key).map(|v| v.0.clone()))
                .unwrap_or_else(|| ABSENT.to_owned());
            let mut values = vec![base.clone()];
            for m in &per_run {
                values.push(m.get(&key).map_or_else(|| base.clone(), |v| v.1.clone()));
            }
            ConfigDiff { key, values }
        })
        .collect();

    for s in &mut summaries {
        s.summary = summary_line(s);
    }
    for (name, row) in &mut rows {
        for c in &mut cells {
            row.cells
                .push(c.remove(name).unwrap_or_else(|| blank("only_in_ref")));
        }
    }
    for (k, extras) in extra_cells.iter().enumerate() {
        for (key, cell) in extras {
            let row = extra_rows.entry(key.clone()).or_insert_with(|| ImageRow {
                name: key.clone(),
                ref_image: None,
                worst: None,
                cells: (0..plan.runs.len()).map(|_| blank("absent")).collect(),
            });
            row.cells[k] = cell.clone();
        }
    }
    let mut images: Vec<ImageRow> = rows.into_values().chain(extra_rows.into_values()).collect();
    for row in &mut images {
        row.worst = row
            .cells
            .iter()
            .filter_map(|c| c.metrics.map(|m| m.mean))
            .filter(|m| m.is_finite())
            .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.max(v))));
    }
    RunsModel {
        schema: RUNS_SCHEMA.to_owned(),
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        reference: RefRun {
            label: plan.ref_input.label.clone(),
            path: plan.ref_input.display.clone(),
            images: ref_images,
        },
        runs: summaries,
        config_differences,
        images,
        progress: Progress {
            done,
            total,
            complete: done == total,
        },
    }
}

/// Measures everything and returns the finished model (no caching): the
/// blocking entry point for the CLI and the MCP tool.
pub fn overview(
    reference: &RunInput,
    runs: &[RunInput],
    opts: &RunsOptions,
    cache: Option<&Path>,
    urls: &dyn AssetUrls,
) -> Result<RunsModel> {
    let plan = plan(reference, runs, opts)?;
    let results: std::sync::Mutex<Vec<Option<PairResult>>> =
        std::sync::Mutex::new(vec![None; plan.tasks().len()]);
    compute_all(&plan, cache, &|i, r| {
        if let Ok(mut v) = results.lock() {
            v[i] = Some(r);
        }
    });
    let results = results.into_inner().unwrap_or_default();
    Ok(assemble(&plan, &results, urls))
}

/// Writes a downscaled PNG of the image at `src` (HDR images tone-mapped)
/// whose longest edge is at most `edge`.
pub(crate) fn write_thumbnail(
    src: &Path,
    dest: &Path,
    edge: u32,
    tm: Tonemapper,
) -> std::result::Result<(), String> {
    let rgba = if crate::hdr::is_hdr_path(src) {
        let hdr = crate::hdr::decode_hdr(src).map_err(|e| e.to_string())?;
        image::DynamicImage::ImageRgb8(crate::hdr::display_image(&hdr, tm)).to_rgba8()
    } else {
        decode(src).map_err(|e| e.to_string())?
    };
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err("image is empty".into());
    }
    let scale = (f64::from(edge) / f64::from(w.max(h))).min(1.0);
    let (tw, th) = (
        ((f64::from(w) * scale).round() as u32).max(1),
        ((f64::from(h) * scale).round() as u32).max(1),
    );
    let thumb = image::imageops::thumbnail(&rgba, tw, th);
    let tmp = dest.with_extension(format!("tmp{}", std::process::id()));
    thumb
        .save_with_format(&tmp, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

/// The overview page: the shared template with `config` (a JSON object the
/// page script reads as `SACCADE_RUNS`).
pub fn render_page(config: &str) -> String {
    // `</` would end the inline script element early.
    let safe = config.replace("</", "<\\/");
    PAGE.replace(
        "/*__RUNS_CSS__*/",
        &crate::render::shared::page_css(&[SERVE_CSS, RUNS_CSS]),
    )
    .replace(
        "/*__RUNS_JS__*/",
        &crate::render::shared::page_js(&[RUNS_JS]),
    )
    .replace("__RUNS_PAGE__", &safe)
}

struct StaticAssets<'a> {
    out: &'a Path,
    cache: &'a Path,
    tm: Tonemapper,
}

impl AssetUrls for StaticAssets<'_> {
    fn image(&self, path: &Path, edge: u32) -> Option<String> {
        let stat = std::fs::metadata(path)
            .map(|m| format!("{}:{:?}", m.len(), m.modified().ok()))
            .unwrap_or_default();
        let key = digest(&[path.to_string_lossy().as_bytes(), stat.as_bytes()]);
        let rel = format!("thumbs/{key}-{edge}.png");
        let dest = self.out.join(&rel);
        if dest.is_file() || write_thumbnail(path, &dest, edge, self.tm).is_ok() {
            Some(rel)
        } else {
            None
        }
    }

    fn heat(&self, key: &str) -> Option<String> {
        let rel = format!("heat/{key}.png");
        std::fs::copy(
            self.cache.join(format!("{key}.heat.png")),
            self.out.join(&rel),
        )
        .ok()
        .map(|_| rel)
    }
}

/// Measures the runs and writes the static overview to `out`:
/// `index.html`, `saccade-runs.v1.json`, `thumbs/` and `heat/`. `cache` holds
/// the measured pairs (shared with `saccade serve`).
pub fn write_static(
    reference: &RunInput,
    runs: &[RunInput],
    opts: &RunsOptions,
    cache: &Path,
    out: &Path,
) -> Result<RunsModel> {
    let mut inputs: Vec<&Path> = vec![reference.dir.as_path()];
    inputs.extend(runs.iter().map(|r| r.dir.as_path()));
    crate::run::guard_output_dir(out, &inputs, &[RUNS_MARKER_FILE])?;
    for sub in ["thumbs", "heat"] {
        let p = out.join(sub);
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).map_err(io_err(format!("creating {}", p.display())))?;
    }
    let assets = StaticAssets {
        out,
        cache,
        tm: opts.hdr.tonemapper,
    };
    let model = overview(reference, runs, opts, Some(cache), &assets)?;
    let json = serde_json::to_string_pretty(&model)?;
    let marker = out.join(RUNS_MARKER_FILE);
    std::fs::write(&marker, format!("{json}\n"))
        .map_err(io_err(format!("writing {}", marker.display())))?;
    let config = serde_json::json!({ "mode": "static", "model": model }).to_string();
    let index = out.join("index.html");
    std::fs::write(&index, render_page(&config))
        .map_err(io_err(format!("writing {}", index.display())))?;
    Ok(model)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
