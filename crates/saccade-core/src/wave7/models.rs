//! Pinned local-model registry and explicit content-addressed installation.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
};

/// Registry contract identifier.
pub const REGISTRY_SCHEMA: &str = "saccade-model-registry.v1";
/// List/status contract identifier.
pub const MODELS_SCHEMA: &str = "saccade-model-status.v1";
/// Largest individual model artifact (16 GiB; large VLMs are external).
pub const MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024 * 1024;
/// Typed failures for standalone vision commands.
#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    /// Dynamic library ABI is incompatible with the compiled ort consumer.
    #[error("ONNX Runtime {required} required: {detail}")]
    RuntimeIncompatible {
        /// Required runtime version line.
        required: String,
        /// Observed ABI/load failure.
        detail: String,
    },
    /// Invalid contract, geometry or input.
    #[error("invalid vision input: {0}")]
    Invalid(String),
    /// Runtime/export/model is unavailable, never a negative observation.
    #[error("vision capability unavailable: {0}")]
    Unavailable(String),
    /// Pinned bytes failed validation.
    #[error("model integrity failure: {0}")]
    Integrity(String),
    /// Bounded IO failed.
    #[error("vision IO: {0}")]
    Io(#[from] std::io::Error),
    /// Structured data failed decoding.
    #[error("vision JSON: {0}")]
    Json(#[from] serde_json::Error),
}
/// Standalone result type.
pub type Result<T> = std::result::Result<T, VisionError>;
/// Read at most a declared limit, including for non-regular files.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut data = Vec::new();
    file.take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(VisionError::Invalid("input exceeds byte limit".into()));
    }
    Ok(data)
}
/// Lower-case SHA-256 of exact bytes.
pub fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
/// Checks a canonical SHA-256 digest.
pub fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Reviewed model family, deliberately distinct from pinned executable artifacts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct Selection {
    /// Stable selection id.
    pub id: &'static str,
    /// Task, not a correctness qualification.
    pub task: &'static str,
    /// Code licence from supplied research.
    pub code_license: &'static str,
    /// Weight licence from supplied research.
    pub weights_license: &'static str,
    /// Source of checkpoint/export provenance.
    pub source_url: &'static str,
    /// Approximate research size, not a download size/pin.
    pub estimated_bytes: Option<u64>,
    /// Selected runtime route.
    pub runtime: &'static str,
}
/// Selections from WAVE7-MODEL-RESEARCH-2026-10-05.md; no fabricated pins.
pub fn selections() -> Vec<Selection> {
    [
        (
            "grounding-dino-tiny",
            "open_vocabulary_detection",
            "Apache-2.0",
            "Apache-2.0",
            "https://huggingface.co/IDEA-Research/grounding-dino-tiny",
            Some(690_000_000),
            "onnx",
        ),
        (
            "owlv2-base",
            "open_vocabulary_detection",
            "Apache-2.0",
            "Apache-2.0",
            "https://huggingface.co/google/owlv2-base-patch16-ensemble",
            Some(600_000_000),
            "onnx",
        ),
        (
            "sam-2.1-tiny",
            "prompted_segmentation",
            "Apache-2.0",
            "Apache-2.0",
            "https://github.com/facebookresearch/sam2",
            Some(156_000_000),
            "onnx",
        ),
        (
            "efficientsam-ti",
            "prompted_segmentation",
            "Apache-2.0",
            "Apache-2.0",
            "https://github.com/yformer/EfficientSAM",
            Some(40_000_000),
            "onnx",
        ),
        (
            "florence-2-base-ft",
            "bounded_extraction",
            "MIT",
            "MIT",
            "https://huggingface.co/onnx-community/Florence-2-base-ft",
            Some(460_000_000),
            "external_http",
        ),
        (
            "qwen3.5-4b",
            "local_reasoning",
            "Apache-2.0",
            "Apache-2.0",
            "https://huggingface.co/Qwen/Qwen3.5-4B",
            Some(8_000_000_000),
            "external_http",
        ),
        (
            "lpips-alex-v0.1",
            "full_reference_quality",
            "BSD-2-Clause",
            "BSD-2-Clause",
            "https://github.com/richzhang/PerceptualSimilarity",
            Some(9_000_000),
            "onnx",
        ),
        (
            "dists",
            "full_reference_quality",
            "MIT",
            "MIT",
            "https://github.com/dingkeyan93/DISTS",
            None,
            "onnx",
        ),
        (
            "musiq-technical",
            "no_reference_quality",
            "Apache-2.0",
            "Apache-2.0",
            "https://github.com/google-research/google-research/tree/master/musiq",
            Some(108_000_000),
            "onnx",
        ),
        (
            "trustmark",
            "watermark_decode",
            "MIT",
            "MIT",
            "https://github.com/adobe/trustmark",
            None,
            "onnx",
        ),
        (
            "yunet-2026may",
            "face_detection",
            "MIT",
            "MIT",
            "https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet",
            Some(230_000),
            "onnx",
        ),
        (
            "ultraface-rfb",
            "face_detection",
            "MIT",
            "MIT",
            "https://github.com/Linzaer/Ultra-Light-Fast-Generic-Face-Detector-1MB",
            Some(1_100_000),
            "onnx",
        ),
    ]
    .into_iter()
    .map(
        |(id, task, code_license, weights_license, source_url, estimated_bytes, runtime)| {
            Selection {
                id,
                task,
                code_license,
                weights_license,
                source_url,
                estimated_bytes,
                runtime,
            }
        },
    )
    .collect()
}
/// One separately pinned graph, tokenizer, backbone, calibration or parity receipt.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Unique role within the model.
    pub role: String,
    /// Immutable export/checkpoint revision.
    pub revision: String,
    /// HTTPS source pinned to the recorded revision.
    pub url: String,
    /// Exact encoded bytes.
    pub bytes: u64,
    /// Exact SHA-256, never the checkpoint's hash in place of an export hash.
    pub sha256: String,
    /// Host-reported digest or locally computed date, retained as provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_provenance: Option<String>,
    /// Licence of this individual artifact (e.g. backbone independent of LPIPS).
    pub license: String,
}
/// Explicit tensor/preprocessing contract for a qualified export adapter.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputContract {
    /// Adapter id: normalized-vision-v1, scalar-pair-v1, scalar-image-v1, yunet-v1.
    pub adapter: String,
    /// Exact processor/tokenizer/resize description and revision.
    pub preprocessing: String,
    /// NCHW input size; zero means native resolution for scalar metrics only.
    pub resolution: [u32; 2],
    /// RGB or BGR channel order.
    pub color: String,
    /// Applied as (channel * scale - mean) / std.
    pub scale: f32,
    /// Per-channel means.
    pub mean: [f32; 3],
    /// Positive per-channel standard deviations.
    pub std: [f32; 3],
    /// Named graph input for pixels.
    pub image_input: String,
    /// Named graph second image input for full-reference metrics.
    pub reference_input: Option<String>,
    /// Named graph output.
    pub output: String,
}
/// Executable model entry. Every artifact must have a reviewed exact pin.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Unique stable id.
    pub id: String,
    /// Explicit task.
    pub task: String,
    /// Checkpoint/metric version.
    pub version: String,
    /// Code licence.
    pub code_license: String,
    /// Weight licence; component licences additionally live on artifacts.
    pub weights_license: String,
    /// Original checkpoint source URL.
    pub source_url: String,
    /// onnx (CPU FP32, dynamic runtime) or external_http.
    pub runtime: String,
    /// Exact export input contract.
    pub input: InputContract,
    /// Graph and all auxiliary artifacts, no external data omitted.
    pub artifacts: Vec<Artifact>,
    /// Source/export parity receipt digest, absent means unqualified.
    pub parity_sha256: Option<String>,
}
/// User-owned registry, never an ambient model download instruction.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// REGISTRY_SCHEMA.
    pub schema: String,
    /// Pinned executable models.
    pub models: Vec<Model>,
    /// Typed wave 6 embedding/OCR contracts sharing this registry's pins.
    #[serde(default)]
    pub contracts: std::collections::BTreeMap<String, serde_json::Value>,
}
fn license_ok(s: &str) -> bool {
    matches!(
        s,
        "MIT" | "Apache-2.0" | "BSD-2-Clause" | "BSD-3-Clause" | "ISC" | "Zlib" | "MPL-2.0"
    )
}
fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}
impl Registry {
    /// Lane's reviewed artifact manifest. Source/export parity remains absent.
    pub fn pinned_wave7() -> Result<Self> {
        let r: Self = serde_json::from_str(include_str!("../../assets/wave7-models.json"))?;
        r.validate()?;
        Ok(r)
    }
    /// Empty registry; research selections are separately listed unavailable.
    pub fn empty() -> Self {
        Self {
            schema: REGISTRY_SCHEMA.into(),
            models: Vec::new(),
            contracts: Default::default(),
        }
    }
    /// Load bounded JSON and reject incomplete/malformed pins.
    pub fn load(path: &Path) -> Result<Self> {
        let r: Self = serde_json::from_slice(&read_bounded(path, 2 * 1024 * 1024)?)?;
        r.validate()?;
        Ok(r)
    }
    /// Validate all licences, identities and resource bounds without IO.
    pub fn validate(&self) -> Result<()> {
        for (id, value) in &self.contracts {
            if !safe_id(id) {
                return Err(VisionError::Invalid("contract id".into()));
            }
            match value["schema"].as_str() {
                Some(crate::general::embedding::MODEL_SCHEMA) => {
                    let m: crate::general::embedding::Model =
                        serde_json::from_value(value.clone())?;
                    crate::general::embedding::validate(&m)
                        .map_err(|e| VisionError::Invalid(e.to_string()))?;
                }
                Some(crate::general::ocr::SCHEMA) => {
                    let c: crate::general::ocr::Contract = serde_json::from_value(value.clone())?;
                    crate::general::ocr::validate(&c)
                        .map_err(|e| VisionError::Invalid(e.to_string()))?;
                }
                Some("saccade-tesseract.v1") => {
                    let _: crate::ui_review::OcrContract = serde_json::from_value(value.clone())?;
                }
                _ => {
                    return Err(VisionError::Invalid(
                        "unknown registry contract schema".into(),
                    ));
                }
            }
        }
        let mut ids = BTreeSet::new();
        if self.schema != REGISTRY_SCHEMA || self.models.len() > 64 {
            return Err(VisionError::Invalid("registry schema/count".into()));
        }
        for m in &self.models {
            let i = &m.input;
            if !safe_id(&m.id)
                || !ids.insert(&m.id)
                || m.task.is_empty()
                || m.version.is_empty()
                || !license_ok(&m.code_license)
                || !license_ok(&m.weights_license)
                || !m.source_url.starts_with("https://")
                || !matches!(m.runtime.as_str(), "onnx" | "external_http")
                || m.artifacts.is_empty()
                || m.artifacts.len() > 32
                || i.preprocessing.is_empty()
                || i.image_input.is_empty()
                || i.output.is_empty()
                || !matches!(
                    i.adapter.as_str(),
                    "normalized-vision-v1"
                        | "scalar-pair-v1"
                        | "scalar-image-v1"
                        | "yunet-v1"
                        | "ultraface-v1"
                        | "external-observation-v1"
                        | "grounding-dino-v1"
                        | "owlv2-v1"
                        | "efficientsam-v1"
                        | "trustmark-q-v1"
                )
                || !matches!(i.color.as_str(), "RGB" | "BGR")
                || !i.scale.is_finite()
                || i.mean.iter().any(|x| !x.is_finite())
                || i.std.iter().any(|x| !x.is_finite() || *x <= 0.)
                || i.resolution.iter().any(|x| *x > 4096)
                || m.parity_sha256.as_ref().is_some_and(|s| !valid_hash(s))
            {
                return Err(VisionError::Invalid(format!(
                    "invalid model contract: {}",
                    m.id
                )));
            }
            if m.parity_sha256.as_ref().is_some_and(|hash| {
                !m.artifacts
                    .iter()
                    .any(|a| a.role == "parity" && &a.sha256 == hash)
            }) {
                return Err(VisionError::Invalid(
                    "parity digest requires a separately pinned parity artifact".into(),
                ));
            }
            let mut roles = BTreeSet::new();
            for a in &m.artifacts {
                if !safe_id(&a.role)
                    || !roles.insert(&a.role)
                    || a.revision.is_empty()
                    || !a.url.starts_with("https://")
                    || a.bytes == 0
                    || a.bytes > MAX_ARTIFACT_BYTES
                    || !valid_hash(&a.sha256)
                    || !license_ok(&a.license)
                {
                    return Err(VisionError::Invalid(format!("invalid artifact: {}", m.id)));
                }
            }
        }
        Ok(())
    }
    /// Resolve a model, failing explicitly when pins have not been supplied.
    pub fn model(&self, id: &str) -> Result<&Model> {
        self.models.iter().find(|m| m.id == id).ok_or_else(|| {
            VisionError::Unavailable(format!(
                "{id}: reviewed artifact pins/export contract required"
            ))
        })
    }
    /// List pin/cache/parity status without loading a runtime or downloading.
    pub fn status(&self, cache: &Path) -> serde_json::Value {
        let models:Vec<_>=self.models.iter().map(|m| {
            let state=if m.artifacts.iter().all(|a| verify(cache,a).is_ok()) {"cached_verified"} else {"missing_or_corrupt"};
            serde_json::json!({"model":m,"status":state,"source_parity":m.parity_sha256.is_some()})
        }).collect();
        let candidates:Vec<_>=selections().into_iter().filter(|s| !self.models.iter().any(|m| m.id==s.id)).map(|s| serde_json::json!({"selection":s,"status":"unavailable","reason":deferred_reason(s.id)})).collect();
        serde_json::json!({"schema":MODELS_SCHEMA,"models":models,"contracts":self.contracts,"unavailable_selections":candidates})
    }
}
fn deferred_reason(id: &str) -> String {
    serde_json::from_str::<serde_json::Value>(include_str!("../../assets/wave7-disposition.json"))
        .ok()
        .and_then(|v| v["deferred"][id].as_str().map(str::to_owned))
        .unwrap_or_else(|| "exact export pins and input contract not supplied by research".into())
}
/// Content-addressed artifact path, with no caller-controlled path component.
pub fn artifact_path(cache: &Path, a: &Artifact) -> Result<PathBuf> {
    if !valid_hash(&a.sha256) {
        return Err(VisionError::Invalid("artifact digest".into()));
    }
    Ok(cache.join(&a.sha256))
}
/// Verify exact artifact bytes every time they are consumed.
pub fn verify(cache: &Path, a: &Artifact) -> Result<PathBuf> {
    let path = artifact_path(cache, a)?;
    let meta = std::fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.len() != a.bytes {
        return Err(VisionError::Integrity("artifact size/type".into()));
    }
    let mut f = std::fs::File::open(&path)?.take(a.bytes + 1);
    let mut hash = Sha256::new();
    let mut b = [0; 65536];
    let mut nbytes = 0;
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        nbytes += n as u64;
        hash.update(&b[..n]);
    }
    if nbytes != a.bytes || format!("{:x}", hash.finalize()) != a.sha256 {
        return Err(VisionError::Integrity("artifact hash".into()));
    }
    Ok(path)
}
/// Atomically install from a bounded reader. Used by explicit pull and fixture tests.
pub fn install(cache: &Path, a: &Artifact, reader: impl Read) -> Result<PathBuf> {
    use std::io::Write;
    if a.bytes == 0 || a.bytes > MAX_ARTIFACT_BYTES || !valid_hash(&a.sha256) {
        return Err(VisionError::Invalid("artifact pin".into()));
    }
    std::fs::create_dir_all(cache)?;
    let mut tmp = tempfile::NamedTempFile::new_in(cache)?;
    let n = std::io::copy(&mut reader.take(a.bytes + 1), &mut tmp)?;
    if n != a.bytes {
        return Err(VisionError::Integrity("download size".into()));
    }
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    let mut f = std::fs::File::open(tmp.path())?;
    let mut hash = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        hash.update(&b[..n]);
    }
    if format!("{:x}", hash.finalize()) != a.sha256 {
        return Err(VisionError::Integrity("download hash".into()));
    }
    let path = artifact_path(cache, a)?;
    match tmp.persist_noclobber(&path) {
        Ok(_) => {}
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            verify(cache, a)?;
        }
        Err(e) => return Err(e.error.into()),
    }
    verify(cache, a)
}
/// Resolve cached artifacts, downloading only under explicit opt-in.
pub fn ensure(model: &Model, cache: &Path, allow_download: bool) -> Result<Vec<PathBuf>> {
    #[cfg(feature = "local-models")]
    {
        use fs2::FileExt;
        if allow_download {
            std::fs::create_dir_all(cache)?;
        }
        let _lock = if allow_download {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(cache.join("wave7.lock"))?;
            f.lock_exclusive()?;
            Some(f)
        } else {
            None
        };
        model
            .artifacts
            .iter()
            .map(|a| {
                let path = artifact_path(cache, a)?;
                if path.exists() {
                    return verify(cache, a);
                }
                if !allow_download {
                    return Err(VisionError::Unavailable(
                        "model missing; use models pull or --allow-download".into(),
                    ));
                }
                let agent = ureq::Agent::config_builder()
                    .timeout_global(Some(std::time::Duration::from_secs(300)))
                    .build()
                    .new_agent();
                let mut r = agent
                    .get(&a.url)
                    .call()
                    .map_err(|_| VisionError::Unavailable("model download failed".into()))?;
                install(cache, a, r.body_mut().as_reader())
            })
            .collect()
    }
    #[cfg(not(feature = "local-models"))]
    {
        if allow_download {
            return Err(VisionError::Unavailable(
                "compile local-models to download".into(),
            ));
        }
        model.artifacts.iter().map(|a| verify(cache, a)).collect()
    }
}

/// Project one typed wave 6 contract from the shared registry, rejecting ambiguity.
pub fn contract(bytes: &[u8], schema: &str) -> Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if value["schema"] != REGISTRY_SCHEMA {
        return Ok(value);
    }
    let r: Registry = serde_json::from_value(value)?;
    r.validate()?;
    let matches: Vec<_> = r
        .contracts
        .values()
        .filter(|v| v["schema"] == schema)
        .collect();
    if matches.len() != 1 {
        return Err(VisionError::Invalid(
            "registry needs exactly one matching contract".into(),
        ));
    }
    Ok(matches[0].clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn artifact(data: &[u8]) -> Artifact {
        Artifact {
            role: "graph".into(),
            revision: "fixture-v1".into(),
            url: "https://example.org/fixture-v1/model.onnx".into(),
            bytes: data.len() as u64,
            sha256: digest(data),
            hash_provenance: None,
            license: "MIT".into(),
        }
    }
    #[test]
    fn registry_requires_real_pins_and_component_licences() {
        let mut v = serde_json::json!({"schema":REGISTRY_SCHEMA,"models":[{"id":"generated-fixture","task":"face_detection","version":"generated-v1","code_license":"MIT","weights_license":"MIT","source_url":"https://example.org/generated-v1/model","runtime":"onnx","input":{"adapter":"yunet-v1","preprocessing":"generated-v1","resolution":[32,32],"color":"BGR","scale":1.,"mean":[0.,0.,0.],"std":[1.,1.,1.],"image_input":"input","reference_input":null,"output":"output"},"artifacts":[{"role":"graph","revision":"generated-v1","url":"https://example.org/generated-v1/model.onnx","bytes":5,"sha256":digest(b"graph"),"license":"MIT"}],"parity_sha256":null}]});
        let r: Registry = serde_json::from_value(v.clone()).unwrap();
        r.validate().unwrap();
        let cache = tempfile::tempdir().unwrap();
        assert!(ensure(&r.models[0], cache.path(), false).is_err());
        assert_eq!(std::fs::read_dir(cache.path()).unwrap().count(), 0);
        v["models"][0]["artifacts"][0]["sha256"] = serde_json::json!("unpinned");
        assert!(
            serde_json::from_value::<Registry>(v.clone())
                .unwrap()
                .validate()
                .is_err()
        );
        v["models"][0]["artifacts"][0]["sha256"] = serde_json::json!(digest(b"graph"));
        v["models"][0]["weights_license"] = serde_json::json!("non-commercial");
        assert!(
            serde_json::from_value::<Registry>(v)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    #[test]
    fn pin_rejects_changed_bytes_and_oversized_stream() {
        let c = tempfile::tempdir().unwrap();
        let a = artifact(b"tiny graph");
        assert!(install(c.path(), &a, &b"tiny graph!"[..]).is_err());
        assert!(install(c.path(), &a, &b"wrong data"[..]).is_err());
        let p = install(c.path(), &a, &b"tiny graph"[..]).unwrap();
        std::fs::write(p, b"corruption").unwrap();
        assert!(verify(c.path(), &a).is_err());
    }
    #[test]
    fn install_is_idempotent_and_content_addressed() {
        let c = tempfile::tempdir().unwrap();
        let a = artifact(b"graph");
        let p = install(c.path(), &a, &b"graph"[..]).unwrap();
        assert_eq!(install(c.path(), &a, &b"graph"[..]).unwrap(), p);
        assert_eq!(p.file_name().unwrap(), a.sha256.as_str());
    }
    #[test]
    fn research_is_not_a_fake_executable_pin() {
        let r = Registry::empty();
        r.validate().unwrap();
        assert!(r.model("yunet-2026may").is_err());
        assert_eq!(selections().len(), 12);
        assert_eq!(
            r.status(Path::new("absent"))["models"],
            serde_json::json!([])
        );
    }
    #[test]
    fn shipped_pins_keep_aux_hash_origin_and_no_invented_parity() {
        let r = Registry::pinned_wave7().unwrap();
        assert_eq!(r.models.len(), 6);
        assert!(r.models.iter().all(|m| m.parity_sha256.is_none()));
        let dino = r.model("grounding-dino-tiny").unwrap();
        assert_eq!(
            dino.artifacts
                .iter()
                .find(|a| a.role == "graph")
                .unwrap()
                .bytes,
            718_761_381
        );
        assert!(
            dino.artifacts
                .iter()
                .filter(|a| a.role != "graph")
                .all(|a| a.hash_provenance.as_deref() == Some("computed locally on 2026-10-05"))
        );
        for missing in [
            "sam-2.1-tiny",
            "lpips-alex-v0.1",
            "dists",
            "musiq-technical",
        ] {
            assert!(r.model(missing).is_err());
        }
    }
    #[test]
    fn bounded_reads_fail_closed() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("input");
        std::fs::write(&p, b"1234").unwrap();
        assert!(read_bounded(&p, 3).is_err());
    }
}
