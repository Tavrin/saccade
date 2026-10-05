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
