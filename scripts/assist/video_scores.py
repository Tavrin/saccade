#!/usr/bin/env python3
"""Export settled video scores to versioned JSONL; missing/invalid roots abstain from calibration."""
import argparse
import hashlib
import json
import pathlib


def read(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 32*1024*1024:
        raise ValueError('unbounded or nonordinary artifact')
    return decode(path.read_bytes())


def decode(data):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate JSON key')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique, parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def export(requests, results):
    rows = read(requests)
    smoke = read(results / 'smoke.json')
    campaign = read(results / 'ledger' / 'campaign.json')
    if campaign['money']['campaign_identity']['requests_hash'] != 'sha256:'+hashlib.sha256(requests.read_bytes()).hexdigest():
        raise ValueError('request file identity mismatch')
    outcomes = smoke['root_outcomes']
    if len(outcomes) != len(rows):
        raise ValueError('incomplete root denominator')
    for index, (row, outcome) in enumerate(zip(rows, outcomes)):
        if outcome['root'] != row['root']:
            raise ValueError('root identity mismatch')
        data = json.loads(row['payload']['messages'][1]['content'][0]['text'])
        packet = data['packet']
        answer = read(results / f'answer-{index}.json') if outcome['code'] == 'completed' else None
        if answer is not None:
            response = read(results / f'response-{index}.json')
            receipt = read(results / f'receipt-{index}.json')
            response_hash = 'sha256:'+hashlib.sha256((results / f'response-{index}.json').read_bytes()).hexdigest()
            if (receipt['response_hash'] != response_hash or receipt['execution_id'] != outcome['execution_id']
                    or receipt['requested_model'] != row['model'] or receipt['sampling_settings'] != row['payload']
                    or receipt['returned_model'] != row['model']
                    or receipt['returned_revision'] != row['revision']
                    or json.loads(response['choices'][0]['message']['content']) != answer):
                raise ValueError('settled response/provenance mismatch')
        if answer is not None and answer['request_hash'] != data['request_hash']:
            raise ValueError('answer identity mismatch')
        for slot, clip in enumerate(packet['clips']):
            score = answer['scores'][slot] if answer else None
            if score is not None and score['slot'] != ('A' if slot == 0 else 'B'):
                raise ValueError('slot identity mismatch')
            yield dict(schema='saccade-video-judge-scores.v1', root=row['root'],
                model=row['model'], revision=row['revision'], request_hash=data['request_hash'],
                source=clip['source'], kind=clip.get('kind','motion'), view_id=clip.get('view_id') or clip['source'],
                sample_id=clip.get('sample_id','sample-0'), order=0 if len(packet['clips']) == 1 or packet['clips'][0]['source'] < packet['clips'][1]['source'] else 1,
                preferred_source=(packet['clips'][0 if answer['preferred'] == 'A' else 1]['source'] if answer and answer['preferred'] in ('A','B') else clip['source'] if answer and answer['preferred'] == 'single' else answer['preferred'] if answer else None),
                criteria=score.get('criteria',[]) if score else [], forbidden=score.get('forbidden',[]) if score else [],
                slot='A' if slot == 0 else 'B',
                outcome=answer['outcome'] if answer else outcome['code'],
                score=score['score'] if score else None, cues=score['cues'] if score else [],
                replay=receipt.get('cache_status') != 'miss' if answer else True,
                returned_model=receipt['returned_model'] if answer else None,
                returned_revision=receipt['returned_revision'] if answer else None,
                provider=response['provider'] if answer else None,
                trusted_for=[], authority='advisory; untrusted calibration scorer')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--requests', type=pathlib.Path, required=True)
    parser.add_argument('--results', type=pathlib.Path, required=True)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    args = parser.parse_args()
    lines = list(export(args.requests, args.results))
    with args.out.open('x') as output:
        for line in lines:
            output.write(json.dumps(line, allow_nan=False)+'\n')
