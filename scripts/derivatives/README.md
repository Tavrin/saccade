# Generated derivative proofs

Run with Python 3 and Pillow, a locally installed **NotoSans-Regular.ttf** (SIL
Open Font License 1.1), and a built binary. No imagery or models are downloaded.

```sh
python3 scripts/derivatives/fixtures.py --font "$(fc-match -f '%{file}' 'Noto Sans')" --out /tmp/derivative-proof --bin saccade
```

Use a new output directory. Generated pixels and declarations are licensed MIT
OR Apache-2.0. `provenance.json` records the generator, Pillow version, font
identity/licence/hash and input hashes. The two constructed domains are a shaded
product-photo-style scene with a portrait and small package text, and a map tile
with a location marker and label. The runner asserts full-size pixel text passes,
tiny text fails, unsafe crops fail and manifests verify. It also replays a
source-bound generated face receipt to prove face-box crop handling.

This is constructed geometry and pixel-text evidence. The portrait is procedural,
the face replay is a stand-in, and neither establishes real-photo face-model
recall, human readability, export fidelity or a publication decision. When native
models are absent, the native section is explicitly unavailable; declared subject
boxes supply the scoped geometry checks.
