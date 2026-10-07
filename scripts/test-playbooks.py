#!/usr/bin/env python3
"""Focused regression tests for playbook acceptance failures and fixture bindings."""
import importlib.util
import json
import pathlib
import shutil
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / file)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


RUNNER = module("runner", "run-playbooks.py")
FIXTURES = module("fixtures", "gen-playbook-fixtures.py")


class AcceptanceTests(unittest.TestCase):
    def test_exit_and_effect_fail_even_when_recording_hashes(self):
        command = dict(exit=0, checks=[dict(name="outside", source="stdout", pointer="/outside", equals=0)])
        with self.assertRaisesRegex(ValueError, "expected exit"):
            RUNNER.verify(command, subprocess.CompletedProcess([],1,'{"outside":0}',""), str)
        with self.assertRaisesRegex(ValueError, "expected 0"):
            RUNNER.verify(command, subprocess.CompletedProcess([],0,'{"outside":1}',""), str)
        with self.assertRaises(KeyError):
            RUNNER.verify(command, subprocess.CompletedProcess([],0,'{}',""), str)

    def test_projection_ignores_only_undeclared_provenance(self):
        command = dict(exit=0,checks=[dict(name="score",source="stdout",pointer="/score")])
        def digest(value):
            return RUNNER.verify(command,subprocess.CompletedProcess([],0,json.dumps(value),""),str)[1]
        self.assertEqual(digest(dict(score=0.123456,port=1000)),digest(dict(score=0.123456,port=2000)))
        self.assertNotEqual(digest(dict(score=0.123456)),digest(dict(score=0.123457)))

    def test_all_thirteen_manifests_have_hashes_and_checks(self):
        paths = sorted((ROOT/"playbooks").glob("W*/commands.json"))
        self.assertEqual([p.parent.name for p in paths],[f"W{n:02d}" for n in range(1,14)])
        for path in paths:
            value = json.loads(path.read_text())
            RUNNER.validate_manifest(value,path.parent.name)
            self.assertTrue(all(c["sha256"] != "0"*64 for c in value["commands"]),path)
        broken = json.loads(paths[0].read_text())
        broken["commands"][0]["checks"] = []
        with self.assertRaises(ValueError):
            RUNNER.validate_manifest(broken,"W01")

    def test_generated_source_and_camera_receipts_bind_exact_inputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)/"inputs"
            FIXTURES.generate(root)
            source = RUNNER.load(root/"text/after-source.json")
            self.assertEqual(source["capture_sha256"],RUNNER.sha((root/"text/after.png").read_bytes()))
            self.assertNotEqual(source["capture_sha256"],RUNNER.load(root/"text/stale-source.json")["capture_sha256"])
            manifest = RUNNER.load(root/"geometry/cameras.json")
            self.assertEqual(len(manifest["views"]),3)
            for view in manifest["views"]:
                for side, asset in zip(("reference","candidate"),manifest["assets"]):
                    render = view[side]
                    self.assertEqual(render["camera_sha256"],RUNNER.sha(RUNNER.canonical(view["camera"])))
                    self.assertEqual(render["context_sha256"],RUNNER.sha(RUNNER.canonical(manifest["context"])))
                    self.assertEqual(render["asset_geometry_sha256"],asset["geometry_sha256"])
                    self.assertEqual(render["image"]["sha256"],RUNNER.sha((root/"geometry"/render["image"]["path"]).read_bytes()))
            provenance = RUNNER.load(root/"provenance.json")
            for path, digest in provenance["files"].items():
                self.assertEqual(RUNNER.sha((root/path).read_bytes()),digest)

    def test_hash_mismatch_fails_and_failed_record_does_not_rewrite_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            kit = root / "kit"
            (kit / "scripts").mkdir(parents=True)
            shutil.copytree(ROOT / "playbooks", kit / "playbooks")
            for name in ("run-playbooks.py", "gen-playbook-fixtures.py", "gen-guide-fixtures.py"):
                shutil.copy(ROOT / "scripts" / name, kit / "scripts" / name)
            path = kit / "playbooks/W04/commands.json"
            manifest = json.loads(path.read_text())
            manifest["commands"][0]["sha256"] = "1" * 64
            path.write_text(json.dumps(manifest))
            original = path.read_bytes()
            binary = root / "saccade"
            good = {"intended_change_detected": True, "outside": {"changed_pixels": 0},
                    "inside": {"changed_pixels": 1280}, "collateral": "preserved"}
            def fake(value):
                binary.write_text("#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps({'compiled_features':[]} if sys.argv[1]=='capabilities' else " + repr(value) + "))\n")
                binary.chmod(0o755)
            fake(good)
            argv = ["python3", kit / "scripts/run-playbooks.py", "--bin", binary, "--out", root / "hash", "--only", "W04"]
            result = subprocess.run(argv,capture_output=True,text=True)
            self.assertEqual(result.returncode,1,result.stderr)
            self.assertIn("output hash mismatch",result.stdout)
            good["outside"]["changed_pixels"] = 1
            fake(good)
            argv[argv.index(root / "hash")] = root / "effect"
            result = subprocess.run(argv + ["--record-hashes"],capture_output=True,text=True)
            self.assertEqual(result.returncode,1,result.stderr)
            self.assertIn("expected 0, got 1",result.stdout)
            self.assertEqual(path.read_bytes(),original)

    def test_missing_bundle_is_explicit_skip_and_unknown_id_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            binary = root/"saccade"
            binary.write_text('#!/usr/bin/env python3\nprint(\'{"compiled_features":[]}\')\n')
            binary.chmod(0o755)
            argv = ["python3",ROOT/"scripts/run-playbooks.py","--bin",binary,"--out",root/"run","--only","W02"]
            result = subprocess.run(argv,capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stderr)
            receipt = RUNNER.load(root/"run/result.json")
            self.assertEqual(receipt["playbooks"][0]["status"],"skip")
            self.assertIn("products",receipt["playbooks"][0]["reason"])
            self.assertEqual(subprocess.run(argv,capture_output=True).returncode,2)
            self.assertEqual(subprocess.run(argv+["--require-all"],capture_output=True).returncode,2)


if __name__ == "__main__":
    unittest.main()
