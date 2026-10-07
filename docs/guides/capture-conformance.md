# Validate external capture receipts

Use the [capture kit](../capture-kit.md) before comparing externally acquired
images. These examples validate generated evidence without executing a producer.

```sh case=known-good exit=0 says='"conformant":true'
saccade capture conform examples/capture-kit/renderer-valid.json --json
saccade capture conform examples/capture-kit/browser-valid.json --json
```

Failed acquisitions remain visible even if an image is still present:

```sh case=known-bad exit=1 says=acquisition_failed
saccade capture conform examples/capture-kit/browser-failed-acquisition.json --json
```

Changed image bindings cannot pass:

```sh case=known-bad exit=1 says=stale_hash
saccade capture conform examples/capture-kit/renderer-stale-hash.json --json
```

```sh case=missing-input exit=2 says=record_unavailable
saccade capture conform absent-receipt.json --json
```

Conformance needs no optional dependency. An unavailable contract version is
refused explicitly; install a reader supporting that version:

```sh case=unavailable-dependency exit=2 says=unsupported_schema
saccade capture conform examples/capture-kit/browser-unsupported-schema.json --json
```
