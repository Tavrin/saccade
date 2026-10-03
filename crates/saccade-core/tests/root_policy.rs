//! Canonical root aliases must retain read-only containment permissions.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

use saccade_core::root_policy::RootPolicy;
use std::os::unix::fs::symlink;

#[test]
fn symlinked_temp_root_preserves_routes_and_rejects_escapes() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("private-var");
    let alias = tmp.path().join("var");
    for dir in ["root", "other/deep", "storage"] {
        std::fs::create_dir_all(real.join(dir)).unwrap();
    }
    symlink(&real, &alias).unwrap();
    std::fs::write(real.join("root/input.json"), b"{}").unwrap();
    std::fs::write(real.join("other/secret.json"), b"{}").unwrap();
    std::fs::write(real.join("storage/input.json"), b"{}").unwrap();
    std::fs::write(tmp.path().join("secret.json"), b"{}").unwrap();

    let root = alias.join("root");
    let other = alias.join("other");
    let output = alias.join("output");
    let storage = alias.join("storage");
    let policy = RootPolicy::new(
        &[root.clone(), other.clone()],
        Some(&output),
        false,
        std::slice::from_ref(&storage),
    )
    .unwrap();
    let input = root.join("input.json");
    assert_eq!(policy.read(&input).unwrap(), input);
    assert!(policy.read(std::path::Path::new("input.json")).is_ok());
    let generated = policy.write(&output.join("report/result.json")).unwrap();
    assert_eq!(
        generated,
        saccade_core::paths::canonicalize(&real)
            .unwrap()
            .join("output/report/result.json")
    );
    std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
    std::fs::write(&generated, b"{}").unwrap();
    assert!(policy.read(&output.join("report/result.json")).is_ok());

    symlink(&storage, root.join("target")).unwrap();
    assert!(policy.read(&root.join("target/input.json")).is_ok());
    assert!(policy.read(&storage.join("input.json")).is_err());
    symlink(&other, root.join("cross-root")).unwrap();
    assert!(policy.read(&root.join("cross-root/secret.json")).is_err());
    symlink(tmp.path(), root.join("escape")).unwrap();
    assert!(policy.read(&root.join("escape/secret.json")).is_err());
    assert!(policy.read(&root.join("../storage/input.json")).is_err());
    // Lexically under root, but `..` is evaluated after following the symlink.
    symlink(real.join("other/deep"), root.join("jump")).unwrap();
    assert!(policy.read(&root.join("jump/../secret.json")).is_err());

    symlink(&root, output.join("capture")).unwrap();
    symlink(&storage, output.join("target")).unwrap();
    symlink(tmp.path(), output.join("escape")).unwrap();
    symlink(&output, root.join("generated")).unwrap();
    for denied in [
        root.join("new.json"),
        root.join("generated/new.json"),
        output.join("capture/new.json"),
        output.join("target/new.json"),
        output.join("escape/new.json"),
        output.join("../new.json"),
    ] {
        assert!(policy.write(&denied).is_err(), "{}", denied.display());
        assert!(!denied.exists());
    }
    assert!(
        RootPolicy::new(
            std::slice::from_ref(&root),
            Some(&root.join("out")),
            false,
            &[]
        )
        .is_err()
    );
}
