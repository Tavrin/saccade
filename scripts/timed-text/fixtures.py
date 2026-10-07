#!/usr/bin/env python3
"""Generated timed-text truth; no downloads, network, or third-party media."""
import argparse
import hashlib
import json
import shutil
from pathlib import Path
import subprocess
import tempfile
from PIL import Image, ImageDraw, ImageFont, __version__ as PIL_VERSION


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate(out):
    out.mkdir(parents=True, exist_ok=True)
    font_path = Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')
    font = ImageFont.truetype(str(font_path), 28)
    ffmpeg = Path(shutil.which('ffmpeg') or 'ffmpeg').resolve()
    encoder = dict(sha256=sha(ffmpeg), version=subprocess.run([str(ffmpeg), '-version'], check=True, capture_output=True, text=True).stdout.splitlines()[0], codec='ffv1', pixel_format='bgr0', fps=4)
    provenance = dict(licence='MIT OR Apache-2.0 (generated artwork and text)',
                      generator='scripts/timed-text/fixtures.py', generator_sha256=sha(Path(__file__)), pillow=PIL_VERSION,
                      font=dict(name=font_path.name, sha256=sha(font_path),
                                licence='DejaVu fonts licence (Bitstream Vera and public-domain additions)',
                                source='https://dejavu-fonts.github.io/License.html'),
                      media='Generated geometric scenes; no imported footage or audio',
                      observations='Synthetic image-bound truth adapter, not executed OCR')
    cases = []
    for name, domain in [('captions', 'captioned-video'), ('slides', 'presentation'), ('scene', 'illustrated-sequence'), ('faint', 'captioned-video')]:
        directory = out / name
        frames = directory / 'frames'
        sources = directory / 'sources'
        frames.mkdir(parents=True, exist_ok=True)
        sources.mkdir(exist_ok=True)
        texts = []
        count = 32 if name == 'captions' else 8
        for i in range(count):
            t = i / 4
            if name == 'captions':
                text = ('FIRST CAPTION' if 1 <= t < 2 else
                        'SHIFTED CAPTION' if 2.5 <= t < 3.5 else
                        'SPELING SAMPLE' if 5 <= t < 6 else
                        'UNEXPECTED CAPTION' if 7 <= t < 7.5 else '')
            else:
                text = 'SIMPLE SAMPLE' if 0.5 <= t < 1.5 else ''
            texts.append(text)
            image = Image.new('RGB', (640, 360), (34, 46, 65))
            draw = ImageDraw.Draw(image)
            if domain == 'presentation':
                draw.rectangle((24, 16, 616, 250), fill=(244, 242, 237))
                if i < 4:
                    draw.rectangle((60, 60, 200, 120), fill=(60, 120, 180))
                    draw.ellipse((280, 65, 380, 165), fill=(200, 110, 60))
                else:
                    draw.rectangle((60, 60, 320, 90), fill=(60, 120, 180))
                    draw.rectangle((60, 120, 230, 150), fill=(200, 110, 60))
            elif domain == 'illustrated-sequence':
                draw.polygon([(0, 250), (160, 80), (300, 250), (470, 90), (640, 250)], fill=(70, 100, 120))
                draw.ellipse((70 + i * 8, 180, 110 + i * 8, 220), fill=(220, 170, 80))
            draw.rectangle((20, 280, 620, 350), fill=(250, 250, 250))
            if text:
                draw.text((40, 296), text, font=font, fill=(205, 205, 205) if name == 'faint' else (15, 15, 15))
            image.save(frames / f'{i:04}.png')
        if name in {'captions', 'slides', 'scene'}:
            subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-framerate', '4',
                            '-i', str(frames / '%04d.png'), '-c:v', 'ffv1', '-pix_fmt', 'bgr0', str(directory / 'video.mkv')], check=True)
            decoded = directory / 'decoded'
            decoded.mkdir(exist_ok=True)
            subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-i', str(directory / 'video.mkv'),
                            '-start_number', '0', str(decoded / '%04d.png')], check=True)
            for i in range(count):
                a, b = frames / f'{i:04}.png', decoded / f'{i:04}.png'
                assert Image.open(a).convert('RGB').tobytes() == Image.open(b).convert('RGB').tobytes(), 'video roundtrip changed pixels'
                a.write_bytes(b.read_bytes())
        entries = []
        for i, text in enumerate(texts):
            path = frames / f'{i:04}.png'
            entries.append(dict(index=i, timestamp_s=i / 4, file=f'frames/{i:04}.png', sha256=sha(path)))
            nodes = [] if not text else [dict(id='line', text=text, bounds=[40, 296, 500, 40], reading_order=None,
                                              keyboard_order=None, ocr_confidence=100)]
            source = dict(schema='saccade-ui-source.v1', capture_sha256=sha(path), dimensions=[640, 360],
                                            kind='provider_ocr', producer=dict(adapter='generated truth; not actual OCR'), complete=False, nodes=nodes)
            write(sources / f'{i}.json', dict(schema='saccade-timed-text-source.v1', source=source,
                                            declared_empty_region_px=None if text else [20, 280, 600, 70]))
        write(directory / 'map.json', dict(schema='saccade-frame-map.v1', nominal_fps=4, frames=entries))
        if name == 'captions':
            content = 'WEBVTT\n\nfirst\n00:01.000 --> 00:02.000\nFIRST CAPTION\n\nshifted\n00:02.000 --> 00:03.000\nSHIFTED CAPTION\n\nmissing\n00:04.000 --> 00:05.000\nMISSING CAPTION\n\nmisspelt\n00:05.000 --> 00:06.000\nSPELLING SAMPLE\n'
            expected = [['aligned', []], ['failed', ['late', 'interrupted']], ['failed', ['missing']], ['failed', ['text_mismatch']]]
        else:
            content = '1\n00:00:00,500 --> 00:00:01,500\nSIMPLE SAMPLE\n'
            expected = [['failed', ['illegible']]] if name == 'faint' else [['aligned', []]]
        caption = directory / ('cues.vtt' if name == 'captions' else 'cues.srt')
        caption.write_text(content)
        identity = dict(provenance, domain=domain, frame_count=count, sampling_fps=4, frame_map_sha256=sha(directory / 'map.json'), timed_text_sha256=sha(caption))
        if (directory / 'video.mkv').is_file():
            identity.update(video_sha256=sha(directory / 'video.mkv'), encoder=encoder)
        write(directory / 'provenance.json', identity)
        cases.append(dict(name=name, expected=expected, captions=caption.name))
    write(out / 'cases.json', cases)
    return cases


def qualify(out, binary, cases):
    binary_sha256 = sha(binary)
    receipts = []
    for case in cases:
        directory = out / case['name']
        command = [str(binary), 'timed-text', str(directory / case['captions']), str(directory / 'map.json'),
                   '--region', '20,280,600,70', '--json']
        with tempfile.TemporaryDirectory(prefix='evidence-', dir=directory) as temporary:
            result = subprocess.run(command + ['--sources', str(directory / 'sources'), '--out', temporary], capture_output=True, text=True)
            report = json.loads(result.stdout)
            artifact = json.loads((Path(temporary) / 'saccade-timed-text.v1.json').read_text())
            assert artifact == report, 'stdout and persisted report differ'
            manifest = subprocess.run([str(binary), 'manifest', 'verify', temporary, '--json'], capture_output=True, text=True)
            assert manifest.returncode == 0, (manifest.stdout, manifest.stderr)
            assert (Path(temporary) / 'index.html').is_file()
            write(directory / 'manifest-verification.json', json.loads(manifest.stdout))
        expected_exit = 1 if any(s == 'failed' for s, _ in case['expected']) else 0
        assert result.returncode == expected_exit, (case['name'], result.returncode, result.stderr, report)
        for actual, (state, findings) in zip(report['cues'], case['expected'], strict=True):
            assert actual['state'] == state, (case['name'], actual)
            assert set(findings).issubset(actual['findings']), (case['name'], actual)
        if case['name'] == 'captions':
            assert report['cues'][1]['onset_offset_s'] == 0.5
            assert report['cues'][2]['matching_frames'] == []
            unexpected = [e for e in report['extra'] if e['text'] == 'UNEXPECTED CAPTION']
            assert len(unexpected) == 1 and unexpected[0]['frame_indices'] == [28, 29]
        write(directory / 'report.json', report)
        # Missing optional evidence must fail closed even on visibly present text.
        skipped = subprocess.run(command + ['--ocr'], capture_output=True, text=True)
        skipped_report = json.loads(skipped.stdout)
        reasons = sorted({o['ocr_reason'] for o in skipped_report['observations'] if o['text'] is None})
        unavailable = all(o['text'] is None for o in skipped_report['observations'])
        if unavailable:
            assert skipped.returncode == 4 and skipped_report['state'] == 'insufficient_evidence'
            assert not any(c['findings'] for c in skipped_report['cues'])
        receipts.append(dict(case=case['name'], exit=result.returncode, binary_sha256=binary_sha256, synthetic_truth='PASS',
                             cached_ocr='SKIP' if unavailable else 'executed, not qualified', reasons=reasons))
    write(out / 'receipts.json', receipts)
    print(json.dumps(receipts, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--binary', type=Path)
    args = parser.parse_args()
    cases = generate(args.out)
    if args.binary:
        qualify(args.out, args.binary.resolve(), cases)


if __name__ == '__main__':
    main()
