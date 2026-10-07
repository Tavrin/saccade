# TrustMark payload decoding

## Licence gate

The gate passed on 2026-10-07 against Adobe's upstream commit
`59bde8b41c973d2fac3be7ef89e3ac0fea2254fa`:

- [LICENSE](https://github.com/adobe/trustmark/blob/59bde8b41c973d2fac3be7ef89e3ac0fea2254fa/LICENSE),
  lines 7–25: MIT permission, including commercial use, with notice retention.
- [README.md, License](https://github.com/adobe/trustmark/blob/59bde8b41c973d2fac3be7ef89e3ac0fea2254fa/README.md#license):
  explicitly applies MIT to both repository code and downloaded model files.
- [rust/LICENSE](https://github.com/adobe/trustmark/blob/59bde8b41c973d2fac3be7ef89e3ac0fea2254fa/rust/LICENSE)
  and [python/LICENSE](https://github.com/adobe/trustmark/blob/59bde8b41c973d2fac3be7ef89e3ac0fea2254fa/python/LICENSE)
  also grant MIT use for the reference implementations.
- [rust/crates/xtask/src/main.rs](https://github.com/adobe/trustmark/blob/59bde8b41c973d2fac3be7ef89e3ac0fea2254fa/rust/crates/xtask/src/main.rs)
  identifies the official Q ONNX encoder and decoder distribution endpoint.

The decoder pin remains SHA-256
`ee3268f057c9dabef680e169302f5973d0589feea86189ed229a896cc3aa88df`
(47,401,222 bytes). Its already provisioned cache bytes were verified again.
No weights are distributed in this repository. Provisioning uses explicit
`models pull` commands; watermark inspection must never download.

## Decoder policy

Use the Q neural decoder and the Python reference data-layer definitions:
GF(128), primitive polynomial 137; BCH_SUPER/5/4/3 schemas 0/1/2/3,
capacities 40/61/68/75 payload bits and correction limits 8/5/4/3.
Data is padded to a byte boundary before computing BCH; those virtual padding
bits are not transmitted. Two reserved version bits are ignored, as in the
Python reference, and the final two bits select the schema. An ECC failure
never exposes a recovered payload. Schema guessing after failure was rejected:
it differs from the Python reference and adds false-positive opportunities.

The native BCH implementation uses bounded syndrome, Berlekamp–Massey and
Chien-search operations and verifies the corrected syndromes. No encoder is
exposed as a product feature. Confidence remains unknown: ECC correction counts
are observable evidence, not a calibrated detection or authenticity probability.

Watermark results use a new strict data contract and its linked successor.
Historical schemas remain frozen. Existing legacy DWT behaviour remains separate.
Changing this policy requires a new contract and renewed reference fixtures.

## Gate isolation

The full default test gate exposed an existing review test that implicitly read
the operator's user policy while asserting the behaviour of an unconfigured
user. That policy failed its output-root/capture-root separation check before
the expected egress denial. The test now selects its own absent user-policy
file through the existing CLI option. Its denial assertion and product policy
are unchanged. Altering the operator's configuration or weakening the assertion
was rejected. Reversal cost is removing two test arguments.

An additional full `local-models,schema` test run failed the existing
`committed_schemas_match_the_rust_types` check for `saccade-quality-report.v1`: the generic
generator adds report-link fields absent from that historical schema. Neither
the quality-report source nor its schema was changed in this lane. Expanding
this lane into an unrelated contract migration was rejected. Required gates
use default and `local-models`; the new watermark contracts also receive the
focused `wave7::schemas` generator check. The extra global-schema failure is
retained as a failed check, not counted as a passing gate.

## Resource receipt

The dedicated target briefly reached 8,172,613,632 sampled bytes (7.61 GiB)
while retaining executables from multiple feature matrices. This exceeded the
decimal 8 GB envelope; no compliance claim is made for that limit. Completed
default-test executables were removed, followed by package-scoped cleanup before
the documentation build. Test receipts and binary hashes were saved first.
Subsequent builds remained below the decimal limit. The final qualification
receipt records target deletion and keeps this transient breach visible.
