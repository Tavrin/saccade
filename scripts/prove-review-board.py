#!/usr/bin/env python3
"""Generate and assert the offline N22 codec/UI trial (no human/provider study)."""
import argparse
import hashlib
import json
import pathlib
import subprocess

from PIL import Image, ImageDraw, __version__ as pillow_version


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin', required=True)
    parser.add_argument('--out', required=True, help='new directory with an existing parent')
    args = parser.parse_args()
    binary = pathlib.Path(args.bin).resolve()
    out = pathlib.Path(args.out).resolve()
    out.mkdir()  # Never overwrite a prior proof.

    def write(name, data):
        (out / name).write_text(json.dumps(data, indent=2) + '\n', encoding='utf-8')

    def run(*argv):
        result = subprocess.run([str(binary), *argv, '--json'], cwd=out,
                                capture_output=True, text=True, check=True)
        return json.loads(result.stdout)

    source = Image.new('RGB', (64, 40))
    source.putdata([((x * 5) % 256, (y * 7) % 256, ((x + y) * 3) % 256)
                    for y in range(40) for x in range(64)])
    source.save(out / 'codec-source.png')
    source.save(out / 'codec-delivery.jpg', quality=30)
    for name, offset in [('ui-first.png', 0), ('ui-second.png', 6)]:
        image = Image.new('RGB', (64, 40), (240, 240, 240))
        ImageDraw.Draw(image).rectangle((8 + offset, 12, 42 + offset, 28), fill=(40, 90, 180))
        image.save(out / name)
    plan = {'schema': 'saccade-review-board-plan.v1',
            'raters': [{'id': 'human-a', 'kind': 'human'},
                       {'id': 'human-b', 'kind': 'human'},
                       {'id': 'offline-tool', 'kind': 'tool'}],
            'pairs': [{'id': 'codec-pair', 'group': 'codec', 'first': 'codec-source.png', 'second': 'codec-delivery.jpg'},
                      {'id': 'ui-pair', 'group': 'ui', 'first': 'ui-first.png', 'second': 'ui-second.png'}]}
    write('plan.json', plan)
    prepare = run('review', 'board', 'prepare', 'plan.json', '--out', 'trial')
    trial = json.loads((out / 'trial/trial.json').read_text())
    for r in range(3):
        packet = out / f'trial/rater-{r + 1:03}'
        ballot = json.loads((packet / 'ballot.json').read_text())
        assert not ballot['blind_confirmed']
        assert all(v['answer'] is None for v in ballot['responses'])
        # Simulated private oracle; never distributed in a rater packet.
        ballot['blind_confirmed'] = True
        for i, response in enumerate(ballot['responses']):
            stable = ('second' if r == 2 else 'first') if i == 0 else ('second' if r < 2 else None)
            reverse = trial['raters'][r]['reversed'][i]
            response['answer'] = ('second' if stable == 'first' else 'first') if reverse and stable else stable
            response['note'] = 'Simulated: codec texture' if i == 0 else 'Simulated: button spacing'
        write(f'returned-{r}.json', ballot)
    collect = run('review', 'board', 'collect', 'trial/trial.json',
                  '--ballot', 'returned-0.json', '--ballot', 'returned-1.json',
                  '--ballot', 'returned-2.json', '--out', 'board')
    board = json.loads((out / 'board/saccade-review-board.v2.json').read_text())
    assert board['items'][0]['agreement'] == 'disagreement'
    assert board['items'][1]['missing'] == 1
    assert board['items'][1]['votes'][2]['answer'] is None
    assert board['raters'][2]['missing'] == 1
    assert abs(board['agreement']['value'] - 1 / 3) < 1e-12
    assert board['approval_authority'] is False and board['verdict'] == 'advisory'
    assert not (out / 'board/trial.json').exists()
    manifest = run('manifest', 'build', 'board')
    run('manifest', 'verify', 'board')
    write('proof.json', {'licence': 'MIT OR Apache-2.0',
                        'provenance': 'Procedural RGB images and simulated ballots from scripts/prove-review-board.py; no downloads, human raters or provider output.',
                        'pillow_version': pillow_version,
                        'generator_sha256': hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
                        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                        'prepare': prepare, 'collect': collect, 'manifest': manifest,
                        'source_hashes': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                                          for p in out.iterdir() if p.suffix in ('.png', '.jpg')},
                        'assertions': {'missing_vote_visible': True, 'disagreement_visible': True,
                                       'alpha': board['agreement']['value'], 'approval_authority': False}})
    print('PASS: codec/UI, three simulated raters, one missing vote, alpha=1/3; no approval authority')


if __name__ == '__main__':
    main()
