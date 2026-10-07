//! Bounded capture-record validation and retention of every planned slot.
#![allow(clippy::unwrap_used)]
use saccade_core::capture::{self, Code, Record};
use std::path::PathBuf;

fn kit() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/capture-kit")
}
fn record() -> Record {
    serde_json::from_slice(&std::fs::read(kit().join("renderer-valid.json")).unwrap()).unwrap()
}
#[test]
fn all_planned_slots_and_multiple_failures_remain_visible() {
    let mut record = record();
    let mut second = record.expected[0].clone();
    second.id = "frame-002".into();
    record.expected.push(second);
    record.acquisitions[0].settings = None;
    record.acquisitions[0].clock = None;
    let result = capture::conform(&record, &kit());
    assert_eq!(result.expected, 2);
    assert_eq!(result.acquired, 1);
    assert_eq!(
        result.findings.iter().map(|f| f.code).collect::<Vec<_>>(),
        vec![
            Code::SettingsMismatch,
            Code::ClockMismatch,
            Code::PartialRun
        ]
    );
    assert_eq!(result.findings[2].id.as_deref(), Some("frame-002"));
}
#[test]
fn observed_bytes_are_verified_after_the_record_was_written() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::copy(kit().join("image.png"), temp.path().join("image.png")).unwrap();
    let record = record();
    assert!(capture::conform(&record, temp.path()).conformant);
    std::fs::write(temp.path().join("image.png"), b"changed bytes").unwrap();
    assert_eq!(
        capture::conform(&record, temp.path()).findings[0].code,
        Code::StaleHash
    );
}
#[test]
fn invalid_plan_and_extra_slots_cannot_be_hidden_by_a_valid_image() {
    let mut record = record();
    record.expected[0].fingerprint["inputs"] = serde_json::Value::Null;
    assert_eq!(
        capture::conform(&record, &kit()).findings[0].code,
        Code::InvalidRecord
    );
    record = self::record();
    record.acquisitions[0].id = "unplanned".into();
    assert_eq!(
        capture::conform(&record, &kit()).findings[0].code,
        Code::InvalidRecord
    );
}
#[test]
fn oversized_records_and_images_are_rejected_without_unbounded_reads() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("receipt.json");
    std::fs::File::create(&file)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        capture::conform_path(&file).findings[0].code,
        Code::InvalidRecord
    );
    std::fs::File::create(temp.path().join("image.png"))
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        capture::conform(&record(), temp.path()).findings[0].code,
        Code::ImageUnavailable
    );
}
#[cfg(unix)]
#[test]
fn direct_and_nested_symlink_images_are_refused() {
    let temp = tempfile::tempdir().unwrap();
    let mut record = record();
    std::os::unix::fs::symlink(kit().join("image.png"), temp.path().join("image.png")).unwrap();
    assert_eq!(
        capture::conform(&record, temp.path()).findings[0].code,
        Code::UnsafeImagePath
    );
    std::os::unix::fs::symlink(kit(), temp.path().join("linked")).unwrap();
    record.acquisitions[0].image.as_mut().unwrap().path = "linked/image.png".into();
    assert_eq!(
        capture::conform(&record, temp.path()).findings[0].code,
        Code::UnsafeImagePath
    );
}

#[test]
fn additional_fingerprint_fields_are_compared_instead_of_discarded() {
    let mut record = record();
    record.acquisitions[0].fingerprint.as_mut().unwrap()["run"]["extra_setting"] =
        serde_json::json!(true);
    assert_eq!(
        capture::conform(&record, &kit()).findings[0].code,
        Code::IdentityMismatch
    );
}

#[test]
fn portable_image_paths_have_the_same_security_meaning_on_every_host() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("nested")).unwrap();
    std::fs::copy(
        kit().join("image.png"),
        temp.path().join("nested/image.png"),
    )
    .unwrap();
    let mut record = record();
    record.acquisitions[0].image.as_mut().unwrap().path = "nested/image.png".into();
    assert!(capture::conform(&record, temp.path()).conformant);
    for path in [
        r"nested\image.png",
        "C:/image.png",
        "//server/share/image.png",
        "//?/C:/image.png",
        "image.png:stream",
        "../image.png",
        "nested/./image.png",
        "nested//image.png",
        "image\n.png",
    ] {
        record.acquisitions[0].image.as_mut().unwrap().path = path.into();
        assert_eq!(
            capture::conform(&record, temp.path()).findings[0].code,
            Code::UnsafeImagePath,
            "{path:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn drive_and_verbatim_roots_produce_identical_conformance() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::copy(kit().join("image.png"), temp.path().join("image.png")).unwrap();
    let verbatim = std::fs::canonicalize(temp.path()).unwrap();
    let ordinary = saccade_core::paths::canonicalize(temp.path()).unwrap();
    for root in [&ordinary, &verbatim] {
        assert!(capture::conform(&record(), root).conformant, "{root:?}");
        std::fs::write(root.join("image.png"), b"changed bytes").unwrap();
        assert_eq!(
            capture::conform(&record(), root).findings[0].code,
            Code::StaleHash
        );
        std::fs::copy(kit().join("image.png"), root.join("image.png")).unwrap();
    }
}

#[cfg(target_os = "macos")]
#[test]
fn system_temp_alias_and_physical_root_produce_identical_conformance() {
    let temp = tempfile::tempdir_in("/var/tmp").unwrap();
    std::fs::copy(kit().join("image.png"), temp.path().join("image.png")).unwrap();
    let physical = saccade_core::paths::canonicalize(temp.path()).unwrap();
    let alias = PathBuf::from("/").join(physical.strip_prefix("/private").unwrap());
    for root in [&physical, &alias] {
        assert!(capture::conform(&record(), root).conformant, "{root:?}");
        std::fs::write(root.join("image.png"), b"changed bytes").unwrap();
        assert_eq!(
            capture::conform(&record(), root).findings[0].code,
            Code::StaleHash
        );
        std::fs::copy(kit().join("image.png"), root.join("image.png")).unwrap();
    }
}
