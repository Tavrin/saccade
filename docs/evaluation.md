# Evaluation

`review eval --manifest FILE` plans offline; `--run` executes a declared job.
Jobs are score, calibrate or conformance. They bind inputs, labels/splits,
request identity, provider/model, rubric, presentation, fallback and policy.
Every retry/fallback reserves from the shared ledger. Root egress and endpoint
policy apply to evaluation as to review.

Keep availability separate from accuracy. An unavailable provider response is
not abstention. Synthetic controls test mechanics and denominators; they do not
count as real support or model-quality evidence.

## Corpus and qualification method

Freeze independent cases, splits, input and label hashes, rubrics, model identities,
request encodings and spend limits before dispatch. Keep related scenes, crops,
repeats and reversed presentation orders in one split; count independent cases.
Fit calibration only on the calibration split. Compare advisory providers with a
deterministic baseline, record abstentions separately from unavailable responses,
and retain failed qualification gates. Human labels require recorded reviewer
exposure and labeling limitations. Generated controls test mechanics, not general
model accuracy or publication rights.

The private renderer corpus, its captures, labels, manifests and run results are
not distributed. No question is qualified by that private evaluation. Public
constructed fixtures and a reproducible freeze/score workflow live in
[`scripts/assist/`](../scripts/assist/) and are described in
[constructed assist qualification](assist-qualification.md).

Publish only permitted artifacts. Provider egress permission does not authorize
image publication; private third-party inputs stay outside the repository.
