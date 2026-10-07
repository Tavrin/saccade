# G12 unknown-cost settlement and partial pilot scoring

An interrupted call without a generation ID cannot establish provider cost by
lookup. `--resume --settle-unknown-at-reservation` explicitly accepts each unknown
receipt's full, pre-admitted reservation as a conservative charge. Monetary
counters already retain that charge and remain unchanged. Known costs, permanent
stops, campaign identity, allowance and answer safety rules remain unchanged.
Without this flag, unknown charges still refuse resume.

The durable money outcome is `settled_conservatively`; usage records the previous
outcome, previous unknown actual cost, full charge, operator method and timestamp.
It remains ineligible for qualification. Provider reconciliation history remains
intact, and later reconciliation skips conservative settlements. The aggregate
summary counts conservative receipts separately and never calls them matched.
The runner validates every receipt's frozen root/payload binding before settlement,
records the request as `not_run_transport_failure`, preserves its previous code,
and skips it permanently, including recovery after a crash before artifact export.
Other untouched requests can then continue under the original envelope.

Rejected alternatives: guessing zero spend, releasing the reservation, calling
conservative settlement reconciliation, redispatching the failed root, resetting
allowance, or clearing permanent stops. Reserved crash receipts are also settled
at their full reservation because dispatch cannot safely be ruled out.

Transport diagnostics use typed ureq/I/O errors and closed codes only: timeout,
connect, reset, TLS, body read/decode, stream/chunk termination and HTTP protocol.
Body-stage errors lacking a more specific type become body read/decode failures.
No endpoint, body or error display text enters a diagnostic. The pinned ureq 3.3
client implements HTTP/1.1 only and exposes no HTTP/2 error type; an HTTP/2-specific
classification cannot be produced truthfully by this client. HTTP protocol errors
are classified without claiming an unobserved HTTP/2 failure. No client migration
or provider call is part of this change.

`scripts/assist/pilot_score.py` is a read-only equivalent of the synthetic scorer
for this partial live dialect. It verifies frozen corpus/oracle witnesses, complete
schedule topology, request/answer/response/provenance/money bindings and local
source-only results. `--source-revision` verifies the historical corpus against an
archived Git source snapshot, requiring unchanged oracle/scoring/payload modules;
it does not relax the synthetic scorer's source identity gate. Input hashes and
the original commit/source hash are included in JSON. All quality judgments use
`score.assertion_correct` and `score.task_evidence`, without padding geometry or
changing qualification gates. Paired orders and counterfactuals remain one root;
missing or inconsistent orders are unavailable, including settled transport
failures even if stale answer files exist. Development, calibration and heldout
results are separate. Empty precision or request-rate denominators are unavailable.

The original 133-answer pilot is partial and unqualified. Nominal one-sided 95%
Clopper-Pearson bounds are reported per metric, with best achievable all-success
and zero-failure limits. Request-rate bounds do not imply request independence;
shared template families and previously inspected heldout roots prevent a fresh
prospective qualification claim. Known billed cost and charges retaining unknown
reservations are separate. Latency includes completed cold calls only.

Focused checks cover retained allowance, duplicate settlement, stopped campaigns,
crash recovery, binding drift, next-root continuation, argument scope, typed
sanitized transport errors, missing orders/descendants, stale failed-root answers,
unknown charges, empty samples and exact confidence support. All execution uses
offline fixtures; no provider behavior is established by these tests.
