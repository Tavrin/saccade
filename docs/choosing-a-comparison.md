# Choosing a comparison

`saccade capabilities --json` returns `saccade-capabilities.v1`: every comparison
family, command, accepted inputs, compiled features, availability and limits. HDR
uses the existing HDR-FLIP exposure/viewing contract; video uses the existing
ColorVideoVDP frame-rate/display contract. Registration changes the question to
geometric overlap. Hashes retrieve candidates; embeddings observe model-dependent
content; text observes strings and positions. These are separate evidence.

```sh
saccade capabilities --json
saccade compare before/ after/ --question same-render --out renders --json
saccade compare a.png b.png --question same-render --align auto --resample reference --out aligned --json
saccade compare a.png b.png --question near-duplicate --threshold 6 --out candidates --json
saccade compare a.png b.png --question quality --out quality --json
saccade compare a.png b.png --question same-text --reference-source a.json --capture-source b.json --out text --json
saccade compare a.png b.png --question same-content --model model.json --cache models --library runtime.so --out content --json
```

| Question | Pipeline | Result boundary |
| --- | --- | --- |
| same-render | Existing comparison, or explicit registration | Existing metric/threshold; registered overlap and exclusions are recorded |
| same-content | Supplied pinned embedding export | Raw cosine/supplied bands; unknown verdict pending calibration |
| same-text | Image-bound text sources, or CLI pinned OCR contract | Literal Unicode CER/WER, changed/missing/added/moved observations |
| near-duplicate | Pairwise pHash Hamming distance | Integer threshold in bits, default 6; collision-prone candidate evidence |
| quality | Paired no-reference measures | Content-dependent deltas; unknown quality verdict |
| optical payload (standalone) | `optical-code --expect` or `--expect-pattern` | Decode/payload/QR pixel gates independent of similarity; requires `optical-code` |

Near-duplicate and quality also pair directories by relative path, up to 100000
pairs and 64 MiB path bytes. Missing/new and decode errors are retained as failures;
empty comparisons fail. Same-content and same-text currently require file pairs;
use `index build|query` for embedding directory retrieval. Quality reports include
per-pair artifacts. SVG/PDF input needs a build with the `documents` feature (`saccade doctor` shows it), see [documents](documents.md).

Every selected route records its question, family, command and reason. Ordinary
same-render keeps the existing immutable report schema and writes a hash-bound
`saccade-pipeline-choice.v1.json` component; its JSON receipt includes that choice.
Registered reports record selection under `pipeline`. Other families contain a
`pipeline_choice` object. Near-duplicate uses `saccade-near-duplicate.v1`, paired
quality uses `saccade-question-report.v1`, and text/similarity keep their family
schema. General JSON receipts point to complete versioned evidence and HTML.

Unsupported option combinations are typed usage errors, never silently discarded.
Only near-duplicate and same-render accept threshold arguments. Other questions
reject FLIP-specific options. Unavailable models/features, missing text sources or
insufficient alignment inliers never fall back to a different family. Successful
unknown measurements exit 0; observed failure/missing/empty results exit 1; typed
configuration, authority or execution errors exit 2. No route approves baselines.

MCP `saccade_general` operations `capabilities` and `compare_question` mirror the
catalogue and routes. Question arguments are `reference`, `capture`, `out`,
`question`, optional `align`, `resample`, `threshold`, `model`, `cache`, `library`,
`reference_source`, `capture_source`. Inputs remain bounded by startup read roots
and artifacts by the distinct output root. MCP imports text only; it does not
execute OCR programs. Native embedding execution additionally requires the
[operator runtime pin](embeddings.md). No downloads or provider calls occur here.

AI advisory, performance statistics, accessibility prechecks, HDR and video remain
under their existing commands and authority/qualification contracts. The catalogue
states conditional and deferred availability; it does not imply qualification.

Rendered labels and document codes are in scope through [optical-code verification](optical-code.md). Physical print grading and carrier compliance remain separate.
