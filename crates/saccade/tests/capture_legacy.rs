//! Fictional historical producers exercise the public adapter boundary.
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn kit() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/capture-legacy")
}
fn run(root: &Path, map: &Path, archive: bool) -> (i32, Vec<Value>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_saccade"));
    command
        .args(["capture", "conform", "--json", "--legacy-map"])
        .arg(map)
        .arg(root);
    if archive {
        command.arg("--archive");
    }
    let output = command.output().unwrap();
    let rows = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    (output.status.code().unwrap(), rows)
}
fn fixture(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for entry in std::fs::read_dir(kit().join(name)).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, dir.path().join(path.file_name().unwrap())).unwrap();
    }
    dir
}
fn load(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn write(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
#[test]
fn two_formats_and_toml_pass_only_as_adapted_and_native_remains_native() {
    for (name, ext) in [("atlas", "json"), ("atlas", "toml"), ("beacon", "json")] {
        let (exit, rows) = run(
            &kit().join(name),
            &kit().join(name).join(format!("map.{ext}")),
            false,
        );
        assert_eq!(exit, 3, "{rows:?}");
        let schema: Value = serde_json::from_str(
            saccade_core::schema_catalog::get(saccade_core::capture::RESULT_SCHEMA).unwrap(),
        )
        .unwrap();
        assert!(
            jsonschema::validator_for(&schema)
                .unwrap()
                .is_valid(&rows[0])
        );
        assert_eq!(rows[0]["provenance"], "adapted_legacy");
        assert_eq!(rows[0]["conformant"], true);
        assert!(rows[0]["unavailable"].as_array().unwrap().is_empty());
        assert!(!rows[0]["fields"].as_array().unwrap().is_empty());
        if name == "atlas" {
            assert_eq!(rows[0]["retry_records"].as_array().unwrap().len(), 1);
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["capture", "conform", "--json"])
        .arg(kit().join("../capture-kit/renderer-valid.json"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["provenance"],
        "native"
    );
}
#[test]
fn failure_missing_evidence_and_check_time_hashes_are_distinct_in_both_formats() {
    for name in ["atlas", "beacon"] {
        let dir = fixture(name);
        let root = dir.path();
        let map = root.join("map.json");
        let record = root.join(if name == "atlas" {
            "ledger.json"
        } else {
            "journal.json"
        });
        let original = load(&record);
        let (array, settings, clock, status) = if name == "atlas" {
            ("attempted", "/settings", "/clock", "/status")
        } else {
            (
                "results",
                "/observed/configuration",
                "/observed/window",
                "/outcome",
            )
        };
        for (path, value, code, exit) in [
            (settings, json!({"different":true}), "settings_mismatch", 1),
            (clock, Value::Null, "observed_clock_unavailable", 4),
            (
                status,
                json!(if name == "atlas" { "failed" } else { "error" }),
                "acquisition_failed",
                1,
            ),
        ] {
            let mut v = original.clone();
            *v[array][0].pointer_mut(path).unwrap() = value;
            write(&record, &v);
            let (actual_exit, rows) = run(root, &map, false);
            let report = &rows[0];
            assert_eq!(actual_exit, exit, "{report}");
            assert!(
                report[if exit == 4 { "unavailable" } else { "findings" }]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["code"] == code),
                "{report}"
            );
        }
        let mut missing_path = original.clone();
        let path = if name == "atlas" {
            "/image/path"
        } else {
            "/artifact/name"
        };
        *missing_path[array][0].pointer_mut(path).unwrap() = Value::Null;
        write(&record, &missing_path);
        let (exit, rows) = run(root, &map, false);
        assert_eq!(exit, 4, "{rows:?}");
        assert!(
            rows[0]["unavailable"]
                .as_array()
                .unwrap()
                .iter()
                .any(|u| u["code"] == "image_path_unavailable")
        );
        write(&record, &original);
        let mut mapping = load(&map);
        mapping["hash_policy"] = "compute_now".into();
        write(&map, &mapping);
        let (exit, rows) = run(root, &map, false);
        assert_eq!(exit, 4, "{rows:?}");
        assert!(
            rows[0]["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["state"] == "hashed_at_check_time")
        );
        assert!(
            rows[0]["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["state"] == "hashed_at_check_time"
                    && f["sha256"]
                        == "8229e9c68c0a509b92a65369083e507c0ac4aafc5f7ccbae11aa77656222ea49")
        );
        assert_eq!(
            rows[0]["unavailable"][0]["code"],
            "acquisition_hash_unavailable"
        );
        // Failed attempts remain failed even when acquisition hashes cannot be established.
        let mut failed = original.clone();
        *failed[array][0].pointer_mut(status).unwrap() =
            (if name == "atlas" { "failed" } else { "error" }).into();
        write(&record, &failed);
        assert_eq!(run(root, &map, false).0, 1);
    }
}
#[test]
fn unavailable_plan_does_not_derive_a_plan_from_outputs_or_hide_failed_attempts() {
    let dir = fixture("atlas");
    let root = dir.path();
    let path = root.join("ledger.json");
    let mut record = load(&path);
    record.as_object_mut().unwrap().remove("planned");
    record["attempted"][0]["status"] = "failed".into();
    write(&path, &record);
    let (exit, rows) = run(root, &root.join("map.json"), false);
    assert_eq!(exit, 1);
    assert_eq!(rows[0]["expected"], 0);
    assert_eq!(rows[0]["unavailable"][0]["code"], "expected_unavailable");
    assert_eq!(rows[0]["findings"][0]["code"], "acquisition_failed");
}
#[test]
fn invalid_defaults_paths_duplicates_and_stale_bytes_fail_closed() {
    let dir = fixture("atlas");
    let root = dir.path();
    let map = root.join("map.json");
    let original = load(&map);
    for change in [
        json!({"path":"run_id", "default":"invented"}),
        json!({"path":"run.*"}),
    ] {
        let mut mapping = original.clone();
        mapping["fields"]["run_id"] = change;
        write(&map, &mapping);
        assert_eq!(run(root, &map, false).0, 2);
    }
    write(&map, &original);
    let path = root.join("ledger.json");
    let original_record = load(&path);
    let mut record = original_record.clone();
    record["attempted"]
        .as_array_mut()
        .unwrap()
        .push(original_record["attempted"][0].clone());
    write(&path, &record);
    assert_eq!(run(root, &map, false).0, 2);
    write(&path, &original_record);
    std::fs::write(root.join("image.png"), b"changed encoded bytes").unwrap();
    assert_eq!(
        run(root, &map, false).1[0]["findings"][0]["code"],
        "stale_hash"
    );
    let mut record = original_record;
    record["attempted"][0]["image"]["path"] = "../escape.png".into();
    write(&path, &record);
    assert_eq!(
        run(root, &map, false).1[0]["findings"][0]["code"],
        "unsafe_image_path"
    );
}
#[test]
fn deterministic_archive_rows_and_bounds_are_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for name in ["b", "a"] {
        std::fs::create_dir(root.join(name)).unwrap();
        for entry in std::fs::read_dir(kit().join("atlas")).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, root.join(name).join(path.file_name().unwrap())).unwrap();
        }
    }
    let map = kit().join("atlas/map.json");
    let before = std::fs::read(root.join("a/ledger.json")).unwrap();
    let (exit, rows) = run(root, &map, true);
    assert_eq!(exit, 3);
    assert_eq!(rows.len(), 2);
    assert!(rows[0]["directory"].as_str().unwrap().ends_with("/a"));
    assert_eq!(std::fs::read(root.join("a/ledger.json")).unwrap(), before);
    let mut deep = root.to_owned();
    for _ in 0..17 {
        deep.push("deep");
        std::fs::create_dir(&deep).unwrap();
    }
    assert_ne!(run(root, &map, true).0, 0);
    let (exit, refused) = run(root, &map, true);
    assert_eq!(exit, 2);
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["report"]["provenance"], "adapted_legacy");
    assert_eq!(
        refused[0]["report"]["findings"][0]["code"],
        "invalid_record"
    );
}
#[cfg(unix)]
#[test]
fn symlinks_in_images_sources_roots_maps_and_archives_are_refused() {
    use std::os::unix::fs::symlink;
    let dir = fixture("atlas");
    let root = dir.path();
    let map = root.join("map.json");
    std::fs::rename(root.join("image.png"), root.join("target.png")).unwrap();
    symlink("target.png", root.join("image.png")).unwrap();
    assert_eq!(
        run(root, &map, false).1[0]["findings"][0]["code"],
        "unsafe_image_path"
    );
    std::fs::remove_file(root.join("image.png")).unwrap();
    std::fs::rename(root.join("target.png"), root.join("image.png")).unwrap();
    symlink("map.json", root.join("link.json")).unwrap();
    assert_eq!(run(root, &root.join("link.json"), false).0, 2);
    std::fs::remove_file(root.join("link.json")).unwrap();
    std::fs::rename(root.join("ledger.json"), root.join("target.json")).unwrap();
    symlink("target.json", root.join("ledger.json")).unwrap();
    assert_eq!(run(root, &map, false).0, 2);
    let parent = tempfile::tempdir().unwrap();
    symlink(root, parent.path().join("linked")).unwrap();
    assert_eq!(run(&parent.path().join("linked"), &map, false).0, 2);
    assert_ne!(run(parent.path(), &map, true).0, 0);
}

#[test]
fn nested_subtree_gaps_and_row_byte_projection_limits_remain_explicit() {
    let dir = fixture("atlas");
    let root = dir.path();
    let path = root.join("ledger.json");
    let map = root.join("map.json");
    let original = load(&path);
    let mut record = original.clone();
    record["planned"][0]["fingerprint"]["producer"]
        .as_object_mut()
        .unwrap()
        .remove("binary");
    write(&path, &record);
    let (exit, rows) = run(root, &map, false);
    assert_eq!(exit, 4, "{rows:?}");
    assert!(
        rows[0]["unavailable"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["field"] == "expected.0.fingerprint.producer.binary"
                && u["code"] == "planned_identity_unavailable")
    );
    let mut record = original.clone();
    record["attempted"] = Value::Array(vec![original["attempted"][0].clone(); 257]);
    write(&path, &record);
    assert_eq!(run(root, &map, false).0, 2);
    let mut record = original;
    record["attempted"][0]["settings"]["large"] = "x".repeat(600_000).into();
    write(&path, &record);
    // Raw source fits 1 MiB, but selecting the array and projecting rows exceeds the cumulative cap.
    assert!(std::fs::metadata(&path).unwrap().len() < 1024 * 1024);
    assert_eq!(run(root, &map, false).0, 2);
}

#[cfg(unix)]
#[test]
fn directory_aliases_and_symlink_parent_traversal_cannot_bypass_refusal() {
    use std::os::unix::fs::symlink;
    let dir = fixture("atlas");
    let parent = tempfile::tempdir().unwrap();
    let alias = parent.path().join("alias");
    symlink(dir.path(), &alias).unwrap();
    assert_eq!(run(&alias, &dir.path().join("map.json"), false).0, 2);
    std::fs::copy(dir.path().join("map.json"), parent.path().join("map.json")).unwrap();
    // Even when lexical folding would erase the link, inspect it before '..'.
    assert_eq!(run(dir.path(), &alias.join("../map.json"), false).0, 2);
    let child = dir.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let route = child.join("../map.json");
    assert_eq!(run(dir.path(), &route, false).0, 3);
}
