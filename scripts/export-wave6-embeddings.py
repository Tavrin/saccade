#!/usr/bin/env python3
"""Coordinator-only, offline DINOv2 export and checkpoint parity vectors. No weights fetched."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True, help='Operator-reviewed local DINOv2 source checkout')
    parser.add_argument('--source-revision', required=True)
    parser.add_argument('--checkpoint', type=Path, required=True)
    parser.add_argument('--checkpoint-sha256', required=True)
    parser.add_argument('--pairs', type=Path, required=True, help='Frozen JSON array of {a,b,split,same_content}, with sample-disjoint splits')
    parser.add_argument('--scope', required=True)
    parser.add_argument('--artifact-url', required=True, help='Revision-pinned runtime HTTPS export URL')
    parser.add_argument('--entry', default='dinov2_vits14')
    parser.add_argument('--parity-tolerance', type=float, default=0.0001)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out must be a new directory')
    if not args.artifact_url.startswith('https://') or not 0 <= args.parity_tolerance <= 0.001:
        parser.error('HTTPS artifact URL and parity tolerance 0..0.001 required')
    root = args.inputs.resolve().parent
    if args.inputs.stat().st_size > 16 * 1024 * 1024 or args.pairs.stat().st_size > 1024 * 1024:
        parser.error('manifest/pair byte limit exceeded')
    inputs_bytes = args.inputs.read_bytes()
    prepared = json.loads(inputs_bytes)
    if prepared.get('schema') != 'saccade-embedding-export-inputs.v1' or not 1 <= len(prepared['samples']) <= 128:
        parser.error('invalid prepared tensor manifest')
    revision = subprocess.check_output(['git', '-C', str(args.source), 'rev-parse', 'HEAD'], text=True).strip()
    dirty = subprocess.check_output(['git', '-C', str(args.source), 'status', '--porcelain'], text=True)
    if revision != args.source_revision or dirty:
        parser.error('export source must be clean at the declared revision')
    if args.checkpoint.stat().st_size > 1024 * 1024 * 1024:
        parser.error('checkpoint exceeds 1 GiB')
    checkpoint_bytes = args.checkpoint.read_bytes()
    checkpoint_hash = digest(checkpoint_bytes)
    if checkpoint_hash != args.checkpoint_sha256:
        parser.error('checkpoint hash mismatch')
    pairs_bytes = args.pairs.read_bytes()
    pairs = json.loads(pairs_bytes)
    if not isinstance(pairs, list) or not 1 <= len(pairs) <= 4096:
        parser.error('invalid pair list')
    # Imports and computation are deliberately inside main: --help is lightweight.
    import numpy as np
    import torch
    import onnx
    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    torch.manual_seed(0)
    model = torch.hub.load(str(args.source.resolve()), args.entry, source='local', pretrained=False)
    weights = torch.load(io.BytesIO(checkpoint_bytes), map_location='cpu', weights_only=True)
    model.load_state_dict(weights.get('model', weights), strict=True)
    model.eval()
    template = prepared['model_template']
    width, height = template['size']
    shape = (1, 3, height, width)
    samples = []
    tensors = []
    args.out.mkdir()
    (args.out / 'images').mkdir()
    with torch.no_grad():
        for i, sample in enumerate(prepared['samples']):
            def contained(name):
                path = (root / name).resolve()
                if not path.is_relative_to(root):
                    parser.error('input escaped tensor corpus root')
                return path
            tensor_path = contained(sample['tensor'])
            if tensor_path.stat().st_size != width * height * 3 * 4:
                parser.error('tensor size mismatch')
            data = tensor_path.read_bytes()
            if digest(data) != sample['tensor_sha256']:
                parser.error('tensor pin mismatch')
            tensor = torch.from_numpy(np.frombuffer(data, dtype='<f4').copy().reshape(shape))
            vector = model(tensor).detach().cpu().numpy().reshape(-1).astype(np.float32)
            if vector.size != template['dimensions'] or not np.isfinite(vector).all() or np.linalg.norm(vector.astype(np.float64)) <= 1e-12:
                parser.error('source output is not the declared pooled finite embedding')
            vector = (vector.astype(np.float64) / np.linalg.norm(vector.astype(np.float64))).astype(np.float32)
            image_path = contained(sample['image'])
            if image_path.stat().st_size > 64 * 1024 * 1024:
                parser.error('parity image exceeds byte limit')
            encoded = image_path.read_bytes()
            if digest(encoded) != sample['sha256']:
                parser.error('parity image pin mismatch')
            image_name = f'images/{i:04}{image_path.suffix}'
            (args.out / image_name).write_bytes(encoded)
            samples.append({'image': image_name, 'sha256': sample['sha256'], 'tensor_sha256': sample['tensor_sha256'], 'embedding': vector.tolist()})
            if not tensors:
                tensors.append(tensor)
        graph_path = args.out / 'model.onnx'
        torch.onnx.export(model, tensors[0], str(graph_path), input_names=[template['input']], output_names=[template['output']], opset_version=17, dynamo=False)
    graph = onnx.load(graph_path, load_external_data=False)
    if any(t.data_location == onnx.TensorProto.EXTERNAL for t in graph.graph.initializer):
        parser.error('self-contained ONNX export required')
    onnx.checker.check_model(graph)
    graph_bytes = graph_path.read_bytes()
    export_hash = digest(graph_bytes)
    template['artifact'].update({'sha256': export_hash, 'bytes': len(graph_bytes), 'url': args.artifact_url, 'version': args.source_revision + ':' + checkpoint_hash, 'format': 'onnx', 'license': 'Apache-2.0'})
    template['calibration'] = None
    corpus = {'schema': 'saccade-embedding-corpus.v1', 'export_sha256': export_hash, 'checkpoint_sha256': checkpoint_hash, 'source_revision': revision, 'scope': args.scope, 'parity_tolerance': args.parity_tolerance, 'samples': samples, 'pairs': pairs}
    for name, value in [('model.json', template), ('corpus.json', corpus), ('export-receipt.json', {'schema': 'saccade-embedding-export-receipt.v1', 'checkpoint_sha256': checkpoint_hash, 'source_revision': revision, 'export_sha256': export_hash, 'inputs_sha256': digest(inputs_bytes), 'pairs_sha256': digest(pairs_bytes), 'torch': torch.__version__, 'numpy': np.__version__, 'onnx': onnx.__version__, 'script_sha256': digest(Path(__file__).read_bytes()), 'qualification': 'requires independent ONNX parity and holdout calibration gate'})]:
        (args.out / name).write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({'export_sha256': export_hash, 'corpus_sha256': digest((args.out / 'corpus.json').read_bytes()), 'models_downloaded': False}))


if __name__ == '__main__':
    main()
