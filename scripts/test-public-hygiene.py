#!/usr/bin/env python3
"""Temporary repositories prove that both index and worktree leaks fail closed."""
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
                              'moss-' + 'heavy', 'gpu-' + 'lease', 'moss-' + 'codex',
                              'codex-' + 'companion', 'moss-' + 'scratch',
                              'moss-' + 'flipdiff-reports'):
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


if __name__ == '__main__':
    unittest.main()
