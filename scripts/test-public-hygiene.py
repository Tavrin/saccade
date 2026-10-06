#!/usr/bin/env python3
"""Temporary repositories prove that both index and worktree leaks fail closed."""
import hashlib
import json
import pathlib
import subprocess
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name('check-public-hygiene.sh').resolve()


class Guard(unittest.TestCase):
    def test_index_worktree_names_and_symlinks(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
            file = root / 'fixture.txt'
            for forbidden in ('/' + 'home/author/cache', '/' + 'mnt/data/cache',
                              'Indigo' + 'Moose', 'Plum' + 'Aspen', 'opus-' + 'perf',
                              'mo' + 'ss-' + 'heavy', 'gpu-' + 'lease', 'mo' + 'ss-' + 'codex',
                              'codex-' + 'companion', 'mo' + 'ss-' + 'scratch',
                              'mo' + 'ss-' + 'flipdiff-reports', 'mo' + 'ss', 'coo' + 'ker',
                              'probe-' + 'cache', 'probe_' + 'cache', 'receiver_' + 'ready',
                              'bi' + 'stro', 'spon' + 'za', 'manhat' + 'tan', 'li' + '01'):
                file.write_text(forbidden)
                subprocess.run(['git', 'add', 'fixture.txt'], cwd=root, check=True)
                file.write_text('portable content')
                result = subprocess.run([SCRIPT], cwd=root, capture_output=True, text=True)
                self.assertEqual(result.returncode, 1, forbidden)
                self.assertIn('index', result.stderr)
                subprocess.run(['git', 'add', 'fixture.txt'], cwd=root, check=True)
                file.write_text(forbidden)
                self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 1)
                file.write_text('portable content')
            subprocess.run(['git', 'add', 'fixture.txt'], cwd=root, check=True)
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 0)
            leak = root / ('gpu-' + 'lease.txt')
            leak.write_text('portable')
            subprocess.run(['git', 'add', leak.name], cwd=root, check=True)
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 1)
            subprocess.run(['git', 'rm', '-f', '-q', leak.name], cwd=root, check=True)
            link = root / 'link'
            link.symlink_to('/' + 'home/author/cache')
            subprocess.run(['git', 'add', 'link'], cwd=root, check=True)
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 1)
            subprocess.run(['git', 'rm', '-f', '-q', 'link'], cwd=root, check=True)
            note = root / 'WAVE10-NOTES.md'
            note.write_text('process')
            subprocess.run(['git', 'add', note.name], cwd=root, check=True)
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 1)

    def test_allowlist_is_exact_and_index_specific(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
            (root / 'scripts').mkdir()
            name = 'README.md'
            line = 'Origin: ' + 'Mo' + 'ss'
            (root / name).write_text(line + '\n')
            allow = root / 'scripts/public-hygiene-allowlist.json'
            allow.write_text(json.dumps([{'path': name, 'sha256': hashlib.sha256(line.encode()).hexdigest()}]))
            subprocess.run(['git', 'add', '.'], cwd=root, check=True)
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 0)
            (root / name).write_text(line + ' extra\n')
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 1)
            (root / name).write_text(line + '\n')
            allow.write_text('[]')
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 1)
            allow.write_text(json.dumps([{'path': name, 'sha256': hashlib.sha256(line.encode()).hexdigest()}]))
            subprocess.run(['git', 'add', name], cwd=root, check=True)
            (root / name).write_text(line + '\n' + 'Mo' + 'ss' + ' new reference\n')
            self.assertEqual(subprocess.run([SCRIPT], cwd=root, capture_output=True).returncode, 1)


if __name__ == '__main__':
    unittest.main()
