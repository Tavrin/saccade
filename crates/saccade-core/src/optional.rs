//! Optional dependency discovery and actionable preflight, without execution/downloads.
use std::path::{Path, PathBuf};
/// Find an executable on the current PATH without running it.
pub fn executable(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for root in std::env::split_paths(&path) {
        let candidate = root.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        if !candidate.is_file() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(&candidate).is_ok_and(|m| m.permissions().mode() & 0o111 == 0) {
                continue;
            }
        }
        return Some(candidate);
    }
    None
}
/// Platform-specific command to provision optional user-installed decoder binaries.
pub fn decoder_fix() -> &'static str {
    if cfg!(target_os = "macos") {
        "brew install ffmpeg dav1d"
    } else if cfg!(windows) {
        "winget install Gyan.FFmpeg"
    } else {
        "sudo apt-get install ffmpeg dav1d-tools"
    }
}
/// Refuse missing optional decoders before reading/decoding inputs or creating output.
pub fn require_binaries(names: &[&str]) -> crate::Result<()> {
    let missing: Vec<_> = names
        .iter()
        .filter(|name| executable(name).is_none())
        .copied()
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(crate::Error::Config(format!(
            "optional binaries missing: {}; fix: {}; or supply pre-extracted frame images",
            missing.join(", "),
            decoder_fix()
        )))
    }
}
/// Doctor's optional dependency inventory; presence never claims ABI or model parity.
pub fn status() -> serde_json::Value {
    let binaries:Vec<_>=["ffmpeg","ffprobe","dav1d"].iter().map(|name|{let path=executable(name);serde_json::json!({"name":name,"status":if path.is_some(){"present"}else{"missing"},"path":path,"fix_command":decoder_fix()})}).collect();
    let cache = crate::media::default_model_dir();
    let explicit = std::env::var_os("ORT_DYLIB_PATH").map(PathBuf::from);
    #[cfg(feature = "local-models")]
    let library = crate::wave7::runtime_install::resolve(explicit.as_deref(), &cache)
        .ok()
        .filter(|p| p.is_file());
    #[cfg(not(feature = "local-models"))]
    let library = explicit.filter(|p| p.is_file());
    let mut models = match crate::wave7::models::Registry::pinned_wave7() {
        Ok(r) => {
            let status = r.status(&cache);
            status["models"].as_array().map(|items|items.iter().map(|m|serde_json::json!({"name":m["model"]["id"],"status":if m["status"]=="cached_verified"{"present"}else{"missing"},"fix_command":format!("saccade models pull {}",m["model"]["id"].as_str().unwrap_or("MODEL_ID")),"source_parity":m["source_parity"]})).collect::<Vec<_>>()).unwrap_or_default()
        }
        Err(_) => Vec::new(),
    };
    if let Ok(c) = crate::general::ocr::default_contract() {
        let present = [&c.detection, &c.recognition, &c.dictionary]
            .iter()
            .all(|a| crate::semantic::artifact_path(&cache, a).is_ok_and(|p| p.is_file()));
        models.push(serde_json::json!({"name":"PP-OCRv5 Latin","status":if present{"present"}else{"missing"},"fix_command":"saccade models pull runtime; saccade text A B --download-model","limit":"artifact file presence; loading verifies pinned sizes and hashes"}));
    }
    serde_json::json!({"binaries":binaries,"onnx_runtime":{"status":if library.is_some(){"present"}else{"missing"},"library":library,"fix_command":"saccade models pull runtime; set ORT_DYLIB_PATH to the reported library","limit":"file/cache presence; ABI/load compatibility checked by model preflight"},"models":models,"model_discovery_command":"saccade models list --json","limits":["No binary executed, runtime loaded, model downloaded or provider contacted by doctor.","dav1d is listed for external follow-up tooling; current video commands use ffmpeg/ffprobe."]})
}
/// Validate an explicitly declared ONNX runtime path before image work.
pub fn require_library(path: &Path) -> crate::Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(crate::Error::Config("optional ONNX Runtime library missing; fix: saccade models pull runtime; set ORT_DYLIB_PATH or --runtime-library to its library".into()))
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn missing_runtime_and_binary_fail_with_fixes() {
        let tmp = tempfile::tempdir().unwrap();
        let e = super::require_library(&tmp.path().join("missing"))
            .unwrap_err()
            .to_string();
        assert!(e.contains("models pull runtime"));
        assert!(
            super::require_binaries(&["saccade-nonexistent-optional-decoder"])
                .unwrap_err()
                .to_string()
                .contains(super::decoder_fix())
        );
        let status = super::status();
        assert!(
            status["binaries"]
                .as_array()
                .unwrap()
                .iter()
                .all(|b| b["fix_command"].is_string())
        );
    }
    #[test]
    #[cfg(not(feature = "local-models"))]
    fn strict_media_dependency_error_precedes_missing_input() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer =
            crate::media::Analyzer::new(crate::media::Profile::CpuLite, tmp.path().into(), false)
                .unwrap();
        let options = crate::media::Options {
            strict: true,
            faces: Some(true),
            ..Default::default()
        };
        let e = analyzer
            .analyze_media("nonexistent-media.png", &options)
            .unwrap_err();
        assert_eq!(e.code, "media_section_failed");
        assert!(e.message.contains("fix:"));
    }
}
