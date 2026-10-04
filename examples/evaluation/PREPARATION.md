# Corpus preparation

Design section 16 supersedes the earlier human-labeling and licence-based egress plan. The evaluation uses constructed truth and recorded outcomes. Every case permits private provider egress; publication contains aggregates and hashes only.

`corpus.py freeze --config PRIVATE_CONFIG` builds on the 26 recorded cases and constructs 60 additional interventions from leased Moss captures. The private configuration supplies the renderer wrapper, pinned binary, capture output root, derived storage root and local executables. Captures remain in wrapper outputs. Derived images, measurements, construction records and responses remain in private corpus storage. The ignored `.local` alias retains the earlier preparation records in that storage.

The ten constructed families each contain six independent interventions. Two camera configurations on each of three procedural scenes provide the sampled inputs. A case can include several frames and presentation crops; they do not increase independent support. Entire scene/change-family groups stay in one split.

Construction truth is written before image intervention and before any provider call. Recorded truth requires a recorded verdict or qualification result. Missing recorded truth remains unknown and is excluded from scoring. Hidden truth and construction records are never provider inputs.

The frozen `moss-pilot.toml` binds the selected cases, splits, truth hashes, catalog, adapters, requested models, presentation seed, retry policy, window and request counts. `corpus-freeze.json` seals its exact bytes before dispatch. Validation rechecks the mapping, source content, generated packets, truth and adapter hashes. Resume uses immutable job identities and private receipts. Changing the manifest, rubric, adapter or packet fails closed.

The network executor lives in `scripts/evaluation/corpus_live.py`; the evaluation entrypoint is a relative symlink with identical frozen bytes. After `corpus.py score`, run `scripts/evaluation/record.py --config PRIVATE_CONFIG` to attach dispatch failures and unresolved-job counts to the public record. That record step changes no requests, questions, rubrics or scores.

`corpus.py run --config PRIVATE_CONFIG` sends actual question batches through `corpus_transport`, using the production R11 evaluation ledger. Caps are 400 Jev and 250 Gemini attempts, including retries and fallback probes. Credentials are loaded only by the production key reader. Direct Jev, enriched Jev, enriched Jev without project priors and Gemini alone have separate scores. Both anonymous orders use the same actual model or remain unresolved. Model revisions and observation response hashes remain attributed.

`corpus.py score --config PRIVATE_CONFIG` replays retained responses without provider calls. Calibration uses only its designated split. Held-out metrics include availability, coverage, conditional accuracy, per-class errors, order disagreement, latency, probability calibration and case-grouped confidence bounds. No question qualifies without all design gates, including 50 held-out applicable cases and a declared important-miss tolerance.

`preparation-pilot.toml`, `pilot.py`, and the workbench preserve the earlier optional human-labeling and replay tools. That plan is dispatch-disabled and superseded by the constructed-truth manifest. `dry-run.json` and `replay-summary.json` are preparation controls, not live quality results. RESULTS.md and RESULTS.json are the authoritative live record.
