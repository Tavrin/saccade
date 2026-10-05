//! One retained input, independently attributed sections, and shared transport contracts.
use crate::general::{assessment, credentials, hashing, input, integrity, registration};
use crate::wave7::{faces, models, observation, vision};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};
/// OpenAI-compatible and Azure fixture endpoint adapters.
#[cfg(feature = "vision-providers")]
pub mod endpoints;
#[cfg(all(
    test,
    feature = "local-models",
    feature = "embeddings",
    feature = "ocr"
))]
mod heavy_tests;
/// Loopback local media HTTP transport.
#[cfg(feature = "workbench")]
pub mod http;
pub mod saliency;
/// Source-record and image usage matching.
pub mod usage;
/// External video and directory keyframe analysis.
pub mod video;
/// Versioned media record discriminator.
pub const SCHEMA: &str = "saccade-media-record.v1";
/// Shared retained-byte FLIP comparison schema.
pub const COMPARE_SCHEMA: &str = "saccade-media-compare.v1";
/// Stable media error, shared by CLI, HTTP and Python.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct MediaError {
    /// Machine-readable stable code.
    pub code: String,
    /// Human-readable explanation (no credentials).
    pub message: String,
}
impl MediaError {
    /// Construct a transport-independent error.
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl From<crate::Error> for MediaError {
    fn from(e: crate::Error) -> Self {
        Self::new("invalid_media_input", e.to_string())
    }
}
impl From<models::VisionError> for MediaError {
    fn from(e: models::VisionError) -> Self {
        let code = match &e {
            models::VisionError::Unavailable(_) => "vision_unavailable",
            models::VisionError::Integrity(_) => "model_integrity",
            models::VisionError::RuntimeIncompatible { .. } => "runtime_incompatible",
            models::VisionError::Io(_) => "io_error",
            models::VisionError::Json(_) => "invalid_json",
            _ => "invalid_vision_input",
        };
        Self::new(code, e.to_string())
    }
}
impl From<serde_json::Error> for MediaError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("invalid_json", e.to_string())
    }
}
/// Result with stable transport errors.
pub type Result<T> = std::result::Result<T, MediaError>;
/// Named presets; overrides in Options remain authoritative.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// Deterministic analysis without large models.
    #[default]
    CpuLite,
    /// Attempt installed optional CPU models.
    CpuFull,
    /// Request GPU execution; this build explicitly refuses unsupported providers.
    Gpu,
}
/// Per-call options. Models are never downloaded implicitly.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Profile override, otherwise Analyzer's profile.
    pub profile: Option<Profile>,
    /// Abort if any attempted section failed.
    pub strict: bool,
    /// Declared output sizes; crop ratios are derived from each size.
    pub output_sizes: Vec<[u32; 2]>,
    /// Additional crop ratios/rectangles.
    pub crops: Vec<faces::CropSpec>,
    /// Override preset face inference.
    pub faces: Option<bool>,
    /// Override preset OCR inference.
    pub text: Option<bool>,
    /// Override preset embedding inference.
    pub embeddings: Option<bool>,
    /// Enable deterministic saliency; defaults to true.
    pub saliency: Option<bool>,
    /// Optional description, off by default; requires an explicitly injected provider.
    pub description: bool,
    /// Include EXIF/XMP location fields only when explicitly declared.
    pub include_gps: bool,
}
/// Section outcome; a missing model is never an empty successful observation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Completed successfully.
    Ok,
    /// Disabled or intentionally unavailable.
    Skipped,
    /// Attempt failed; data is absent.
    Failed,
}
/// Attribution on every section, including skipped sections.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provenance {
    /// Algorithm/model selection.
    pub id: String,
    /// Algorithm version or model export revision.
    pub version: String,
    /// Artifact digests, or built-in algorithm version pin.
    pub pins: Vec<String>,
}
/// Independent timed section.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Section {
    /// Section outcome.
    pub status: Status,
    /// Attributed implementation/version/pins.
    pub provenance: Provenance,
    /// Elapsed wall time in milliseconds, diagnostic only.
    pub timing_ms: f64,
    /// Successful data or null.
    pub data: Value,
    /// Reason for skipping, or typed error.
    pub reason: Option<String>,
    /// Stable code for failed sections.
    pub error_code: Option<String>,
}
/// Complete single-image record; video adds its own section and keyframe records.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    /// SCHEMA.
    pub schema: String,
    /// Effective preset.
    pub profile: Profile,
    /// SHA-256, size, format and colour-profile container evidence.
    pub identity: Section,
    /// Unsigned metadata, credit candidates and offline C2PA summary.
    pub metadata: Section,
    /// No-reference measures and output fitness.
    pub quality: Section,
    /// Face receipt, saliency, focal basis and crop checks.
    pub focal: Section,
    /// OCR boxes/text with honest confidence availability.
    pub text: Section,
    /// Perceptual and keypoint fingerprints.
    pub fingerprints: Section,
    /// Base64 little-endian float32 arrays with model identity.
    pub embeddings: Section,
    /// Optional draft observation.
    pub description: Section,
    /// Video timing/shots and keyframe records; skipped for image input.
    pub video: Section,
}
fn provenance(id: &str) -> Provenance {
    Provenance {
        id: id.into(),
        version: "1".into(),
        pins: vec![format!("saccade/{id}/1")],
    }
}
fn skipped(id: &str, reason: &str) -> Section {
    Section {
        status: Status::Skipped,
        provenance: provenance(id),
        timing_ms: 0.,
        data: Value::Null,
        reason: Some(reason.into()),
        error_code: None,
    }
}
fn section(id: &str, f: impl FnOnce() -> Result<Value>) -> Section {
    let start = Instant::now();
    let r = f();
    let timing_ms = start.elapsed().as_secs_f64() * 1000.;
    match r {
        Ok(data) => Section {
            status: Status::Ok,
            provenance: provenance(id),
            timing_ms,
            data,
            reason: None,
            error_code: None,
        },
        Err(e) => Section {
            status: Status::Failed,
            provenance: provenance(id),
            timing_ms,
            data: Value::Null,
            reason: Some(e.message),
            error_code: Some(e.code),
        },
    }
}
#[derive(Default)]
struct Sessions {
    #[cfg(feature = "local-models")]
    faces: Option<crate::wave7::runtime::OnnxModel>,
    #[cfg(feature = "ocr")]
    ocr: Option<crate::general::ocr::Engine>,
    #[cfg(feature = "embeddings")]
    embeddings: Option<crate::general::embedding::Engine>,
}
/// Thread-safe analyzer. Model sessions load once on demand under a shared compute lock.
/// Deterministic sections do not take that lock; no network work without explicit opt-in.
pub struct Analyzer {
    profile: Profile,
    model_dir: PathBuf,
    registry: models::Registry,
    allow_download: bool,
    sessions: Mutex<Sessions>,
}
impl Analyzer {
    /// Use the official pinned model registry and a caller-owned cache.
    pub fn new(profile: Profile, model_dir: PathBuf, allow_download: bool) -> Result<Self> {
        Self::with_registry(
            profile,
            model_dir,
            allow_download,
            models::Registry::pinned_wave7()?,
        )
    }
    /// Use an explicitly supplied, validated registry (including embedding/OCR contracts).
    pub fn with_registry(
        profile: Profile,
        model_dir: PathBuf,
        allow_download: bool,
        registry: models::Registry,
    ) -> Result<Self> {
        registry.validate()?;
        Ok(Self {
            profile,
            model_dir,
            registry,
            allow_download,
            sessions: Mutex::new(Sessions::default()),
        })
    }
    /// Analyze a local file or a bounded HTTP(S) URL. This does not pull models.
    pub fn analyze_media(&self, source: &str, options: &Options) -> Result<Record> {
        if !source.starts_with("http://")
            && !source.starts_with("https://")
            && video::is_video(Path::new(source))
        {
            return self.analyze_video(Path::new(source), options, &video::VideoOptions::default());
        }
        let bytes = self.read(source)?;
        self.analyze_bytes(&bytes, options)
    }
    /// Read one encoded input. Remote errors are redacted and requests time out.
    pub fn read(&self, source: &str) -> Result<Vec<u8>> {
        if source.starts_with("http://") || source.starts_with("https://") {
            #[cfg(feature = "media-http")]
            {
                use std::io::Read;
                if source.len() > 8192 || source.split('/').nth(2).is_some_and(|s| s.contains('@'))
                {
                    return Err(MediaError::new(
                        "invalid_media_input",
                        "URL credentials/length refused",
                    ));
                }
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .timeout_global(Some(std::time::Duration::from_secs(30)))
                    .build()
                    .into();
                let mut response = agent.get(source).call().map_err(|_| {
                    MediaError::new("media_fetch_failed", "HTTP request failed (URL redacted)")
                })?;
                let mut bytes = Vec::new();
                response
                    .body_mut()
                    .as_reader()
                    .take(input::MAX_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| MediaError::new("media_fetch_failed", "HTTP body read failed"))?;
                if bytes.len() as u64 > input::MAX_BYTES {
                    return Err(MediaError::new(
                        "request_too_large",
                        "encoded input exceeds 64 MiB",
                    ));
                }
                return Ok(bytes);
            }
            #[cfg(not(feature = "media-http"))]
            return Err(MediaError::new(
                "media_fetch_unavailable",
                "compile media-http for URL inputs",
            ));
        }
        Ok(input::bytes(Path::new(source), input::MAX_BYTES)?)
    }
    /// Analyze bytes retained by the caller; input is decoded and hashed from this buffer.
    pub fn analyze_bytes(&self, bytes: &[u8], options: &Options) -> Result<Record> {
        if bytes.len() as u64 > input::MAX_BYTES {
            return Err(MediaError::new(
                "request_too_large",
                "encoded input exceeds 64 MiB",
            ));
        }
        if options.output_sizes.len() + options.crops.len() > 32
            || options.output_sizes.iter().any(|s| s.contains(&0))
        {
            return Err(MediaError::new(
                "invalid_media_options",
                "declare at most 32 positive output sizes/crops",
            ));
        }
        let identity_started = Instant::now();
        let pixels = input::decode(bytes)?;
        let size = [pixels.width(), pixels.height()];
        let hash = models::digest(bytes);
        let profile = options.profile.unwrap_or(self.profile);
        let identity_elapsed = identity_started.elapsed().as_secs_f64() * 1000.;
        let headers_started = Instant::now();
        let headers = integrity::headers(bytes, &pixels, options.include_gps);
        let headers_elapsed = headers_started.elapsed().as_secs_f64() * 1000.;
        let mut identity = section("sha256-raster-container", || {
            Ok(
                json!({"sha256":hash,"dimensions":size,"format":image::guess_format(bytes).ok().map(|f|format!("{f:?}").to_lowercase()),"colour_profile":headers.as_ref().ok().map(|v|v["metadata"]["colour_profile"].clone()),"pixel_policy":"encoded orientation; SDR RGBA8; colour profile presence is not profile validation"}),
            )
        });
        identity.timing_ms += identity_elapsed;
        let mut metadata = section("exif-xmp-iptc-c2pa", || {
            let h = headers?;
            let candidates = credit_candidates(&h["metadata"]);
            Ok(
                json!({"fields":h["metadata"],"candidates":candidates,"c2pa":credentials::inspect(bytes)?,"assurance":"unsigned metadata candidates are not verified rights"}),
            )
        });
        metadata.timing_ms += headers_elapsed;
        let quality = section("assess-output-fitness", || {
            let m = assessment::assess(&pixels)?;
            let fitness:Vec<_>=options.output_sizes.iter().map(|s| {
                let scale=(size[0] as f64/s[0] as f64).min(size[1] as f64/s[1] as f64);
                json!({"output_size":s,"adequate_resolution":scale>=1.,"upscale_factor":(1./scale).max(1.),"retained_area_fraction":(s[0] as f64*s[1] as f64*scale*scale)/(size[0] as f64*size[1] as f64)})
            }).collect();
            Ok(
                json!({"measures":m,"fitness":fitness,"thresholds":"content-dependent; no heuristic quality verdict"}),
            )
        });
        let image = vision::VisionImage {
            pixels: crate::compare::flatten_over(&pixels, 255),
            sha256: hash.clone(),
        };
        let full = profile != Profile::CpuLite;
        let faces_started = Instant::now();
        let face_result = if options.faces.unwrap_or(full) {
            Some(self.faces(&image, profile))
        } else {
            None
        };
        let face_elapsed = faces_started.elapsed().as_secs_f64() * 1000.;
        let mut focal = section("faces-colour-surround-crop", || {
            let saliency = options
                .saliency
                .unwrap_or(true)
                .then(|| saliency::compute(&pixels));
            let (basis, point) = if let Some(Ok(r)) = &face_result {
                if let Some(face) = r.faces.iter().max_by(|a, b| a.score.total_cmp(&b.score)) {
                    (
                        "faces",
                        [
                            face.bbox.x + face.bbox.width / 2.,
                            face.bbox.y + face.bbox.height / 2.,
                        ],
                    )
                } else if let Some(s) = &saliency {
                    (
                        if s.informative { "saliency" } else { "centre" },
                        s.focal_point,
                    )
                } else {
                    ("centre", [size[0] as f32 / 2., size[1] as f32 / 2.])
                }
            } else if let Some(s) = &saliency {
                (
                    if s.informative { "saliency" } else { "centre" },
                    s.focal_point,
                )
            } else {
                ("centre", [size[0] as f32 / 2., size[1] as f32 / 2.])
            };
            let detection = match &face_result {
                Some(Ok(r)) => r.clone(),
                _ => faces::FaceReport {
                    schema: faces::FACES_SCHEMA.into(),
                    image_sha256: hash.clone(),
                    image_size: size,
                    faces: vec![],
                    provenance: vision::Provenance::fixture("no-face-inference"),
                    limitations: faces::FACE_LIMIT.into(),
                },
            };
            let specs: Vec<_> = options
                .crops
                .iter()
                .cloned()
                .chain(options.output_sizes.iter().map(|s| faces::CropSpec::Ratio {
                    width: s[0] as f32,
                    height: s[1] as f32,
                }))
                .collect();
            let crops = if specs.is_empty() {
                None
            } else {
                Some(faces::crop_check(&image, &detection, &specs, Some(point))?.crops)
            };
            Ok(
                json!({"faces":match &face_result{Some(Ok(r))=>json!({"status":"ok","report":r}),Some(Err(e))=>json!({"status":"failed","error_code":e.code,"reason":e.message}),None=>json!({"status":"skipped","reason":"disabled by preset or options"})},"saliency":saliency,"focal_point":point,"basis":basis,"crops":crops,"face_crop_checks_available":matches!(face_result,Some(Ok(_)))}),
            )
        });
        focal.timing_ms += face_elapsed;
        let text = if options.text.unwrap_or(full) {
            section("ocrs-rten", || self.ocr(bytes, profile))
        } else {
            skipped("ocrs-rten", "disabled by preset or options")
        };
        let fingerprints = section("perceptual-fast-brief", || {
            Ok(
                json!({"hashes":hashing::hash(&pixels),"keypoints":registration::fingerprint(&pixels),"size":size,"keypoint_algorithm":"FAST-oriented-BRIEF/1"}),
            )
        });
        let embeddings = if options.embeddings.unwrap_or(full) {
            section("image-embedding", || self.embedding_value(&pixels, profile))
        } else {
            skipped("image-embedding", "disabled by preset or options")
        };
        let description = if options.description {
            section("observation-draft", || {
                Err(MediaError::new(
                    "description_unavailable",
                    "supply an observation provider via analyze_with_provider",
                ))
            })
        } else {
            skipped("observation-draft", "off by default")
        };
        let mut record = Record {
            schema: SCHEMA.into(),
            profile,
            identity,
            metadata,
            quality,
            focal,
            text,
            fingerprints,
            embeddings,
            description,
            video: skipped("external-ffmpeg-keyframes", "image input"),
        };
        if record.embeddings.status == Status::Ok
            && let Ok(model) = self.contract(crate::general::embedding::MODEL_SCHEMA)
        {
            record.embeddings.provenance = Provenance {
                id: model["family"].as_str().unwrap_or("embedding").into(),
                version: model["artifact"]["version"]
                    .as_str()
                    .unwrap_or("supplied")
                    .into(),
                pins: vec![model["artifact"]["sha256"].as_str().unwrap_or("").into()],
            };
        }
        if let Some(Ok(face)) = &face_result {
            record
                .focal
                .provenance
                .pins
                .extend(face.provenance.artifact_sha256.clone());
        }
        if record.text.status == Status::Ok
            && let Ok(c) = self.contract(crate::general::ocr::SCHEMA)
        {
            record.text.provenance.version = "ocrs-0.10.4/rten-0.21.0".into();
            record.text.provenance.pins = ["detection", "recognition"]
                .iter()
                .filter_map(|k| c[*k]["sha256"].as_str().map(str::to_owned))
                .collect();
        }
        // An attempted face failure remains independent of the successful saliency fallback.
        if let Some(Err(e)) = face_result {
            record.focal.status = Status::Failed;
            record.focal.reason = Some(e.message);
            record.focal.error_code = Some(e.code);
        }
        enforce_strict(record, options.strict)
    }
    /// Opt-in draft description through the established closed observation interface.
    /// Provider transport authority remains the caller's responsibility.
    pub fn analyze_with_provider(
        &self,
        bytes: &[u8],
        options: &Options,
        request: &observation::ObservationRequest,
        provider: &mut dyn observation::ObservationProvider,
    ) -> Result<Record> {
        let mut relaxed = options.clone();
        relaxed.strict = false;
        relaxed.description = false;
        let mut r = self.analyze_bytes(bytes, &relaxed)?;
        if options.description {
            r.description = section("observation-draft", || {
                request.validate()?;
                if !matches!(request.task, observation::Task::Caption)
                    || request.images.len() != 1
                    || models::digest(&request.images[0].bytes) != models::digest(bytes)
                {
                    return Err(MediaError::new(
                        "invalid_media_options",
                        "description request must bind the retained image and describe task",
                    ));
                }
                let result = provider.observe(request)?;
                result.validate(request)?;
                Ok(
                    json!({"draft":true,"description_and_alt_text_draft":result.statements,"observation":result}),
                )
            });
        }
        enforce_strict(r, options.strict)
    }
    fn faces(&self, image: &vision::VisionImage, profile: Profile) -> Result<faces::FaceReport> {
        if profile == Profile::Gpu {
            return Err(MediaError::new(
                "gpu_unavailable",
                "GPU execution provider is not provisioned",
            ));
        }
        #[cfg(feature = "local-models")]
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| MediaError::new("analyzer_poisoned", "model session lock poisoned"))?;
            if sessions.faces.is_none() {
                let model = self.registry.model("yunet-2026may")?;
                let library = crate::wave7::runtime_install::resolve(None, &self.model_dir)?;
                sessions.faces = Some(crate::wave7::runtime::OnnxModel::load(
                    model,
                    &self.model_dir,
                    &library,
                    self.allow_download,
                )?);
            }
            let runtime = sessions
                .faces
                .as_mut()
                .ok_or_else(|| MediaError::new("vision_unavailable", "face session unavailable"))?;
            Ok(faces::detect(image, runtime)?)
        }
        #[cfg(not(feature = "local-models"))]
        {
            let _ = image;
            Err(MediaError::new(
                "vision_unavailable",
                "compile local-models for face inference",
            ))
        }
    }
    fn ocr(&self, bytes: &[u8], profile: Profile) -> Result<Value> {
        if profile == Profile::Gpu {
            return Err(MediaError::new("gpu_unavailable", "OCR supports CPU only"));
        }
        #[cfg(feature = "ocr")]
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| MediaError::new("analyzer_poisoned", "model lock poisoned"))?;
            if sessions.ocr.is_none() {
                let contract = self.contract(crate::general::ocr::SCHEMA)?;
                let contract: crate::general::ocr::Contract =
                    serde_json::from_value(contract.clone())?;
                sessions.ocr = Some(crate::general::ocr::Engine::load(
                    &contract,
                    &self.model_dir,
                    self.allow_download,
                )?);
            }
            let source = sessions
                .ocr
                .as_ref()
                .ok_or_else(|| MediaError::new("ocr_unavailable", "OCR session unavailable"))?
                .recognize(bytes)?;
            Ok(
                json!({"words":source.nodes,"provenance":source.producer,"confidence":"unavailable in upstream API; never invented"}),
            )
        }
        #[cfg(not(feature = "ocr"))]
        {
            let _ = bytes;
            Err(MediaError::new(
                "ocr_unavailable",
                "compile ocr and supply its pinned contract",
            ))
        }
    }
    /// Reusable installed image-embedding inference; no model supplied means typed unavailability.
    pub fn embed_image(&self, bytes: &[u8]) -> Result<Vec<f32>> {
        if bytes.len() as u64 > input::MAX_BYTES {
            return Err(MediaError::new(
                "request_too_large",
                "encoded input exceeds 64 MiB",
            ));
        }
        self.embed_pixels(&input::decode(bytes)?, self.profile)
    }
    fn embed_pixels(&self, pixels: &image::RgbaImage, profile: Profile) -> Result<Vec<f32>> {
        if profile == Profile::Gpu {
            return Err(MediaError::new(
                "gpu_unavailable",
                "embedding GPU provider not provisioned",
            ));
        }
        #[cfg(feature = "embeddings")]
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| MediaError::new("analyzer_poisoned", "model session lock poisoned"))?;
            if sessions.embeddings.is_none() {
                let model = self.contract(crate::general::embedding::MODEL_SCHEMA)?;
                let model = crate::general::embedding::parse_model(&serde_json::to_vec(model)?)?;
                #[cfg(feature = "local-models")]
                let library = crate::wave7::runtime_install::resolve(None, &self.model_dir)?;
                #[cfg(not(feature = "local-models"))]
                let library = std::env::var_os("ORT_DYLIB_PATH")
                    .map(PathBuf::from)
                    .ok_or_else(|| {
                        MediaError::new("runtime_incompatible", "set an explicit ORT_DYLIB_PATH")
                    })?;
                sessions.embeddings = Some(crate::general::embedding::Engine::load(
                    model,
                    &self.model_dir,
                    &library,
                    self.allow_download,
                )?);
            }
            let engine = sessions.embeddings.as_mut().ok_or_else(|| {
                MediaError::new("embedding_unavailable", "embedding session unavailable")
            })?;
            Ok(engine.embed(pixels)?)
        }
        #[cfg(not(feature = "embeddings"))]
        {
            let _ = pixels;
            Err(MediaError::new(
                "embedding_unavailable",
                "compile embeddings and supply a pinned contract",
            ))
        }
    }
    fn embedding_value(&self, pixels: &image::RgbaImage, profile: Profile) -> Result<Value> {
        let vector = self.embed_pixels(pixels, profile)?;
        #[cfg(feature = "embeddings")]
        {
            use base64::Engine;
            let model = self.contract(crate::general::embedding::MODEL_SCHEMA)?;
            let raw: Vec<u8> = vector.iter().flat_map(|v| v.to_le_bytes()).collect();
            Ok(
                json!({"model_id":model["family"],"model_sha256":model["artifact"]["sha256"],"encoding":"base64-f32-le","dimensions":vector.len(),"data":base64::engine::general_purpose::STANDARD.encode(raw),"calibration":"uncalibrated"}),
            )
        }
        #[cfg(not(feature = "embeddings"))]
        {
            let _ = vector;
            Err(MediaError::new(
                "embedding_unavailable",
                "compile embeddings",
            ))
        }
    }
    /// Text inference is unavailable until an official permissive pinned joint export is supplied.
    pub fn embed_text(&self, _text: &str) -> Result<Vec<f32>> {
        Err(MediaError::new(
            "text_embedding_unavailable",
            "SigLIP 2 official checkpoint/export pins and licence evidence deferred; image-only vectors cannot answer text queries",
        ))
    }
    /// Model contract identity used by an index, independent of cache paths.
    pub fn embedding_id(&self) -> Result<String> {
        let m = self.contract(crate::general::embedding::MODEL_SCHEMA)?;
        Ok(models::digest(&serde_json::to_vec(
            &crate::general::embedding::parse_model(&serde_json::to_vec(m)?)?,
        )?))
    }
}
fn enforce_strict(record: Record, strict: bool) -> Result<Record> {
    if strict
        && [
            &record.identity,
            &record.metadata,
            &record.quality,
            &record.focal,
            &record.text,
            &record.fingerprints,
            &record.embeddings,
            &record.description,
            &record.video,
        ]
        .iter()
        .any(|s| s.status == Status::Failed)
    {
        Err(MediaError::new(
            "media_section_failed",
            "strict analysis rejected a failed section",
        ))
    } else {
        Ok(record)
    }
}
/// Portable consumer cache default. Lane gates always pass the prescribed cache explicitly.
pub fn default_model_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("saccade/models")
}
/// Convenience single-call library entry point using a CPU-lite analyzer and no downloads.
pub fn analyze_media(source: &str, options: &Options) -> Result<Record> {
    Analyzer::new(Profile::CpuLite, default_model_dir(), false)?.analyze_media(source, options)
}
/// Credit/copyright candidates retain the exact source field and value; never infer ownership.
pub fn credit_candidates(metadata: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for (source, kind) in [
        ("/exif/creator", "credit"),
        ("/exif/copyright", "copyright"),
        ("/xmp/fields/creator", "credit"),
        ("/xmp/fields/Credit", "credit"),
        ("/xmp/fields/rights", "copyright"),
    ] {
        if let Some(v) = metadata.pointer(source) {
            let values = if let Some(a) = v.as_array() {
                a.clone()
            } else {
                vec![v.clone()]
            };
            for value in values {
                if value.as_str().is_some_and(|s| !s.trim().is_empty()) {
                    out.push(json!({"kind":kind,"value":value,"source_field":source}));
                }
            }
        }
    }
    if let Some(records) = metadata["iptc"]["records"].as_array() {
        for r in records {
            let kind = match r["dataset"].as_u64() {
                Some(80 | 110) => "credit",
                Some(116) => "copyright",
                _ => continue,
            };
            out.push(json!({"kind":kind,"value":r["text"],"source_field":format!("IPTC:2:{}",r["dataset"])}));
        }
    }
    out
}
// Feature-independent builds retain identical record types, without native dependencies.
#[cfg(not(any(feature = "local-models", feature = "embeddings", feature = "ocr")))]
#[allow(dead_code)]
fn portable_configuration(a: &Analyzer) {
    let _ = (&a.model_dir, &a.registry, a.allow_download, &a.sessions);
}
/// Exact flat index shared by Python and HTTP transports.
pub mod search;
impl Analyzer {
    fn contract(&self, schema: &str) -> Result<&Value> {
        let mut matches = self
            .registry
            .contracts
            .values()
            .filter(|v| v["schema"] == schema);
        let first = matches.next().ok_or_else(|| {
            MediaError::new(
                if schema == crate::general::ocr::SCHEMA {
                    "ocr_unavailable"
                } else {
                    "embedding_unavailable"
                },
                "no matching pinned registry contract",
            )
        })?;
        if matches.next().is_some() {
            return Err(MediaError::new(
                "invalid_media_options",
                "registry contract selection is ambiguous",
            ));
        }
        Ok(first)
    }
    /// Canonical pinned embedding contract (paths and runtime location excluded).
    pub fn embedding_model(&self) -> Result<Value> {
        let value = self.contract(crate::general::embedding::MODEL_SCHEMA)?;
        let model = crate::general::embedding::parse_model(&serde_json::to_vec(value)?)?;
        Ok(serde_json::to_value(model)?)
    }
    /// Compare retained SDR encoded inputs through the established FLIP computation.
    pub fn compare(&self, a: &[u8], b: &[u8], ppd: f32) -> Result<Value> {
        if a.len() as u64 > input::MAX_BYTES || b.len() as u64 > input::MAX_BYTES {
            return Err(MediaError::new(
                "request_too_large",
                "encoded input exceeds 64 MiB",
            ));
        }
        let aa = input::decode(a)?;
        let bb = input::decode(b)?;
        let options = crate::compare::CompareOptions {
            pixels_per_degree: ppd,
            ..Default::default()
        };
        let result = crate::compare::compare_rgba(&aa, &bb, &options)?;
        Ok(
            json!({"schema":COMPARE_SCHEMA,"reference_sha256":models::digest(b),"capture_sha256":models::digest(a),"metrics":result.metrics,"ppd":ppd}),
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn encoded() -> Vec<u8> {
        let im = image::RgbaImage::from_fn(40, 30, |x, y| {
            image::Rgba([x as u8 * 3, y as u8 * 4, 50, 255])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        im.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }
    #[test]
    fn cpu_lite_sections_and_override_strict() {
        let a = Analyzer::new(Profile::CpuLite, "cache".into(), false).unwrap();
        let b = encoded();
        let mut opts = Options {
            output_sizes: vec![[400, 300], [20, 10]],
            ..Default::default()
        };
        let r = a.analyze_bytes(&b, &opts).unwrap();
        assert_eq!(r.identity.data["sha256"], models::digest(&b));
        assert_eq!(r.quality.data["fitness"][0]["adequate_resolution"], false);
        assert_eq!(r.text.status, Status::Skipped);
        assert_eq!(r.fingerprints.status, Status::Ok);
        opts.description = true;
        assert_eq!(
            a.analyze_bytes(&b, &opts).unwrap().description.status,
            Status::Failed
        );
        opts.strict = true;
        assert_eq!(
            a.analyze_bytes(&b, &opts).unwrap_err().code,
            "media_section_failed"
        );
    }
    #[cfg(unix)]
    #[test]
    fn special_file_is_refused_before_any_read() {
        let analyzer = Analyzer::new(Profile::CpuLite, "cache".into(), false).unwrap();
        assert!(
            analyzer
                .analyze_media("/dev/zero", &Options::default())
                .is_err()
        );
    }
    #[test]
    fn optional_description_uses_bound_observation_provider() {
        struct Provider;
        impl observation::ObservationProvider for Provider {
            fn observe(
                &mut self,
                r: &observation::ObservationRequest,
            ) -> models::Result<observation::ObservationReport> {
                let mut provenance = vision::Provenance::fixture(&r.model);
                provenance.runtime = "provider-fixture".into();
                provenance.input_resolution = r.images[0].presented_size;
                Ok(observation::ObservationReport {
                    schema: observation::OBSERVATION_SCHEMA.into(),
                    advisory_only: true,
                    request_sha256: r.hash()?,
                    response_sha256: models::digest(b"fixture"),
                    data_sha256: models::digest(r.data.as_bytes()),
                    requested_model: r.model.clone(),
                    returned_model: r.model.clone(),
                    returned_revision: None,
                    provider: "fixture".into(),
                    provenance,
                    statements: vec![observation::Statement {
                        image_id: "image".into(),
                        text: "A draft description".into(),
                        bbox: None,
                        point: None,
                        confidence: None,
                    }],
                    usage: observation::Usage::default(),
                    cost_usd: None,
                })
            }
        }
        let bytes = encoded();
        let request = observation::ObservationRequest {
            task: observation::Task::Caption,
            data: "describe data".into(),
            images: vec![observation::ImageInput {
                id: "image".into(),
                bytes: bytes.clone(),
                media_type: "image/png".into(),
                original_size: [40, 30],
                presented_size: [40, 30],
                scale: [1., 1.],
                offset: [0., 0.],
            }],
            model: "fixture-model".into(),
            encoder_version: "fixture/1".into(),
            max_output_tokens: 128,
        };
        let a = Analyzer::new(Profile::CpuLite, "cache".into(), false).unwrap();
        let options = Options {
            description: true,
            strict: true,
            ..Default::default()
        };
        let r = a
            .analyze_with_provider(&bytes, &options, &request, &mut Provider)
            .unwrap();
        assert_eq!(r.description.status, Status::Ok);
        assert_eq!(r.description.data["draft"], true);
        let mut stale = request.clone();
        stale.images[0].bytes = b"wrong".to_vec();
        assert!(
            a.analyze_with_provider(&bytes, &options, &stale, &mut Provider)
                .is_err()
        );
    }
    #[test]
    fn credit_values_are_sourced_and_absence_is_empty() {
        assert!(credit_candidates(&json!({})).is_empty());
        let c = credit_candidates(
            &json!({"exif":{"creator":"Zoé","copyright":"© 2026"},"xmp":{"fields":{"creator":["Other"]}}}),
        );
        assert_eq!(c.len(), 3);
        assert_eq!(c[0]["source_field"], "/exif/creator");
    }
    #[test]
    fn concurrent_calls_share_analyzer() {
        let a =
            std::sync::Arc::new(Analyzer::new(Profile::CpuLite, "cache".into(), false).unwrap());
        let b = encoded();
        std::thread::scope(|s| {
            for _ in 0..4 {
                let a = a.clone();
                let b = &b;
                s.spawn(move || {
                    assert_eq!(
                        a.analyze_bytes(b, &Options::default())
                            .unwrap()
                            .identity
                            .status,
                        Status::Ok
                    )
                });
            }
        });
    }
}
