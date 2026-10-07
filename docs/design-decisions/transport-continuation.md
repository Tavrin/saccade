# Transport continuation at full reservation

Both native campaign runners accept `--continue-on-unknown-cost-at-reservation`.
Without it, an incomplete execution still stops collection. With it, an unknown
cost is immediately charged at its entire reservation, recorded as
`settled_conservatively`, and the root becomes `not_run_transport_failure`.
The unavailable root contributes no answer and is never dispatched again in the
campaign. Known pre-dispatch pacing failures also retain the entire reservation
in this mode, increasing the former zero charge. Allowance checks include every
conservative charge. Authorization, identity, usage and reconciliation breaches
remain hard failures.

The shared safety valve stops subsequent dispatch after more than five consecutive
transport failures, or more than 10 percent transport failures after 20 settled
attempts. Restored settled roots rebuild the same history before any new dispatch.
A successful answer resets the consecutive count; an unexecuted tail does not.
Checkpoints retain receipts, unavailable codes and the declared valve policy.

Transport classifications are closed sanitized values: timeout, connect, reset,
TLS, HTTP protocol, body read/decode, stream termination, client deadline, and
other with a closed error-kind marker. Untrusted error text is never retained.
Previously, `once_detailed` returned a rejection without a transport class for
pacing deadlines and configuration/accounting refusals before dispatch.
The executor recorded a zero-cost, not-dispatched incomplete receipt, so neither
the transport class nor the HTTP error explained the campaign stop. The saved G12
stop was an OpenRouter concurrent-consumer accounting refusal; that spending stop
remains binding and is not cleared by transport continuation. New receipts classify that path;
legacy null classes remain explicitly historical rather than guessed socket errors.

G12 retains its frozen campaign identity and existing resume bindings. Explicit
continuation upgrades a legacy unavailable pacing root to full reservation and
skips it. OpenRouter may later reconcile a conservative receipt with an existing
generation ID: authoritative matching identity may lower its charge, while a
higher amount or identity mismatch stops spending and records a mismatch. The
root remains unavailable even after reconciliation.

Jev accepts `--resume OUTPUT` with the original requests, campaign ID, allowance,
policy and prepaid attestation. It validates every topology position, payload,
receipt, settled response hash and answer before changing settlement. The legacy
runner's sequential receipt prefix is converted atomically to durable root
markers. New dispatches bind their root before money reservation, allowing a
checkpoint lost after settlement to recover without replay. Campaign locking
prevents concurrent runners. Changed topology, response, scorer, tariff or schema
refuses resume. Transport, accounting and runner code may be upgraded; the
original identity and binary hash remain in the ledger, and the new binary and
self-test appear in `resume-preflight.json`.

All continuation fixtures and saved-campaign replay checks are offline. No
provider completion, billing lookup or real credential read is part of these
gates. Collection, billing reconciliation and model qualification remain separate.
