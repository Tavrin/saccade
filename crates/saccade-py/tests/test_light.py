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
