# Rust library workflows

Saccade 0.2.9 exposes image measurements through `saccade-core`. The additive
`workflows` module exposes command-level artifact writing, explicit approval
policy and verdicts for Rust callers. It does not exit or print. CLI parsing,
terminal rendering and startup policy discovery stay in the executable.

This lane uses the specification's reduced-scope allowance. Stock directory/image
compare and exact identity proof, signed approval validation/application, complete
history operations, manifest operations, batch intake and generic screenshot
ingest moved. Authorized review and offline preview engines moved; the other
review families retain their existing core measurements and CLI orchestration.
The table names those boundaries explicitly; it does not claim every CLI branch
has an equivalent command-level Rust entry point.

All existing APIs remain available. Features remain opt-in: review requires `ai`;
batch/history require a desktop (`unix` or `windows`) as their existing locking
and process transport does. Signed trust-file enforcement fails closed outside
Unix. OpenSSH verification uses an explicitly configured external executable,
never a private key. Library policies are independent values, with no process-wide
policy installation. The CLI retains its legacy one-time guard for other consumers.
Scoped root I/O and report-link contexts are inherited from the existing core;
callers must establish their desired context just as for existing measurement APIs.

`workflows::batch::run_batch_in_process` combines intake, existing core-only
sections and typed completion without an installed CLI. The new
`workflows::batch::run_batch` preserves the CLI subprocess transport and requires
an explicit executable path. Signed policy refuses this transport because it
cannot propagate trusted policy to workers. This is a refusal, never a pass.

## Audit

Classification is **before this extraction**, at the command/workflow level:
(a) equivalent behavior already publicly callable (function named),
(b) core measurement exists but CLI orchestrates inputs/artifacts/verdict,
(c) substantive command logic exists only in the CLI. Groups with subcommands
are counted as rows too, matching the recursively compiled command catalogue.
Hidden `batch-probe` is included. `--help`/`--version` are parser flags, not commands.

**168 rows: (a) 14, (b) 120, (c) 34.**
The catalogue was read from the all-supported-features binary, excluding only
`imgtune-avif`; source enums were inspected for feature and hidden-command coverage.
AVIF adds encoding support, not another command. Deprecated argv spellings in
`local_cmd::deprecated_alias` route to these operations and are not separate APIs.

| CLI command / subcommand | Before | CLI source | Existing core API / boundary | After this lane | Feature |
| --- | --- | --- | --- | --- | --- |
| `batch` | b | `batch_cmd.rs` | `batch::{intake,run,run_in_process}` | `workflows::batch::run_batch` | default / runtime feature refusal where applicable |
| `assist` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `assist explain` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `assist audit-mask` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `assist check-ui` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `assist batch` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `assist batch submit` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `assist batch status` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `assist batch collect` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `replay` | c | `replay_cmd.rs` | `replay::{safe_path,verify_files,comparison_id}` | `deferred` | default / runtime feature refusal where applicable |
| `replay pack` | c | `replay_cmd.rs` | `replay::{safe_path,verify_files,comparison_id}` | `deferred` | default / runtime feature refusal where applicable |
| `replay verify` | c | `replay_cmd.rs` | `replay::{safe_path,verify_files,comparison_id}` | `deferred` | default / runtime feature refusal where applicable |
| `manifest` | b | `manifest_cmd.rs` | `manifest::{write,verify,link,classify}; coverage::analyze` | `workflows::manifest::{build,views,verify,link,classify}` | default / runtime feature refusal where applicable |
| `manifest build` | b | `manifest_cmd.rs` | `manifest::{write,verify,link,classify}; coverage::analyze` | `workflows::manifest::{build,views,verify,link,classify}` | default / runtime feature refusal where applicable |
| `manifest views` | b | `manifest_cmd.rs` | `manifest::{write,verify,link,classify}; coverage::analyze` | `workflows::manifest::{build,views,verify,link,classify}` | default / runtime feature refusal where applicable |
| `manifest verify` | b | `manifest_cmd.rs` | `manifest::{write,verify,link,classify}; coverage::analyze` | `workflows::manifest::{build,views,verify,link,classify}` | default / runtime feature refusal where applicable |
| `manifest link` | b | `manifest_cmd.rs` | `manifest::{write,verify,link,classify}; coverage::analyze` | `workflows::manifest::{build,views,verify,link,classify}` | default / runtime feature refusal where applicable |
| `manifest classify` | b | `manifest_cmd.rs` | `manifest::{write,verify,link,classify}; coverage::analyze` | `workflows::manifest::{build,views,verify,link,classify}` | default / runtime feature refusal where applicable |
| `export-regions` | c | `region_export_cmd.rs` | `explain::explain (related, not equivalent)` | `deferred` | default / runtime feature refusal where applicable |
| `print` | a | `print_cmd.rs` | `saccade_print::compare` | `existing extension` | print |
| `geo` | b | `geo_cmd.rs` | `saccade_geo::{compare,mask_metrics}` | `deferred` | geo |
| `geo compare` | a | `geo_cmd.rs` | `saccade_geo::compare` | `existing extension` | geo |
| `geo tiles` | b | `geo_cmd.rs` | `saccade_geo::compare` | `deferred` | geo |
| `geo mask-metrics` | a | `geo_cmd.rs` | `saccade_geo::mask_metrics` | `existing extension` | geo |
| `timing` | b | `wave11_cmd.rs` | `timing::{load,analyze}` | `deferred` | default / runtime feature refusal where applicable |
| `timing ab` | b | `wave11_cmd.rs` | `timing::{load,analyze}` | `deferred` | default / runtime feature refusal where applicable |
| `mask-metrics` | b | `measure_cmd.rs` | `mask_metrics::compare` | `deferred` | default / runtime feature refusal where applicable |
| `boxes` | b | `measure_cmd.rs` | `boxes::{to_coco,from_coco,to_yolo,from_yolo,crop,resize}` | `deferred` | default / runtime feature refusal where applicable |
| `boxes export` | b | `measure_cmd.rs` | `boxes::{to_coco,from_coco,to_yolo,from_yolo,crop,resize}` | `deferred` | default / runtime feature refusal where applicable |
| `boxes import` | b | `measure_cmd.rs` | `boxes::{to_coco,from_coco,to_yolo,from_yolo,crop,resize}` | `deferred` | default / runtime feature refusal where applicable |
| `boxes transform` | b | `measure_cmd.rs` | `boxes::{to_coco,from_coco,to_yolo,from_yolo,crop,resize}` | `deferred` | default / runtime feature refusal where applicable |
| `frame-map` | b | `measure_cmd.rs` | `frame_map::check` | `deferred` | default / runtime feature refusal where applicable |
| `frame-map check` | b | `measure_cmd.rs` | `frame_map::check` | `deferred` | default / runtime feature refusal where applicable |
| `render-evidence` | c | `wave11_cmd.rs` | `render::render_html (related, not equivalent)` | `deferred` | default / runtime feature refusal where applicable |
| `schema` | a | `schema_cmd.rs` | `schema_catalog::get; schema_catalog::DOCUMENTS` | `existing` | default / runtime feature refusal where applicable |
| `schema list` | a | `schema_cmd.rs` | `schema_catalog::get; schema_catalog::DOCUMENTS` | `existing` | default / runtime feature refusal where applicable |
| `schema get` | a | `schema_cmd.rs` | `schema_catalog::get; schema_catalog::DOCUMENTS` | `existing` | default / runtime feature refusal where applicable |
| `schema path` | c | `schema_cmd.rs` | `schema_catalog::get (content); path discovery is CLI` | `deferred` | default / runtime feature refusal where applicable |
| `perf` | b | `perf_cmd.rs` | `perf::{pair,noise_record}` | `deferred` | default / runtime feature refusal where applicable |
| `perf validate` | b | `perf_cmd.rs` | `perf::{pair,noise_record}` | `deferred` | default / runtime feature refusal where applicable |
| `arms` | a | `arms_cmd.rs` | `arms::check_paths` | `existing` | default / runtime feature refusal where applicable |
| `arms check` | a | `arms_cmd.rs` | `arms::check_paths` | `existing` | default / runtime feature refusal where applicable |
| `capture` | a | `capture_cmd.rs` | `capture::conform_path` | `existing` | default / runtime feature refusal where applicable |
| `capture conform` | a | `capture_cmd.rs` | `capture::conform_path` | `existing` | default / runtime feature refusal where applicable |
| `sweep` | c | `sweep_cmd.rs` | `run::run (pair measurement only)` | `deferred` | products |
| `sweep plan` | c | `sweep_cmd.rs` | `run::run (pair measurement only)` | `deferred` | products |
| `sweep compare` | c | `sweep_cmd.rs` | `run::run (pair measurement only)` | `deferred` | products |
| `imgtune` | c | `imgtune_cmd.rs` | `quality::{score,butteraugli_distance} (measurement only)` | `deferred` | products (AVIF: imgtune-avif) |
| `imgtune audit` | c | `imgtune_cmd.rs` | `quality::{score,butteraugli_distance} (measurement only)` | `deferred` | products (AVIF: imgtune-avif) |
| `imgtune search` | c | `imgtune_cmd.rs` | `quality::{score,butteraugli_distance} (measurement only)` | `deferred` | products (AVIF: imgtune-avif) |
| `design` | c | `design_cmd.rs` | `run::run; color (measurement only)` | `deferred` | products |
| `design pull` | c | `design_cmd.rs` | `run::run; color (measurement only)` | `deferred` | products |
| `design compare` | c | `design_cmd.rs` | `run::run; color (measurement only)` | `deferred` | products |
| `notify` | c | `notifier_cmd.rs` | `none: webhook adapters and transport in CLI` | `deferred` | products |
| `capabilities` | c | `capability_cmd.rs` | `COMPILED_FEATURES (inventory only)` | `deferred` | default / runtime feature refusal where applicable |
| `inspect-image` | b | `inspect_image_cmd.rs` | `general::{integrity,metadata,forensics,credentials}` | `deferred` | default / runtime feature refusal where applicable |
| `assess` | b | `assess_cmd.rs` | `general::assessment::assess` | `deferred` | default / runtime feature refusal where applicable |
| `text` | b | `text_cmd.rs` | `general::text::compare` | `deferred` | default / runtime feature refusal where applicable |
| `tofu` | b | `text_quality_cmd.rs` | `text_quality::tofu::detect` | `deferred` | text-quality |
| `optical-code` | b | `optical_code_cmd.rs` | `general::optical_code::verify` | `deferred` | optical-code |
| `text-legibility` | b | `text_quality_cmd.rs` | `text_quality::legibility` | `deferred` | text-quality |
| `critical-text` | b | `critical_text_cmd.rs` | `critical_text::evaluate` | `deferred` | default / runtime feature refusal where applicable |
| `timed-text` | b | `timed_text_cmd.rs` | `timed_text::{parse,check}` | `deferred` | default / runtime feature refusal where applicable |
| `similar` | b | `embedding_cmd.rs` | `general::embedding::{cosine,preprocess}` | `deferred` | embeddings |
| `index` | b | `embedding_cmd.rs` | `general::embedding_index::Index` | `deferred` | embeddings |
| `index export` | b | `embedding_cmd.rs` | `general::embedding_index::Index` | `deferred` | embeddings |
| `index export-inputs` | b | `embedding_cmd.rs` | `general::embedding_index::Index` | `deferred` | embeddings |
| `index calibrate` | b | `embedding_cmd.rs` | `general::embedding_index::Index` | `deferred` | embeddings |
| `index build` | b | `embedding_cmd.rs` | `general::embedding_index::Index` | `deferred` | embeddings |
| `index update` | b | `embedding_cmd.rs` | `general::embedding_index::Index` | `deferred` | embeddings |
| `index query` | b | `embedding_cmd.rs` | `general::embedding_index::Index` | `deferred` | embeddings |
| `hash` | b | `hash_cmd.rs` | `general::hashing::hash` | `deferred` | default / runtime feature refusal where applicable |
| `dedupe` | b | `hash_cmd.rs` | `general::hashing::clusters` | `deferred` | default / runtime feature refusal where applicable |
| `split-review` | b | `split_review_cmd.rs` | `general::split_review::review` | `deferred` | default / runtime feature refusal where applicable |
| `analyze-media` | b | `media_cmd.rs` | `media::Analyzer::analyze_media` | `deferred` | default / runtime feature refusal where applicable |
| `keyframes` | b | `media_cmd.rs` | `media::video::keyframes` | `deferred` | default / runtime feature refusal where applicable |
| `find-usage` | b | `media_cmd.rs` | `media::usage::find_usage` | `deferred` | default / runtime feature refusal where applicable |
| `models` | b | `wave7_cmd.rs` | `wave7::models::{selections,ensure,verify}` | `deferred` | default / runtime feature refusal where applicable |
| `models list` | b | `wave7_cmd.rs` | `wave7::models::{selections,ensure,verify}` | `deferred` | default / runtime feature refusal where applicable |
| `models config` | b | `wave7_cmd.rs` | `wave7::models::{selections,ensure,verify}` | `deferred` | default / runtime feature refusal where applicable |
| `models pull` | b | `wave7_cmd.rs` | `wave7::models::{selections,ensure,verify}` | `deferred` | default / runtime feature refusal where applicable |
| `locate` | b | `wave7_cmd.rs` | `wave7::locating` | `deferred` | local-models |
| `quality-score` | b | `wave7_cmd.rs` | `wave7::quality::measure` | `deferred` | local-models |
| `watermark` | b | `wave7_cmd.rs` | `wave7::watermark::inspect` | `deferred` | default / runtime feature refusal where applicable |
| `faces` | b | `wave7_cmd.rs` | `wave7::faces::detect` | `deferred` | local-models |
| `crop-check` | b | `wave7_cmd.rs` | `wave7::faces::crop_check` | `deferred` | default / runtime feature refusal where applicable |
| `observe-local` | b | `wave7_cmd.rs` | `wave7::observing` | `deferred` | local-vlm |
| `provider-map` | b | `wave7_cmd.rs` | `wave7::providers` | `deferred` | vision-providers |
| `renderdoc-localize` | b | `renderdoc_cmd.rs` | `renderdoc::localize` | `deferred` | default / runtime feature refusal where applicable |
| `regions` | b | `region_cmd.rs` | `semantic::{import,cache,runtime_probe}` | `deferred` | semantic-regions (runtime parts) |
| `regions import` | b | `region_cmd.rs` | `semantic::{import,cache,runtime_probe}` | `deferred` | semantic-regions (runtime parts) |
| `regions status` | b | `region_cmd.rs` | `semantic::{import,cache,runtime_probe}` | `deferred` | semantic-regions (runtime parts) |
| `regions cache` | b | `region_cmd.rs` | `semantic::{import,cache,runtime_probe}` | `deferred` | semantic-regions (runtime parts) |
| `regions runtime-probe` | b | `region_cmd.rs` | `semantic::{import,cache,runtime_probe}` | `deferred` | semantic-regions (runtime parts) |
| `explain-grounded` | b | `grounded_cmd.rs` | `grounded::{report_catalog,verify}` | `deferred` | default / runtime feature refusal where applicable |
| `localized-check` | b | `localized_cmd.rs` | `localized::{freeze,measure}` | `deferred` | default / runtime feature refusal where applicable |
| `inventory` | b | `inventory_cmd.rs` | `inventory::reconcile` | `deferred` | default / runtime feature refusal where applicable |
| `quality-sweep` | b | `quality_cmd.rs` | `quality::sweep` | `deferred` | compression |
| `history` | c | `history.rs` | `onset::detect (onset statistic only)` | `workflows::history::{record_report,analyze_history,analyze_onset}` | default / runtime feature refusal where applicable |
| `history onset` | c | `history.rs` | `onset::detect (onset statistic only)` | `workflows::history::{record_report,analyze_history,analyze_onset}` | default / runtime feature refusal where applicable |
| `history record` | c | `history.rs` | `onset::detect (onset statistic only)` | `workflows::history::{record_report,analyze_history,analyze_onset}` | default / runtime feature refusal where applicable |
| `history analyze` | c | `history.rs` | `onset::detect (onset statistic only)` | `workflows::history::{record_report,analyze_history,analyze_onset}` | default / runtime feature refusal where applicable |
| `bisect` | c | `git_bisect.rs` | `bisect (capture-run divergence, not Git driver)` | `deferred` | default / runtime feature refusal where applicable |
| `ingest` | c | `ingest.rs + engine_ingest.rs` | `run::run (measurement only)` | `workflows::ingest::{run_playwright,run_engine}` | default / runtime feature refusal where applicable |
| `ingest blender` | c | `ingest.rs + engine_ingest.rs` | `run::run (measurement only)` | `workflows::ingest::{run_playwright,run_engine}` | default / runtime feature refusal where applicable |
| `ingest bevy` | c | `ingest.rs + engine_ingest.rs` | `run::run (measurement only)` | `workflows::ingest::{run_playwright,run_engine}` | default / runtime feature refusal where applicable |
| `ingest unity` | c | `ingest.rs + engine_ingest.rs` | `run::run (measurement only)` | `workflows::ingest::{run_playwright,run_engine}` | default / runtime feature refusal where applicable |
| `ingest unreal` | c | `ingest.rs + engine_ingest.rs` | `run::run (measurement only)` | `workflows::ingest::{run_playwright,run_engine}` | default / runtime feature refusal where applicable |
| `ingest playwright` | c | `ingest.rs + engine_ingest.rs` | `run::run (measurement only)` | `workflows::ingest::{run_playwright,run_engine}` | default / runtime feature refusal where applicable |
| `doctor` | c | `main.rs` | `optional::diagnose (optional dependencies only)` | `deferred` | default / runtime feature refusal where applicable |
| `init` | c | `f1.rs` | `none: scaffolding/sample generation` | `retained CLI` | default / runtime feature refusal where applicable |
| `demo` | c | `f1.rs + demo_assets.rs` | `none: demonstration assets and browser launch` | `retained CLI` | default / runtime feature refusal where applicable |
| `compare` | b | `main.rs + capability_cmd.rs + general_cmd.rs + documents_cmd.rs` | `run::{run,run_approved}` | `workflows::run_compare (stock); non-stock routes deferred` | default / runtime feature refusal where applicable |
| `identity` | b | `main.rs` | `run::run with Mode::Identity` | `workflows::run_prove` | default / runtime feature refusal where applicable |
| `prove` | b | `main.rs + perf_cmd.rs + geometry_cmd.rs` | `run::run; ablate::run_repeats; geometry` | `workflows::run_prove (identity); performance/mesh wrappers deferred` | default / runtime feature refusal where applicable |
| `prove mesh-identity` | b | `main.rs + perf_cmd.rs + geometry_cmd.rs` | `run::run; ablate::run_repeats; geometry` | `workflows::run_prove (identity); performance/mesh wrappers deferred` | geometry |
| `prove identity` | b | `main.rs + perf_cmd.rs + geometry_cmd.rs` | `run::run; ablate::run_repeats; geometry` | `workflows::run_prove (identity); performance/mesh wrappers deferred` | default / runtime feature refusal where applicable |
| `prove performance` | b | `main.rs + perf_cmd.rs + geometry_cmd.rs` | `run::run; ablate::run_repeats; geometry` | `workflows::run_prove (identity); performance/mesh wrappers deferred` | graphics |
| `noise` | b | `f1.rs + perf_cmd.rs` | `ergonomics::noise_with_perf_options; perf::noise_record` | `deferred` | default / runtime feature refusal where applicable |
| `noise build` | b | `f1.rs + perf_cmd.rs` | `ergonomics::noise_with_perf_options; perf::noise_record` | `deferred` | default / runtime feature refusal where applicable |
| `view` | b | `main.rs + local_cmd.rs` | `view::build_view; view::unblind` | `retained CLI` | default / runtime feature refusal where applicable |
| `approve` | c | `approval.rs + signed_approval.rs` | `evidence::human::HumanDecision::validate_for (binding only)` | `workflows::approval::run; workflows::signed_approval::Policy` | default / runtime feature refusal where applicable |
| `serve` | b | `main.rs` | `serve::serve` | `retained CLI` | workbench |
| `mcp` | c | `mcp.rs` | `none: interactive protocol host` | `retained CLI` | mcp |
| `inspect` | b | `local_cmd.rs + f1.rs` | `ergonomics::entries; explain::explain; exclusions::text; render` | `deferred` | default / runtime feature refusal where applicable |
| `inspect exclusions` | b | `local_cmd.rs + f1.rs` | `ergonomics::entries; explain::explain; exclusions::text; render` | `deferred` | default / runtime feature refusal where applicable |
| `inspect evidence` | b | `local_cmd.rs + f1.rs` | `ergonomics::entries; explain::explain; exclusions::text; render` | `deferred` | default / runtime feature refusal where applicable |
| `inspect export` | b | `local_cmd.rs + f1.rs` | `ergonomics::entries; explain::explain; exclusions::text; render` | `deferred` | default / runtime feature refusal where applicable |
| `inspect config` | b | `local_cmd.rs + f1.rs` | `ergonomics::entries; explain::explain; exclusions::text; render` | `deferred` | default / runtime feature refusal where applicable |
| `inspect capabilities` | c | `capability_cmd.rs` | `COMPILED_FEATURES (inventory only)` | `deferred` | default / runtime feature refusal where applicable |
| `review` | b | `review_cmd.rs + local_cmd.rs` | `judge_evidence; decision_provider; evidence` | `workflows::review::{run_review,preview_local}; other families below` | default / runtime feature refusal where applicable |
| `review board` | a | `review_board_cmd.rs` | `review_board::{prepare,collect}` | `existing` | default / runtime feature refusal where applicable |
| `review board prepare` | a | `review_board_cmd.rs` | `review_board::{prepare,collect}` | `existing` | default / runtime feature refusal where applicable |
| `review board collect` | a | `review_board_cmd.rs` | `review_board::{prepare,collect}` | `existing` | default / runtime feature refusal where applicable |
| `review trial` | b | `runs_cmd.rs` | `runs` | `deferred` | default / runtime feature refusal where applicable |
| `review trial register` | b | `runs_cmd.rs` | `runs` | `deferred` | default / runtime feature refusal where applicable |
| `review trial start` | b | `runs_cmd.rs` | `runs` | `deferred` | default / runtime feature refusal where applicable |
| `review trial vote` | b | `runs_cmd.rs` | `runs` | `deferred` | default / runtime feature refusal where applicable |
| `review trial import` | b | `runs_cmd.rs` | `runs` | `deferred` | default / runtime feature refusal where applicable |
| `review assist` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review assist batch` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review assist batch submit` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review assist batch status` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review assist batch collect` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review explain` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review audit-mask` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review check-ui` | b | `advice_cmd.rs + assist_cmd.rs + assist_batch_cmd.rs` | `assist` | `deferred` | assist |
| `review brand` | b | `brand_cmd.rs` | `brand` | `deferred` | default / runtime feature refusal where applicable |
| `review ui` | b | `ui_review_cmd.rs` | `ui_review` | `deferred` | default / runtime feature refusal where applicable |
| `review motion` | b | `motion_cmd.rs` | `dense_motion; frame_map` | `deferred` | default / runtime feature refusal where applicable |
| `review request` | b | `local_cmd.rs` | `evidence::request` | `deferred` | default / runtime feature refusal where applicable |
| `review propose` | b | `local_cmd.rs` | `evidence::decision; decision_provider` | `deferred` | default / runtime feature refusal where applicable |
| `review ask` | b | `local_cmd.rs` | `evidence::human` | `deferred` | default / runtime feature refusal where applicable |
| `review eval` | b | `review_cmd.rs` | `judge_bench::evaluator::{run,plan}` | `deferred` | evaluation |
| `experiment` | b | `main.rs + perf_cmd.rs + temporal_cmd.rs + s6.rs` | `ablate::run_repeats; sequence::run_sequence; rank::run_rank` | `deferred` | default / runtime feature refusal where applicable |
| `experiment transition` | b | `captured_sequence_cmd.rs` | `captured_sequence::transition` | `deferred` | graphics |
| `experiment animation` | b | `captured_sequence_cmd.rs` | `captured_sequence::animation` | `deferred` | graphics |
| `experiment settle` | b | `wave11_cmd.rs` | `settling::analyze` | `deferred` | default / runtime feature refusal where applicable |
| `experiment reference` | b | `wave9_cmd.rs` | `asset_views` | `deferred` | default / runtime feature refusal where applicable |
| `experiment geometry` | b | `geometry_cmd.rs` | `geometry` | `deferred` | geometry |
| `experiment ablate` | b | `main.rs + perf_cmd.rs + temporal_cmd.rs + s6.rs` | `ablate::run_repeats; sequence::run_sequence; rank::run_rank` | `deferred` | graphics |
| `experiment temporal` | b | `temporal_cmd.rs` | `sequence; ColorVideoVDP adapter in CLI` | `deferred` | graphics |
| `experiment sequence` | b | `main.rs + perf_cmd.rs + temporal_cmd.rs + s6.rs` | `ablate::run_repeats; sequence::run_sequence; rank::run_rank` | `deferred` | default / runtime feature refusal where applicable |
| `experiment rank` | b | `main.rs + perf_cmd.rs + temporal_cmd.rs + s6.rs` | `ablate::run_repeats; sequence::run_sequence; rank::run_rank` | `deferred` | default / runtime feature refusal where applicable |
| `experiment bisect` | b | `main.rs + perf_cmd.rs + temporal_cmd.rs + s6.rs` | `ablate::run_repeats; sequence::run_sequence; rank::run_rank` | `deferred` | default / runtime feature refusal where applicable |
| `experiment safety` | b | `precheck.rs` | `safety` | `deferred` | prechecks |
| `experiment a11y` | b | `precheck.rs` | `a11y` | `deferred` | prechecks |
| `batch-probe` | a | `batch_cmd.rs` | `batch::probe` | `existing` | default / runtime feature refusal where applicable |

## Remaining orchestration and retained hosts

These are source-file sizes in lines after extraction, including parser, formatting
and tests. They are upper bounds for a later move, not effort estimates or duplicated
backlog work. The command table assigns each deferred operation to its files.
Non-stock compare routes (question dispatch, registration and document workers),
performance/mesh proof wrappers and auxiliary review families remain explicitly
deferred because they have distinct output contracts and execution envelopes.

| Deferred CLI source | Lines |
| --- | ---: |
| `crates/saccade/src/advice_cmd.rs` | 116 |
| `crates/saccade/src/assess_cmd.rs` | 67 |
| `crates/saccade/src/assist_batch_cmd.rs` | 440 |
| `crates/saccade/src/assist_cmd.rs` | 1279 |
| `crates/saccade/src/brand_cmd.rs` | 54 |
| `crates/saccade/src/capability_cmd.rs` | 806 |
| `crates/saccade/src/captured_sequence_cmd.rs` | 236 |
| `crates/saccade/src/critical_text_cmd.rs` | 91 |
| `crates/saccade/src/design_cmd.rs` | 980 |
| `crates/saccade/src/documents_cmd.rs` | 250 |
| `crates/saccade/src/embedding_cmd.rs` | 719 |
| `crates/saccade/src/f1.rs` | 562 |
| `crates/saccade/src/general_cmd.rs` | 442 |
| `crates/saccade/src/geo_cmd.rs` | 154 |
| `crates/saccade/src/geometry_cmd.rs` | 139 |
| `crates/saccade/src/git_bisect.rs` | 349 |
| `crates/saccade/src/grounded_cmd.rs` | 109 |
| `crates/saccade/src/hash_cmd.rs` | 152 |
| `crates/saccade/src/imgtune_cmd.rs` | 573 |
| `crates/saccade/src/inspect_image_cmd.rs` | 256 |
| `crates/saccade/src/inventory_cmd.rs` | 55 |
| `crates/saccade/src/local_cmd.rs` | 1288 |
| `crates/saccade/src/localized_cmd.rs` | 197 |
| `crates/saccade/src/main.rs` | 3051 |
| `crates/saccade/src/measure_cmd.rs` | 444 |
| `crates/saccade/src/media_cmd.rs` | 291 |
| `crates/saccade/src/motion_cmd.rs` | 98 |
| `crates/saccade/src/notifier_cmd.rs` | 214 |
| `crates/saccade/src/optical_code_cmd.rs` | 147 |
| `crates/saccade/src/perf_cmd.rs` | 322 |
| `crates/saccade/src/precheck.rs` | 113 |
| `crates/saccade/src/quality_cmd.rs` | 47 |
| `crates/saccade/src/region_cmd.rs` | 138 |
| `crates/saccade/src/region_export_cmd.rs` | 207 |
| `crates/saccade/src/renderdoc_cmd.rs` | 117 |
| `crates/saccade/src/replay_cmd.rs` | 882 |
| `crates/saccade/src/review_cmd.rs` | 254 |
| `crates/saccade/src/runs_cmd.rs` | 179 |
| `crates/saccade/src/s6.rs` | 95 |
| `crates/saccade/src/schema_cmd.rs` | 205 |
| `crates/saccade/src/split_review_cmd.rs` | 117 |
| `crates/saccade/src/sweep_cmd.rs` | 685 |
| `crates/saccade/src/temporal_cmd.rs` | 398 |
| `crates/saccade/src/text_cmd.rs` | 335 |
| `crates/saccade/src/text_quality_cmd.rs` | 357 |
| `crates/saccade/src/timed_text_cmd.rs` | 343 |
| `crates/saccade/src/ui_review_cmd.rs` | 316 |
| `crates/saccade/src/wave11_cmd.rs` | 346 |
| `crates/saccade/src/wave7_cmd.rs` | 1161 |
| `crates/saccade/src/wave9_cmd.rs` | 220 |

MCP stays in the CLI because it owns a process lifetime, protocol streams and startup
authority. `view`/`serve` stay because they open browsers or host an interactive HTTP
session; existing `view::build_view` and `serve::serve` remain usable explicitly.
`demo`/`init` stay because they scaffold sample projects and drive the CLI demonstration.
No new dependencies or command spellings are required by those retention decisions.

The previously CLI-only `saccade-playwright.v1` intake and
`saccade-playwright-mapping.v1` output schemas are now embedded. Schema discovery
additively lists these existing contracts; their payload bytes are unchanged.

## Rust examples

The following examples compile as `saccade-core` doctests. Paths are placeholders;
no example performs network/provider calls or downloads. All command errors retain
stable `code`, `message` and `hint` fields.

### Compare and exact proof

```no_run
use std::path::Path;
use saccade_core::{config::RunConfig, report::Mode, workflows::*};
let config = RunConfig::default();
let policy = signed_approval::Policy::default();
let intent = IntentOptions::default();
let options = CompareRun { baseline: Path::new("reference"),
    capture: Path::new("candidate"), out: Path::new("report"),
    config: &config, policy: &policy, approved: false, verified: None,
    junit: None, intent: &intent };
let comparison = run_compare(&options)?;
assert_eq!(comparison.is_regression(), comparison.report.is_regression());
let mut exact = config.clone();
exact.mode = Mode::Identity;
let proof = run_prove(&CompareRun { config: &exact, ..options })?;
assert!(!proof.report.entries.is_empty());
# Ok::<(), CommandError>(())
```

### Approval and external signatures

```no_run
use std::path::Path;
use saccade_core::workflows::{approval, signed_approval, CommandError};
let policy = signed_approval::Policy::load(Path::new("/etc/saccade/approval-policy.json"),
    Some(Path::new("/users/reviewer")), true, None, Path::new("/project"))?;
let inventory = policy.check_baseline(Path::new("reference"))?;
assert!(!inventory.is_empty());
let draft = approval::run(Path::new("report/saccade-report.v1.json"),
    Path::new("candidate"), Path::new("reference"), None,
    approval::Options { policy, names: vec!["page.png".into()], dry_run: true,
        out: Some("review-plan".into()), ..Default::default() })?;
assert!(draft.dry_run);
# Ok::<(), CommandError>(())
```

### History

```no_run
# #[cfg(any(unix, windows))] {
use std::path::Path;
use saccade_core::workflows::{history::*, CommandError};
let recorded = record_report(Path::new("report/saccade-report.v1.json"),
    Path::new("history"), None)?;
let analyzed = analyze_history(&AnalyzeOptions { store: Path::new("history"),
    entry: None, drift: true, out: None, limit: 10 })?;
let preview = analyzed.preview()?;
assert_eq!(preview["policy_changed"], false);
# }
# Ok::<(), saccade_core::workflows::CommandError>(())
```

### Manifest and coverage

```no_run
use std::path::Path;
use saccade_core::workflows::{manifest::*, signed_approval::Policy, CommandError};
let policy = Policy::default();
let built = build(&BuildOptions { dir: Path::new("report"), approved_anchor: None,
    last_good: None, cases: Some(Path::new("cases.json")), policy: &policy })?;
verify(&built.path, &policy)?;
let coverage = views(&ViewsOptions { target: &built.path, out: Path::new("coverage"),
    group_by: &[], now_unix: 1_800_000_000, max_age_seconds: 2_592_000,
    policy: &policy })?;
assert!(coverage.counts.expected >= coverage.counts.measured);
# Ok::<(), CommandError>(())
```

### Batch

```no_run
# #[cfg(any(unix, windows))] {
use std::path::Path;
use saccade_core::{batch::Options, workflows::{batch::*, signed_approval::Policy, CommandError}};
let options = Options::default();
let policy = Policy::default();
let completed = run_batch(&BatchRun { source: Path::new("captures"), reference_dir: None,
    out: Path::new("batch-report"), options: &options,
    executable: Path::new("/usr/local/bin/saccade"), policy: &policy })?;
assert_eq!(completed.rows, completed.counts.values().sum::<usize>());
# }
# Ok::<(), saccade_core::workflows::CommandError>(())
```

### Review

```no_run
# #[cfg(feature = "ai")] {
use std::path::Path;
use saccade_core::workflows::{review::*, CommandError};
let user = load_user(Path::new("user.toml"))?;
let roots = saccade_core::root_policy::RootPolicy::new(&["captures".into()],
    Some(Path::new("reports")), false, &[])?;
let authorization = authorization(false, 4, "review-run", &user);
let preview = run_review(ReviewRun { file: Path::new("captures/case.json"), out: None,
    run: false, budget: 4, config: Path::new("user.toml"), roots: &roots,
    authorization: &authorization, intent: None })?;
assert_eq!(preview.value["data"]["provider_calls_authorized"], false);
# }
# Ok::<(), saccade_core::workflows::CommandError>(())
```

### Screenshot ingest

```no_run
use std::path::Path;
use saccade_core::workflows::{ingest::*, signed_approval::Policy, CommandError};
let report = run_playwright(Path::new("screenshots.json"), Path::new("ingested"),
    None, false, &Policy::default())?;
assert!(!report.report.entries.is_empty());
# Ok::<(), CommandError>(())
```
