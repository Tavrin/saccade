# TrustMark reference fixtures

These eight small PNGs are generated RGB ramps carrying two different payloads
under each of TrustMark Q's four BCH schemas. The generated image content is MIT
licensed. No source photographs or model weights are included.

`provenance.json` records exact image hashes, payloads, model hashes, generator
environment and upstream commit. The generator runs the unmodified upstream
`TrustMark.encode` method and upstream BCH encoder. Only its neural forward call
is replaced with Adobe's official Q ONNX encoder. The constructor is bypassed
to avoid automatic provisioning. Each fixture also passes the upstream data-layer
decoder with the official Q ONNX decoder before it is written to the manifest.

Reproduce with explicitly provisioned graphs and a clean tracked reference tree:

```sh
python scripts/fixtures/generate-trustmark.py \
  --upstream "$TRUSTMARK_SOURCE" --encoder "$TRUSTMARK_ENCODER" \
  --decoder "$TRUSTMARK_DECODER" \
  --out crates/saccade-core/tests/fixtures/trustmark
```

The encoder's mutable official distribution URL is recorded in the manifest;
its graph is 17,312,208 bytes, SHA-256
`19b3d1b25836130ffd78775a8f61539f993375d1823ef0e59ba5b8dffb4f892d`.
It is a fixture-generation input, never a shipped model or encoding feature.
The repository-wide [licence decision](../../../../../docs/design-decisions/trustmark-decode.md)
links the upstream MIT grants for code and model files.

The ignored `trustmark_decode` acceptance test exercises all eight fixtures at
native size, JPEG quality 90, and 90% resize, then 128 generated unwatermarked
images. It records both any ECC-valid false positives and exact target-payload
hits. Synthetic coverage does not qualify a real-world false-positive rate.

```sh
G25_TRUSTMARK_CACHE="$SACCADE_MODEL_CACHE" \
G25_TRUSTMARK_RUNTIME="$ORT_DYLIB_PATH" \
G25_TRUSTMARK_RECEIPT=trustmark-acceptance.json \
cargo test -p saccade-core --features local-models --test trustmark_decode \
  -- --ignored --nocapture
```
