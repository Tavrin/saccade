"""Coordinator-owned installed model qualification; never collected by the light gate."""
import os
import pytest
import saccade
from test_light import png

@pytest.mark.heavy
def test_installed_models_and_index_roundtrip(tmp_path):
    registry = os.environ['SACCADE_W8_REGISTRY']
    cache = os.environ['SACCADE_W8_MODEL_DIR']
    a = saccade.Analyzer(profile='cpu-full', model_dir=cache, registry=registry)
    result = a.analyze_media(png(), faces=True, text=True, embeddings=True)
    assert result['embeddings']['status'] == 'ok'
    assert result['focal']['data']['faces']['status'] == 'ok'
    assert result['text']['status'] == 'ok'
    assert len(a.embed_image(png())) > 0
    image = tmp_path / 'image.png'
    image.write_bytes(png())
    index = saccade.Index.build(a, [str(image)])
    saved = tmp_path / 'index'
    index.save(str(saved))
    loaded = saccade.Index.load(a, str(saved))
    assert loaded.query(image=png())['hits'][0]['cosine'] > .999

@pytest.mark.heavy
def test_joint_text_queries_rank_generated_colors(tmp_path):
    import struct
    import zlib
    def image(color):
        def chunk(kind, data):
            return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
        rows = []
        for y in range(224):
            rows.append(b'\0'+b''.join(bytes(color if 48 <= x < 176 and 48 <= y < 176 else (255,255,255)) for x in range(224)))
        return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',224,224,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(rows)))+chunk(b'IEND',b'')
    analyzer = saccade.Analyzer(profile='cpu-full', model_dir=os.environ['SACCADE_W8_MODEL_DIR'], registry=os.environ['SACCADE_W8_JOINT_REGISTRY'])
    paths = []
    for name, color in [('red',(255,0,0)),('blue',(0,0,255))]:
        path = tmp_path / (name+'.png'); path.write_bytes(image(color)); paths.append(str(path))
    index = saccade.Index.build(analyzer, paths)
    saved = tmp_path / 'joint-index'; index.save(str(saved))
    loaded = saccade.Index.load(analyzer, str(saved))
    for name in ['red','blue']:
        result = loaded.query(text='a '+name+' square',top=1)
        assert result['schema'] == 'saccade-media-index-query.v1'
        assert result['query_kind'] == 'text' and result['calibration'] == 'uncalibrated'
        assert result['hits'][0]['row']['path'].endswith(name+'.png')
