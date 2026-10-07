#!/usr/bin/env python3
"""Generate small MIT TrustMark fixtures through the pinned upstream encoder.

No downloads or package installation. Supply a checked-out reference repository,
the two explicitly provisioned official ONNX graphs and an output directory.
Requires numpy, Pillow, torch, torchvision, omegaconf and onnxruntime.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

REVISION = "59bde8b41c973d2fac3be7ef89e3ac0fea2254fa"
ENCODER = "19b3d1b25836130ffd78775a8f61539f993375d1823ef0e59ba5b8dffb4f892d"
DECODER = "ee3268f057c9dabef680e169302f5973d0589feea86189ed229a896cc3aa88df"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--encoder", type=Path, required=True)
    parser.add_argument("--decoder", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    revision = subprocess.check_output(
        ["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        raise ValueError("reference checkout revision mismatch")
    if subprocess.check_output(["git", "-C", str(args.upstream), "status", "--porcelain", "--untracked-files=no"]):
        raise ValueError("reference checkout must be clean")
    if digest(args.encoder) != ENCODER or digest(args.decoder) != DECODER:
        raise ValueError("reference graph digest mismatch")

    import numpy as np
    import onnxruntime as ort
    from PIL import Image
    import torch
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(args.upstream / "python"))
    from trustmark import TrustMark
    from trustmark.datalayer import DataLayer

    torch.set_num_threads(1)
    options = ort.SessionOptions()
    options.intra_op_num_threads = 1
    encoder = ort.InferenceSession(str(args.encoder), options, providers=["CPUExecutionProvider"])
    decoder = ort.InferenceSession(str(args.decoder), options, providers=["CPUExecutionProvider"])

    class ReferenceGraph:
        device = "cpu"

        def __call__(self, cover, secret):
            # Official Q export produces stego in [-1,1], like the reference architecture.
            output = torch.from_numpy(encoder.run(["image"], {
                "onnx::Concat_0": cover.numpy(), "onnx::Gemm_1": secret.numpy()})[0])
            return output, output - cover

    # Bypass only the constructor's implicit downloads. Run unmodified encode(),
    # including the upstream data layer, crop/resize, residual mean and feathering.
    reference = TrustMark.__new__(TrustMark)
    reference.model_type = "Q"
    reference.use_ECC = True
    reference.device = "cpu"
    reference.model_resolution_enc = 256
    reference.aspect_ratio_lim = 2.0
    reference.concentrate_wm_region = 1.0
    reference.encoder = ReferenceGraph()

    args.out.mkdir(parents=True, exist_ok=True)
    y, x = np.mgrid[:256, :320]
    cover = Image.fromarray(np.stack([
        40 + (x * 3 + y * 2) % 170,
        50 + (x + y * 3) % 160,
        60 + (x * 2 + y) % 150], axis=-1).astype("uint8"))
    cases, vectors = [], []
    capacities = [40, 61, 68, 75]
    for schema, capacity in enumerate(capacities):
        reference.ecc = DataLayer(100, encoding_mode=schema)
        for variant in range(2):
            payload = "".join(str(int(((i * 13 + i // 3 + schema) % 7 < 3) ^ bool(variant)))
                              for i in range(capacity))
            image = reference.encode(cover, payload, MODE="binary")
            name = f"schema-{schema}-payload-{variant}.png"
            image.save(args.out / name, optimize=True)
            a = np.asarray(image.resize((256, 256), Image.Resampling.BILINEAR), dtype="float32")
            a = a.transpose(2, 0, 1)[None] / 127.5 - 1.0
            logits = decoder.run(["output"], {"image": a})[0]
            bits = logits > 0
            recovered, detected, decoded_schema = reference.ecc.decode_bitstream(bits, MODE="binary")[0]
            if not detected or recovered != payload or decoded_schema != schema:
                raise ValueError(f"reference decoder failed: {name}")
            cases.append({"file": name, "sha256": digest(args.out / name),
                          "payload_bits": payload, "schema_value": schema})
            packet = reference.ecc.encode_binary([payload])[0]
            vectors.append({"packet": "".join(str(int(b)) for b in packet),
                            "payload_bits": payload, "schema_value": schema})
    receipt = {
        "license": "MIT", "reference_repository": "https://github.com/adobe/trustmark",
        "reference_commit": REVISION,
        "reference_method": "python/trustmark/trustmark.py TrustMark.encode; constructor bypassed to prevent downloads; official ONNX encoder replaces only network forward",
        "encoder_sha256": ENCODER, "decoder_sha256": DECODER,
        "encoder_bytes": args.encoder.stat().st_size,
        "encoder_url": "https://cai-watermark.adobe.net/watermarking/trustmark-models/encoder_Q.onnx",
        "cover": "generated RGB modular ramps, 320x256; no external source images",
        "reference_decode": "Pillow bilinear 256x256, RGB/127.5-1, logits>0, upstream DataLayer.decode_bitstream MODE=binary",
        "packages": {"numpy": np.__version__, "onnxruntime": ort.__version__, "torch": torch.__version__,
                     "Pillow": Image.__version__},
        "cases": cases, "bch_vectors": vectors,
    }
    (args.out / "provenance.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"reference fixture roundtrips: {len(cases)}/{len(cases)}")


if __name__ == "__main__":
    main()
