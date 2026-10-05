#!/usr/bin/env bash
# CPU-only local export. No package-index access; tooling comes from a local wheelhouse.
set -euo pipefail
cd "$(dirname "$0")/../.."
export CUDA_VISIBLE_DEVICES=""
export OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
cache=/mnt/linux-extra/saccade-models
free=$(df -BG --output=avail /mnt/linux-extra | tail -n1 | tr -cd '0-9')
test "${free:-0}" -ge 25 || { echo 'DEFERRED: less than 25 GB free'; exit 2; }
python3 scripts/models/sam2_export.py --prepare
python3 -m venv "$cache/venv"
# Do not fetch from PyPI or PyTorch: these hosts are outside this lane's allowed URL set.
"$cache/venv/bin/python" -m pip install --no-index --no-cache-dir --find-links "$cache/wheelhouse" \
    torch==2.5.1 torchvision==0.20.1 onnx==1.17.0 onnxruntime==1.22.0 hydra-core==1.3.2 \
    iopath==0.1.10 pillow==11.0.0 numpy==1.26.4 tqdm==4.66.6 || {
    echo 'DEFERRED: offline export tooling unavailable; no unapproved package download'; exit 2;
}
# Export only. Smoke is separately bounded in Rust once graph pins are installed.
timeout 300 nice -n 19 "$cache/venv/bin/python" scripts/models/sam2_export.py --export
