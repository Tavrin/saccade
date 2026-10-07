//! abi3 Python bindings. All pixel/model/IO compute runs outside the Python GIL.
use pyo3::{
    prelude::*,
    types::{PyAny, PyBytes, PyDict, PyType},
};
use saccade_core::{
    media::{self, Analyzer as CoreAnalyzer, MediaError, Options, Profile},
    wave7::models,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
/// Stable Saccade binding version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pyo3::create_exception!(
    saccade,
    SaccadeError,
    pyo3::exceptions::PyException,
    "Stable Saccade error carrying a machine-readable code."
);
pyo3::create_exception!(
    saccade,
    InputError,
    SaccadeError,
    "Invalid media input or options."
);
pyo3::create_exception!(
    saccade,
    ModelError,
    SaccadeError,
    "Model, runtime or inference unavailable."
);
pyo3::create_exception!(
    saccade,
    AnalysisError,
    SaccadeError,
    "Analysis or persistence failure."
);
fn error(py: Python<'_>, e: MediaError) -> PyErr {
    let ty =
        if e.code.contains("unavailable") || e.code.contains("model") || e.code.contains("runtime")
        {
            py.get_type::<ModelError>()
        } else if e.code.starts_with("invalid") || e.code == "request_too_large" {
            py.get_type::<InputError>()
        } else {
            py.get_type::<AnalysisError>()
        };
    let err = PyErr::from_type(ty, (e.message,));
    let _ = err.value(py).setattr("code", e.code);
    err
}
fn json(py: Python<'_>, value: media::Result<serde_json::Value>) -> PyResult<Py<PyAny>> {
    let value = value.map_err(|e| error(py, e))?;
    let text = serde_json::to_string(&value)
        .map_err(|e| error(py, MediaError::new("invalid_json", e.to_string())))?;
    Ok(py.import("json")?.call_method1("loads", (text,))?.unbind())
}
/// Emit a `DeprecationWarning` for a superseded argument; behaviour is unchanged.
fn deprecated(py: Python<'_>, what: &str, replacement: &str) -> PyResult<()> {
    let message = format!(
        "{what} is deprecated; use {replacement}. It keeps working and will not be removed before {}",
        saccade_core::model_config::REMOVAL_NOT_BEFORE
    );
    py.import("warnings")?.call_method1(
        "warn",
        (
            message,
            py.get_type::<pyo3::exceptions::PyDeprecationWarning>(),
            3,
        ),
    )?;
    Ok(())
}
/// Shared operator configuration (the same resolver the CLI and MCP use) with the
/// deprecated `model_dir` / `registry` arguments applied over it.
fn resolved(
    py: Python<'_>,
    model_dir: Option<&str>,
    registry: Option<&str>,
) -> PyResult<saccade_core::model_config::ModelConfig> {
    let mut cfg = saccade_core::model_config::ModelConfig::resolve()
        .map_err(|e| error(py, MediaError::new("invalid_media_options", e.to_string())))?;
    if let Some(d) = model_dir {
        deprecated(
            py,
            "model_dir=",
            "SACCADE_MODELS_DIR or [models].dir (see saccade.model_config())",
        )?;
        cfg.dir = PathBuf::from(d);
    }
    if let Some(r) = registry {
        deprecated(
            py,
            "registry=",
            "SACCADE_MODELS_REGISTRY or [models].registry",
        )?;
        cfg.registry = Some(PathBuf::from(r));
    }
    Ok(cfg)
}
/// The resolved model configuration and where each value came from.
#[pyfunction]
fn model_config(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let cfg = saccade_core::model_config::ModelConfig::resolve()
        .map_err(|e| error(py, MediaError::new("invalid_media_options", e.to_string())))?;
    json(py, Ok(cfg.to_json()))
}
enum Input {
    Bytes(Vec<u8>),
    Path(String),
}
fn input(value: &Bound<'_, PyAny>) -> PyResult<Input> {
    if let Ok(v) = value.downcast::<PyBytes>() {
        if v.as_bytes().len() as u64 > saccade_core::general::input::MAX_BYTES {
            return Err(error(
                value.py(),
                MediaError::new("request_too_large", "encoded input exceeds 64 MiB"),
            ));
        }
        return Ok(Input::Bytes(v.as_bytes().to_vec()));
    }
    if let Ok(v) = value.extract::<String>() {
        return Ok(Input::Path(v));
    }
    let path = value.call_method0("__fspath__")?.extract::<String>()?;
    Ok(Input::Path(path))
}
fn read(a: &CoreAnalyzer, value: Input) -> media::Result<Vec<u8>> {
    match value {
        Input::Bytes(b) => Ok(b),
        Input::Path(p) => a.read(&p),
    }
}
fn options(py: Python<'_>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Options> {
    let Some(k) = kwargs else {
        return Ok(Options::default());
    };
    let raw = py
        .import("json")?
        .call_method1("dumps", (k,))?
        .extract::<String>()?;
    serde_json::from_str(&raw)
        .map_err(|e| error(py, MediaError::new("invalid_media_options", e.to_string())))
}
/// Reusable thread-safe lazy analyzer.
#[pyclass(name = "Analyzer", module = "saccade._native")]
struct Analyzer {
    inner: Arc<CoreAnalyzer>,
}
#[pymethods]
impl Analyzer {
    #[new]
    #[pyo3(signature=(profile="cpu-lite",model_dir=None,allow_download=false,registry=None))]
    fn new(
        py: Python<'_>,
        profile: &str,
        model_dir: Option<&str>,
        allow_download: bool,
        registry: Option<&str>,
    ) -> PyResult<Self> {
        let profile = match profile {
            "cpu-lite" => Profile::CpuLite,
            "cpu-full" => Profile::CpuFull,
            "gpu" => Profile::Gpu,
            _ => {
                return Err(error(
                    py,
                    MediaError::new("invalid_media_options", "unknown profile"),
                ));
            }
        };
        if allow_download {
            deprecated(
                py,
                "allow_download=",
                "saccade.pull_models([...]) (the one provisioning call)",
            )?;
        }
        let cfg = resolved(py, model_dir, registry)?;
        let a = py
            .allow_threads(|| match cfg.registry {
                Some(p) => CoreAnalyzer::with_registry(
                    profile,
                    cfg.dir,
                    allow_download,
                    models::Registry::load(&p)?,
                ),
                None => CoreAnalyzer::new(profile, cfg.dir, allow_download),
            })
            .map_err(|e| error(py, e))?;
        Ok(Self { inner: Arc::new(a) })
    }
    #[pyo3(signature=(source,**kwargs))]
    fn analyze_media(
        &self,
        py: Python<'_>,
        source: &Bound<'_, PyAny>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let source = input(source)?;
        let options = options(py, kwargs)?;
        json(
            py,
            py.allow_threads(|| {
                let record = match source {
                    Input::Path(path) => self.inner.analyze_media(&path, &options)?,
                    Input::Bytes(bytes) => self.inner.analyze_bytes(&bytes, &options)?,
                };
                Ok(serde_json::to_value(record)?)
            }),
        )
    }
    #[pyo3(signature=(a,b,*,ppd=67.0))]
    fn compare(
        &self,
        py: Python<'_>,
        a: &Bound<'_, PyAny>,
        b: &Bound<'_, PyAny>,
        ppd: f32,
    ) -> PyResult<Py<PyAny>> {
        let a = input(a)?;
        let b = input(b)?;
        json(
            py,
            py.allow_threads(|| {
                self.inner
                    .compare(&read(&self.inner, a)?, &read(&self.inner, b)?, ppd)
            }),
        )
    }
    fn hash(&self, py: Python<'_>, source: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let s = input(source)?;
        json(
            py,
            py.allow_threads(|| {
                let b = read(&self.inner, s)?;
                let image = saccade_core::general::input::decode(&b)?;
                Ok(serde_json::to_value(saccade_core::general::hashing::hash(
                    &image,
                ))?)
            }),
        )
    }
    fn embed_image(&self, py: Python<'_>, source: &Bound<'_, PyAny>) -> PyResult<Vec<f32>> {
        let s = input(source)?;
        py.allow_threads(|| self.inner.embed_image(&read(&self.inner, s)?))
            .map_err(|e| error(py, e))
    }
    fn embed_text(&self, py: Python<'_>, text: &str) -> PyResult<Vec<f32>> {
        py.allow_threads(|| self.inner.embed_text(text))
            .map_err(|e| error(py, e))
    }
}
/// Reusable exact search index with checked model identity.
#[pyclass(name = "Index", module = "saccade._native")]
struct Index {
    inner: Mutex<media::search::Index>,
    analyzer: Arc<CoreAnalyzer>,
}
#[pymethods]
impl Index {
    #[new]
    fn new(py: Python<'_>, analyzer: PyRef<'_, Analyzer>) -> PyResult<Self> {
        let a = analyzer.inner.clone();
        let i = py
            .allow_threads(|| media::search::Index::new(a.embedding_model()?))
            .map_err(|e| error(py, e))?;
        Ok(Self {
            inner: Mutex::new(i),
            analyzer: a,
        })
    }
    #[classmethod]
    fn build(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        analyzer: PyRef<'_, Analyzer>,
        paths: Vec<String>,
    ) -> PyResult<Self> {
        let a = analyzer.inner.clone();
        let index = py
            .allow_threads(|| media::search::Index::build(&a, &paths))
            .map_err(|e| error(py, e))?;
        Ok(Self {
            inner: Mutex::new(index),
            analyzer: a,
        })
    }
    #[pyo3(signature=(source,*,label="image"))]
    fn add(&self, py: Python<'_>, source: &Bound<'_, PyAny>, label: &str) -> PyResult<()> {
        let source = input(source)?;
        py.allow_threads(|| {
            self.lock()?
                .add_image(&self.analyzer, label, &read(&self.analyzer, source)?)
        })
        .map_err(|e| error(py, e))
    }
    #[pyo3(signature=(*,image=None,text=None,top=10))]
    fn query(
        &self,
        py: Python<'_>,
        image: Option<&Bound<'_, PyAny>>,
        text: Option<&str>,
        top: usize,
    ) -> PyResult<Py<PyAny>> {
        let image = image.map(input).transpose()?;
        json(
            py,
            py.allow_threads(|| match (image, text) {
                (Some(i), None) => {
                    self.lock()?
                        .query_image(&self.analyzer, &read(&self.analyzer, i)?, top)
                }
                (None, Some(t)) => self.lock()?.query_text(&self.analyzer, t, top),
                _ => Err(MediaError::new(
                    "invalid_media_options",
                    "supply exactly one of image or text",
                )),
            }),
        )
    }
    fn save(&self, py: Python<'_>, path: &str) -> PyResult<()> {
        py.allow_threads(|| self.lock()?.save(&PathBuf::from(path)))
            .map_err(|e| error(py, e))
    }
    #[classmethod]
    fn load(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        analyzer: PyRef<'_, Analyzer>,
        path: &str,
    ) -> PyResult<Self> {
        let i = py
            .allow_threads(|| media::search::Index::load(&PathBuf::from(path)))
            .map_err(|e| error(py, e))?;
        Ok(Self {
            inner: Mutex::new(i),
            analyzer: analyzer.inner.clone(),
        })
    }
}
impl Index {
    fn lock(&self) -> media::Result<std::sync::MutexGuard<'_, media::search::Index>> {
        self.inner
            .lock()
            .map_err(|_| MediaError::new("analyzer_poisoned", "index lock poisoned"))
    }
}
#[pyfunction]
#[pyo3(signature=(model_dir=None))]
fn pull_runtime(py: Python<'_>, model_dir: Option<&str>) -> PyResult<String> {
    #[cfg(feature = "models")]
    {
        let dir = resolved(py, model_dir, None)?.dir;
        py.allow_threads(|| {
            saccade_core::wave7::runtime_install::pull(&dir)
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(MediaError::from)
        })
        .map_err(|e| error(py, e))
    }
    #[cfg(not(feature = "models"))]
    {
        let _ = model_dir;
        Err(error(
            py,
            MediaError::new(
                "vision_unavailable",
                "install a wheel built with models feature",
            ),
        ))
    }
}
#[pyfunction]
#[pyo3(signature=(ids,model_dir=None,registry=None))]
fn pull_models(
    py: Python<'_>,
    ids: Vec<String>,
    model_dir: Option<&str>,
    registry: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let cfg = resolved(py, model_dir, registry)?;
    json(
        py,
        py.allow_threads(|| {
            let reg = match &cfg.registry {
                Some(p) => models::Registry::load(p)?,
                None => models::Registry::pinned_wave7()?,
            };
            let mut rows = Vec::new();
            for id in ids {
                let model = reg.model(&id)?;
                let paths = models::ensure(model, &cfg.dir, true)?;
                rows.push(serde_json::json!({"id":id,"artifact_count":paths.len()}));
            }
            Ok(serde_json::json!({"schema":models::MODELS_SCHEMA,"models":rows}))
        }),
    )
}
/// Native module loaded by the ordinary Python package.
/// Return the native FLIP map and numerical tile grids as NumPy float32 arrays.
#[pyfunction]
#[pyo3(signature=(reference,candidate,tile_size=32))]
fn compare_maps(
    py: Python<'_>,
    reference: &Bound<'_, PyAny>,
    candidate: &Bound<'_, PyAny>,
    tile_size: u32,
) -> PyResult<Py<PyAny>> {
    // Resolve numpy before any image computation; package declares it as a dependency.
    let numpy = py.import("numpy")?;
    let (reference, candidate) = (input(reference)?, input(candidate)?);
    let maps = py
        .allow_threads(|| -> saccade_core::Result<_> {
            let read = |input: Input| -> saccade_core::Result<Vec<u8>> {
                match input {
                    Input::Bytes(b) => Ok(b),
                    Input::Path(p) => {
                        saccade_core::evidence_quality::read(std::path::Path::new(&p), 64 << 20)
                    }
                }
            };
            let b = saccade_core::evidence_quality::decode(
                &read(reference)?,
                std::path::Path::new("reference"),
            )?
            .to_rgba8();
            let c = saccade_core::evidence_quality::decode(
                &read(candidate)?,
                std::path::Path::new("candidate"),
            )?
            .to_rgba8();
            let options = Default::default();
            let comparison = saccade_core::compare::compare_rgba(&c, &b, &options)?;
            let policy = saccade_core::evidence_quality::spatial::Policy {
                tile_size,
                ..Default::default()
            };
            let spatial = saccade_core::evidence_quality::spatial::analyze(
                &b,
                &c,
                &comparison.error_map,
                None,
                None,
                &policy,
                &options,
                saccade_core::diagnostics::ChangeClass::LocalStructure,
            )?;
            saccade_core::evidence_quality::maps::collect(
                &comparison.error_map,
                b.dimensions(),
                None,
                Some(&spatial),
            )
        })
        .map_err(|e| error(py, MediaError::new("invalid_map_input", e.to_string())))?;
    let result = PyDict::new(py);
    for map in maps {
        let bytes: Vec<u8> = map.values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let array = numpy
            .call_method1("frombuffer", (PyBytes::new(py, &bytes), "<f4"))?
            .call_method0("copy")?
            .call_method1("reshape", (map.dimensions[1], map.dimensions[0]))?;
        result.set_item(map.name, array)?;
    }
    Ok(result.into_any().unbind())
}
/// Run bounded, resumable batch intake through an explicitly installed matching CLI.
#[pyfunction]
#[pyo3(signature=(source, out, executable="saccade", options_json=None, reference_dir=None))]
fn batch(
    py: Python<'_>,
    source: PathBuf,
    out: PathBuf,
    executable: &str,
    options_json: Option<&str>,
    reference_dir: Option<PathBuf>,
) -> PyResult<Py<PyAny>> {
    let options = options_json
        .map(serde_json::from_str::<saccade_core::batch::Options>)
        .transpose()
        .map_err(|e| error(py, MediaError::new("invalid_batch_options", e.to_string())))?
        .unwrap_or_default();
    let executable = PathBuf::from(executable);
    let rows = py
        .allow_threads(|| {
            if source.is_dir()
                && saccade_core::run::normalise_path(&out).starts_with(
                    saccade_core::paths::canonicalize(&source)
                        .map_err(|e| saccade_core::Error::Config(e.to_string()))?,
                )
            {
                return Err(saccade_core::Error::Config(
                    "batch output is inside an input directory".into(),
                ));
            }
            let inputs = saccade_core::batch::intake(&source, reference_dir.as_deref())?;
            saccade_core::batch::run(&executable, &inputs, &options, &out)
        })
        .map_err(|e| error(py, MediaError::new("batch_failed", e.to_string())))?;
    json(py, Ok(serde_json::Value::Array(rows)))
}
#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", VERSION)?;
    m.add_function(wrap_pyfunction!(compare_maps, m)?)?;
    m.add_function(wrap_pyfunction!(batch, m)?)?;
    m.add_class::<Analyzer>()?;
    m.add_class::<Index>()?;
    m.add("SaccadeError", py.get_type::<SaccadeError>())?;
    m.add("InputError", py.get_type::<InputError>())?;
    m.add("ModelError", py.get_type::<ModelError>())?;
    m.add("AnalysisError", py.get_type::<AnalysisError>())?;
    m.add_function(wrap_pyfunction!(pull_runtime, m)?)?;
    m.add_function(wrap_pyfunction!(pull_models, m)?)?;
    m.add_function(wrap_pyfunction!(model_config, m)?)?;
    Ok(())
}
