# Check a configured gate's sensitivity

Freeze declared defects before measuring policy changes.
[Catalogue, rates, units and limits](../sensitivity.md).
Sensitivity on injected defects is not field recall.

A zero-tolerance policy detects a declared local edit. All inputs remain unchanged:

```sh case=known-good exit=0 says='"state":"complete"'
mkdir baseline
cp testdata/critical-text/app-decimal/baseline.png baseline/image.png
cat > catalogue.json <<'JSON'
{"schema":"saccade-sensitivity-catalogue.v1","provenance":"Procedural pack; MIT OR Apache-2.0","injections":[{"id":"local","defect":{"class":"local_edit","rect":[0,0,0.02,0.02],"colour":[255,0,0]},"magnitudes":[1]}]}
JSON
printf 'threshold=0.0\nmetric="max"\n' > strict.toml
saccade sensitivity baseline --catalogue catalogue.json --config strict.toml --out detected --json
```

The same edit passes a permissive gate, so sensitivity reports a miss:

```sh case=known-bad exit=1 says='"miss_rate":1.0'
printf 'threshold=1.0\nmetric="mean"\n' > permissive.toml
saccade sensitivity baseline --catalogue catalogue.json --config permissive.toml --out missed --json
```

Missing input is an execution failure:

```sh case=missing-input exit=2
saccade sensitivity absent --catalogue catalogue.json --config strict.toml --out absent-report --json
```

When policy selection excludes every source, detection evidence is unavailable:

```sh case=unavailable-dependency exit=4 says=insufficient_evidence
printf 'threshold=0.0\nignore=["*.png"]\n' > excluded.toml
saccade sensitivity baseline --catalogue catalogue.json --config excluded.toml --out excluded-report --json
```

Read each class/magnitude row and its eligibility counts; a null rate is no evidence.
The minimum is one detected injection at a tested value, not an all-images guarantee.
