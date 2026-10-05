#!/usr/bin/env python3
"""Frozen lane artifact URLs/digests, including the explicitly authorized mutable TrustMark Q URL."""
import argparse, hashlib, json, os, pathlib, shutil, urllib.request, fcntl, tempfile
BASE = pathlib.Path(__file__).resolve().parent
LIMIT = 8_000_000_000

def fetch(entry, cache):
    if not entry.get('sha256') or not entry.get('bytes'):
        raise RuntimeError('incomplete frozen pin')
    dest = cache / entry['sha256']
    if dest and dest.exists():
        data_hash = hashlib.file_digest(dest.open('rb'), 'sha256').hexdigest()
        if data_hash != entry['sha256'] or (entry['bytes'] and dest.stat().st_size != entry['bytes']):
            raise RuntimeError('HARD FAILURE: corrupt cached pin '+entry['model'])
        entry['bytes'] = dest.stat().st_size
        return dest
    if os.environ.get('SACCADE_REQUIRE_IMMUTABLE_DOWNLOADS') == '1' and entry['model'] == 'trustmark':
        raise RuntimeError('TrustMark mutable source is cache-only in integration round 2')
    if shutil.disk_usage(cache).free < 25 * 1024**3 + 800_000_000:
        raise RuntimeError('disk headroom would fall below 25 GiB')
    fd, temporary_name = tempfile.mkstemp(prefix='download-',dir=cache)
    os.close(fd)
    temporary = pathlib.Path(temporary_name)
    try:
        with urllib.request.urlopen(entry['url'], timeout=45) as response, temporary.open('wb') as f:
            digest = hashlib.sha256(); count = 0
            while chunk := response.read(1024*1024):
                count += len(chunk)
                if count > (entry['bytes'] or 800_000_000):
                    raise RuntimeError('HARD FAILURE: oversized artifact')
                digest.update(chunk); f.write(chunk)
            f.flush(); os.fsync(f.fileno())
        actual = digest.hexdigest()
        if entry.get('sha256') and actual != entry['sha256']:
            raise RuntimeError('HARD FAILURE: SHA-256 mismatch '+entry['model']+' '+entry['role'])
        if entry['bytes'] and count != entry['bytes']:
            raise RuntimeError('HARD FAILURE: byte mismatch')
        entry['bytes'] = count
        dest = cache / actual
        temporary.rename(dest)
        return dest
    finally:
        temporary.unlink(missing_ok=True)

def main():
    a=argparse.ArgumentParser(); a.add_argument('--cache',default='/mnt/linux-extra/saccade-models'); a.add_argument('--model', action='append'); args=a.parse_args()
    cache=pathlib.Path(args.cache)
    if cache.resolve()!=pathlib.Path('/mnt/linux-extra/saccade-models'):
        raise RuntimeError('lane downloads are authorized only into /mnt/linux-extra/saccade-models')
    cache.mkdir(parents=True,exist_ok=True)
    lock=(cache/'pull.lock').open('a+b'); fcntl.flock(lock,fcntl.LOCK_EX)
    if sum(p.stat().st_size for p in cache.rglob('*') if p.is_file()) > LIMIT:
        raise RuntimeError('model cache exceeds 8 GB')
    entries=json.loads((BASE/'artifacts.json').read_text())
    if args.model: entries=[e for e in entries if e['model'] in args.model]
    receipt=[]
    try:
        for e in entries:
            path=fetch(e,cache); receipt.append(e)
            print(e['model'],e['role'],e['bytes'],e['sha256'],flush=True)
            if sum(p.stat().st_size for p in cache.rglob('*') if p.is_file()) > LIMIT:
                raise RuntimeError('model cache exceeds 8 GB')
    finally:
        (cache/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
if __name__=='__main__': main()
