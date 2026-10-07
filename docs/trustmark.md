# TrustMark payload decoding

The optional `local-models` feature decodes the official SHA-256-pinned TrustMark
Q ONNX graph on CPU. Models are user provisioned and never bundled:

```sh
saccade models pull runtime --json
saccade models pull trustmark --json
saccade watermark image.png --trustmark --json
```

Inspection never downloads. A missing model, runtime or compiled feature produces
an explicit `unavailable` finding, with no payload. `--allow-download` is rejected
for TrustMark inspection; provision through `models pull`. Integrity errors remain
errors. MCP `saccade_measure` operation `vision_watermark` accepts `trustmark: true`
and uses only the operator's configured registry, cache and runtime.

The new data contract is `saccade-watermark.v3`; CLI/MCP linked reports use
`saccade-watermark.v4`. Historical v1/v2 schemas and observation readers are
retained. Each verified Q finding reports:

- `status`: `detected`, `not_detected` after ECC failure, or `unavailable`.
- `payload_bits`: the exact corrected 40/61/68/75-bit payload, including padding
  originally chosen by the encoder. No text interpretation or trimming is applied.
- `payload_hex`: the same bits packed MSB first, with zero padding only in the
  final byte. Use `payload_bits` to retain the exact non-byte-aligned length.
- `schema_value`: 0/1/2/3 for BCH_SUPER/BCH_5/BCH_4/BCH_3.
- `ecc`: variant, `valid|corrected|failed`, corrected data/ECC bit count and the
  correction limit (8/5/4/3). Payload and verified schema are absent on failure.
- `confidence`: null; no calibrated confidence probability is claimed.
- `provenance`: consumed graph identity, CPU runtime and preprocessing policy.

The final two neural bits select the schema. The other two version bits are
reserved; schema bits have no BCH protection. The decoder follows the Python
reference data/ECC padding and polynomial 137, checks corrected syndromes and
does not guess other schemas when decoding fails. Q images are center-cropped
only outside aspect ratios 0.5–2 and resized with antialiased triangle filtering,
then normalized to RGB × 2/255 − 1. This corrects the earlier neural-only adapter's
normalization. Old custom `trustmark-q-v1` registry entries need the current
`trustmark-q-bch-v1` preprocessing contract.

An ECC-valid codeword is watermark evidence, not authentication of a generator,
signer or origin. Chance detections are possible. A missing watermark cannot
establish human origin or rule out AI generation. Cropping, stronger compression,
rotation and other transformations can prevent detection.

See the [licence and implementation decision](design-decisions/trustmark-decode.md)
and [reference fixtures](../crates/saccade-core/tests/fixtures/trustmark/README.md).
The constructed acceptance corpus covers all four schemas and mild JPEG/resize;
it does not establish checkpoint/export numerical parity, other model variants,
GPU execution, cross-platform behaviour or domain-wide accuracy.

The 2026-10-07 Linux CPU run with ONNX Runtime 1.22.0 recovered exact payloads
and schemas in **24/24** cases: eight reference fixtures, each tested at native
size, JPEG quality 90 and 90% resize. The twelve cases carrying the alternate
payload never matched a target payload (**0/12**). The fixed generated negative
set produced **0/128** target hits and **0/128** ECC-valid detections (observed
false-positive rate **0%**). These numbers describe this constructed corpus,
not a population rate or authenticity guarantee.

The [qualification receipt](trustmark-qualification.json) records source, model,
runtime and binary identities, gate exits, the additional global-schema failure,
and the transient decimal target-budget breach and subsequent target deletion.
