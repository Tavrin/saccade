//! Offline C2PA validation. Unsigned metadata and failed credentials never assert generation.
use serde_json::{Value, json};

/// Read and validate embedded credentials without remote manifests or OCSP fetching.
/// Signer assertions are declarations; even a trusted signature does not establish visual truth.
pub fn inspect(bytes: &[u8]) -> crate::Result<Value> {
    if bytes.len() as u64 > super::input::MAX_BYTES {
        return Err(crate::Error::Config("C2PA input exceeds byte limit".into()));
    }
    #[cfg(not(feature = "credentials"))]
    {
        Ok(
            json!({"status":"feature_unavailable","required_feature":"credentials","ai_generation":"unknown"}),
        )
    }
    #[cfg(feature = "credentials")]
    {
        std::panic::catch_unwind(|| enabled(bytes))
            .map_err(|_| crate::Error::Config("C2PA reader failed".into()))?
    }
}
#[cfg(feature = "credentials")]
fn enabled(bytes: &[u8]) -> crate::Result<Value> {
    let context = c2pa::Context::new().with_settings(r#"{"verify":{"verify_after_reading":true,"ocsp_fetch":false,"remote_manifest_fetch":false}}"#)
        .map_err(|e| crate::Error::Config(format!("C2PA settings: {e}")))?;
    let format = if bytes.starts_with(&[0xff, 0xd8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else {
        return Ok(json!({"status":"unsupported_container","ai_generation":"unknown"}));
    };
    let reader = match c2pa::Reader::from_context(context)
        .with_stream(format, std::io::Cursor::new(bytes))
    {
        Ok(reader) => reader,
        Err(c2pa::Error::JumbfNotFound) => {
            return Ok(json!({"status":"absent","ai_generation":"unknown","network":"disabled"}));
        }
        Err(c2pa::Error::RemoteManifestUrl(_)) => {
            return Ok(
                json!({"status":"remote_manifest_unavailable","ai_generation":"unknown","network":"disabled"}),
            );
        }
        Err(error) => {
            return Ok(
                json!({"status":"invalid_or_unreadable","error":error.to_string(),"ai_generation":"unknown","network":"disabled"}),
            );
        }
    };
    if reader.manifests().len() > 128 {
        return Err(crate::Error::Config("C2PA manifest count limit".into()));
    }
    let state = reader.validation_state();
    let valid = matches!(
        state,
        c2pa::ValidationState::Valid | c2pa::ValidationState::Trusted
    );
    let active = reader.active_manifest();
    let mut actions = Vec::new();
    let mut ai = Vec::new();
    if let Some(manifest) = active {
        if manifest.assertions().len() > 256 || manifest.ingredients().len() > 128 {
            return Err(crate::Error::Config(
                "C2PA assertion/ingredient count limit".into(),
            ));
        }
        for assertion in manifest
            .assertions()
            .iter()
            .filter(|a| a.label().starts_with("c2pa.actions"))
        {
            // Typed decoding supports CBOR/JSON assertions. Never dump arbitrary metadata/GPS.
            let value: Value = manifest
                .find_assertion_with_instance(assertion.label(), assertion.instance())
                .map_err(|e| crate::Error::Config(format!("C2PA actions: {e}")))?;
            if let Some(list) = value["actions"].as_array() {
                if list.len() > 256 || actions.len() + list.len() > 256 {
                    return Err(crate::Error::Config("C2PA action count limit".into()));
                }
                for action in list {
                    if action
                        .get("action")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s.len() > 4096)
                    {
                        return Err(crate::Error::Config("C2PA action text limit".into()));
                    }
                    let source = action
                        .get("digitalSourceType")
                        .or_else(|| value.get("digitalSourceType"));
                    if source
                        .and_then(Value::as_str)
                        .is_some_and(|s| s.len() > 4096)
                    {
                        return Err(crate::Error::Config("C2PA source text limit".into()));
                    }
                    let source = source.and_then(Value::as_str);
                    if source.is_some_and(|s| s == "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia" || s == "http://c2pa.org/digitalsourcetype/trainedAlgorithmicData") {
                        ai.push(json!({"assertion":assertion.label(),"digital_source_type":source,"signed_integrity_valid":valid}));
                    }
                    actions.push(json!({"action":action.get("action").and_then(Value::as_str),"digital_source_type":source,"signed_integrity_valid":valid}));
                }
            }
        }
    }
    let ingredients: Vec<_> = active.map(|m|m.ingredients()).unwrap_or(&[]).iter().map(|i| {
        // Select provenance fields; ingredient thumbnails/metadata/location are excluded.
        let value = serde_json::to_value(i)?;
        if ["title","format","relationship","instance_id","active_manifest"].iter().any(|k|value.get(k).and_then(Value::as_str).is_some_and(|s|s.len()>4096)) {return Err(crate::Error::Config("C2PA ingredient text limit".into()));}
        Ok(json!({"title":value.get("title"),"format":value.get("format"),"relationship":value.get("relationship"),"instance_id":value.get("instance_id"),"active_manifest":value.get("active_manifest")}))
    }).collect::<crate::Result<Vec<_>>>()?;
    let statuses: Vec<_> = reader
        .validation_status()
        .unwrap_or(&[])
        .iter()
        .take(256)
        .map(|s| json!({"code":s.code()}))
        .collect();
    let value = json!({"status":"read_and_validated","validation_state":state,"signed_integrity_valid":valid,"active_manifest":reader.active_label(),"manifest_count":reader.manifests().len(),"signer":active.and_then(|m|m.signature_info()),"claim_generator":active.and_then(|m|m.claim_generator()),"actions":actions,"ingredients":ingredients,"validation_status":statuses,"ai_assertions":ai,"ai_generation":if valid&&!ai.is_empty(){"declared_in_validated_signed_credentials"}else{"unknown"},"network":"disabled; no HTTP backend, remote manifests or OCSP fetch","can_show":"embedded claim integrity, signer identity and explicitly recorded actions/ingredients","cannot_show":"credential absence/failure proves no authenticity claim; signatures do not establish depicted truth; certificate trust and content truth differ"});
    if serde_json::to_vec(&value)?.len() > 2 * 1024 * 1024 {
        return Err(crate::Error::Config("C2PA projection byte limit".into()));
    }
    Ok(value)
}
#[cfg(all(test, feature = "credentials"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn unsigned_and_malformed_never_assert_generation() {
        let image = image::RgbImage::from_pixel(16, 16, image::Rgb([100; 3]));
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode_image(&image)
            .unwrap();
        let result = inspect(&bytes).expect("inspect");
        assert_eq!(result["status"], "absent");
        assert_eq!(result["ai_generation"], "unknown");
        let bad = inspect(b"\xff\xd8\xff\xeb\x00\x08JPbroken").expect("invalid");
        assert_eq!(bad["ai_generation"], "unknown");
        assert_ne!(bad["signed_integrity_valid"], true);
    }
    #[test]
    #[ignore = "heavy: credentials"]
    fn pinned_signed_asset_validates_and_tampering_is_rejected() {
        let path =
            std::env::var_os("SACCADE_W6_C2PA_ASSET").expect("generated signed credential fixture");
        let bytes =
            super::super::input::bytes(std::path::Path::new(&path), super::super::input::MAX_BYTES)
                .unwrap();
        assert_eq!(
            crate::localized::digest(&bytes),
            std::env::var("SACCADE_W6_C2PA_SHA256").expect("fixture pin")
        );
        let result = inspect(&bytes).unwrap();
        assert_eq!(result["signed_integrity_valid"], true, "{result}");
        assert!(result["signer"].is_object());
        // Re-encoding preserves decoded pixels but removes embedded credentials.
        let image = super::super::input::decode(&bytes).unwrap();
        let mut altered = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut altered)
            .encode_image(&image)
            .unwrap();
        assert_ne!(inspect(&altered).unwrap()["signed_integrity_valid"], true);
        assert!(
            bytes.starts_with(&[0xff, 0xd8]),
            "signed fixture must be generated JPEG for claim transplant test"
        );
        let mut claims = Vec::new();
        let mut at = 2;
        while at + 4 <= bytes.len() && bytes[at] == 0xff {
            let marker = bytes[at + 1];
            if marker == 0xda || marker == 0xd9 {
                break;
            }
            let len = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
            assert!(len >= 2 && at + 2 + len <= bytes.len());
            if marker == 0xeb {
                claims.extend_from_slice(&bytes[at..at + 2 + len]);
            }
            at += 2 + len;
        }
        assert!(!claims.is_empty(), "embedded C2PA APP11 needed");
        let mut transplant = vec![0xff, 0xd8];
        transplant.extend_from_slice(&claims);
        transplant.extend_from_slice(&altered[2..]);
        let rejected = inspect(&transplant).unwrap();
        assert_ne!(
            rejected["signed_integrity_valid"], true,
            "asset binding must reject unchanged credential over new encoded pixels: {rejected}"
        );
        assert_eq!(rejected["ai_generation"], "unknown");
    }
}
