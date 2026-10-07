//! Generated hostile intake and explicit correspondence through the real worker.
#![cfg(feature = "documents")]
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .unwrap()
}
fn compare(a: &Path, b: &Path, out: &Path, map: Option<&Path>) -> Output {
    let mut args = vec![
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--json",
    ];
    if let Some(map) = map {
        args.extend(["--page-map", map.to_str().unwrap()]);
    }
    cli(&args)
}
fn fixtures() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/gen-document-hostile.py"
            ))
            .arg(dir.path())
            .status()
            .unwrap()
            .success()
    );
    dir
}
#[test]
fn hostile_corpus_refuses_with_stable_caps_and_host_survives() {
    let dir = fixtures();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("manifest.json")).unwrap()).unwrap();
    for case in manifest["hostile"].as_array().unwrap() {
        let path = dir.path().join(case["file"].as_str().unwrap());
        let result = compare(&path, &path, &dir.path().join("refused"), None);
        assert_eq!(
            result.status.code(),
            Some(2),
            "{case}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["errors"][0]["code"], case["expected_code"], "{case}");
        assert!(!dir.path().join("refused").exists());
    }
    let svg = dir.path().join("good.svg");
    std::fs::write(&svg,br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="red"/></svg>"#).unwrap();
    assert!(
        compare(&svg, &svg, &dir.path().join("survived"), None)
            .status
            .success()
    );
    // Even the internal entry point cannot parse outside the OS envelope.
    assert_eq!(cli(&["--document-worker"]).status.code(), Some(2));
}
#[test]
fn inserted_reordered_pages_are_explicit_and_never_shift_paired() {
    let dir = fixtures();
    for domain in ["report", "figure"] {
        let a = dir.path().join(format!("{domain}-before.pdf"));
        let b = dir.path().join(format!("{domain}-after.pdf"));
        let map = dir.path().join(format!("{domain}-map.json"));
        let out = dir.path().join(domain);
        let unmapped = compare(&a, &b, &out, None);
        assert_eq!(unmapped.status.code(), Some(2));
        assert_eq!(
            serde_json::from_slice::<Value>(&unmapped.stdout).unwrap()["errors"][0]["code"],
            "document_page_map_required"
        );
        let result = compare(&a, &b, &out, Some(&map));
        assert_eq!(
            result.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let report: Value =
            serde_json::from_slice(&std::fs::read(out.join("saccade-documents.v3.json")).unwrap())
                .unwrap();
        let schema: Value = serde_json::from_str(
            saccade_core::schema_catalog::get("saccade-documents.v3").unwrap(),
        )
        .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&report)
            .unwrap();
        assert_eq!(report["pages"][0]["status"], "pass");
        assert_eq!(report["pages"][1]["status"], "pass");
        assert_eq!(report["pages"][0]["candidate_page"], 3);
        assert_eq!(report["pages"][1]["candidate_page"], 1);
        assert_eq!(report["pages"][2]["status"], "new");
        assert_eq!(report["counts"]["failures"], 1);
        assert_eq!(report["caps"]["memory_bytes"], 536870912);
        // A map bound to another export, duplicate pair, omitted page or out-of-range page cannot yield a verdict.
        let original: Value = serde_json::from_slice(&std::fs::read(&map).unwrap()).unwrap();
        for change in [
            json!({"reference_sha256":"0".repeat(64)}),
            json!({"pairs":[{"reference":1,"candidate":1},{"reference":1,"candidate":2}]}),
            json!({"pairs":[{"reference":1,"candidate":1}]}),
            json!({"pairs":[{"reference":501,"candidate":1}]}),
            json!({"pairs":[{"reference":1}]}),
        ] {
            let mut bad = original.clone();
            for (k, v) in change.as_object().unwrap() {
                bad[k] = v.clone();
            }
            std::fs::write(&map, serde_json::to_vec(&bad).unwrap()).unwrap();
            let result = compare(&a, &b, &dir.path().join("bad-map"), Some(&map));
            assert_eq!(result.status.code(), Some(2));
            assert_eq!(
                serde_json::from_slice::<Value>(&result.stdout).unwrap()["errors"][0]["code"],
                "document_page_map_invalid"
            );
        }
    }
}
#[cfg(target_os = "linux")]
#[test]
fn worker_allocation_and_deadline_are_enforced_by_parent_and_os() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let svg = dir.path().join("input.svg");
    std::fs::write(
        &svg,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#,
    )
    .unwrap();
    for (name, body, code) in [
        (
            "memory",
            "exec /usr/bin/python3 -c 'import resource; assert resource.getrlimit(resource.RLIMIT_AS) == (536870912,536870912); assert resource.getrlimit(resource.RLIMIT_CPU) == (10,10); assert resource.getrlimit(resource.RLIMIT_FSIZE)[0] == 67112960; assert resource.getrlimit(resource.RLIMIT_NOFILE)[0] == 32; assert resource.getrlimit(resource.RLIMIT_CORE)[0] == 0; bytearray(600 * 1024 * 1024)'",
            "document_worker_resource_limit",
        ),
        ("time", "exec /bin/sleep 60", "document_wall_time_limit"),
    ] {
        let worker = dir.path().join(name);
        std::fs::write(&worker, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o700)).unwrap();
        let started = std::time::Instant::now();
        let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .env("SACCADE_DOCUMENT_WORKER", &worker)
            .args([
                "compare",
                svg.to_str().unwrap(),
                svg.to_str().unwrap(),
                "--out",
                dir.path().join(name.to_string() + "-out").to_str().unwrap(),
                "--json",
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap()["errors"][0]["code"],
            code
        );
        assert!(started.elapsed().as_secs() < 18);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn comparison_refuses_large_parent_raster_before_registration() {
    let dir = tempfile::tempdir().unwrap();
    let svg = dir.path().join("large.svg");
    std::fs::write(
        &svg,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="1025" height="1024"/>"#,
    )
    .unwrap();
    let out = dir.path().join("out");
    let result = compare(&svg, &svg, &out, None);
    assert_eq!(
        result.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["errors"][0]["code"],
        "document_total_pixel_limit"
    );
    assert!(!out.join("page-0001").exists());
    assert_eq!(cli(&["--document-operation"]).status.code(), Some(2));
}

#[cfg(target_os = "linux")]
#[test]
fn worker_revokes_inherited_file_and_socket_even_above_nofile_limit() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let svg = dir.path().join("input.svg");
    std::fs::write(
        &svg,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#,
    )
    .unwrap();
    let sentinel = dir.path().join("sentinel");
    std::fs::write(&sentinel, b"generated-inherited-authority").unwrap();
    let adapter = dir.path().join("adapter.py");
    std::fs::write(
        &adapter,
        format!(
            r#"#!/usr/bin/python3
import os, sys
for fd in (200, 201):
    try:
        os.fstat(fd)
    except OSError:
        pass
    else:
        sys.exit(9)
os.execv({exe:?}, [{exe:?}, '--document-worker'])
"#,
            exe = env!("CARGO_BIN_EXE_saccade")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&adapter, std::fs::Permissions::from_mode(0o700)).unwrap();
    // The outer launcher deliberately installs non-CLOEXEC file and socket
    // descriptors above the worker's lowered NOFILE ceiling, then execs the CLI.
    for command in ["compare", "hash"] {
        let mut command_args = vec![command, svg.to_str().unwrap()];
        if command == "compare" {
            command_args.push(svg.to_str().unwrap());
        }
        let out = dir.path().join(command);
        command_args.extend(["--out", out.to_str().unwrap(), "--json"]);
        let result = Command::new("python3")
            .args([
                "-c",
                r#"
import os, socket, sys
fd = os.open(sys.argv[1], os.O_RDONLY)
os.dup2(fd, 200, inheritable=True)
a, b = socket.socketpair()
os.dup2(a.fileno(), 201, inheritable=True)
os.execv(sys.argv[2], sys.argv[2:])
"#,
            ])
            .arg(&sentinel)
            .arg(env!("CARGO_BIN_EXE_saccade"))
            .args(command_args)
            .env("SACCADE_DOCUMENT_WORKER", &adapter)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn pdf_lexer_nul_and_ambiguous_filters_refuse_before_upstream_decode() {
    let dir = fixtures();
    let safe = dir.path().join("nul-filter-safe.pdf");
    let control = compare(&safe, &safe, &dir.path().join("safe-nul"), None);
    assert!(
        control.status.success(),
        "{}",
        String::from_utf8_lossy(&control.stdout)
    );
    for (name, code) in [
        ("nul-filter-bomb.pdf", "document_decompressed_limit"),
        ("vt-filter-bomb.pdf", "document_unsupported"),
        ("unknown-filter.pdf", "document_unsupported"),
        ("filter-as-value-filter.pdf", "document_unsupported"),
        ("array-filter.pdf", "document_unsupported"),
        ("duplicate-array-filter.pdf", "document_malformed"),
    ] {
        let input = dir.path().join(name);
        let out = dir.path().join("refused-filter");
        let result = compare(&input, &input, &out, None);
        assert_eq!(result.status.code(), Some(2), "{name}");
        assert_eq!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap()["errors"][0]["code"],
            code,
            "{name}"
        );
        assert!(!out.exists());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn complete_comparison_process_has_independent_os_limits() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let svg = dir.path().join("input.svg");
    std::fs::write(
        &svg,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#,
    )
    .unwrap();
    let adapter = dir.path().join("parent-limits.py");
    std::fs::write(&adapter, format!(r#"#!/usr/bin/python3
import os
limits = open('/proc/%d/limits' % os.getppid()).read().splitlines()
for name, expected in [('Max address space', 1073741824), ('Max cpu time', 60), ('Max file size', 67112960), ('Max core file size', 0), ('Max open files', 32)]:
    values = next(line[len(name):].split()[:2] for line in limits if line.startswith(name))
    assert values == [str(expected), str(expected)], (name, values)
os.execv({exe:?}, [{exe:?}, '--document-worker'])
"#, exe=env!("CARGO_BIN_EXE_saccade"))).unwrap();
    std::fs::set_permissions(&adapter, std::fs::Permissions::from_mode(0o700)).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .env("SACCADE_DOCUMENT_WORKER", &adapter)
        .args([
            "compare",
            svg.to_str().unwrap(),
            svg.to_str().unwrap(),
            "--out",
            dir.path().join("out").to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
}
