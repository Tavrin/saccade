#![allow(missing_docs, clippy::unwrap_used)]
use std::process::Command;
#[test]
fn extension_feature_boundary_is_explicit() {
    let catalogue = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["capabilities", "--json"])
        .output()
        .unwrap();
    assert!(catalogue.status.success());
    let catalogue: serde_json::Value = serde_json::from_slice(&catalogue.stdout).unwrap();
    assert_eq!(
        catalogue["compiled_features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "geo"),
        cfg!(feature = "geo")
    );
    let raster = catalogue["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["family"] == "native_rasters")
        .unwrap();
    assert_eq!(
        raster["status"],
        if cfg!(feature = "geo") {
            "available_bounded"
        } else {
            "feature_unavailable"
        }
    );

    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["geo", "--help"])
        .output()
        .unwrap();
    if cfg!(feature = "geo") {
        assert!(result.status.success());
        let help = String::from_utf8(result.stdout).unwrap();
        assert!(
            help.contains("mask-metrics") && help.contains("tiles") && help.contains("compare")
        );
    } else {
        assert_eq!(result.status.code(), Some(2));
    }
}
#[cfg(feature = "geo")]
#[test]
fn raster_contracts_are_discoverable_from_compiled_binary() {
    for id in [
        "saccade-raster.v1",
        "saccade-raster-mask.v1",
        "saccade-tiles.v1",
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(["schema", "get", id])
            .output()
            .unwrap();
        assert!(result.status.success());
        let schema: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(schema["$id"], id);
    }
}
