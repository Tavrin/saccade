# Review disagreement board

Use `review board` for an offline blind preference trial with several human or
local tool raters. The question is fixed: which displayed image do you prefer
visually? Choices are Image 1, Image 2, Tie and Unsure. This collects categorical
preferences, not correctness, approval or task success.

Prepare an operator-only plan, with paths relative to the plan file:

```json
{
  "schema": "saccade-review-board-plan.v1",
  "raters": [
    {"id": "reviewer-a", "kind": "human"},
    {"id": "reviewer-b", "kind": "human"},
    {"id": "local-tool", "kind": "tool"}
  ],
  "pairs": [
    {"id": "codec-small", "group": "codec", "first": "source.png", "second": "encoded.jpg"},
    {"id": "ui-button", "group": "ui", "first": "layout-a.png", "second": "layout-b.png"}
  ]
}
```

```sh
saccade review board prepare plan.json --out trial --json
# Give each rater only their assigned trial/rater-001 (002, 003) directory.
# Each opens index.html and privately returns returned-ballot.json.
saccade review board collect trial/trial.json \
  --ballot reviewer-a.json --ballot reviewer-b.json --ballot local-tool.json \
  --out disagreement --json
saccade view disagreement/index.html --open
saccade manifest build disagreement --json
```

The trial and output directories must be new, with existing parents. Keep
`trial.json`, the plan, source context, tool verdicts and all returned ballots
private until voting closes. The board must be outside the prepared bundle;
collection cannot add peer answers to a rater packet. Distribute only one
assigned directory per rater, never the whole operator bundle. The generated
form includes no tool verdict, peer vote, source path, source ID, grouping or
order mapping. It uses neutral names and PNG pixels re-encoded without source
metadata. Presentation order is deterministically shuffled per rater and pair;
collection privately remaps preferences to the plan's first/second labels.
This cannot hide identifying information already visible in the pixels.

The form starts with Missing selected and disables download until the rater
confirms no prior exposure to peer votes or a tool verdict. Tool raters receive
`packet.json` and the same blank `ballot.json`, fill only their own responses,
and set `blind_confirmed` only when that condition holds. No live provider
calls occur. This is an exposure declaration and packet-content boundary, not
person authentication or proof of independent judgement. An exposed rater must
be excluded or a new blind trial conducted; collection rejects an unconfirmed
ballot. It also checks exact trial, presentation and pixel hashes and rejects
duplicate rater ballots, duplicate item responses and unsupported categories.
Select a single revision explicitly before collecting.

Missing ballots, omitted responses and null answers remain visibly missing.
Unsure and Tie are recorded nominal categories; neither is imputed as a
preference. Notes are displayed as data and escaped in HTML. Each item shows
all rater cells, category counts, missing count and unanimous/disagreement/
insufficient status. Unanimous refers only to recorded votes; missing cells
remain visible. Per-rater summaries report received ballots, recorded/missing
votes, matches and disagreements over available peer comparisons, with a null
agreement rate when none exist.

The overall and per-group statistic is nominal Krippendorff alpha:
`alpha = 1 - observed_disagreement / expected_disagreement`. Coincidences on
an item with n recorded votes are weighted by `1 / (n - 1)`; items with fewer
than two votes are excluded from the statistic and remain on the board. The
expected disagreement uses pooled category counts from comparable items with
the finite-sample `N * (N - 1)` denominator. Zero expected disagreement or no
comparable items yields null with a reason, never a fabricated perfect score.
The report records both disagreement terms and contributing counts. Pooling
assumes a shared nominal question and exchangeable raters; group statistics
help avoid mixing unrelated populations. Agreement is descriptive, not rater
accuracy, calibrated reliability, independence or a statistical acceptance gate.

Collection exits 0 even for disagreement or missing votes: it completed an
advisory report. Invalid inputs, stale evidence or protocol failures exit 2
with a typed error. Newer schema versions require an upgraded reader.
`board_blind_protocol` identifies an unconfirmed exposure declaration;
`board_evidence_changed` identifies changed displayed PNGs.
No vote, aggregate, note or alpha value can approve or modify a baseline, or
override a deterministic failure. Use the separate human-authorized approval
workflow after reviewing the underlying evidence.

The output is `saccade-review-board.v2.json` and `index.html`. The frozen v1
unlinked schema has a linked v2 successor carrying the existing `report_id`
and `source_refs`; Lane C manifest discovery uses that same identity. Input
plans, trials, packets and ballots use separate v1 schemas and never become
approval records. Limits: 2–32 raters, 1–256 pairs, 4 MiB JSON files, 2048-byte
notes, bounded 8-bit PNG/JPEG inputs. No extra dependencies or Cargo feature.
MCP mirrors are a follow-up outside this lane.

The generated three-rater codec/UI fixture in
[`review_board.rs`](../crates/saccade-core/tests/review_board.rs) records both
agreement and disagreement, explanations, an explicit Unsure and a missing
vote. Its procedural images and simulated ballots are MIT OR Apache-2.0;
no downloaded imagery, human study or provider output is represented.

Reproduce a retained proof outside the input directories:

```sh
python3 scripts/prove-review-board.py --bin /path/to/saccade --out /path/to/new-proof
```

Python 3 and Pillow generate all images. `proof.json` records fixture licence,
generator and binary hashes, image hashes, command results and asserted
missingness/disagreement/alpha. The three votes are simulated fixture inputs.
