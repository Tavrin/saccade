#!/usr/bin/env python3
"""Extract bounded native Vulkan replay evidence through RenderDoc 1.34 bindings.

Run on a qualified replay host. No GUI, image conversion or capture parser is
embedded. A second independent replay establishes byte-repeatability only.
"""
import argparse
import collections
import hashlib
import json
import platform
import os
import signal
import stat
import subprocess
import tempfile
from pathlib import Path
import sys

VERSION = '1.34'
SCHEMA = 'saccade-renderdoc-extract.v1'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_hash(path, max_bytes=2 * 1024 * 1024 * 1024):
    if not stat.S_ISREG(Path(path).stat().st_mode):
        raise RuntimeError('input must be a regular file')
    flags = os.O_RDONLY | getattr(os, 'O_NONBLOCK', 0)
    fd = os.open(path, flags)
    with os.fdopen(fd, 'rb') as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode):
            raise RuntimeError('input must be a regular file')
        if info.st_size > max_bytes:
            raise RuntimeError('input byte budget exceeded')
        h, consumed = hashlib.sha256(), 0
        while True:
            block = stream.read(min(65536, max_bytes + 1 - consumed))
            if not block:
                return h.hexdigest()
            consumed += len(block)
            if consumed > max_bytes:
                raise RuntimeError('input byte budget exceeded')
            h.update(block)


def extract(rd, filename, out, budget, save):
    capture = rd.OpenCaptureFile()
    controller = None
    try:
        status = capture.OpenFile(str(filename), '', None)
        if status != rd.ResultCode.Succeeded:
            raise RuntimeError(f'capture open failed: {status}')
        if capture.LocalReplaySupport() != rd.ReplaySupport.Supported:
            raise RuntimeError('local replay unavailable for this capture/device')
        options = rd.ReplayOptions()
        options.optimisation = rd.ReplayOptimisationLevel.Conservative
        status, controller = capture.OpenCapture(options, None)
        if status != rd.ResultCode.Succeeded:
            raise RuntimeError(f'replay open failed: {status}')
        if controller.GetAPIProperties().pipelineType != rd.GraphicsAPI.Vulkan:
            raise RuntimeError('this worker qualifies Vulkan extraction only')
        textures = {str(t.resourceId): t for t in controller.GetTextures()}
        buffers = {str(b.resourceId): b for b in controller.GetBuffers()}
        structured = controller.GetStructuredFile()
        actions = []
        consumed = 0

        def resource(role, descriptor, event):
            nonlocal consumed
            rid = descriptor.resource
            identity = str(rid)
            record = dict(role=role, resource_id=identity, format='unknown', dimensions=[0, 0, 0],
                          subresource=[0, 0, 0], interpretation='native_bytes', payload=None,
                          sha256=None, bytes=None, error=None)
            try:
                if identity in textures:
                    t = textures[identity]
                    if int(t.byteSize) > min(budget, 64 * 1024 * 1024):
                        raise RuntimeError('resource byte budget exceeded')
                    # One explicitly recorded view subresource, sample zero. No resolve,
                    # tonemapping, checkerboard alpha, orientation or thumbnail conversion.
                    mip, layer, sample = int(descriptor.firstMip), int(descriptor.firstSlice), 0
                    record.update(format=t.format.Name(), dimensions=[max(1, int(t.width) >> mip), max(1, int(t.height) >> mip), max(1, int(t.depth) >> mip)],
                                  subresource=[mip, layer, sample])
                    data = bytes(controller.GetTextureData(rid, rd.Subresource(mip, layer, sample)))
                elif identity in buffers:
                    b = buffers[identity]
                    length = int(b.length)
                    if length > min(budget, 64 * 1024 * 1024):
                        raise RuntimeError('buffer byte budget exceeded')
                    record.update(format='buffer_bytes', dimensions=[length, 1, 1])
                    data = bytes(controller.GetBufferData(rid, 0, length))
                else:
                    raise RuntimeError('bound resource has no texture/buffer description')
                if not data or consumed + len(data) > budget:
                    raise RuntimeError('empty resource or extraction byte budget exceeded')
                consumed += len(data)
                payload = f'payloads/{event}-{digest(role.encode())[:16]}.bin'
                if save:
                    with (out / payload).open('xb') as stream:
                        stream.write(data)
                record.update(payload=payload, sha256=digest(data), bytes=len(data))
            except Exception as exc:
                record['error'] = str(exc)
            return record

        def walk(nodes, marker_path=(), unique=True):
            sibling_names = collections.Counter(a.customName for a in nodes if a.children)
            occurrences = collections.Counter()
            for action in nodes:
                if action.children:
                    name = action.customName or action.GetName(structured)
                    occurrence = occurrences[name]
                    occurrences[name] += 1
                    walk(action.children, marker_path + (f'{name}#{occurrence}',), unique and bool(action.customName) and sibling_names[action.customName] == 1)
                else:
                    flags = action.flags
                    if flags & rd.ActionFlags.Dispatch:
                        kind = 'dispatch'
                        signature = 'dispatch:' + ','.join(map(str, action.dispatchDimension))
                    elif flags & rd.ActionFlags.Drawcall:
                        kind = 'draw'
                        signature = f'draw:{action.numIndices}:{action.numInstances}'
                    elif flags & rd.ActionFlags.Clear:
                        kind, signature = 'clear', 'clear'
                    else:
                        continue
                    if len(actions) >= 2048:
                        raise RuntimeError('action budget exceeded; select a smaller capture')
                    if kind == 'clear':
                        # Graphics bindings do not identify the cleared destination.
                        # Until destination/view extraction is qualified, retain the
                        # action with explicitly unobserved resources.
                        actions.append(dict(event_id=int(action.eventId), marker_path=list(marker_path), marker_unique=bool(unique and marker_path),
                                            kind=kind, action_key=action.customName or signature, resources=[], candidate_inputs=[]))
                        continue
                    controller.SetFrameEvent(action.eventId, True)
                    pipe = controller.GetPipelineState()
                    descriptors = []
                    if kind != 'dispatch':
                        descriptors.extend((f'color{i}', d) for i, d in enumerate(pipe.GetOutputTargets()))
                        descriptors.append(('depth', pipe.GetDepthTarget()))
                    stages = [rd.ShaderStage.Compute] if kind == 'dispatch' else [rd.ShaderStage.Vertex, rd.ShaderStage.Fragment]
                    inputs = set()
                    for stage in stages:
                        for used in pipe.GetReadWriteResources(stage):
                            access = used.access
                            descriptors.append((f'rw:{stage}:{access.index}:{access.arrayElement}', used.descriptor))
                        for used in pipe.GetReadOnlyResources(stage):
                            if used.descriptor.resource != rd.ResourceId.Null():
                                inputs.add(str(used.descriptor.resource))
                    roles = collections.Counter(role for role, _ in descriptors)
                    resources = []
                    seen = set()
                    for role, descriptor in descriptors:
                        if descriptor.resource == rd.ResourceId.Null() or role in seen:
                            continue
                        seen.add(role)
                        record = resource(role, descriptor, action.eventId)
                        if roles[role] != 1:
                            record.update(payload=None, sha256=None, bytes=None, error='ambiguous resource binding role')
                        resources.append(record)
                    actions.append(dict(event_id=int(action.eventId), marker_path=list(marker_path), marker_unique=bool(unique and marker_path),
                                        kind=kind, action_key=action.customName or signature, resources=resources, candidate_inputs=sorted(inputs)))
        walk(controller.GetRootActions())
        return actions
    finally:
        if controller is not None:
            controller.Shutdown()
        capture.Shutdown()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('--out', type=Path, required=True, help='new extraction directory')
    parser.add_argument('--max-bytes', type=int, default=512 * 1024 * 1024)
    parser.add_argument('--single-replay', action='store_true', help='diagnostic only; repeatability stays unqualified')
    parser.add_argument('--timeout-seconds', type=float, default=120.0, help='external deadline for hashing, native replay and readback')
    parser.add_argument('--_worker', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    if not 0.01 <= args.timeout_seconds <= 3600:
        parser.error('timeout-seconds must be 0.01..3600')
    if not args._worker:
        command = [sys.executable, str(Path(__file__).resolve()), *(sys.argv[1:] if argv is None else argv), '--_worker']
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            process = subprocess.Popen(command, stdout=stdout, stderr=stderr, start_new_session=os.name == 'posix')
            try:
                process.wait(timeout=args.timeout_seconds)
            except subprocess.TimeoutExpired:
                if os.name == 'posix':
                    os.killpg(process.pid, signal.SIGKILL)
                else:
                    process.kill()
                process.wait()
                print(json.dumps({'schema': SCHEMA, 'capability': 'unavailable', 'reason': 'native worker deadline exceeded'}))
                return 2
            stdout.seek(0); stderr.seek(0)
            sys.stdout.buffer.write(stdout.read(65536))
            sys.stderr.buffer.write(stderr.read(65536))
            return process.returncode
    if not 1 <= args.max_bytes <= 2 * 1024 * 1024 * 1024:
        parser.error('max-bytes must be 1..2 GiB')
    try:
        source_hash = file_hash(args.capture)
    except Exception as exc:
        print(json.dumps({'schema': SCHEMA, 'capability': 'unavailable', 'reason': str(exc)}))
        return 2
    try:
        import renderdoc as rd
    except ImportError:
        print(json.dumps({'schema': SCHEMA, 'capability': 'unavailable', 'reason': 'official RenderDoc Python bindings are not installed', 'platform': platform.system()}))
        return 2
    version = rd.GetVersionString()
    if version != VERSION:
        print(json.dumps({'schema': SCHEMA, 'capability': 'unavailable', 'reason': f'requires RenderDoc {VERSION}, found {version}'}))
        return 2
    initialized = False
    try:
        args.out.mkdir()
        (args.out / 'payloads').mkdir()
        rd.InitialiseReplay(rd.GlobalEnvironment(), [])
        initialized = True
        first = extract(rd, args.capture, args.out, args.max_bytes, True)
        repeatability = 'unqualified'
        if not args.single_replay:
            second = extract(rd, args.capture, args.out, args.max_bytes, False)
            if first == second and all(a['resources'] and all(r['error'] is None for r in a['resources']) for a in first) and first:
                repeatability = 'self_replay_byte_identical'
        if file_hash(args.capture) != source_hash:
            raise RuntimeError('capture bytes changed during extraction')
        result = dict(schema=SCHEMA, capture_sha256=source_hash, worker_sha256=file_hash(__file__), renderdoc_version=version,
                      api='Vulkan', replay_mode='conservative', repeatability=repeatability, actions=first,
                      limits=['Clear destinations are explicitly unobserved; graphics bindings are never substituted for clear resources.',
                              'Native resource bytes from one bound view mip/layer and sample zero; other subresources are unobserved.',
                              'Unbound, indirect and uninstrumented resources, uploads and synchronization are not a complete dependency graph.',
                              'Marker/action signatures provide candidate correspondence, not semantic equivalence or cause.',
                              'Live Vulkan replay and host/device compatibility must be separately qualified.'])
        with (args.out / 'extraction.json').open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({'schema': SCHEMA, 'capability': 'available', 'actions': len(first), 'repeatability': repeatability, 'artifact': str(args.out / 'extraction.json')}))
        return 0
    except Exception as exc:
        print(json.dumps({'schema': SCHEMA, 'capability': 'unavailable', 'reason': str(exc)}))
        return 2
    finally:
        if initialized:
            rd.ShutdownReplay()


if __name__ == '__main__':
    sys.exit(main())
