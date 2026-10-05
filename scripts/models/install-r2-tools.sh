#!/bin/bash
set -euo pipefail
p=/mnt/linux-extra/saccade-models/venv/bin/python
"$p" -m pip install --no-cache-dir torch==2.5.1+cpu torchvision==0.20.1+cpu --index-url https://download.pytorch.org/whl/cpu
"$p" -m pip install --no-cache-dir transformers==4.46.3 onnx==1.17.0 onnxruntime==1.22.0 numpy==1.26.4 pillow==11.0.0 hydra-core==1.3.2 iopath==0.1.10 tqdm==4.66.6
