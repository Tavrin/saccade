# Wave 4 decisions

Scope: Shared rules and Part A only. Worktree `wave4`, branch `feat/wave4`, starting at `cccd6e35d52f1472d7b51297cd20d75ea8127fe6` (Wave 3 source contract present). No web reads, provider calls, model downloads, browser runs or heavy gates during development.

- Use a separate feature-gated assist module and immutable sidecars. Reject authority-bearing response fields through closed schemas. Rejected: changing grounded explanation or deterministic verdict contracts. Reversal cost: remove assist registrations and feature; existing reports remain readable.
- Reuse canonical digests, root policy, provider HTTP authorization and the existing locked ledger. Monetary accounting extends that ledger in an isolated additive namespace. Rejected: another independently trusted HTTP client or ambient credential lookup. Reversal cost: remove assist extension; persisted reservations remain conservative audit receipts.
- Require requested pinned models plus returned revisions; no fallback aliases. Unknown usage retains the full reservation and produces unknown cost. Rejected: guessing zero cost or silently substituting models. Reversal cost: explicit policy version change and a new qualification epoch.
- Keep historical evaluation unchanged. Constructed truth receives its own schema, frozen root splits and feature decisions. Rejected: fabricated human labels. Reversal cost: new constructed epoch, no historical relabelling.

Lane 1 validation: core check succeeded; seven focused assist tests passed (geometry, closed/duplicate JSON, condition identity, usage/thinking cost, cache separation/age, money failure persistence, uncertain Batch submission and partial collection, generated schema drift). Core clippy with assist/schema and `-D warnings` passed. Heavy gates and provider qualification were not run.

- Fixed rates expire at 2027-01-01; absent thinking/total counts or inconsistent usage means unknown cost. Reservations remain fully consumed when unknown, including Batch collection. Rejected: refunds based on job-level success or guessed usage. Reversal cost: explicit audited reconciliation implementation; existing receipts retain conservative charges.
- Jev's immutable returned revision format is not established by the brief. Require an explicit observed revision binding and refuse drift; expose this as a configuration requirement rather than researching the web. Reversal cost: adapter-only mapping change and a fresh epoch.
