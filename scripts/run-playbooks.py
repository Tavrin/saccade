#!/usr/bin/env python3
"""Run W01-W13 acceptance controls; exits and hashes must match reviewed manifests."""
import argparse
import contextlib
import csv
import hashlib
import http.server
import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import threading

ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFEST_SCHEMA = "saccade-playbook.v1"
RESULT_SCHEMA = "saccade-playbook-run.v1"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False).encode()


def load(path):
    if path.stat().st_size > 16 * 1024 * 1024:
        raise ValueError(f"JSON exceeds 16 MiB: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def pointer(value, path):
    if path == "":
        return value
    if not path.startswith("/"):
        raise ValueError("JSON pointer must start with /")
    for key in path[1:].split("/"):
        key = key.replace("~1", "/").replace("~0", "~")
        value = value[int(key)] if isinstance(value, list) else value[key]
    return value


def validate_manifest(manifest, identity):
    required = {"schema", "id", "bundle", "features", "dependencies", "generated_inputs", "cwd", "commands"}
    if set(manifest) != required or manifest["schema"] != MANIFEST_SCHEMA or manifest["id"] != identity:
        raise ValueError("manifest schema/ID/fields mismatch")
    if not manifest["commands"] or not isinstance(manifest["commands"], list):
        raise ValueError("manifest needs commands")
    names = set()
    for command in manifest["commands"]:
        if set(command) != {"name", "argv", "exit", "checks", "sha256"} or command["name"] in names:
            raise ValueError("invalid or duplicate command")
        names.add(command["name"])
        if not re.fullmatch(r"[a-z0-9-]+", command["name"]) or not re.fullmatch(r"[0-9a-f]{64}", command["sha256"]):
            raise ValueError("invalid command name or SHA-256")
        if type(command["exit"]) is not int or not 0 <= command["exit"] <= 255 or not command["argv"] or not command["checks"]:
            raise ValueError("command needs argv, exit and output checks")
        check_names = set()
        for check in command["checks"]:
            if check["name"] in check_names or ("pointer" in check) == ("contains" in check):
                raise ValueError("duplicate check name or invalid selector")
            check_names.add(check["name"])


def verify(command, result, expand):
    if result.returncode != command["exit"]:
        raise ValueError(f"expected exit {command['exit']}, got {result.returncode}: {result.stderr[-1500:]} {result.stdout[-1500:]}")
    projection = {}
    for check in command["checks"]:
        source = check["source"]
        data = result.stdout if source == "stdout" else pathlib.Path(expand(source)).read_text(encoding="utf-8")
        if "contains" in check:
            value = check["contains"] in data
        else:
            value = pointer(json.loads(data), check["pointer"])
        if "equals" in check and value != check["equals"]:
            raise ValueError(f"{source} {check.get('pointer', check.get('contains'))}: expected {check['equals']!r}, got {value!r}")
        projection[check["name"]] = value
    if not projection:
        raise ValueError("no checked output projection")
    return projection, sha(canonical(projection))


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        route = self.path.strip("/")
        side = self.server.side
        if route.startswith("after/"):
            route = route[6:]
        if route == "failure" and side == "after":
            self.send_error(503)
            return
        if route == "image.png":
            data = self.server.image.read_bytes()
            content = "image/png"
        else:
            colour = "#c04020" if route == "change" and side == "after" else "#204080"
            data = f'<!doctype html><html><body style="margin:0;background:{colour}"><div style="width:32px;height:24px;background:#eee"></div></body></html>'.encode()
            content = "text/html"
        self.send_response(200)
        self.send_header("Content-Type", content)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


@contextlib.contextmanager
def server(image, side="before"):
    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler) as web:
        web.image = image
        web.side = side
        thread = threading.Thread(target=web.serve_forever, daemon=True)
        thread.start()
        try:
            yield f"http://127.0.0.1:{web.server_port}"
        finally:
            web.shutdown()
            thread.join()


def helper(args):
    kind, binary, inputs, output = args.helper, args.bin, args.inputs, args.out
    if kind == "capture":
        driver = ROOT / "integrations/playwright/sweep.cjs"
        proc = subprocess.run(["node", driver, inputs / "sweep.json", output / "captures", inputs / "capture-options.json"], capture_output=True, text=True, timeout=90)
        sys.stdout.write(proc.stdout)
        sys.stderr.write(proc.stderr)
        return proc.returncode
    if kind == "batch":
        # Catalogue W06 wrapper, with the same recursive scan/status semantics.
        files = sorted(p for p in inputs.rglob("*") if p.is_file() and p.suffix.lower() in {".png", ".jpg", ".jpeg"})
        if not files or output.exists() or output == inputs or inputs in output.parents:
            raise ValueError("batch requires inputs and a fresh output outside the source")
        output.mkdir()
        rows, issues = [], 0
        for n, image in enumerate(files, 1):
            item = f"item-{n:06d}"
            if image.is_symlink() or inputs not in image.resolve().parents:
                rows.append([item,str(image.relative_to(inputs)),"skipped_symlink","",""])
                issues += 1
                continue
            proc = subprocess.run([binary,"assess",image,"--out",output/item,"--json"], capture_output=True,text=True,timeout=120)
            (output / f"{item}.stdout.txt").write_text(proc.stdout)
            (output / f"{item}.stderr.txt").write_text(proc.stderr)
            state = "command_failed" if proc.returncode else "completed_review_verdict_separately"
            if not proc.returncode:
                json.loads(proc.stdout)
            issues += bool(proc.returncode)
            rows.append([item,str(image.relative_to(inputs)),state,proc.returncode,item])
        with (output / "index.csv").open("w", newline="") as stream:
            writer = csv.writer(stream)
            writer.writerow(["item","source","execution","exit_code","report"])
            writer.writerows(rows)
        print(json.dumps(dict(selected=len(files),issues=issues,states=[row[2] for row in rows])))
        return 2 if issues else 0
    if kind == "video":
        output.mkdir()
        subprocess.run(["ffmpeg","-v","error","-framerate","1","-i",str(inputs/"frame%03d.png"),"-c:v","mpeg4","-pix_fmt","yuv420p",str(output/"clip.mp4")], check=True,timeout=30)
        print(json.dumps({"frames":3,"clip_created":(output/"clip.mp4").is_file()}))
        return 0
    if kind == "mcp":
        output.mkdir()
        messages = [dict(jsonrpc="2.0",id=1,method="initialize",params={}),dict(jsonrpc="2.0",method="notifications/initialized"),dict(jsonrpc="2.0",id=2,method="tools/list",params={})]
        proc = subprocess.run([binary,"mcp","--root",inputs,"--out-root",output], input="".join(json.dumps(m)+"\n" for m in messages), capture_output=True,text=True,timeout=30)
        if proc.returncode:
            raise ValueError(proc.stderr)
        responses = [json.loads(line) for line in proc.stdout.splitlines()]
        (output / "responses.json").write_text(json.dumps(responses,indent=2)+"\n")
        by_id = {v["id"]:v for v in responses}
        tools = [v["name"] for v in by_id[2]["result"]["tools"]]
        print(json.dumps(dict(server=by_id[1]["result"]["serverInfo"]["name"],measure_available="saccade_measure" in tools,baseline_write_available=any("baseline" in t for t in tools))))
        return 0
    raise ValueError("unknown helper")


def dependencies(required):
    for name in required:
        if name == "browser":
            proc = subprocess.run(["node","-e","const p=require('@playwright/test'); const fs=require('fs'); if(!fs.existsSync(p.chromium.executablePath()))process.exit(1)"], cwd=ROOT/"integrations/playwright",capture_output=True,text=True,timeout=10)
            if proc.returncode:
                return "pinned Playwright package / Chromium unavailable; install @playwright/test@1.51.1 and its Chromium"
        elif not shutil.which(name):
            return f"external dependency {name} unavailable"
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", type=pathlib.Path, required=True, help="fresh run directory")
    ap.add_argument("--require-all",action="store_true",help="fail on any skipped playbook (CI)")
    ap.add_argument("--record-hashes",action="store_true",help="maintainer-only: record after exit and effect checks, never used by CI")
    ap.add_argument("--only",nargs="+",help="W01 through W13")
    ap.add_argument("--helper",choices=["capture","batch","video","mcp"],help=argparse.SUPPRESS)
    ap.add_argument("--inputs",type=pathlib.Path,help=argparse.SUPPRESS)
    args = ap.parse_args()
    args.bin = str(pathlib.Path(shutil.which(args.bin) or args.bin).resolve(strict=True))
    args.out = args.out.resolve()
    if args.helper:
        return helper(args)
    if args.out.exists():
        ap.error("--out must be new; existing evidence is never overwritten")
    manifests = sorted((ROOT/"playbooks").glob("W*/commands.json"))
    if [p.parent.name for p in manifests] != [f"W{n:02d}" for n in range(1,14)]:
        raise ValueError("exactly W01-W13 manifests required")
    if args.only and args.require_all:
        ap.error("--require-all requires all thirteen playbooks; omit --only")
    if args.only and not set(args.only).issubset(p.parent.name for p in manifests):
        ap.error("unknown playbook ID")
    proc = subprocess.run([args.bin,"capabilities","--json"],capture_output=True,text=True,check=True,timeout=30)
    features = set(json.loads(proc.stdout)["compiled_features"])
    args.out.mkdir(parents=True)
    inputs = args.out/"inputs"
    module_spec = importlib.util.spec_from_file_location("playbook_fixtures", ROOT/"scripts/gen-playbook-fixtures.py")
    module = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(module)
    module.generate(inputs)
    write_json = module.write_json
    # Resolve models/config only in this run; no ambient credentials or caches.
    env = dict(os.environ,LC_ALL="C",NO_COLOR="1",XDG_CONFIG_HOME=str(args.out/"config"),XDG_CACHE_HOME=str(args.out/"cache"))
    for key in list(env):
        if key.startswith("SACCADE_"):
            del env[key]
    results = []
    with server(inputs/"tuning/source.png", "before") as origin, server(inputs/"tuning/source.png", "after") as after:
        for path in manifests:
            if args.only and path.parent.name not in args.only:
                continue
            manifest = load(path)
            validate_manifest(manifest, path.parent.name)
            missing = set(manifest["features"]) - features
            reason = f"bundle {manifest['bundle']} missing features: {', '.join(sorted(missing))}" if missing else dependencies(manifest["dependencies"])
            row = dict(id=manifest["id"],bundle=manifest["bundle"],status="skip" if reason else "pass",reason=reason,commands=[])
            if reason:
                print(f"SKIP {row['id']}: {reason}",flush=True)
                results.append(row)
                continue
            work = args.out/manifest["id"]
            work.mkdir()
            replacements = {"@INPUT@":str(inputs),"@OUTPUT@":str(work),"@ORIGIN@":origin,"@BEFORE@":origin,"@AFTER@":after,"@BIN@":args.bin,"@PYTHON@":sys.executable,"@RUNNER@":str(pathlib.Path(__file__).resolve())}
            def expand(value):
                for key, replacement in replacements.items():
                    value = value.replace(key,replacement)
                return value
            # Files with local server origins are generated only for this run.
            (inputs/"tuning/images.txt").write_text(origin+"/image.png\n")
            (inputs/"web/urls.txt").write_text("".join(origin+"/"+route+"\n" for route in ("same","change","failure")))
            for command in manifest["commands"]:
                entry = dict(name=command["name"],status="pass")
                try:
                    argv = [expand(v) for v in command["argv"]]
                    completed = subprocess.run(argv,cwd=expand(manifest["cwd"]),env=env,capture_output=True,text=True,timeout=120)
                    (work / f"{command['name']}.stdout.txt").write_text(completed.stdout)
                    (work / f"{command['name']}.stderr.txt").write_text(completed.stderr)
                    projection, actual = verify(command,completed,expand)
                    entry.update(exit=completed.returncode,sha256=actual,projection=projection)
                    if args.record_hashes:
                        command["sha256"] = actual
                    elif actual != command["sha256"]:
                        raise ValueError(f"output hash mismatch: expected {command['sha256']}, got {actual}")
                except (OSError,ValueError,KeyError,IndexError,subprocess.SubprocessError) as error:
                    entry.update(status="fail",reason=str(error))
                    row["status"] = "fail"
                row["commands"].append(entry)
                if entry["status"] == "fail":
                    break
            if args.record_hashes and row["status"] == "pass":
                write_json(path,manifest)
            print(f"{row['status'].upper()} {row['id']}: {row['commands'][-1].get('reason', str(len(row['commands']))+' commands checked')}",flush=True)
            results.append(row)
    failed = any(r["status"] == "fail" or (args.require_all and r["status"] == "skip") for r in results)
    write_json(args.out/"result.json",dict(schema=RESULT_SCHEMA,binary_sha256=sha(pathlib.Path(args.bin).read_bytes()),compiled_features=sorted(features),
        require_all=args.require_all,recorded_hashes=args.record_hashes,playbooks=results,passed=not failed))
    return 1 if failed else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError,ValueError,subprocess.SubprocessError) as error:
        print(f"playbooks: {error}",file=sys.stderr)
        sys.exit(2)
