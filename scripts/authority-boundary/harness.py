#!/usr/bin/env python3
"""Offline CLI and stdio MCP authority probes, using only generated fixtures."""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

SCHEMA = "saccade-authority-boundary.v1"



def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def png(path, value):
    """Generate an original 8x8 RGB PNG; no fonts or downloaded imagery."""
    def chunk(kind, data):
        return (struct.pack(">I", len(data)) + kind + data
                + struct.pack(">I", zlib.crc32(kind + data)))
    path.parent.mkdir(parents=True, exist_ok=True)
    data = b"\x89PNG\r\n\x1a\n"
    data += chunk(b"IHDR", struct.pack(">IIBBBBB", 8, 8, 8, 2, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress((b"\0" + bytes([value] * 24)) * 8))
    path.write_bytes(data + chunk(b"IEND", b""))


def error_code(value):
    errors = value.get("errors", [])
    return errors[0].get("code") if errors else None


def classify(expected_exit, expected_code, exit_code, code, invariant, finding=None):
    matched = (exit_code == expected_exit and code == expected_code and invariant)
    if finding:
        # A changed known failure is never automatically accepted as a fix.
        return "XFAIL" if matched else "FAIL"
    return "PASS" if matched else "FAIL"


class Harness:
    def __init__(self, binary, work, protected_baseline=None):
        self.binary, self.work = binary, work
        self.protected_baseline = protected_baseline
        self.inputs, self.outputs, self.outside = [work / n for n in (
            "inputs", "outputs", "outside")]
        for path in (self.inputs, self.outputs, self.outside, work / "config"):
            path.mkdir()
        # Whitelist child environment: never inherit provider keys or user policy.
        self.env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"),
                    "HOME": str(work / "config"),
                    "XDG_CONFIG_HOME": str(work / "config"),
                    "XDG_CACHE_HOME": str(work / "cache"),
                    "XDG_DATA_HOME": str(work / "data"), "RAYON_NUM_THREADS": "2"}
        self.cases = []
        self.policy = work / "config/user.toml"
        self.policy.write_text(
            f"out_root = {json.dumps(str(self.outputs))}\n"
            f"[[roots]]\nid = 'inputs'\npath = {json.dumps(str(self.inputs))}\n"
            "egress = 'deny'\n")

    def cli(self, argv):
        result = subprocess.run([str(self.binary), *map(str, argv)], cwd=self.work,
                                env=self.env, capture_output=True, text=True, timeout=30)
        try:
            value = json.loads(result.stdout)
        except ValueError as error:
            raise RuntimeError(f"CLI output is not JSON: exit={result.returncode}; "
                               f"stderr={result.stderr[:256]}") from error
        return result.returncode, value, {"argv": list(map(str, argv)),
                                        "stdout": result.stdout, "stderr": result.stderr}

    def record(self, name, transport, expected_exit, expected_code, result,
               invariant=True, finding=None):
        exit_code, value, receipt = result
        code = error_code(value)
        status = classify(expected_exit, expected_code, exit_code, code, invariant, finding)
        case = {"id": name, "transport": transport, "status": status,
                "expected_exit": expected_exit, "expected_code": expected_code,
                "observed_exit": exit_code, "observed_code": code,
                "invariant": invariant, "reason": finding, "receipt": receipt}
        self.cases.append(case)
        print(f"{status} {name}: exit={exit_code} code={code}", flush=True)

    def mcp(self, calls, startup=()):
        messages = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": {"name": "authority-agent", "version": "1"}}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "ping"},
        ]
        for index, (_, name, args, _) in enumerate(calls, 4):
            messages.append({"jsonrpc": "2.0", "id": index, "method": "tools/call",
                             "params": {"name": name, "arguments": args}})
        argv = [str(self.binary), "mcp", "--root", str(self.inputs), "--out-root",
                str(self.outputs), "--user-config", str(self.policy), *startup]
        result = subprocess.run(argv, input="".join(json.dumps(m) + "\n" for m in messages),
                                cwd=self.work, env=self.env, text=True,
                                capture_output=True, timeout=60)
        replies = [json.loads(line) for line in result.stdout.splitlines()]
        if (result.returncode != 0 or len(replies) != len(messages) - 1
                or [r.get("id") for r in replies] != list(range(1, len(replies) + 1))
                or replies[0]["result"]["protocolVersion"] != "2025-06-18"
                or replies[2].get("result") != {}):
            raise RuntimeError("MCP initialization/framing/process failure")
        tools = {t["name"] for t in replies[1]["result"]["tools"]}
        if not {"saccade_measure", "saccade_review"} <= tools:
            raise RuntimeError("MCP required tools missing")
        for (case, _, _, expected), request, reply in zip(calls, messages[4:], replies[3:]):
            body = reply.get("result", {})
            value = body.get("structuredContent", {})
            rpc = reply.get("error")
            if rpc:
                value = {"errors": [{"code": f"rpc:{rpc['code']}"}]}
            # Tool failures are normal JSON-RPC results, not process failures.
            failed = bool(rpc) or body.get("isError") is True
            expected_failed = expected is not None
            receipt = {"request": request, "reply": reply, "stderr": result.stderr}
            self.record(case, "mcp", 0, expected, (0, value, receipt),
                        failed == expected_failed and (not case.startswith("mcp-positive")
                        or (self.outputs / "mcp-positive/saccade-report.v1.json").is_file()))

    def run(self):
        for name, value in (("baseline", 20), ("candidate", 220)):
            png(self.inputs / name / "sample.png", value)
        report_dir = self.outputs / "report"
        setup = self.cli(["compare", self.inputs / "baseline", self.inputs / "candidate",
                          "--out", report_dir, "--json"])
        self.record("cli-positive-compare", "cli", 1, None, setup,
                    (report_dir / "saccade-report.v1.json").is_file())
        report = report_dir / "saccade-report.v1.json"
        foreign = self.outside / "report.json"
        shutil.copyfile(report, foreign)
        (self.inputs / "escape").symlink_to(self.outside, target_is_directory=True)
        (self.outputs / "escape").symlink_to(self.outside, target_is_directory=True)
        # Out-root is a sibling of the read roots, including lookalike names.
        sibling = self.work / "outputs-other"
        sibling.mkdir()
        cli_cases = [
            ("read-outside", foreign, self.outputs / "read-denied.json"),
            ("read-traversal", self.inputs / "../outside/report.json", self.outputs / "traverse.json"),
            ("read-symlink", self.inputs / "escape/report.json", self.outputs / "link.json"),
            ("write-outside", report, self.outside / "write.json"),
            ("write-prefix", report, sibling / "write.json"),
            ("write-traversal", report, self.outputs / "../outside/traverse.json"),
            ("write-symlink", report, self.outputs / "escape/new/child.json"),
        ]
        for name, source, output in cli_cases:
            for mode, flags in (("run", ["--run", "--budget-calls", "1"]), ("preview", [])):
                result = self.cli(["review", source, *flags, "--user-config", self.policy,
                                   "--out", output, "--json"])
                self.record(f"cli-{mode}-{name}", "cli", 2, "config", result, not output.exists())
        # Invalid JSON outside the roots must be refused before it is parsed.
        intent = self.outside / "intent.json"
        intent.write_text("not JSON")
        for mode, flags in (("run", ["--run"]), ("preview", [])):
            result = self.cli(["review", report, *flags, "--user-config", self.policy,
                               "--intent-file", intent, "--json"])
            self.record(f"cli-{mode}-outside-intent", "cli", 2, "config", result)
        intent.unlink()
        for mode in ([], ["--run", "--budget-calls", "1"]):
            name = "cli-egress-denied" if mode else "cli-positive-preview"
            output = self.outputs / ("denied.json" if mode else "preview")
            result = self.cli(["review", report, *mode, "--user-config", self.policy,
                               "--out", output, "--json"])
            self.record(name, "cli", 2 if mode else 0,
                        "egress_denied" if mode else None, result,
                        not output.exists() if mode else (output / "requests.json").is_file())
        measure = {"operation": "compare", "baseline_dir": str(self.inputs / "baseline"),
                   "capture_dir": str(self.inputs / "candidate"), "out": "mcp-positive"}
        calls = [("mcp-positive-compare", "saccade_measure", measure, None)]
        for name, source, output in cli_cases:
            calls.append(("mcp-" + name, "saccade_review",
                          {"operation": "preview", "artifact": str(source),
                           "out": str(output)}, "unsafe_path"))
        calls += [
            ("mcp-network-startup-denied", "saccade_review",
             {"operation": "run", "artifact": str(report), "out": "network.json"},
             "network_authorization_required"),
            ("mcp-forged-startup", "saccade_review",
             {"operation": "run", "artifact": str(report), "allow_provider_calls": True}, "usage"),
            ("mcp-no-baseline-tool", "saccade_approve", {}, "rpc:-32602"),
            ("mcp-no-baseline-operation", "saccade_measure", {"operation": "approve"}, "usage"),
        ]
        self.mcp(calls)
        # A valid report with a transitive companion escape must fail before parsing it.
        evidence = report_dir / "evidence.json"
        saved = evidence.read_bytes()
        evidence.unlink()
        evidence.symlink_to(foreign)
        try:
            result = self.cli(["review", report, "--user-config", self.policy, "--json"])
            self.record("cli-preview-companion-symlink", "cli", 2, "config", result)
        finally:
            evidence.unlink()
            evidence.write_bytes(saved)
        self.mcp([("mcp-egress-denied", "saccade_review", {"operation": "run",
                   "artifact": str(report), "out": "egress.json"}, "egress_denied")],
                 ["--allow-provider-calls", "--budget-calls", "1"])
        # Symlinks in the image tree must be refused before comparison walks it.
        nested = self.inputs / "candidate/nested.png"
        nested.symlink_to(foreign)
        try:
            self.mcp([("mcp-nested-input-symlink", "saccade_measure",
                       {**measure, "out": "nested-denied"}, "unsafe_path")])
        finally:
            nested.unlink()
        # Valid reviewed content prevents a stale/invalid decision from masking the boundary.
        plan = self.outputs / "approval-plan"
        result = self.cli(["approve", "--report", report, "--entry", "sample.png",
                           "--dry-run", "--out", plan, "--json"])
        self.record("cli-positive-approval-plan", "cli", 0, None, result,
                    (plan / "decision.json").is_file())
        baseline = self.inputs / "baseline/sample.png"
        before = digest(baseline)
        args = ["approve", "--report", report, "--decisions", plan / "decision.json", "--json"]
        if self.protected_baseline:
            frozen = self.protected_baseline / "sample.png"
            if digest(frozen) != before:
                raise RuntimeError("protected mount must contain the generated baseline fixture")
            try:
                frozen.chmod(frozen.stat().st_mode)
            except OSError as error:
                if error.errno != errno.EROFS:
                    raise RuntimeError("protected fixture is not a read-only mount") from error
            else:
                raise RuntimeError("protected fixture is not a read-only mount")
            frozen_dir = self.outputs / "frozen-report"
            result = self.cli(["compare", self.protected_baseline, self.inputs / "candidate",
                               "--out", frozen_dir, "--json"])
            self.record("cli-positive-mounted-compare", "cli", 1, None, result)
            frozen_report = frozen_dir / "saccade-report.v1.json"
            frozen_plan = self.outputs / "frozen-plan"
            result = self.cli(["approve", "--report", frozen_report, "--entry", "sample.png",
                               "--dry-run", "--out", frozen_plan, "--json"])
            self.record("cli-positive-mounted-plan", "cli", 0, None, result)
            result = self.cli(["approve", "--report", frozen_report, "--decisions",
                               frozen_plan / "decision.json", "--out",
                               self.outputs / "protected-approval", "--json"])
            self.record("cli-os-protected-baseline", "cli", 2, "io", result,
                        digest(frozen) == before)
        else:
            # Native smoke only: the same owner can restore these fixture modes.
            baseline.chmod(0o444)
            baseline.parent.chmod(0o555)
            try:
                result = self.cli([*args, "--out", self.outputs / "protected-approval"])
                self.record("cli-unix-mode-protected-baseline", "cli", 2, "io", result,
                            digest(baseline) == before)
            finally:
                baseline.parent.chmod(0o755)
                baseline.chmod(0o644)
        result = self.cli([*args, "--require-signed-approval", "--out",
                           self.outputs / "signed-policy-denied"])
        self.record("cli-uncredentialed-approval-policy-on", "cli", 2,
                    "approval_signature_required", result, digest(baseline) == before)
        result = self.cli([*args, "--out", self.outputs / "uncredentialed-approval"])
        self.record("cli-uncredentialed-approval-policy-off", "cli", 0, None, result,
                    digest(baseline) == digest(self.inputs / "candidate/sample.png")
                    and before != digest(baseline)
                    and result[1].get("data", {}).get("authority") == "cli"
                    and "human_attestation" in result[1].get("data", {})
                    and result[1]["data"]["human_attestation"] is None)
        # Denied calls must not create a dispatch ledger or escaped artifacts.
        self.record("no-denied-dispatch-or-output", "harness", 0, None, (0, {}, {}),
                    not (self.work / "config/attempts").exists()
                    and list(self.outside.iterdir()) == [foreign]
                    and not list(sibling.iterdir()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--protected-baseline", type=Path,
                        help="read-only mount of the generated baseline fixture (container proof)")
    parser.add_argument("--allow-known-findings", action="store_true",
                        help="CI regression mode: retain XFAIL/failed acceptance in the report")
    args = parser.parse_args()
    binary = args.bin.resolve(strict=True)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {"schema": SCHEMA, "acceptance": "failed", "cases": [],
              "binary_sha256": digest(binary), "fixture_provenance": {
                  "generator": "scripts/authority-boundary/harness.py:png",
                  "license": "CC0-1.0", "source": "original procedural RGB squares"},
              "limitations": ["Static filesystem probes; concurrent rename races unverified.",
                              "Unrestricted CLI/shell requires OS confinement; MCP is the policy surface.",
                              "MCP mirrors for anchor/approval remain a follow-up."]}
    report["baseline_protection"] = "read_only_mount" if args.protected_baseline else "unix_modes_only"
    try:
        if not hasattr(os, "geteuid") or os.geteuid() == 0:
            raise RuntimeError("run as an unprivileged Unix user (see container recipe)")
        with tempfile.TemporaryDirectory(prefix="authority-") as directory:
            harness = Harness(binary, Path(directory), args.protected_baseline)
            report["build"] = harness.cli(["doctor", "--json"])[1].get("build")
            try:
                harness.run()
            finally:
                report["cases"] = harness.cases
                # Bind the generated input bytes before their temporary directory is removed.
                report["fixture_provenance"]["candidate_sha256"] = digest(
                    harness.inputs / "candidate/sample.png")
        failed = any(c["status"] == "FAIL" for c in report["cases"])
        known = any(c["status"] == "XFAIL" for c in report["cases"])
        report["acceptance"] = "failed" if failed or known else "passed"
        exit_code = int(failed or (known and not args.allow_known_findings))
        report["regression_gate"] = "failed" if exit_code else "passed"
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        report["harness_error"] = str(error)
        report["regression_gate"] = "failed"
        exit_code = 2
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"acceptance={report['acceptance']} regression_gate={report['regression_gate']}")
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
