#!/usr/bin/env python3
"""Generated temporary repositories exercise the external-denylist contract."""
import pathlib, subprocess, tempfile, unittest
SCRIPT = pathlib.Path(__file__).with_name('check-genericity.sh').resolve()
class Guard(unittest.TestCase):
    def test_staged_and_worktree_matches_and_private_terms(self):
        with tempfile.TemporaryDirectory() as temp:
            root=pathlib.Path(temp); repo=root/'repo';repo.mkdir()
            subprocess.run(['git','init','-q'],cwd=repo,check=True)
            deny=root/'deny.txt';deny.write_text('generated-forbidden-marker\n')
            file=repo/'fixture.txt';file.write_text('generated-forbidden-marker\n')
            subprocess.run(['git','add','fixture.txt'],cwd=repo,check=True)
            file.write_text('safe\n')
            result=subprocess.run([SCRIPT,deny],cwd=repo,text=True,capture_output=True)
            self.assertEqual(result.returncode,1);self.assertIn('index',result.stderr);self.assertNotIn('generated-forbidden-marker',result.stderr)
            subprocess.run(['git','add','fixture.txt'],cwd=repo,check=True)
            self.assertEqual(subprocess.run([SCRIPT,deny],cwd=repo,capture_output=True).returncode,0)
            file.write_text('generated-forbidden-marker\n')
            self.assertEqual(subprocess.run([SCRIPT,deny],cwd=repo,capture_output=True).returncode,1)
            self.assertEqual(subprocess.run([SCRIPT,repo/'internal.txt'],cwd=repo,capture_output=True).returncode,1)
            self.assertEqual(subprocess.run([SCRIPT,root/'absent.txt'],cwd=repo,capture_output=True).returncode,0)
if __name__=='__main__':unittest.main()
