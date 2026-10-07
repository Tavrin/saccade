# Batch intake and advisory navigation

Batch execution uses the installed CLI in bounded worker processes. Threads alone
cannot stop a decoder; adding a distributed scheduler would change the scope. Reversal
cost: replace the worker boundary while preserving immutable row and intake contracts.
Python uses the same core runner with an explicit installed executable. This keeps
command semantics and feature availability identical; an in-process Python analyzer
would require separate cancellation machinery. Reversal cost: a new executor preserving
receipt semantics. Streaming remains optional future work.

Completed failures remain terminal receipts. New content or reference content produces
another row; removed inputs remain visible. Rejected: replacing failed rows on retry,
which would discard evidence. Reversal cost: add explicit versioned attempt history
before introducing a retry policy. Analysis uses snapshots to bind sections to recorded content. Comparison retains original
paths and checks hashes afterward so sidecar provenance is not lost. Rejected: image-only
snapshots for comparison, which silently discard adjacent capture metadata. The small existing MIT/Apache-2.0 fs2 dependency is enabled on Unix/Windows for
one-writer locking, including Python without AI features; no new dependency was added. Portable core targets do not acquire native batch IO.

Batch status describes execution, while each section retains its own measurement
verdict. Exit 1 from a deterministic command is a completed measurement, not a worker
failure. Unknown/unavailable states, including capture provenance, remain partial.
The deadline proof checks killable workers separately from whole-invocation overhead;
a debug executable hash is not charged to an individual item. Reversal cost: a successor status
schema if execution and measurement semantics change.

The additive assist namespace delegates to existing advice commands. The preview is
written and shown before provider dispatch, including on compatibility routes. Exact
payloads stay in the requested local output. Rejected: a second provider implementation
or granting advice baseline/exclusion authority. Reversal cost: remove the alias while
retaining preview artifacts and existing review routes. Advice remains unqualified;
fixture protocol checks do not qualify a model. MCP intake mirrors are a follow-up.
