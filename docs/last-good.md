# Last-good and notifications

`compare` retains its two-directory invocation. To use history instead:

```sh
saccade history record report/saccade-report.v1.json --store history --json
saccade compare captures/ --baseline last-good --history-store history --out next-report --json
saccade sweep compare sweep.json --captures captures/captures.json \
  --baseline last-good --history-store history --out next-sweep --json
```

Last-good selects the most recent complete passing regression report from the
existing `saccade-history.v1` store, with no invalid capture entries. "Accepted"
here means the deterministic comparison passed; it does not claim human review or
grant baseline-write authority. Timestamps order runs, with history record order breaking
ties deterministically. The history object is hash-verified, then every retained report capture (or original capture for legacy/
HDR provenance) is checked against its recorded encoded hash and copied to a temporary
immutable snapshot. Missing/changed latest passing captures are errors; selecting
an older convenient run would conceal missing evidence. No original is rewritten.

New history records retain an origin sidecar for relative capture provenance.
Legacy relative records without origins fail with `last_good_unavailable`; record
the original report again with this build. Absolute-path historical provenance can
be resolved directly. Keep retained report captures available (and HDR originals where needed). Sweep IDs and image
names must match the accepted run to establish pairing; missing pairs stay failures.

Notifications use `products` and a generic JSON, Slack-compatible plain-text or
Teams-compatible Adaptive Card payload:

```sh
saccade notify report/saccade-report.v1.json --template generic \
  --report-link https://example.com/report --json
```

Configure `WEBHOOK_URL` and optional `WEBHOOK_TOKEN` (Bearer header) only in
`~/.config/saccade/webhook.env`. The URL must use HTTPS. No CLI/project input can
supply a webhook secret. Payloads contain `saccade-notification.v1` counts, up to
five worst items and report link/path; delivery returns `saccade-notify-result.v1`.
Endpoint/token and response bodies never appear in the result or transport errors.
Requests have bounded bodies, 30-second timeouts, finite rate-limit backoff, and
no redirects. Explicit `notify` is the only CLI operation that sends a webhook;
comparisons do not send messages automatically. Exit 0 is successful delivery;
2 is a config/transport/status failure. Network tests use an ignored local-server
fixture, never a real webhook.
