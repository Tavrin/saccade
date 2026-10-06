//! The `saccade` command-line tool. `compare`, `prove` and `review` are the
//! main entry points.
//!
//! Exit codes: `0` ok, `1` regression or claim not proven, `2` could not run,
//! `3` strict-arm refusal for a difference, `4` strict-arm refusal for a missing key.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use saccade_core::config::RunConfig;
use saccade_core::report::{Labels, Metric, Mode, Report, Status};
use saccade_core::view::{ViewOptions, build_view, is_safe_name};

// wave7
mod vision_checks;
mod wave7_cmd;
// wave8
mod media_cmd;
#[cfg(feature = "mcp")]
mod wave7_mcp;
// wave9
mod arms_cmd;
#[cfg(feature = "mcp")]
mod arms_mcp;
mod wave9_cmd;
#[cfg(feature = "mcp")]
mod wave9_mcp;

mod agent;
mod agent_ui;
mod approval;
mod assess_cmd;
mod brand_cmd;
mod capability_cmd;
#[cfg(feature = "products")]
mod design_cmd;
mod documents_cmd;
mod embedding_cmd;
mod engine_ingest;
mod f1;
mod general_cmd;
#[cfg(feature = "geometry")]
mod geometry_cmd;
mod git_bisect;
mod grounded_cmd;
mod hash_cmd;
mod history;
#[cfg(feature = "products")]
mod imgtune_cmd;
mod ingest;
mod inspect_image_cmd;
mod inventory_cmd;
mod last_good;
mod local_cmd;
mod localized_cmd;
mod motion_cmd;
#[cfg(feature = "products")]
mod notifier_cmd;
#[cfg(feature = "products")]
mod product_io;
#[cfg(feature = "compression")]
mod quality_cmd;
mod region_cmd;
mod renderdoc_cmd;
#[cfg(feature = "ai")]
mod review_cmd;
#[cfg(feature = "products")]
mod sweep_cmd;
mod text_cmd;

#[cfg(feature = "mcp")]
mod mcp;
mod outdirs;
mod perf_cmd;
// wave10
#[cfg(feature = "prechecks")]
mod precheck;
#[cfg(all(feature = "prechecks", feature = "mcp"))]
mod precheck_mcp;
mod schema_cmd;
#[cfg(feature = "graphics")]
mod temporal_cmd;
mod ui_review_cmd;
mod wave10_cmd;
#[cfg(feature = "mcp")]
mod wave10_mcp;

#[cfg(feature = "graphics")]
mod s6;

use agent::CliError;

/// Purpose, usage, then the explanation and examples, then the grouped flags.
const HELP_TEMPLATE: &str =
    "{about-with-newline}\n{usage-heading} {usage}\n\n{after-help}\n\n{all-args}";
const FRONT_HELP_TEMPLATE: &str =
    "{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}\n{after-help}";

#[derive(Parser)]
#[command(
    name = "saccade",
    version = env!("SACCADE_DISPLAY_VERSION"),
    disable_help_subcommand = true,
    help_template = FRONT_HELP_TEMPLATE,
    about = "Tell when visual or performance evidence is not good enough to support a claim",
    after_help = "\
Start here:
  saccade compare baseline/ captures/ --out report
  saccade prove identity parent/ candidate/ --out proof
  saccade prove performance --base 'base_r*' --arm 'candidate=candidate_r*'
  saccade review report/saccade-report.v1.json --out review

Tasks (full map and guides: docs/quickstart.md, docs/guides/):
  Did a render or screenshot change?      saccade compare
  Is a refactor pixel-identical?          saccade prove identity
  Did it get faster, accounting noise?    saccade prove performance
  Are two capture setups comparable?      saccade arms check
  Which images are near-duplicates?       saccade dedupe
  What does a finished report say?        saccade inspect, saccade review
Advanced: demo, identity, noise, view, inspect, experiment, approve, init,
serve, mcp, ingest, bisect, history, doctor. Existing commands keep working; use `saccade COMMAND --help`.
`saccade doctor` lists what this build and machine can run.

Exit codes (a command that cannot produce a measurement never exits 0):
  0  success: no regression, claim proven, or the requested output was written
  1  regression found, or the claim was not proven (differs, missing, new, unreadable)
  2  the command could not run: usage, config, input or unavailable feature/model
  3  strict producer check refused: an undeclared difference (--require-valid-arms)
  4  strict producer check refused: a required key is missing (--require-valid-arms)
Units: --threshold on FLIP scores is a 0-1 score (lower = more alike); hash thresholds count bits."
)]
struct Cli {
    /// Silence warnings when --out is next to capture metadata.
    #[arg(long, global = true, help_heading = "Global options")]
    allow_out_near_captures: bool,
    /// Opt in to absolute local paths in reports and machine-readable output.
    #[arg(long, global = true, help_heading = "Global options")]
    record_absolute_paths: bool,
    #[command(subcommand)]
    command: Command,
}

/// HDR-FLIP flags shared by `compare` and `view`.
#[derive(clap::Args, Clone, Default)]
#[command(next_help_heading = "HDR images")]
struct HdrArgs {
    /// Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard.
    #[arg(long, value_name = "NAME")]
    hdr_tonemapper: Option<String>,
    /// Exposure range in stops and count, `START:STOP:N` (default: computed
    /// from the baseline image).
    #[arg(long, value_name = "START:STOP:N", allow_hyphen_values = true)]
    hdr_exposures: Option<String>,
}

impl HdrArgs {
    fn apply(&self, hdr: &mut saccade_core::hdr::HdrConfig) -> Result<(), CliError> {
        if let Some(t) = &self.hdr_tonemapper {
            hdr.tonemapper = saccade_core::hdr::Tonemapper::parse(t)?;
        }
        if let Some(e) = &self.hdr_exposures {
            hdr.parse_exposures(e)?;
        }
        Ok(())
    }
}

/// Structured intent and predeclared changes bound into local evidence.
#[derive(clap::Args, Clone, Default)]
#[command(next_help_heading = "Review context")]
struct IntentArgs {
    /// What the change is meant to do, in one sentence, recorded in the evidence.
    #[arg(long, value_name = "TEXT", conflicts_with = "intent_file")]
    intent: Option<String>,
    /// Structured evidence intent or visual declaration JSON, written before capture.
    #[arg(long, value_name = "FILE")]
    intent_file: Option<PathBuf>,
    /// JSON list of expected changes; needs --intent or --intent-file.
    #[arg(long, value_name = "FILE")]
    changes_file: Option<PathBuf>,
}
/// Metadata-sidecar flags shared by `compare`, `identity` and `view`.
#[derive(clap::Args, Clone, Default)]
#[command(next_help_heading = "Metadata sidecars")]
struct MetaArgs {
    /// Sidecar file name (default `saccade-meta.json`); the per-image sidecar is
    /// `<stem>.<name>` and overrides the directory-level one.
    #[arg(long, value_name = "NAME")]
    meta_name: Option<String>,
    /// Extra sidecar key globs to ignore, added to the built-in timing,
    /// timestamp and run-id defaults.
    #[arg(long, value_delimiter = ',', value_name = "GLOB,...")]
    meta_ignore: Vec<String>,
}

/// Metadata enforcement flags for `compare` and `identity`.
#[derive(clap::Args, Clone, Default)]
#[command(next_help_heading = "Metadata sidecars")]
struct MetaRequireArgs {
    #[command(flatten)]
    arms: arms_cmd::StrictArgs,
    /// Intended experiment metadata variables (exact keys or globs).
    #[arg(long = "intended-variable", value_delimiter = ',')]
    intended_variables: Vec<String>,
    /// Make an entry an error when a sidecar key differs and is not declared.
    #[arg(long)]
    require_matching_meta: bool,
    /// Sidecar keys (or globs) that may differ with --require-matching-meta.
    #[arg(
        long,
        value_delimiter = ',',
        value_name = "KEY,...",
        requires = "require_matching_meta"
    )]
    declare: Vec<String>,
}

impl MetaArgs {
    fn apply(&self, meta: &mut saccade_core::meta::MetaOptions) {
        if let Some(n) = &self.meta_name {
            meta.name.clone_from(n);
        }
        meta.ignore.extend(self.meta_ignore.iter().cloned());
    }
}

impl MetaRequireArgs {
    fn apply(&self, meta: &mut saccade_core::meta::MetaOptions) {
        self.arms.apply(meta);
        meta.intended
            .extend(self.intended_variables.iter().cloned());
        meta.required |= self.require_matching_meta;
        meta.declared.extend(self.declare.iter().cloned());
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum MetricArg {
    Mean,
    P95,
    P99,
    Max,
}

impl From<MetricArg> for Metric {
    fn from(m: MetricArg) -> Self {
        match m {
            MetricArg::Mean => Metric::Mean,
            MetricArg::P95 => Metric::P95,
            MetricArg::P99 => Metric::P99,
            MetricArg::Max => Metric::Max,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    // wave10
    /// Compare structural rendering evidence with explicit scope and ID attribution.
    RenderEvidence(wave10_cmd::RenderArgs),
    /// Discover JSON Schemas without a source checkout.
    Schema(schema_cmd::Args),
    /// Validate producer performance sidecars.
    Perf(schema_cmd::PerfArgs),
    /// Validate producer identity before comparing pixels.
    Arms(arms_cmd::Args),
    /// Plan and compare deterministic page sweeps.
    #[cfg(feature = "products")]
    Sweep(sweep_cmd::SweepArgs),
    /// Audit delivery formats and search perceptual-target encodings.
    #[cfg(feature = "products")]
    Imgtune(imgtune_cmd::ImgtuneArgs),
    /// Pull design-source frames and compare implementation captures.
    #[cfg(feature = "products")]
    Design(design_cmd::DesignArgs),
    /// Send a generic report summary to a user-configured webhook.
    #[cfg(feature = "products")]
    Notify(notifier_cmd::NotifyArgs),
    /// List comparison questions, inputs, features and honest availability.
    Capabilities(capability_cmd::Args),
    /// Inspect provenance/integrity indicators without a real/fake verdict.
    InspectImage(inspect_image_cmd::Args),
    /// Measure content-dependent no-reference quality indicators.
    Assess(assess_cmd::Args),
    /// Compare image-bound OCR/text observations and literal expected strings.
    Text(text_cmd::Args),
    /// Cosine similarity with an explicitly pinned optional ONNX export.
    Similar(embedding_cmd::SimilarArgs),
    /// Build or query a streaming exact flat embedding index.
    Index(embedding_cmd::IndexArgs),
    /// Compute perceptual hashes without changing originals.
    Hash(hash_cmd::HashArgs),
    /// Cluster near-duplicates with bounded Hamming search; never delete images.
    Dedupe(hash_cmd::DedupeArgs),
    // wave8
    /// Analyze an image into a versioned media record (no model downloads by default).
    AnalyzeMedia(media_cmd::AnalyzeArgs),
    /// Extract shot representatives with timestamps, without linking a video decoder.
    Keyframes(media_cmd::KeyframesArgs),
    /// Match an image or media record against generic target images.
    FindUsage(media_cmd::UsageArgs),
    // wave7
    /// List or explicitly pull pinned local models.
    Models(wave7_cmd::ModelsArgs),
    /// Locate a phrase with boxes, optional masks, and an overlay PNG.
    Locate(wave7_cmd::LocateArgs),
    /// Measure a separately named learned quality score.
    #[command(alias = "score")]
    QualityScore(wave7_cmd::QualityArgs),
    /// Decode explicitly compatible watermark schemes without an origin verdict.
    Watermark(wave7_cmd::WatermarkArgs),
    /// Detect faces and optionally create a privacy-redacted PNG.
    Faces(wave7_cmd::FacesArgs),
    /// Assess declared crops against detected faces, without identity recognition.
    CropCheck(wave7_cmd::CropArgs),
    /// Bounded advisory observations from an explicitly configured local VLM.
    #[cfg(feature = "local-vlm")]
    ObserveLocal(wave7_cmd::ObserveArgs),
    /// Map provider requests or decode recorded vision responses; no live calls.
    #[cfg(feature = "vision-providers")]
    ProviderMap(wave7_cmd::ProviderArgs),
    /// Align optional Vulkan replay evidence and locate native-resource divergence.
    RenderdocLocalize(renderdoc_cmd::Args),
    /// Import and freeze phrase regions, or inspect optional model plumbing.
    Regions(region_cmd::Args),
    /// Render verified atomic numerical claims with region and evidence citations.
    ExplainGrounded(grounded_cmd::Args),
    /// Measure intended-region, boundary and protected-complement changes independently.
    LocalizedCheck(localized_cmd::Args),
    /// Reconcile expected and supplied stable capture cases against a comparison report.
    Inventory(inventory_cmd::Args),
    /// Measure externally encoded quality candidates under a frozen score and byte budget.
    #[cfg(feature = "compression")]
    QualitySweep(quality_cmd::Args),
    /// Record and inspect local visual-test variation across runs.
    #[command(hide = true)]
    History(history::HistoryArgs),
    /// Locate the first commit whose fresh capture fails its baseline.
    #[command(hide = true)]
    Bisect(git_bisect::BisectArgs),
    /// Convert a test runner's screenshot artifacts into compared image pairs.
    #[command(hide = true)]
    Ingest(ingest::IngestArgs),
    /// Print installed version, features and supported evidence schemas.
    #[command(display_order = 13, hide = true)]
    Doctor {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Bootstrap a commented configuration and print baseline adoption steps.
    #[command(display_order = 7, hide = true)]
    Init(f1::InitArgs),
    /// Run the bundled example and explain its expected regression.
    #[command(
        display_order = 1,
        hide = true,
        help_template = HELP_TEMPLATE,
        after_help = "\
Example:
  saccade demo --out saccade-demo
  saccade view saccade-demo          Print where the demo report is

The demo exits 1 on purpose: it contains a regression and a missing capture."
    )]
    Demo(f1::DemoArgs),
    /// Compare a directory of captures against a directory of baselines.
    #[command(
        allow_missing_positional = true,
        display_order = 1,
        help_template = HELP_TEMPLATE,
        after_help = "\
Images are paired by relative path. Each pair gets a FLIP score; a pair fails when
its deciding metric is above the threshold. The report directory holds index.html
(open it in a browser) and saccade-report.v1.json.

Examples:
  saccade compare baseline/ captures/ --out report
  saccade compare baseline/ captures/ --threshold 0.02 --metric p95
  saccade compare baseline/ captures/ --entry 'ui/*' --junit report/junit.xml
  saccade compare baseline/ captures/ --json        One bounded JSON result on stdout

Exit codes: 0 no regression, 1 regression found, 2 the command could not run."
    )]
    Compare {
        #[command(flatten)]
        general: Box<general_cmd::CompareArgs>,
        #[command(flatten)]
        field: Box<wave10_cmd::CompareArgs>,
        /// Directory of approved baseline images.
        #[arg(required_unless_present = "baseline", conflicts_with = "baseline")]
        baseline_dir: Option<PathBuf>,
        /// Resolve the latest complete passing history run as an immutable baseline.
        #[arg(long, value_parser = ["last-good"])]
        baseline: Option<String>,
        /// Local history store for --baseline last-good.
        #[arg(long, requires = "baseline")]
        history_store: Option<PathBuf>,
        /// Directory of fresh captures.
        capture_dir: PathBuf,
        /// Report output directory.
        #[arg(long, default_value = "report", help_heading = "Output")]
        out: PathBuf,
        /// FLIP score limit in 0-1 (0 = identical): a pair fails when its --metric value is above it. Overrides the config file.
        #[arg(long, help_heading = "Gate")]
        threshold: Option<f64>,
        /// Default deciding metric (overrides the config file's top level).
        #[arg(long, value_enum, help_heading = "Gate")]
        metric: Option<MetricArg>,
        /// Config file; defaults to ./saccade.toml when it exists.
        #[arg(long, help_heading = "Gate")]
        config: Option<PathBuf>,
        /// Treat new images (no baseline) as a regression.
        #[arg(long, help_heading = "Gate")]
        fail_on_new: bool,
        /// Accept a run that compared no pair (for example the first run, with
        /// an empty baseline directory). Without it, nothing compared exits 1.
        #[arg(long, help_heading = "Gate")]
        allow_empty: bool,
        /// Print a bounded machine-readable result.
        #[arg(long, help_heading = "Output")]
        json: bool,
        /// Viewing condition in pixels per degree of visual angle (default 67; larger = finer detail is visible).
        #[arg(long, help_heading = "Gate")]
        ppd: Option<f32>,
        /// Display names of the two sides, `baseline,capture`.
        #[arg(
            long,
            value_delimiter = ',',
            value_name = "A,B",
            help_heading = "Output"
        )]
        labels: Option<Vec<String>>,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long = "entry", value_name = "GLOB", help_heading = "Selection")]
        entries: Vec<String>,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml", help_heading = "Output")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
        #[command(flatten)]
        intent: IntentArgs,
    },
    /// Establish exact native decoded-sample equality in the selected scope.
    #[command(
        display_order = 3,
        hide = true,
        help_template = HELP_TEMPLATE,
        after_help = "\
Use it to prove a refactor or optimization renders the same pixels. There is no
threshold: any differing sample fails. Different file encodings of equal pixels pass.

Examples:
  saccade identity parent/ candidate/ --out report
  saccade identity parent/ candidate/ --json      One bounded JSON result on stdout

Exit codes: 0 every pair identical, 1 identity not proven (a pair differs, is missing,
new or unreadable), 2 the command could not run."
    )]
    Identity {
        /// Directory of images from the parent build.
        parent_dir: PathBuf,
        /// Directory of images from the candidate build.
        candidate_dir: PathBuf,
        /// Report output directory.
        #[arg(long, default_value = "report", help_heading = "Output")]
        out: PathBuf,
        /// Accept a run that compared no pair. Without it, nothing compared
        /// exits 1.
        #[arg(long, help_heading = "Gate")]
        allow_empty: bool,
        /// Rejected for identity; use compare for perceptual thresholds.
        #[arg(long, hide = true)]
        threshold: Option<f64>,
        /// Rejected for identity; use compare for perceptual metrics.
        #[arg(long, value_enum, hide = true)]
        metric: Option<MetricArg>,
        /// Config file; defaults to ./saccade.toml when it exists.
        #[arg(long, help_heading = "Gate")]
        config: Option<PathBuf>,
        /// Print a bounded machine-readable result.
        #[arg(long, help_heading = "Output")]
        json: bool,
        /// Viewing condition in pixels per degree of visual angle (default 67), used only to describe differences.
        #[arg(long, help_heading = "Gate")]
        ppd: Option<f32>,
        /// Display names of the two sides, `parent,candidate`.
        #[arg(
            long,
            value_delimiter = ',',
            value_name = "A,B",
            help_heading = "Output"
        )]
        labels: Option<Vec<String>>,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long = "entry", value_name = "GLOB", help_heading = "Selection")]
        entries: Vec<String>,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml", help_heading = "Output")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
        #[command(flatten)]
        intent: IntentArgs,
    },
    /// Check whether image identity or performance evidence proves a claim.
    #[command(display_order = 2)]
    Prove {
        #[command(subcommand)]
        operation: ProveOperation,
    },
    /// Calibrate thresholds from repeated captures of an unchanged build.
    #[command(display_order = 8, hide = true)]
    Noise(f1::NoiseArgs),
    /// Write a self-contained review viewer for 2 to 6 image directories.
    #[command(
        display_order = 4,
        hide = true,
        help_template = HELP_TEMPLATE,
        after_help = "\
Examples:
  saccade view before/ after/ --out view          Swipe, flicker and heatmap viewer
  saccade view a/ b/ c/ --labels a,b,c --reference a
  saccade view my-report                          Print where an existing report's page is
  saccade view a/ b/ --blind --key-out ../key.json --out judge-view"
    )]
    View {
        /// Directories to compare, paired by relative image path (2 to 6).
        #[arg(num_args = 1..=6, required_unless_present = "unblind")]
        dirs: Vec<PathBuf>,
        /// Resolve recorded anonymous choices after review.
        #[arg(
            long,
            requires = "key",
            conflicts_with = "dirs",
            help_heading = "Blind judging"
        )]
        unblind: Option<PathBuf>,
        /// The key written by --blind, used with --unblind.
        #[arg(long, requires = "unblind", help_heading = "Blind judging")]
        key: Option<PathBuf>,
        /// Comma-separated labels, one per directory (default: directory names).
        #[arg(long, value_delimiter = ',', help_heading = "Output")]
        labels: Option<Vec<String>>,
        /// FLIP reference: a label or one of the directories (default: the first).
        #[arg(long, help_heading = "Comparison")]
        reference: Option<String>,
        /// Pairwise judging: shuffle panes and hide labels until "Reveal".
        #[arg(long, help_heading = "Blind judging")]
        blind: bool,
        /// Seed for the blind shuffle (default: random). A blind page never
        /// embeds it; it is recorded in the key.
        #[arg(long, help_heading = "Blind judging")]
        seed: Option<u64>,
        /// Where a blind view's key goes. Required with --blind; keep it
        /// outside --out so the judge never receives it.
        #[arg(
            long,
            value_name = "PATH",
            requires = "blind",
            help_heading = "Blind judging"
        )]
        key_out: Option<PathBuf>,
        /// Output directory.
        #[arg(long, default_value = "view", help_heading = "Output")]
        out: PathBuf,
        /// Viewing condition in pixels per degree of visual angle (default 67; larger = finer detail is visible).
        #[arg(long, help_heading = "Comparison")]
        ppd: Option<f32>,
        /// Config file whose `[[region]]` tables become preset ROIs
        /// (default: `./saccade.toml` when present).
        #[arg(long, help_heading = "Comparison")]
        config: Option<PathBuf>,
        /// Print a JSON summary (`saccade-view-summary.v1`) instead of text.
        #[arg(long, help_heading = "Output")]
        json: bool,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long = "entry", value_name = "GLOB", help_heading = "Selection")]
        entries: Vec<String>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
    },
    /// Copy reviewed captures over baselines.
    #[command(
        display_order = 5,
        hide = true,
        help_template = HELP_TEMPLATE,
        after_help = "\
Example (two steps: plan, then apply the reviewed decision):
  saccade approve --report report/saccade-report.v1.json --entry ui.png --dry-run --out plan
  saccade approve --report report/saccade-report.v1.json --decisions plan/decision.json --out receipt

Review the report, plan/manifest.json and plan/decision.json between the two steps.
The dry run writes no baseline; content hashes must still match when applying."
    )]
    Approve {
        /// Directory of fresh captures.
        capture_dir: Option<PathBuf>,
        /// Baseline directory to update.
        baseline_dir: Option<PathBuf>,
        /// Derive the input directories from this report.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Image names (relative paths) to approve.
        names: Vec<String>,
        /// Select a report entry without positional directories; repeatable.
        #[arg(long = "entry", value_name = "NAME")]
        entries: Vec<String>,
        /// Also approve every fail and new entry of this report JSON.
        #[arg(long, value_name = "REPORT_JSON", num_args = 0..=1, default_missing_value = "__report__")]
        all_failing: Option<PathBuf>,
        /// Explicit canonical CLI decision bound to this report, inputs and scope.
        #[arg(long, value_name = "DECISIONS_JSON")]
        decisions: Option<PathBuf>,
        /// With --all-failing: also approve `error` entries (for example a size
        /// change) whose capture exists and decodes.
        #[arg(long)]
        include_errors: bool,
        /// With --all-failing: delete the baselines of every `missing` entry
        /// of the report (capture absent). Only files inside the baseline
        /// directory are removed; each removal is printed.
        #[arg(long)]
        prune_missing: bool,
        /// Print `{"schema":"saccade-approve.v1","copied":[...],"pruned":[...]}`
        /// instead of one line per file.
        #[arg(long)]
        json: bool,
        /// Removed: stale reviewed content cannot be overridden.
        #[arg(long, hide = true)]
        force: bool,
        /// Prepare a selected update manifest and unattested CLI decision draft.
        #[arg(long)]
        dry_run: bool,
        /// Empty directory for the plan, decision and applied receipt.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Browse report and image archives in a local web workbench.
    #[cfg(feature = "workbench")]
    #[command(
        display_order = 6,
        hide = true,
        help_template = HELP_TEMPLATE,
        after_help = "\
The server listens on 127.0.0.1 only. Archive roots are read-only: sessions,
thumbnails and uploads go to the cache directory, decisions to the decisions directory.

Examples:
  saccade serve captures/ --open               Browse and compare runs in the browser
  saccade serve captures/ reports/ --port 0    Several roots; pick a free port"
    )]
    Serve {
        // wave8
        /// Serve the local versioned media API instead of the archive viewer.
        #[arg(long)]
        api: bool,
        /// Largest accepted request body in bytes (default 16777216 = 16 MiB).
        #[arg(long,default_value_t=16*1024*1024)]
        api_max_bytes: usize,
        #[arg(long, default_value = "127.0.0.1")]
        api_bind: std::net::IpAddr,
        /// Optional bearer-token env file; default ~/.config/saccade/api.env if present.
        #[arg(long)]
        api_token_file: Option<PathBuf>,
        #[arg(long)]
        api_model_dir: Option<PathBuf>,
        #[arg(long)]
        api_registry: Option<PathBuf>,
        /// Archive roots to browse (read-only). With several, each is a
        /// top-level entry named after its directory.
        #[arg(num_args = 0..)]
        roots: Vec<PathBuf>,
        /// Additional read-only archive roots (repeatable).
        #[arg(long = "root")]
        registered_roots: Vec<PathBuf>,
        /// Explicit generated-artifact root.
        #[arg(long)]
        out_root: Option<PathBuf>,
        /// Let a symlink that resolves inside any of the roots be browsed and
        /// served; a symlink to anywhere else stays refused.
        #[arg(long)]
        follow_symlinks_within_roots: bool,
        /// Allow symlinks reached below a root to resolve into DIR (repeatable).
        #[arg(long = "symlink-target")]
        symlink_targets: Vec<PathBuf>,
        /// Storage deadline in milliseconds, at least 1 (default: 3000).
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        fs_timeout_ms: Option<u64>,
        /// Port on 127.0.0.1 (0 picks a free one).
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Cache directory for sessions, thumbnails and uploads
        /// (default: `$XDG_CACHE_HOME/saccade`).
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Directory the viewer's decisions are written to
        /// (default: `$XDG_DATA_HOME/saccade/decisions`).
        #[arg(long)]
        decisions_dir: Option<PathBuf>,
        /// Config file for sidecar settings and preset regions
        /// (default: `./saccade.toml` when present).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Viewing condition in pixels per degree of visual angle (default 67; larger = finer detail is visible).
        /// Viewing condition in pixels per degree of visual angle (default 67).
        #[arg(long)]
        ppd: Option<f32>,
        /// Open the page in the default browser.
        #[arg(long)]
        open: bool,
        #[command(flatten)]
        hdr: HdrArgs,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
    },
    /// Serve the agent tools over MCP on stdio, confined to the given roots.
    #[cfg(feature = "mcp")]
    #[command(
        display_order = 12,
        hide = true,
        help_template = HELP_TEMPLATE,
        after_help = "\
Every path a client passes must resolve under a --root. Generated reports go under
--out-root, which must be separate from the read-only roots.

Example:
  saccade mcp --root examples --out-root agent-reports"
    )]
    Mcp {
        /// Read-only roots (repeatable).
        #[arg(long = "root", required = true)]
        roots: Vec<PathBuf>,
        /// Generated artifacts require this separate root.
        #[arg(long)]
        out_root: Option<PathBuf>,
        /// Let a symlink that resolves inside any of the roots be read.
        #[arg(long)]
        follow_symlinks_within_roots: bool,
        /// Allow symlinks reached below a root to resolve into DIR (repeatable).
        #[arg(long = "symlink-target", value_name = "DIR")]
        symlink_targets: Vec<PathBuf>,
        #[cfg(feature = "ai")]
        #[command(flatten)]
        providers: review_cmd::Startup,
        /// Authorize product HTTP operations from registered roots.
        #[cfg(feature = "products")]
        #[arg(long)]
        allow_product_network: bool,
        /// Authorize explicit webhook tool calls using user configuration.
        #[cfg(feature = "products")]
        #[arg(long)]
        allow_webhook_notifications: bool,
    },
    /// Read, explain, prepare or export existing evidence.
    #[command(display_order = 9, hide = true)]
    Inspect(local_cmd::InspectArgs),
    /// Preview a review plan or handle a local closed decision request.
    #[command(display_order = 3)]
    Review(local_cmd::ReviewArgs),
    /// Analyze existing graphics captures: ablation, sequences, ranking, bisection.
    #[command(display_order = 11, hide = true)]
    Experiment {
        #[command(subcommand)]
        operation: ExperimentOperation,
    },
}

#[derive(Subcommand)]
enum ProveOperation {
    /// Prove exact ordered static mesh geometry identity (appearance excluded).
    #[cfg(feature = "geometry")]
    MeshIdentity(geometry_cmd::IdentityArgs),
    /// Prove exact native decoded-sample equality over the selected images.
    Identity(Box<ProveIdentityArgs>),
    /// Evaluate performance claims from ablation arms and repeat noise.
    #[cfg(feature = "graphics")]
    Performance(Box<perf_cmd::AblateArgs>),
}

#[derive(clap::Args)]
struct ProveIdentityArgs {
    parent_dir: PathBuf,
    candidate_dir: PathBuf,
    #[arg(long, default_value = "report")]
    out: PathBuf,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    allow_empty: bool,
    /// Viewing condition in pixels per degree of visual angle (default 67).
    #[arg(long)]
    ppd: Option<f32>,
    #[arg(long, value_delimiter = ',', value_name = "A,B")]
    labels: Option<Vec<String>>,
    #[arg(long, value_name = "FILE.xml")]
    junit: Option<PathBuf>,
    #[arg(long = "entry", value_name = "GLOB")]
    entries: Vec<String>,
    #[command(flatten)]
    meta: MetaArgs,
    #[command(flatten)]
    require: MetaRequireArgs,
    #[command(flatten)]
    perf: perf_cmd::PerfArgs,
    #[command(flatten)]
    intent: IntentArgs,
}

#[derive(Subcommand)]
enum ExperimentOperation {
    // wave9
    /// Compare a render with a noisy offline reference and record alignment/noise floors.
    Reference(wave9_cmd::ReferenceArgs),
    /// Measure bidirectional triangle-surface distance and oriented normal deviation.
    #[cfg(feature = "geometry")]
    Geometry(geometry_cmd::GeometryArgs),
    /// Compare ablation arms against a base with image and performance evidence.
    #[cfg(feature = "graphics")]
    Ablate(perf_cmd::AblateArgs),
    /// Compare numbered SDR frames with the ColorVideoVDP temporal model.
    #[cfg(feature = "graphics")]
    Temporal(temporal_cmd::TemporalArgs),
    /// Compare numbered colour frames by sorted index and measure added flicker.
    #[cfg(feature = "graphics")]
    Sequence {
        /// Declare a fixed camera and measure per-tile flicker with motion qualification.
        #[arg(long)]
        fixed_camera: bool,
        baseline_dir: PathBuf,
        capture_dir: PathBuf,
        /// Relative-name glob; frames must end in an integer before the extension.
        #[arg(long, default_value = "*")]
        pattern: String,
        #[arg(long, default_value = "sequence-report")]
        out: PathBuf,
        /// FLIP score limit in 0-1 (0 = identical); above it fails.
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long, value_enum)]
        metric: Option<MetricArg>,
        #[arg(long)]
        config: Option<PathBuf>,
        /// Viewing condition in pixels per degree of visual angle (default 67).
        #[arg(long)]
        ppd: Option<f32>,
        #[arg(long)]
        fail_on_new: bool,
        #[arg(long)]
        allow_empty: bool,
        #[arg(long, value_delimiter = ',')]
        labels: Option<Vec<String>>,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
    },
    /// Rank candidate directories against one common FLIP reference.
    #[cfg(feature = "graphics")]
    Rank {
        reference_dir: PathBuf,
        #[arg(required = true, num_args = 1..)]
        candidate_dirs: Vec<PathBuf>,
        /// One unique, safe directory label per candidate, comma separated.
        #[arg(long, value_delimiter = ',')]
        labels: Option<Vec<String>>,
        #[arg(long, value_enum, default_value = "mean")]
        metric: MetricArg,
        #[arg(long, default_value = "rank-report")]
        out: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        /// FLIP score limit in 0-1 (0 = identical); above it fails.
        #[arg(long)]
        threshold: Option<f64>,
        /// Viewing condition in pixels per degree of visual angle (default 67).
        #[arg(long)]
        ppd: Option<f32>,
        #[arg(long)]
        fail_on_new: bool,
        #[arg(long)]
        allow_empty: bool,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
    },
    /// Find the first diverging run or revision in an ordered series.
    #[cfg(feature = "graphics")]
    Bisect(s6::BisectArgs),
    /// Photosensitivity PRE-CHECK only; not certification or formal compliance.
    #[cfg(feature = "prechecks")]
    Safety(precheck::SafetyArgs),
    /// Accessibility PRE-CHECK only; not certification or formal compliance.
    #[cfg(feature = "prechecks")]
    A11y(precheck::A11yArgs),
}

/// Whether the command line asks for JSON output (`--json`, `--json=full`,
/// `--format json`), judged from the raw arguments so that even a command line
/// clap rejects gets a JSON error.
fn args_want_json(args: &[std::ffi::OsString]) -> bool {
    let args: Vec<String> = args
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    args.iter().enumerate().any(|(i, a)| {
        a == "--json"
            || a.starts_with("--json=")
            || a == "--format=json"
            || (a == "--format" && args.get(i + 1).is_some_and(|v| v == "json"))
    })
}

/// Prints `err` as `saccade-error.v1` on stdout.
fn emit_json_error(err: &CliError) {
    let text = serde_json::to_string_pretty(&err.value()).unwrap_or_default();
    let _ = emit(&format!("{text}\n"));
}

fn main() -> ExitCode {
    // The debug clap command builder alone uses almost 1 MiB of stack. Windows
    // gives the process's main thread 1 MiB, so parse and execute on an explicit
    // stack on every platform, independent of linker defaults or RUST_MIN_STACK.
    match std::thread::Builder::new()
        .name("saccade-cli".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(cli_main)
    {
        Ok(thread) => match thread.join() {
            Ok(code) => code,
            Err(panic) => std::panic::resume_unwind(panic),
        },
        Err(error) => {
            let error = CliError::io(format!("starting CLI thread: {error}"));
            let args: Vec<_> = std::env::args_os().collect();
            if args_want_json(&args) {
                emit_json_error(&error);
            } else {
                eprintln!("saccade: error: {error}\n  fix: {}", error.hint);
            }
            ExitCode::from(2)
        }
    }
}

fn cli_main() -> ExitCode {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if let Some(message) = local_cmd::migration(&args) {
        let err = CliError::new("interface_removed", message);
        if args_want_json(&args) {
            emit_json_error(&err);
        } else {
            eprintln!("saccade: {err}");
        }
        return ExitCode::from(2);
    }
    if let Some(replacement) = local_cmd::deprecated_alias(&mut args) {
        eprintln!("saccade: deprecated command; use saccade {replacement}");
    }
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            if e.use_stderr()
                && let Some(feature) = requested_feature(&args)
            {
                let err = CliError::from(saccade_core::Error::FeatureUnavailable { feature });
                if args_want_json(&args) {
                    emit_json_error(&err);
                } else {
                    eprintln!("saccade: error: {err}\n  fix: {}", err.hint);
                }
                return ExitCode::from(2);
            }
            if e.use_stderr() && args_want_json(&args) {
                let text = e.to_string();
                // The message is the text before clap's `Usage:` block.
                let message = text
                    .split("\n\n")
                    .next()
                    .unwrap_or("invalid arguments")
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                emit_json_error(&CliError::usage(
                    message.trim_start_matches("error: ").to_string(),
                ));
                return ExitCode::from(2);
            }
            if e.use_stderr() {
                // clap already names the argument, prints the usage line and
                // points at `--help`; repeating it as a hint only adds noise.
                let _ = e.print();
                return ExitCode::from(2);
            }
            e.exit()
        }
    };
    let json_errors = args_want_json(&args);
    match outdirs::warn(&cli.command, cli.allow_out_near_captures)
        .and_then(|()| dispatch(cli.command, cli.record_absolute_paths))
    {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            if json_errors {
                emit_json_error(&err);
            } else {
                if let Some(check) = &err.arm_check {
                    let _ = arms_cmd::emit(check, false);
                }
                eprintln!(
                    "saccade: error: {}\n  fix: {}",
                    escape_multiline(&err.message),
                    escape_control(&err.hint)
                );
            }
            ExitCode::from(err.arm_check.as_ref().map_or(2, |c| c.exit_code))
        }
    }
}

/// Prints a run's result: the table, the lean result or the whole report.
fn emit_run(
    report: &Report,
    out: &Path,
    json: bool,
    record_absolute_paths: bool,
) -> Result<(), CliError> {
    let mut missing = std::collections::BTreeMap::<String, usize>::new();
    let mut missing_pairs = 0usize;
    let mut same_capture = 0usize;
    for entry in &report.entries {
        if entry
            .warnings
            .iter()
            .any(|w| w.contains("provenance is absent"))
        {
            missing_pairs += 1;
        }
        for warning in &entry.warnings {
            if warning.contains("provenance is absent") {
                *missing
                    .entry(
                        warning
                            .split(" provenance is absent")
                            .next()
                            .unwrap_or(warning)
                            .to_owned(),
                    )
                    .or_default() += 1;
            }
            if warning == "same capture, not a repeat" {
                same_capture += 1;
            }
        }
    }
    if !missing.is_empty() || same_capture > 0 {
        eprintln!(
            "saccade: warning: provenance is absent on {} image pairs ({}){}; inspect REPORT --validity-reasons --json for detail",
            missing_pairs,
            missing
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", "),
            if same_capture > 0 {
                format!("; same capture, not a repeat on {same_capture} pairs")
            } else {
                String::new()
            }
        );
    }
    if json {
        let mut value = agent::result_value(
            report,
            &out.join(saccade_core::report::REPORT_FILE_NAME),
            agent::DEFAULT_TOP_FAILING,
            false,
        );
        // Preserve the identity discriminator used by existing consumer integrations.
        // The payload is the bounded shared envelope; no old-format mode exists.
        if report.config.mode == Mode::Identity {
            value["schema"] = serde_json::json!("saccade-result.v1");
        }
        let verification_file = out.join(saccade_core::intent::RESULT_FILE);
        if verification_file.is_file() {
            let finding: saccade_core::intent::Verification =
                serde_json::from_value(local_cmd::read_value(&verification_file)?)?;
            let matched = finding.unexpected.is_empty()
                && finding.missing.is_empty()
                && finding.unmeasurable.is_empty();
            value["intent_verification"] = serde_json::json!({"status":if matched {"matched"} else {"mismatch"},"matched":finding.matched.len(),"unexpected":finding.unexpected.len(),"missing":finding.missing.len(),"unmeasurable":finding.unmeasurable.len(),"artifact":local_cmd::reference(&verification_file)?});
        }
        emit(&format!(
            "{}\n",
            serde_json::to_string(&local_cmd::bounded(value, 4096)?)?
        ))
    } else {
        for entry in &report.entries {
            if let Some(field) = &entry.field_evidence {
                emit(&format!(
                    "scope: {} ({})\n",
                    field.scope,
                    escape_control(&entry.name)
                ))?;
            }
        }
        if let Some(check) = &report.config.meta.arm_validation
            && !check.allowed_unreached.is_empty()
        {
            emit(&format!(
                "allowed unreached: {}\n",
                serde_json::to_string(&check.allowed_unreached)?
            ))?;
        }
        emit(&text_table(report))?;
        let verification_file = out.join(saccade_core::intent::RESULT_FILE);
        if verification_file.is_file() {
            let finding: saccade_core::intent::Verification =
                serde_json::from_value(local_cmd::read_value(&verification_file)?)?;
            emit(&format!(
                "intent: {} matched, {} unexpected, {} missing, {} unmeasurable\n",
                finding.matched.len(),
                finding.unexpected.len(),
                finding.missing.len(),
                finding.unmeasurable.len()
            ))?;
        }
        emit(&run_footer(report, out, record_absolute_paths))?;
        if report.config.mode == Mode::Identity
            && (!report.config.entries.is_empty() || !report.config.ignore.is_empty())
        {
            emit(&format!(
                "scope: selected {:?}; excluded {:?}\n",
                report.config.entries, report.config.ignore
            ))?;
        }
        Ok(())
    }
}

fn dispatch(command: Command, record_absolute_paths: bool) -> Result<u8, CliError> {
    match command {
        Command::RenderEvidence(args) => wave10_cmd::render(args),
        Command::Schema(args) => schema_cmd::run(args),
        Command::Perf(args) => schema_cmd::perf(args),
        Command::Arms(args) => arms_cmd::run(args),
        Command::Capabilities(args) => capability_cmd::run(args),
        Command::InspectImage(args) => inspect_image_cmd::run(args),
        Command::Assess(args) => assess_cmd::run(args),
        Command::Text(args) => text_cmd::run(args),
        Command::Similar(args) => embedding_cmd::similar(args),
        Command::Index(args) => embedding_cmd::index(args),
        Command::Hash(args) => hash_cmd::run_hash(args),
        Command::Dedupe(args) => hash_cmd::run_dedupe(args),
        // wave8
        Command::AnalyzeMedia(args) => media_cmd::analyze(args),
        Command::Keyframes(args) => media_cmd::keyframes(args),
        Command::FindUsage(args) => media_cmd::usage(args),
        // wave7
        Command::Models(args) => wave7_cmd::models(args),
        Command::Locate(args) => wave7_cmd::locate(args),
        Command::QualityScore(args) => wave7_cmd::quality(args),
        Command::Watermark(args) => wave7_cmd::watermark(args),
        Command::Faces(args) => wave7_cmd::faces(args),
        Command::CropCheck(args) => wave7_cmd::crop(args),
        #[cfg(feature = "local-vlm")]
        Command::ObserveLocal(args) => wave7_cmd::observe(args),
        #[cfg(feature = "vision-providers")]
        Command::ProviderMap(args) => wave7_cmd::provider(args),
        Command::Prove {
            operation: ProveOperation::Identity(args),
        } => dispatch(
            Command::Identity {
                parent_dir: args.parent_dir,
                candidate_dir: args.candidate_dir,
                out: args.out,
                allow_empty: args.allow_empty,
                threshold: None,
                metric: None,
                config: args.config,
                json: args.json,
                ppd: args.ppd,
                labels: args.labels,
                entries: args.entries,
                junit: args.junit,
                meta: args.meta,
                require: args.require,
                perf: args.perf,
                intent: args.intent,
            },
            record_absolute_paths,
        ),
        #[cfg(feature = "graphics")]
        Command::Prove {
            operation: ProveOperation::Performance(args),
        } => perf_cmd::ablate(*args, record_absolute_paths),
        Command::Doctor { json } => doctor(json),
        Command::Bisect(args) => git_bisect::run(args),
        Command::Ingest(args) => ingest::run(args, record_absolute_paths),
        #[cfg(feature = "geometry")]
        Command::Prove {
            operation: ProveOperation::MeshIdentity(args),
        } => geometry_cmd::identity(args),
        #[cfg(feature = "geometry")]
        Command::Experiment {
            operation: ExperimentOperation::Geometry(args),
        } => geometry_cmd::compare(args),
        Command::History(args) => history::run(args),
        Command::RenderdocLocalize(args) => renderdoc_cmd::run(args),
        Command::Regions(args) => region_cmd::run(args),
        Command::ExplainGrounded(args) => grounded_cmd::run(args),
        Command::LocalizedCheck(args) => localized_cmd::run(args),
        #[cfg(feature = "products")]
        Command::Sweep(args) => sweep_cmd::run(args),
        #[cfg(feature = "products")]
        Command::Imgtune(args) => imgtune_cmd::run(args),
        #[cfg(feature = "products")]
        Command::Design(args) => design_cmd::run(args),
        #[cfg(feature = "products")]
        Command::Notify(args) => notifier_cmd::run(args),
        Command::Inventory(args) => inventory_cmd::run(args),
        #[cfg(feature = "compression")]
        Command::QualitySweep(args) => quality_cmd::run(args),
        Command::Inspect(args) => local_cmd::inspect(args, record_absolute_paths),
        Command::Review(args) => local_cmd::review(args, record_absolute_paths),
        #[cfg(feature = "prechecks")]
        Command::Experiment {
            operation: ExperimentOperation::Safety(args),
        } => precheck::safety(args),
        #[cfg(feature = "prechecks")]
        Command::Experiment {
            operation: ExperimentOperation::A11y(args),
        } => precheck::a11y(args),
        Command::Init(args) => f1::init(args),
        Command::Noise(args) => f1::noise(args, record_absolute_paths),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation: ExperimentOperation::Ablate(args),
        } => perf_cmd::ablate(args, record_absolute_paths),
        Command::Experiment {
            operation: ExperimentOperation::Reference(args),
        } => wave9_cmd::reference(args),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation: ExperimentOperation::Temporal(args),
        } => temporal_cmd::run(args, record_absolute_paths),
        Command::Demo(args) => f1::demo(args, record_absolute_paths),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation: ExperimentOperation::Bisect(args),
        } => s6::bisect(args),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation:
                ExperimentOperation::Sequence {
                    fixed_camera,
                    baseline_dir,
                    capture_dir,
                    pattern,
                    out,
                    threshold,
                    metric,
                    config,
                    ppd,
                    fail_on_new,
                    allow_empty,
                    labels,
                    json,
                    hdr,
                    junit,
                    meta,
                    require,
                },
        } => {
            let mut cfg = load_config(config.as_deref())?;
            if fixed_camera && cfg.temporal_tiles.is_none() {
                cfg.temporal_tiles = Some(Default::default());
            }
            cfg.record_absolute_paths = record_absolute_paths;
            if let Some(t) = threshold {
                cfg.default_threshold = t;
            }
            if let Some(m) = metric {
                cfg.default_metric = m.into();
            }
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            if let Some(l) = labels {
                cfg.labels = parse_labels(&l)?;
            }
            cfg.fail_on_new |= fail_on_new;
            cfg.allow_empty |= allow_empty;
            hdr.apply(&mut cfg.hdr)?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(junit.as_deref(), &[&baseline_dir, &capture_dir], &out)?;
            let report = saccade_core::sequence::run_sequence(
                &baseline_dir,
                &capture_dir,
                &out,
                &pattern,
                &cfg,
            )?;
            if let Some(path) = junit {
                f1::junit_reports(&out, &path, false)?;
            }
            if json {
                let value = local_cmd::analysis_result(&serde_json::to_value(&report)?, &out)?;
                emit(&format!(
                    "{}
",
                    serde_json::to_string(&value)?
                ))?;
            } else {
                emit(&report.text())?;
            }
            Ok(u8::from(report.is_regression()))
        }
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation:
                ExperimentOperation::Rank {
                    reference_dir,
                    candidate_dirs,
                    labels,
                    metric,
                    out,
                    config,
                    threshold,
                    ppd,
                    fail_on_new,
                    allow_empty,
                    json,
                    hdr,
                    junit,
                    meta,
                    require,
                },
        } => {
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            if let Some(t) = threshold {
                cfg.default_threshold = t;
            }
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            cfg.fail_on_new |= fail_on_new;
            cfg.allow_empty |= allow_empty;
            hdr.apply(&mut cfg.hdr)?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(
                junit.as_deref(),
                &std::iter::once(reference_dir.as_path())
                    .chain(candidate_dirs.iter().map(PathBuf::as_path))
                    .collect::<Vec<_>>(),
                &out,
            )?;
            let report = saccade_core::rank::run_rank(
                &reference_dir,
                &candidate_dirs,
                labels.as_deref(),
                metric.into(),
                &out,
                &cfg,
            )?;
            if let Some(path) = junit {
                f1::junit_reports(&out, &path, true)?;
            }
            if json {
                let value = local_cmd::analysis_result(&serde_json::to_value(&report)?, &out)?;
                emit(&format!(
                    "{}
",
                    serde_json::to_string(&value)?
                ))?;
            } else {
                emit(&report.text())?;
            }
            Ok(
                if report.common_images != report.reference_images
                    || report
                        .overall
                        .iter()
                        .any(|c| c.totals.error > 0 || c.totals.missing > 0 || c.totals.new > 0)
                {
                    2
                } else {
                    0
                },
            )
        }
        Command::Compare {
            general,
            field,
            baseline_dir,
            baseline,
            history_store,
            capture_dir,
            out,
            threshold,
            metric,
            config,
            fail_on_new,
            allow_empty,
            json,
            ppd,
            labels,
            hdr,
            junit,
            entries,
            meta,
            require,
            perf,
            intent,
        } => {
            let last_good = if baseline.is_some() {
                Some(last_good::resolve(history_store.as_deref().ok_or_else(
                    || CliError::usage("--baseline last-good requires --history-store"),
                )?)?)
            } else {
                None
            };
            let baseline_dir = last_good
                .as_ref()
                .map(|dir| dir.path().to_path_buf())
                .or(baseline_dir)
                .ok_or_else(|| CliError::usage("baseline directory required"))?;
            let mut arm_cfg = load_config(config.as_deref())?;
            meta.apply(&mut arm_cfg.meta);
            require.apply(&mut arm_cfg.meta);
            arm_cfg.entries.clone_from(&entries);
            saccade_core::arms::enforce(&baseline_dir, &capture_dir, &arm_cfg)?;
            // Explicit questions never discard unrelated evidence options or fall back.
            capability_cmd::validate(&general)?;
            // Document files stream pages into a separate versioned summary.
            let document_pair = baseline_dir.is_file()
                && capture_dir.is_file()
                && (documents_cmd::is_document(&baseline_dir)
                    || documents_cmd::is_document(&capture_dir));
            if document_pair
                && general
                    .question
                    .is_none_or(|q| q == capability_cmd::Question::SameRender)
            {
                if field.requested()
                    || config.is_some()
                    || !entries.is_empty()
                    || junit.is_some()
                    || ppd.is_some()
                    || labels.is_some()
                    || fail_on_new
                    || allow_empty
                    || !general_cmd::plain_options(&intent, &meta, &require, &perf, &hdr)
                {
                    return Err(CliError::usage(
                        "document compare supports DPI, threshold, metric and alignment; other evidence options require explicit raster inputs",
                    ));
                }
                return documents_cmd::compare(
                    &baseline_dir,
                    &capture_dir,
                    &out,
                    &general,
                    threshold.unwrap_or(0.02),
                    metric.unwrap_or(MetricArg::Mean),
                    json,
                );
            }
            if general.dpi.is_some() {
                return Err(CliError::usage(
                    "--dpi requires a document file pair with same-render comparison",
                ));
            }

            if general
                .question
                .is_some_and(|q| q != capability_cmd::Question::SameRender)
            {
                if field.requested()
                    || config.is_some()
                    || !entries.is_empty()
                    || junit.is_some()
                    || ppd.is_some()
                    || labels.is_some()
                    || fail_on_new
                    || allow_empty
                    || metric.is_some()
                    || !general_cmd::plain_options(&intent, &meta, &require, &perf, &hdr)
                {
                    return Err(CliError::usage(
                        "routed question supports its own declared inputs and threshold units; use the dedicated family command for other options",
                    ));
                }
                return capability_cmd::route(
                    &baseline_dir,
                    &capture_dir,
                    &out,
                    &general,
                    threshold,
                    json,
                );
            }
            // Explicit registration has its own evidence contract.
            if general.align.is_some() {
                if field.requested()
                    || config.is_some()
                    || !entries.is_empty()
                    || junit.is_some()
                    || ppd.is_some()
                    || labels.is_some()
                    || fail_on_new
                    || allow_empty
                    || !general_cmd::plain_options(&intent, &meta, &require, &perf, &hdr)
                {
                    return Err(CliError::usage(
                        "explicit registration supports threshold/metric/resample only; other evidence options require the existing unregistered pipeline",
                    ));
                }
                return general_cmd::compare(
                    &baseline_dir,
                    &capture_dir,
                    &out,
                    &general,
                    threshold.unwrap_or(0.02),
                    metric.unwrap_or(MetricArg::Mean),
                    json,
                );
            }
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            field.apply(&mut cfg)?;
            perf.apply(&mut cfg.perf)?;
            cfg.entries = entries;
            cfg.allow_empty |= allow_empty;
            hdr.apply(&mut cfg.hdr)?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(junit.as_deref(), &[&baseline_dir, &capture_dir], &out)?;
            if let Some(t) = threshold {
                cfg.default_threshold = t;
            }
            if let Some(m) = metric {
                cfg.default_metric = m.into();
            }
            if fail_on_new {
                cfg.fail_on_new = true;
            }
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            if let Some(l) = labels {
                cfg.labels = parse_labels(&l)?;
            }
            let visual = local_cmd::visual_intent(&intent)?;
            if let Some((declaration, source)) = &visual {
                saccade_core::intent::apply_effects(declaration, source, &mut cfg)?;
            }
            let report = saccade_core::run::run(&baseline_dir, &capture_dir, &out, &cfg)?;
            if let Some(path) = junit {
                saccade_core::ergonomics::junit(&report, &path)?;
            }
            local_cmd::persist_case(
                &report,
                &out.join(saccade_core::report::REPORT_FILE_NAME),
                &intent,
            )?;
            let intent_mismatch = local_cmd::verify_visual_intent(&report, &out, visual.as_ref())?;
            // Hash-bound route component preserves the ordinary immutable report contract.
            if general.question.is_some() {
                let choice = capability_cmd::record_render(&report, &out, &general)?;
                if json {
                    let mut value = agent::result_value(
                        &report,
                        &out.join(saccade_core::report::REPORT_FILE_NAME),
                        agent::DEFAULT_TOP_FAILING,
                        false,
                    );
                    value["data"]["pipeline_choice"] = choice;
                    emit(&format!("{}\n", value))?;
                } else {
                    emit_run(&report, &out, false, record_absolute_paths)?;
                }
            } else {
                emit_run(&report, &out, json, record_absolute_paths)?;
            }
            Ok(u8::from(report.is_regression() || intent_mismatch))
        }
        Command::Identity {
            parent_dir,
            candidate_dir,
            out,
            allow_empty,
            threshold,
            metric,
            config,
            json,
            ppd,
            labels,
            junit,
            entries,
            meta,
            require,
            perf,
            intent,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            perf.apply(&mut cfg.perf)?;
            cfg.entries = entries;
            cfg.allow_empty |= allow_empty;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(junit.as_deref(), &[&parent_dir, &candidate_dir], &out)?;
            if threshold.is_some() || metric.is_some() {
                return Err(CliError::usage(if threshold.is_some() {
                    "identity is exact; use compare --threshold"
                } else {
                    "identity is exact; use compare --metric"
                }));
            }
            cfg.mode = Mode::Identity;
            cfg.labels = Labels {
                baseline: "parent".into(),
                capture: "candidate".into(),
            };
            // Identity is always exact; metrics describe differences only.
            cfg.default_threshold = threshold.unwrap_or(0.0);
            cfg.default_metric = metric.map_or(Metric::Max, Into::into);
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            if let Some(l) = labels {
                cfg.labels = parse_labels(&l)?;
            }
            let visual = local_cmd::visual_intent(&intent)?;
            if let Some((declaration, source)) = &visual {
                saccade_core::intent::apply_effects(declaration, source, &mut cfg)?;
            }
            let report = saccade_core::run::run(&parent_dir, &candidate_dir, &out, &cfg)?;
            if let Some(path) = junit {
                saccade_core::ergonomics::junit(&report, &path)?;
            }
            local_cmd::persist_case(
                &report,
                &out.join(saccade_core::report::REPORT_FILE_NAME),
                &intent,
            )?;
            let intent_mismatch = local_cmd::verify_visual_intent(&report, &out, visual.as_ref())?;
            emit_run(&report, &out, json, record_absolute_paths)?;
            Ok(u8::from(report.is_regression() || intent_mismatch))
        }
        Command::Approve {
            capture_dir,
            baseline_dir,
            mut names,
            entries,
            report,
            all_failing,
            decisions,
            include_errors,
            prune_missing,
            json,
            force,
            dry_run,
            out,
        } => {
            names.extend(entries);
            let report_path = report.or_else(|| {
                all_failing
                    .as_ref()
                    .filter(|p| p.as_os_str() != "__report__")
                    .cloned()
            });
            if all_failing
                .as_ref()
                .is_some_and(|p| p.as_os_str() == "__report__")
                && report_path.is_none()
            {
                return Err(CliError::usage(
                    "--all-failing needs --report REPORT_JSON or a legacy REPORT_JSON value",
                ));
            }
            if (include_errors || prune_missing) && report_path.is_none() {
                return Err(CliError::usage(
                    "--include-errors and --prune-missing need --report REPORT_JSON or --all-failing REPORT_JSON",
                ));
            }
            for name in &names {
                if !is_safe_name(name) {
                    return Err(CliError::new(
                        "unsafe_path",
                        format!("unsafe image name {name:?}"),
                    ));
                }
            }
            if force {
                return Err(CliError::new(
                    "approve_mismatch",
                    "--force was removed; stale content must be reviewed again",
                ));
            }
            let report_path = report_path.ok_or_else(|| CliError::new("approve_mismatch", "approval requires --report and a canonical --decisions file; legacy finals must be reviewed again"))?;
            let (capture, baseline) =
                f1::approve_dirs(capture_dir, baseline_dir, Some(&report_path), None)?;
            approval::run(
                &report_path,
                &capture,
                &baseline,
                decisions.as_deref(),
                approval::Options {
                    names,
                    all_failing: all_failing.is_some(),
                    include_errors,
                    prune_missing,
                    dry_run,
                    out,
                    json,
                    absolute: record_absolute_paths,
                },
            )
        }
        #[cfg(feature = "workbench")]
        Command::Serve {
            api,
            api_max_bytes,
            api_bind,
            api_token_file,
            api_model_dir,
            api_registry,
            mut roots,
            registered_roots,
            out_root,
            follow_symlinks_within_roots,
            symlink_targets,
            fs_timeout_ms,
            port,
            cache_dir,
            decisions_dir,
            config,
            ppd,
            open,
            hdr,
            meta,
            perf,
        } => {
            roots.extend(registered_roots);
            // wave8
            if api {
                return media_cmd::serve_api(
                    roots,
                    port,
                    api_max_bytes,
                    api_bind,
                    api_token_file,
                    api_model_dir.unwrap_or_else(saccade_core::media::default_model_dir),
                    api_registry,
                );
            }
            if roots.is_empty() {
                return Err(CliError::usage("serve requires ROOT or --root DIR"));
            }
            let loaded = load_config(config.as_deref())?;
            if !loaded.symlink_targets.is_empty() {
                return Err(CliError::usage(
                    "project config cannot authorize symlink targets; pass --symlink-target at startup",
                ));
            }
            let mut opts = saccade_core::serve::ServeOptions::new(roots.remove(0));
            opts.extra_roots = roots;
            opts.follow_symlinks_within_roots = follow_symlinks_within_roots;
            opts.symlink_targets.extend(symlink_targets);
            opts.fs_timeout_ms = fs_timeout_ms.unwrap_or(loaded.fs_timeout_ms);
            opts.port = port;
            if let Some(root) = out_root {
                let all_roots = std::iter::once(opts.root.clone())
                    .chain(opts.extra_roots.clone())
                    .collect::<Vec<_>>();
                let policy = saccade_core::root_policy::RootPolicy::new(
                    &all_roots,
                    Some(&root),
                    opts.follow_symlinks_within_roots,
                    &opts.symlink_targets,
                )?;
                opts.cache_dir = policy.write(Path::new("cache"))?;
                opts.decisions_dir = policy.write(Path::new("decisions"))?;
            }
            if let Some(d) = cache_dir {
                opts.cache_dir = d;
            }
            if let Some(d) = decisions_dir {
                opts.decisions_dir = d;
            }
            opts.view.record_absolute_paths = record_absolute_paths;
            opts.view.regions = loaded.regions;
            opts.view.meta = loaded.meta;
            opts.view.diagnostics = loaded.diagnostics;
            opts.view.perf = loaded.perf;
            perf.apply(&mut opts.view.perf)?;
            hdr.apply(&mut opts.view.hdr)?;
            meta.apply(&mut opts.view.meta);
            if let Some(p) = ppd {
                opts.view.pixels_per_degree = p;
            }
            let handle = saccade_core::serve::start(opts)?;
            let url = format!("http://127.0.0.1:{}/", handle.port());
            emit(&format!(
                "saccade serve: {url}\n  cache:     {}\n  decisions: {}\n  (Ctrl-C to stop)\n",
                escape_control(&handle.cache_dir().display().to_string()),
                escape_control(&handle.decisions_dir().display().to_string())
            ))?;
            if open {
                open_browser(&url);
            }
            handle.wait();
            Ok(0)
        }
        Command::View {
            dirs,
            unblind,
            key,
            labels,
            reference,
            blind,
            seed,
            key_out,
            out,
            ppd,
            config,
            json,
            hdr,
            entries,
            meta,
            perf,
        } => {
            if let Some(decisions) = unblind {
                return local_cmd::unblind(
                    &decisions,
                    key.as_deref()
                        .ok_or_else(|| CliError::usage("--unblind requires --key"))?,
                    &out,
                    record_absolute_paths,
                );
            }
            if dirs.len() == 1 {
                return local_cmd::view_artifact(&dirs[0], &out, json);
            }
            if blind && key_out.is_none() {
                return Err(CliError::usage(
                    "--blind requires --key-out outside the view bundle",
                ));
            }
            let loaded = load_config(config.as_deref())?;
            let mut opts = ViewOptions {
                entries,
                record_absolute_paths,
                regions: loaded.regions,
                meta: loaded.meta,
                diagnostics: loaded.diagnostics,
                perf: loaded.perf,
                labels,
                reference,
                blind,
                seed,
                key_out: key_out.clone(),
                ..ViewOptions::default()
            };
            perf.apply(&mut opts.perf)?;
            hdr.apply(&mut opts.hdr)?;
            meta.apply(&mut opts.meta);
            if let Some(p) = ppd {
                opts.pixels_per_degree = p;
            }
            let model = build_view(&dirs, &out, &opts)?;
            let key = key_out.unwrap_or_else(|| saccade_core::view::private_key_path(&out));
            if json {
                let mut value = local_cmd::base_result("view");
                value["artifact"] =
                    local_cmd::reference(&out.join(saccade_core::view::VIEW_MARKER_FILE))?;
                value["counts"] =
                    serde_json::json!({"sets":model.sets.len(),"directories":model.labels.len()});
                value["data"] = serde_json::json!({"blind":blind});
                emit(&format!("{}\n", serde_json::to_string(&value)?))?;
                return Ok(0);
            }
            emit(&format!(
                "wrote {} ({} image sets, {} directories)\n",
                escape_control(&out.join("index.html").display().to_string()),
                model.sets.len(),
                model.labels.len()
            ))?;
            if blind {
                emit(&format!(
                    "blind key (keep it away from the judge): {}\n",
                    escape_control(&key.display().to_string())
                ))?;
            }
            Ok(0)
        }
        #[cfg(feature = "mcp")]
        Command::Mcp {
            roots,
            out_root,
            follow_symlinks_within_roots,
            symlink_targets,
            #[cfg(feature = "ai")]
            providers,
            #[cfg(feature = "products")]
            allow_product_network,
            #[cfg(feature = "products")]
            allow_webhook_notifications,
        } => {
            mcp::serve_stdio(
                &roots,
                out_root.as_deref(),
                follow_symlinks_within_roots,
                &symlink_targets,
                #[cfg(feature = "ai")]
                providers,
                #[cfg(feature = "products")]
                allow_product_network,
                #[cfg(feature = "products")]
                allow_webhook_notifications,
            )?;
            Ok(0)
        }
    }
}

fn doctor(json: bool) -> Result<u8, CliError> {
    let mut features = saccade_core::COMPILED_FEATURES.to_vec();
    if cfg!(feature = "ocr") {
        features.push("ocr");
    }
    if cfg!(feature = "mcp") {
        features.push("mcp");
    }
    if cfg!(feature = "products") {
        features.push("products");
    }
    if cfg!(feature = "imgtune-avif") {
        features.push("imgtune-avif");
    }
    features.sort_unstable();
    features.dedup();
    // These names are a script-facing contract. Add new names; keep existing
    // ones until they have an explicit deprecation path.
    let mut capabilities = vec![
        "compat-aliases",
        "removed-flag-errors",
        "version-skew-errors",
        "provenance-warnings",
        "repeat-detection",
        "perf-v2",
        "paired-robust-perf-v1",
        "identity-json-v1",
        "history-v1",
        "inventory-v1",
        "localized-v1",
        "grounded-v1",
        "frozen-region-v1",
        "renderdoc-v1",
        "brand-review-v1",
        "ui-review-v1",
    ];
    if cfg!(feature = "dense-motion") {
        capabilities.push("dense-motion-v1");
    }
    if cfg!(all(feature = "geometry", feature = "graphics")) {
        capabilities.push("asset-views-v1");
    }
    if cfg!(feature = "compression") {
        capabilities.push("quality-v1");
    }
    if cfg!(feature = "prechecks") {
        capabilities.push("prechecks");
    }
    if cfg!(feature = "ocr") {
        capabilities.push("ocr-tesseract-v1");
    }
    if cfg!(feature = "mcp") {
        capabilities.push("mcp");
    }
    if cfg!(feature = "ai") {
        capabilities.push("review");
    }
    if cfg!(feature = "assist") {
        capabilities.push("experimental-assist");
    }
    // wave7
    capabilities.extend([
        "local-model-registry-v1",
        "mask-mode-v1",
        "vision-replay-v1",
        "crop-safety-v1",
        "watermark-dwt-v1",
    ]);
    if cfg!(feature = "local-models") {
        capabilities.push("onnx-cpu-adapter-v1");
    }
    if cfg!(feature = "local-vlm") {
        capabilities.push("local-vlm-http-v1");
    }
    if cfg!(feature = "vision-providers") {
        capabilities.push("vision-provider-mapping-v1");
    }
    // wave8
    capabilities.push("media-record-v1");
    capabilities.sort_unstable();
    let git_commit = option_env!("SACCADE_GIT_COMMIT").filter(|value| !value.is_empty());
    let git_commit_short =
        option_env!("SACCADE_GIT_COMMIT_SHORT").filter(|value| !value.is_empty());
    let git_dirty = match option_env!("SACCADE_GIT_DIRTY") {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let value = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "build": {
            "git_commit": git_commit,
            "git_commit_short": git_commit_short,
            "git_dirty": git_dirty,
            "profile": env!("SACCADE_BUILD_PROFILE"),
            "rustc_version": env!("SACCADE_RUSTC_VERSION"),
        },
        "features": features,
        "capabilities": capabilities,
        "schemas": {
            // wave7
            "local_vision": ["saccade-model-registry.v1", "saccade-model-status.v1", "saccade-locate.v1", "saccade-vision-observation.v1", "saccade-learned-quality.v1", "saccade-watermark.v1", "saccade-faces.v1", "saccade-crop-check.v1", "saccade-provider-mapping.v1"],
            "report": ["saccade-report.v1"],
            "result": ["saccade-result.v1", "saccade-result.v2"],
            "evidence": ["saccade-evidence.v1"],
            "image_noise": ["saccade-noise.v1"],
            "inventory": ["saccade-inventory.v1", "saccade-inventory-report.v1"],
            "localized": ["saccade-localized.v1"],
            "grounded": ["saccade-grounded.v1"],
            "frozen_region": ["saccade-frozen-region.v1"],
            "dom_regions": ["saccade-dom-regions.v1"],
            "region_models": ["saccade-region-models.v1"],
            "quality": ["saccade-quality-sweep.v1", "saccade-quality-report.v1"],
            "renderdoc": ["saccade-renderdoc-extract.v1", "saccade-renderdoc-localization.v1"],
            "performance": ["saccade-perf.v1", "saccade-perf.v2", "saccade-perf-plan.v1", "saccade-perf-pairs.v1"],
            "brand": ["saccade-brand-source.v1", "saccade-brand-review.v1"],
            "ui": ["saccade-ui-source.v1", "saccade-ui-review.v1", "saccade-tesseract.v1"],
            "motion": ["saccade-motion-review.v1", "saccade-motion-vectors.v1", "saccade-vector-buffer.v1"],
            "asset_views": ["saccade-asset-views.v1", "saccade-asset-view-report.v1"]
        }
    });
    let mut value = value;
    value["optional_dependencies"] = saccade_core::optional::status();
    value["command_availability"] = command_availability();
    if json {
        emit(&format!("{}\n", serde_json::to_string(&value)?))?;
    } else {
        emit(&format!(
            "saccade {}\nbuild: {}\nfeatures: {}\ncapabilities: {}\nschemas: {}\n",
            env!("SACCADE_DISPLAY_VERSION"),
            value["build"],
            features.join(", "),
            capabilities.join(", "),
            value["schemas"]
        ))?;
    }
    if !json {
        emit(&format!(
            "optional dependencies (present/missing and fix commands): {}\n",
            value["optional_dependencies"]
        ))?;
        emit("command availability in this build:\n")?;
        for row in value["command_availability"]
            .as_array()
            .into_iter()
            .flatten()
        {
            emit(&format!(
                "  {:<11} {} ({}; {})\n",
                row["status"].as_str().unwrap_or(""),
                row["commands"].as_str().unwrap_or(""),
                row["requires"].as_str().unwrap_or(""),
                row["note"].as_str().unwrap_or("")
            ))?;
        }
    }
    Ok(0)
}

/// Which command groups this build can run, with the missing piece and its fix.
/// Models and runtimes are provisioned separately: see `optional_dependencies`.
fn command_availability() -> serde_json::Value {
    let rows: [(&str, &str, bool, &str, &str); 8] = [
        (
            "compare, prove identity, inspect, review (plan/request/ask), view, init, approve",
            "builtin",
            true,
            "built in",
            "no models or network needed",
        ),
        (
            "locate, faces, crop-check, quality-score, watermark (model decode)",
            "local-models",
            cfg!(feature = "local-models"),
            "needs the local-models feature",
            "also needs a pulled model and runtime: `saccade models list --json`, then `saccade models pull ID` and `saccade models pull runtime`",
        ),
        (
            "similar, index",
            "embeddings",
            cfg!(feature = "embeddings"),
            "needs the embeddings feature",
            "also needs a pinned model export: docs/embeddings.md",
        ),
        (
            "compare a.svg b.pdf",
            "documents",
            cfg!(feature = "documents"),
            "needs the documents feature",
            "static SVG and PDF pages: docs/documents.md",
        ),
        (
            "text with the default OCR contract",
            "ocr",
            cfg!(feature = "ocr"),
            "needs the ocr feature",
            "or pass --a-source/--b-source text files instead: docs/text.md",
        ),
        (
            "experiment a11y",
            "prechecks",
            cfg!(feature = "prechecks"),
            "needs the prechecks feature",
            "docs/safety-a11y.md",
        ),
        (
            "sweep, imgtune, design, notify, last-good baselines",
            "products",
            cfg!(feature = "products"),
            "needs the products feature",
            "docs/sweep.md, docs/imgtune.md",
        ),
        (
            "review explain, audit-mask, check-ui, assist",
            "assist",
            cfg!(feature = "assist"),
            "needs the assist feature",
            "advisory only, experimental: docs/assist.md",
        ),
    ];
    serde_json::Value::Array(
        rows.iter()
            .map(|(commands, feature, on, requires, note)| {
                serde_json::json!({
                    "commands": commands,
                    "feature": feature,
                    "status": if *on { "available" } else { "unavailable" },
                    "requires": requires,
                    "note": note,
                })
            })
            .collect(),
    )
}

/// Parses `--labels a,b` into the two side names.
fn parse_labels(parts: &[String]) -> Result<Labels, String> {
    match parts {
        [a, b] if !a.trim().is_empty() && !b.trim().is_empty() => Ok(Labels {
            baseline: a.trim().to_string(),
            capture: b.trim().to_string(),
        }),
        _ => Err("--labels takes exactly two non-empty names, `A,B`".into()),
    }
}

fn load_config(explicit: Option<&Path>) -> Result<RunConfig, CliError> {
    let path = match explicit {
        Some(p) => Some(p),
        None => Some(Path::new("saccade.toml")).filter(|p| p.is_file()),
    };
    match path {
        Some(p) => Ok(RunConfig::from_toml_file(p)?),
        None => Ok(RunConfig::default()),
    }
}

/// Opens `url` in the default browser; failures are ignored.
#[cfg(feature = "workbench")]
fn open_browser(url: &str) {
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn read_report(path: &Path) -> Result<Report, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::io(format!("reading report {}: {e}", path.display())))?;
    parse_contract(text.as_bytes(), saccade_core::report::REPORT_SCHEMA)
}

/// Parses the exact retained bytes that callers hash; newer producers fail distinctly.
fn parse_contract<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    schema: &str,
) -> Result<T, CliError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if let Some(actual) = value.get("schema").and_then(|v| v.as_str())
        && actual != schema
        && !(schema == "saccade-report.v1" && actual == "flipdiff-report.v1")
    {
        let prefix = schema
            .rsplit_once('v')
            .map(|(prefix, _)| format!("{prefix}v"))
            .unwrap_or_else(|| schema.to_owned());
        let supported = schema
            .rsplit_once('v')
            .and_then(|(_, v)| v.parse::<u32>().ok())
            .unwrap_or(1);
        if actual
            .strip_prefix(prefix.as_str())
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v > supported)
        {
            return Err(CliError::new(
                "version_skew",
                format!("written by {actual}; installed saccade supports up to {schema}, upgrade"),
            ));
        }
        return Err(CliError::usage(format!(
            "expected {schema}, found {actual}"
        )));
    }
    reject_newer_nested_schemas(&value)?;
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let mut ignored = None;
    let parsed: T = serde_ignored::deserialize(&mut parser, |path| {
        if ignored.is_none() { ignored = Some(path.to_string()); }
    }).map_err(|e| {
        if e.to_string().starts_with("unknown field ") {
            CliError::new("version_skew", format!("written by a newer producer; installed saccade supports up to {schema}, upgrade: {e}"))
        } else { CliError::io(format!("JSON error: {e}")) }
    })?;
    if let Some(path) = ignored {
        return Err(CliError::new(
            "version_skew",
            format!(
                "written by a newer producer; installed saccade supports up to {schema}, upgrade: unknown field {path}"
            ),
        ));
    }
    Ok(parsed)
}

fn reject_newer_nested_schemas(value: &serde_json::Value) -> Result<(), CliError> {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(actual) = fields.get("schema").and_then(|v| v.as_str()) {
                for prefix in [
                    // wave7
                    "saccade-model-registry.v",
                    "saccade-model-status.v",
                    "saccade-locate.v",
                    "saccade-vision-observation.v",
                    "saccade-learned-quality.v",
                    "saccade-watermark.v",
                    "saccade-faces.v",
                    "saccade-crop-check.v",
                    "saccade-provider-mapping.v",
                    "saccade-report.v",
                    "saccade-perf-diff.v",
                    "saccade-noise.v",
                    "saccade-history.v",
                    "saccade-onset.v",
                    "saccade-inventory.v",
                    "saccade-inventory-report.v",
                    "saccade-localized.v",
                    "saccade-frozen-region.v",
                    "saccade-dom-regions.v",
                    "saccade-grounded.v",
                    "saccade-quality-sweep.v",
                    "saccade-quality-report.v",
                    "saccade-region-models.v",
                    "saccade-renderdoc-extract.v",
                    "saccade-renderdoc-localization.v",
                    "saccade-brand-source.v",
                    "saccade-brand-review.v",
                    "saccade-ui-source.v",
                    "saccade-tesseract.v",
                    "saccade-ui-review.v",
                    "saccade-perf-plan.v",
                    "saccade-perf-pairs.v",
                    "saccade-vector-buffer.v",
                    "saccade-motion-vectors.v",
                    "saccade-motion-review.v",
                    "saccade-asset-views.v",
                    "saccade-asset-view-report.v",
                ] {
                    if actual
                        .strip_prefix(prefix)
                        .and_then(|v| v.parse::<u32>().ok())
                        .is_some_and(|v| v > 1)
                    {
                        return Err(CliError::new(
                            "version_skew",
                            format!(
                                "written by {actual}; installed saccade supports up to {prefix}1, upgrade"
                            ),
                        ));
                    }
                }
            }
            for child in fields.values() {
                reject_newer_nested_schemas(child)?;
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                reject_newer_nested_schemas(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Writes `text` to stdout. A closed pipe (for example `| head`) is not an error.
fn emit(text: &str) -> Result<(), CliError> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(CliError::io(format!("writing to stdout: {e}"))),
    }
}

/// Escapes control characters (newline, escape, ...) so a file name or error
/// text cannot forge terminal output or workflow commands.
fn escape_control(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out
}

/// Like [`escape_control`] but keeps line breaks, so multi-line errors (a TOML
/// parse error with its source excerpt) stay readable. Every continuation line
/// is indented, so file-controlled text cannot start a line with a workflow
/// command such as `::error::`.
fn escape_multiline(s: &str) -> String {
    s.lines()
        .map(escape_control)
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// Refuses a destination whose existing components below `baseline_dir` include a symlink.
fn check_no_symlinks(baseline_dir: &Path, rel: &Path) -> Result<(), CliError> {
    let mut cur = baseline_dir.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        match std::fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(CliError::new(
                    "unsafe_path",
                    format!("refusing to write through symlink {}", cur.display()),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(CliError::io(format!("inspecting {}: {e}", cur.display()))),
        }
    }
    Ok(())
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Fail => "FAIL",
        Status::New => "new",
        Status::Missing => "MISSING",
        Status::Error => "ERROR",
    }
}

fn metric_label(m: Metric) -> &'static str {
    match m {
        Metric::Mean => "mean",
        Metric::P95 => "p95",
        Metric::P99 => "p99",
        Metric::Max => "max",
    }
}

/// Whether stdout should carry ANSI colour: a terminal, and `NO_COLOR` unset or empty.
fn stdout_color() -> bool {
    use std::io::IsTerminal;
    std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty()) && std::io::stdout().is_terminal()
}

/// Wraps `text` in an SGR colour when `on`; status colour always accompanies a word.
fn paint(text: &str, sgr: &str, on: bool) -> String {
    if on {
        format!("\x1b[{sgr}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

fn status_sgr(s: Status) -> &'static str {
    match s {
        Status::Pass => "32",
        Status::Fail => "1;31",
        Status::New => "36",
        Status::Missing | Status::Error => "33",
    }
}

/// The one-line result of a regression run, printed above the table.
fn compare_headline(report: &Report) -> String {
    let t = &report.totals;
    let problems: Vec<String> = [
        (t.fail, "fail"),
        (t.error, "error"),
        (t.missing, "missing"),
        (t.new, "new"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect();
    let images = if t.total == 1 { "image" } else { "images" };
    if report.is_empty_run() {
        format!("compare: ❌ nothing compared ({} {images} found)", t.total)
    } else if report.is_regression() {
        format!(
            "compare: ❌ regression: {} of {} {images}",
            problems.join(", "),
            t.total
        )
    } else if problems.is_empty() {
        format!(
            "compare: ✅ no regression: {} of {} {images} passed",
            t.pass, t.total
        )
    } else {
        format!(
            "compare: ✅ no regression: {} pass, {} not gated",
            t.pass,
            problems.join(", ")
        )
    }
}

/// Where the report is and what to do next, printed below the table.
fn run_footer(report: &Report, out: &Path, absolute: bool) -> String {
    let page = saccade_core::paths::cwd(&out.join("index.html"), absolute);
    let json =
        saccade_core::paths::cwd(&out.join(saccade_core::report::REPORT_FILE_NAME), absolute);
    let mut text = format!("report: {}\n", escape_control(&page));
    let t = &report.totals;
    let next = if report.is_empty_run() {
        "check that both directories hold images with the same relative names".to_owned()
    } else if !report.is_regression() {
        return text;
    } else if report.config.mode == Mode::Identity {
        "open the report to see where the samples differ; use `saccade compare` when a perceptual tolerance is acceptable".to_owned()
    } else if t.fail + t.error == 0 && t.missing > 0 && t.new == 0 {
        "restore the missing captures, or remove their baselines after review".to_owned()
    } else {
        format!(
            "open the report and check the numbered hotspots; if a change is intended, plan its approval with\n        saccade approve --report {} --entry NAME --dry-run --out plan",
            escape_control(&json)
        )
    };
    text.push_str(&format!("next:   {next}\n"));
    text
}

/// Aligned plain-text table, non-pass rows first (then by name).
fn text_table(report: &Report) -> String {
    let color = stdout_color();
    let mut rows: Vec<&saccade_core::Entry> = report.entries.iter().collect();
    rows.sort_by_key(|e| (e.status == Status::Pass, e.name.clone()));
    // A warning every compared pair shares is said once below the table, not per row.
    let compared: Vec<&saccade_core::Entry> = report
        .entries
        .iter()
        .filter(|e| matches!(e.status, Status::Pass | Status::Fail))
        .collect();
    let shared: Vec<&String> = match compared.split_first() {
        Some((first, rest)) if !rest.is_empty() => first
            .warnings
            .iter()
            .filter(|w| {
                !w.contains("provenance is absent") && rest.iter().all(|e| e.warnings.contains(w))
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut cells: Vec<[String; 5]> = vec![[
        "STATUS".into(),
        "NAME".into(),
        "METRIC".into(),
        "VALUE".into(),
        "THRESHOLD".into(),
    ]];
    // Everything that is not a table column goes on `↳` lines under its row.
    let mut notes: Vec<Vec<String>> = vec![Vec::new()];
    let mut statuses: Vec<Option<Status>> = vec![None];
    for e in rows {
        let mut lines = Vec::new();
        if let Some(d) = &e.diagnostics {
            if e.status == Status::Pass {
                // A passing pair says something only when timings come with it.
                if !d.perf.is_empty() || !d.perf_not_comparable.is_empty() {
                    lines.push(escape_control(&d.verdict_line(e.bit_identical)));
                }
            } else {
                lines.push(format!(
                    "{}: {}",
                    d.class.as_str(),
                    escape_control(&d.description)
                ));
                lines.extend(
                    d.perf_summary()
                        .map(|p| format!("perf: {}", escape_control(&p))),
                );
            }
        }
        if e.status == Status::Fail {
            lines.extend(saccade_core::hotspots::summary_line(&e.hotspots));
        }
        if let Some(note) = e.local_hotspot_note() {
            lines.push(note);
        }
        if let Some(msg) = &e.error {
            lines.push(format!("error: {}", escape_control(msg)));
        }
        // An undeclared-difference error already names the keys.
        let error_names_keys = e
            .error
            .as_deref()
            .is_some_and(|m| m.contains("configuration differs"));
        if !e.meta_diff.is_empty() && !error_names_keys {
            let keys: Vec<&str> = e.meta_diff.iter().map(|d| d.key.as_str()).collect();
            lines.push(format!(
                "config differs: {}",
                escape_control(&keys.join(", "))
            ));
        }
        for w in e
            .warnings
            .iter()
            .filter(|w| !shared.contains(w) && !w.contains("provenance is absent"))
        {
            lines.push(format!("warning: {}", escape_control(w)));
        }
        notes.push(lines);
        statuses.push(Some(e.status));
        let (value, threshold) = match e.status {
            Status::Pass | Status::Fail => (
                e.value.map_or("-".into(), |v| format!("{v:.5}")),
                format!("{}", e.threshold),
            ),
            _ => ("-".into(), "-".into()),
        };
        cells.push([
            status_label(e.status).into(),
            escape_control(&e.name),
            e.buffer.as_ref().map_or_else(
                || metric_label(e.metric_used).to_string(),
                |b| format!("{} ({})", metric_label(b.metric), b.unit),
            ),
            value,
            threshold,
        ]);
    }
    let mut widths = [0usize; 5];
    for row in &cells {
        for (w, c) in widths.iter_mut().zip(row) {
            *w = (*w).max(c.chars().count());
        }
    }
    let mut out = String::new();
    for (i, (row, note)) in cells.iter().zip(&notes).enumerate() {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect();
        let line = line.join("  ");
        let line = line.trim_end();
        // Row 0 is the header; the status word leads every other row.
        match (i, statuses.get(i)) {
            (0, _) => out.push_str(&paint(line, "2", color)),
            (_, Some(Some(s))) => {
                let (word, rest) = line.split_at(widths[0].min(line.len()));
                out.push_str(&paint(word, status_sgr(*s), color));
                out.push_str(rest);
            }
            _ => out.push_str(line),
        }
        out.push('\n');
        for note in note {
            out.push_str(&format!("  ↳ {note}\n"));
        }
    }
    if !shared.is_empty() {
        out.push_str(&format!(
            "\nShared by all {} compared pairs:\n",
            compared.len()
        ));
        for w in &shared {
            out.push_str(&format!("  ↳ warning: {}\n", escape_control(w)));
        }
    }
    let headline = saccade_core::render::identity_headline(report).unwrap_or_else(|| {
        if report.config.mode == Mode::Regression {
            compare_headline(report)
        } else {
            String::new()
        }
    });
    if !headline.is_empty() {
        let sgr = if report.is_regression() {
            "1;31"
        } else {
            "1;32"
        };
        out.insert_str(
            0,
            &format!("{}\n\n", paint(&escape_control(&headline), sgr, color)),
        );
    }
    if let Some(v) = &report.combined_verdict {
        out.push_str(&format!("\n{}\n", escape_control(v)));
    }
    if let Some(d) = &report.perf_diff {
        out.push_str(&format!("{}\n", escape_control(&d.summary(3))));
        for w in &d.warnings {
            out.push_str(&format!("warning: {}\n", escape_control(w)));
        }
    }
    if report.perf_diff.is_some() || !report.perf_errors.is_empty() {
        out.push_str("performance evidence present; run `saccade experiment ablate BASE CAPTURE --out DIR --json` for the ablation outcome.\n");
    }
    let t = &report.totals;
    out.push_str(&format!(
        "\n{} fail, {} error, {} missing, {} new, {} pass ({} total)\n",
        t.fail, t.error, t.missing, t.new, t.pass, t.total
    ));
    if report.is_empty_run() {
        out.push_str(
            "nothing compared: no image exists in both directories (--allow-empty accepts this)\n",
        );
    }
    out
}

/// The feature required by an optional CLI operation or MCP tool.
fn required_feature(operation: &str) -> Option<&'static str> {
    match operation {
        "ablate" | "bisect" | "sequence" | "temporal" | "rank" | "saccade_ablate"
        | "saccade_bisect" | "saccade_sequence" | "saccade_rank" => Some("graphics"),
        "saccade_review" => Some("ai"),
        "calibrate"
        | "selftest"
        | "bench"
        | "collect-labels"
        | "saccade_judge_calibrate"
        | "saccade_judge_bench" => Some("evaluation"),
        "serve" | "ask" | "saccade_ask_human" | "saccade_inbox_get" => Some("workbench"),
        "safety" | "a11y" | "saccade_safety" | "saccade_a11y" => Some("prechecks"),
        "mcp" => Some("mcp"),
        "geometry" | "mesh-identity" => Some("geometry"),
        "sweep" | "imgtune" | "design" | "notify" => Some("products"),
        _ => None,
    }
}

fn feature_enabled(feature: &str) -> bool {
    if feature == "mcp" {
        cfg!(feature = "mcp")
    } else if feature == "products" {
        cfg!(feature = "products")
    } else {
        saccade_core::COMPILED_FEATURES.contains(&feature)
    }
}

fn unavailable_feature(operation: &str) -> Option<&'static str> {
    required_feature(operation).filter(|f| !feature_enabled(f))
}

fn requested_feature(args: &[std::ffi::OsString]) -> Option<&'static str> {
    let operation = args
        .iter()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with('-'))?
        .to_str()?;
    unavailable_feature(operation).or_else(|| {
        if operation == "experiment" || operation == "prove" {
            args.iter()
                .skip(2)
                .find(|a| !a.to_string_lossy().starts_with('-'))
                .and_then(|a| a.to_str().and_then(unavailable_feature))
        } else if operation == "init" && !feature_enabled("ai") {
            args.windows(2)
                .any(|w| {
                    w[0] == "--template"
                        && matches!(w[1].to_str(), Some("ci" | "nightly" | "lookdev"))
                })
                .then_some("ai")
        } else {
            None
        }
    })
}

pub(crate) fn capabilities(json: bool) -> Result<u8, CliError> {
    use clap::CommandFactory;
    fn operations(command: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for child in command.get_subcommands() {
            let name = if prefix.is_empty() {
                child.get_name().to_owned()
            } else {
                format!("{prefix} {}", child.get_name())
            };
            out.push(name.clone());
            operations(child, &name, out);
        }
    }
    let mut names = Vec::new();
    operations(&Cli::command(), "", &mut names);
    let mut features = saccade_core::COMPILED_FEATURES.to_vec();
    if cfg!(feature = "ocr") {
        features.push("ocr");
    }
    if cfg!(feature = "mcp") {
        features.push("mcp");
    }
    if cfg!(feature = "products") {
        features.push("products");
    }
    if cfg!(feature = "imgtune-avif") {
        features.push("imgtune-avif");
    }
    features.sort_unstable();
    features.dedup();
    let value = serde_json::json!({
        "features": features,
        "operations": names,
        "contract_versions": ["saccade-report.v1", "saccade-result.v2", "saccade-evidence.v1", "saccade-noise.v1", "saccade-brand-review.v1", "saccade-ui-review.v1", "saccade-motion-review.v1", "saccade-asset-view-report.v1"],
    });
    if json {
        let mut result = local_cmd::base_result("capabilities");
        result["data"] = value.clone();
        emit(&format!("{}\n", serde_json::to_string_pretty(&result)?))?;
    } else {
        emit(&format!(
            "features: {}\noperations: {}\ncontracts: {}\n",
            features.join(", "),
            names.join(", "),
            value["contract_versions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ))?;
    }
    Ok(0)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod wave3_schema_tests {
    #[test]
    fn w3_f08_all_nested_families() {
        for id in [
            "saccade-brand-source",
            "saccade-brand-review",
            "saccade-ui-source",
            "saccade-ui-review",
            "saccade-tesseract",
            "saccade-perf-plan",
            "saccade-perf-pairs",
            "saccade-motion-review",
            "saccade-motion-vectors",
            "saccade-vector-buffer",
            "saccade-asset-views",
            "saccade-asset-view-report",
        ] {
            let value = serde_json::json!({"outer":[{"schema":format!("{id}.v2")} ]});
            let error = super::reject_newer_nested_schemas(&value).expect_err("upgrade required");
            assert_eq!(error.code, "version_skew");
            assert!(error.message.contains("upgrade"));
            let value = serde_json::json!({"outer":[{"schema":format!("{id}.v1")} ]});
            super::reject_newer_nested_schemas(&value).expect("supported");
        }
    }
}

#[cfg(feature = "assist")]
mod assist_batch_cmd;
#[cfg(feature = "assist")]
mod assist_cmd;

// OCR lane: optional document provider transport.
mod document_ocr_cmd;
