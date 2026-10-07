"""Generated fixtures; no network, runtime, models or browser."""
import concurrent.futures
import hashlib
import struct
import zlib
import pytest
import saccade

def png(width=40, height=30, colour=(20,40,70)):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    data = b''.join(b'\0' + bytes(colour) * width for _ in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(data)) + chunk(b'IEND', b''))

def test_retained_path_bytes_record_and_fitness(tmp_path):
    data = png()
    path = tmp_path / 'image.png'
    path.write_bytes(data)
    a = saccade.Analyzer()
    result = a.analyze_media(path, output_sizes=[[400, 300]])
    assert result['schema'] == 'saccade-media-record.v1'
    assert result['identity']['data']['sha256'] == hashlib.sha256(data).hexdigest()
    assert result['quality']['data']['fitness'][0]['adequate_resolution'] is False
    assert result['metadata']['data']['candidates'] == []
    assert result['description']['status'] == 'skipped'
    assert result['focal']['data']['basis'] == 'centre'
    assert a.hash(data) == a.hash(path)
    assert a.compare(data, data)['metrics']['mean'] == 0
    for section in [v for k, v in result.items() if k not in ('schema', 'profile')]:
        assert section['timing_ms'] >= 0 and section['provenance']['pins']

def test_error_codes_and_failed_sections():
    a = saccade.Analyzer()
    with pytest.raises(saccade.InputError) as e:
        a.analyze_media(b'broken')
    assert e.value.code == 'invalid_media_input'
    result = a.analyze_media(png(), description=True)
    assert result['description']['status'] == 'failed'
    with pytest.raises(saccade.AnalysisError) as e:
        a.analyze_media(png(), description=True, strict=True)
    assert e.value.code == 'media_section_failed'
    with pytest.raises(saccade.ModelError) as e:
        a.embed_text('an object')
    assert e.value.code == 'text_embedding_unavailable'
    with pytest.raises(saccade.InputError):
        a.analyze_media(png(), nonexistent=True)

def test_concurrent_analyzer_calls():
    a = saccade.Analyzer()
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        rows = list(pool.map(lambda _: a.analyze_media(png()), range(12)))
    assert len({r['identity']['data']['sha256'] for r in rows}) == 1


def test_package_version_matches_distribution_source():
    import pathlib
    import re
    import saccade
    text = (pathlib.Path(__file__).parents[1] / "pyproject.toml").read_text()
    assert saccade.__version__ == re.search(r'^version = "([^"]+)"', text, re.M).group(1)


def test_compare_maps_returns_native_numpy_error_and_tile_grids():
    import numpy as np
    import saccade
    def fixture(value):
        return png(16,16,(value,value,value))
    maps = saccade.compare_maps(fixture(100), fixture(120), tile_size=8)
    assert maps["flip"].dtype == np.float32
    assert maps["flip"].shape == (16, 16)
    assert np.min(maps["flip"]) > 0
    assert maps["tile_signed_shift"].shape == (2, 2)
    assert np.allclose(maps["tile_signed_shift"], 20 / 255)
    assert np.max(saccade.compare_maps(fixture(100), fixture(100))["flip"]) == 0


def test_model_config_is_the_shared_resolver_and_old_arguments_warn(tmp_path, monkeypatch):
    monkeypatch.setenv('SACCADE_MODELS_DIR', str(tmp_path))
    config = saccade.model_config()
    assert config['schema'] == 'saccade-model-config.v1'
    assert config['dir']['value'] == str(tmp_path)
    assert config['dir']['source']['name'] == 'SACCADE_MODELS_DIR'
    with pytest.warns(DeprecationWarning, match='model_dir='):
        saccade.Analyzer(model_dir=str(tmp_path))


def test_batch_returns_rows_and_resumes(tmp_path, monkeypatch):
    monkeypatch.delenv('SACCADE_BIN', raising=False)
    monkeypatch.setenv('PATH', '')
    folder = tmp_path / 'inputs'
    folder.mkdir()
    (folder / 'image.png').write_bytes(png())
    (folder / 'broken.png').write_bytes(b'broken')
    output = tmp_path / 'batch'
    rows = saccade.batch(folder, output)
    assert len(rows) == 2
    assert {r['status'] for r in rows} == {'ok', 'corrupt'}
    assert saccade.batch(folder, output) == rows
    assert (folder / 'image.png').read_bytes() == png()
    assert len((output / 'rows.jsonl').read_text().splitlines()) == 2


def test_batch_history_manifest_and_resume_identity(tmp_path, monkeypatch):
    import json
    monkeypatch.delenv('SACCADE_BIN', raising=False)
    monkeypatch.setenv('PATH', '')
    source = tmp_path / 'images'
    source.mkdir()
    image = source / 'image.png'
    image.write_bytes(png())
    out = tmp_path / 'out'
    first = saccade.batch(source, out)
    run = json.loads((out / 'batch-run.json').read_text())
    assert run['backend'] == 'in-process'
    assert len(run['library_sha256']) == 64
    assert 'executable' not in run
    assert first[0]['sections'][0]['result']['identity']['data']['sha256'] == hashlib.sha256(png()).hexdigest()
    receipt = (out / 'rows' / (first[0]['id'] + '.json')).read_bytes()
    image.write_bytes(png(colour=(1, 2, 3)))
    rows = saccade.batch(source, out)
    assert len(rows) == 2 and first[0] in rows
    assert (out / 'rows' / (first[0]['id'] + '.json')).read_bytes() == receipt
    image.unlink()
    (source / 'new.bin').write_bytes(b'unknown format')
    rows = saccade.batch(source, out)
    assert len(rows) == 3 and {r['status'] for r in rows} == {'ok', 'unsupported'}
    manifest = json.loads((out / 'saccade-manifest.json').read_text())
    assert manifest['schema'] == 'saccade-manifest.v1'
    for artifact in manifest['artifacts']:
        path = out / artifact['path']
        assert hashlib.sha256(path.read_bytes()).hexdigest() == artifact['sha256']
    with pytest.raises(saccade.AnalysisError, match='resume configuration differs'):
        saccade.batch(source, out, options_json=json.dumps({'sections': [{'command': 'analyze-media', 'args': []}], 'concurrency': 1, 'timeout_ms': 30000}))
    run['library_sha256'] = '0' * 64
    (out / 'batch-run.json').write_text(json.dumps(run))
    with pytest.raises(saccade.AnalysisError, match='resume configuration differs'):
        saccade.batch(source, out)


def test_batch_options_skips_pairs_and_partial_rows(tmp_path, monkeypatch):
    import json
    monkeypatch.setenv('PATH', '')
    image = tmp_path / 'image.png'
    image.write_bytes(png())
    source = tmp_path / 'inputs.json'
    source.write_text(json.dumps({'schema': 'saccade-batch-input.v1', 'inputs': [
        {'path': 'image.png'}, {'path': 'image.png'}, {'path': 'image.png', 'skip': True},
    ]}))
    options = {'sections': [{'command': 'analyze-media', 'args': ['--output-size', '400x300']}], 'concurrency': 2, 'timeout_ms': 30000}
    rows = saccade.batch(source, tmp_path / 'out', options_json=json.dumps(options))
    assert len(rows) == 3 and {r['status'] for r in rows} == {'duplicate-basename', 'skipped'}
    assert all(r['duplicate_basename'] for r in rows)
    assert sorted(r['occurrence'] for r in rows if r['status'] != 'skipped') == [0, 1]
    for row in rows:
        if row['status'] != 'skipped':
            assert row['sections'][0]['result']['quality']['data']['fitness'][0]['adequate_resolution'] is False
    options['sections'] = [{'command': 'compare', 'args': []}]
    rows = saccade.batch(source, tmp_path / 'pairs', options_json=json.dumps(options))
    # Equal-path rows sort by identity hash, which includes the absolute path.
    # Select by retained section evidence rather than a platform-dependent index.
    processed = [r for r in rows if r['sections']]
    skipped = [r for r in rows if not r['sections']]
    assert len(rows) == 3 and len(processed) == 2 and len(skipped) == 1
    assert skipped[0]['status'] == 'skipped' and skipped[0]['thumbnail'] is None
    assert sorted(r['occurrence'] for r in processed) == [0, 1]
    for row in processed:
        assert row['status'] == 'partial'
        assert row['probe']['status'] == 'ok'
        assert row['sections'] == [{'command': 'compare', 'status': 'skipped',
                                    'error': 'paired reference required'}]
    options['sections'] = [{'command': 'analyze-media', 'args': ['--nonexistent', 'yes']}]
    rows = saccade.batch(source, tmp_path / 'partial', options_json=json.dumps(options))
    row = next(r for r in rows if r['status'] == 'partial')
    assert row['sections'][0]['status'] == 'partial'
    assert 'unavailable in-process' in row['sections'][0]['error']


def test_batch_refuses_unsafe_outputs_and_options(tmp_path):
    import json
    source = tmp_path / 'inputs'
    source.mkdir()
    image = source / 'image.png'
    image.write_bytes(png())
    with pytest.raises(saccade.AnalysisError, match='inside an input directory'):
        saccade.batch(source, source / 'out')
    out = tmp_path / 'unrelated'
    out.mkdir()
    retain = out / 'retain.txt'
    retain.write_bytes(b'retain')
    with pytest.raises(saccade.AnalysisError, match='must be empty'):
        saccade.batch(source, out)
    assert retain.read_bytes() == b'retain' and image.read_bytes() == png()
    options = {'sections': [{'command': 'analyze-media', 'args': ['--description']}], 'concurrency': 2, 'timeout_ms': 30000}
    with pytest.raises(saccade.AnalysisError, match='dispatch providers'):
        saccade.batch(source, tmp_path / 'unsafe', options_json=json.dumps(options))


def test_batch_pair_uses_core_report_and_mask_metrics(tmp_path, monkeypatch):
    import json
    monkeypatch.setenv('PATH', '')
    reference = tmp_path / 'reference.png'
    image = tmp_path / 'image.png'
    reference.write_bytes(png())
    image.write_bytes(png())
    source = tmp_path / 'inputs.json'
    source.write_text(json.dumps({'schema': 'saccade-batch-input.v1', 'inputs': [
        {'path': 'image.png', 'reference': 'reference.png'},
    ]}))
    options = {'sections': [{'command': 'compare', 'args': []}, {'command': 'mask-metrics', 'args': []}], 'concurrency': 1, 'timeout_ms': 30000}
    rows = saccade.batch(source, tmp_path / 'out', options_json=json.dumps(options))
    compare, masks = rows[0]['sections']
    assert compare['result']['schema'] in {'saccade-report.v1', 'saccade-report.v2'}
    assert compare['exit_code'] == 0
    assert masks['result']['schema'] == 'saccade-mask-metrics.v1'
    assert masks['result']['summary']['macro_iou'] == 1
    assert saccade.batch(source, tmp_path / 'out', options_json=json.dumps(options)) == rows
