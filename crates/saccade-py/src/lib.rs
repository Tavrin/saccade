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
pyo3::create_exception!(saccade, SaccadeError, pyo3::exceptions::PyException);
pyo3::create_exception!(saccade, InputError, SaccadeError);
pyo3::create_exception!(saccade, ModelError, SaccadeError);
pyo3::create_exception!(saccade, AnalysisError, SaccadeError);
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
enum Input {
    Bytes(Vec<u8>),
    Path(String),
}
fn input(value: &Bound<'_, PyAny>) -> PyResult<Input> {
    if let Ok(v) = value.downcast::<PyBytes>() {
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
    #[pyo3(signature=(profile="cpu-lite",model_dir="/mnt/linux-extra/saccade-models",allow_download=false,registry=None))]
    fn new(
        py: Python<'_>,
        profile: &str,
        model_dir: &str,
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
        let a = py
            .allow_threads(|| match registry {
                Some(p) => CoreAnalyzer::with_registry(
                    profile,
                    model_dir.into(),
                    allow_download,
                    models::Registry::load(&PathBuf::from(p))?,
                ),
                None => CoreAnalyzer::new(profile, model_dir.into(), allow_download),
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
                let b = read(&self.inner, source)?;
                Ok(serde_json::to_value(
                    self.inner.analyze_bytes(&b, &options)?,
                )?)
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
#[pyo3(signature=(model_dir="/mnt/linux-extra/saccade-models"))]
fn pull_runtime(py: Python<'_>, model_dir: &str) -> PyResult<String> {
    #[cfg(feature = "models")]
    {
        py.allow_threads(|| {
            saccade_core::wave7::runtime_install::pull(&PathBuf::from(model_dir))
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
#[pyo3(signature=(ids,model_dir="/mnt/linux-extra/saccade-models",registry=None))]
fn pull_models(
    py: Python<'_>,
    ids: Vec<String>,
    model_dir: &str,
    registry: Option<&str>,
) -> PyResult<Py<PyAny>> {
    json(
        py,
        py.allow_threads(|| {
            let reg = match registry {
                Some(p) => models::Registry::load(&PathBuf::from(p))?,
                None => models::Registry::pinned_wave7()?,
            };
            let mut rows = Vec::new();
            for id in ids {
                let model = reg.model(&id)?;
                let paths = models::ensure(model, &PathBuf::from(model_dir), true)?;
                rows.push(serde_json::json!({"id":id,"artifact_count":paths.len()}));
            }
            Ok(serde_json::json!({"schema":models::MODELS_SCHEMA,"models":rows}))
        }),
    )
}
/// Native module loaded by the ordinary Python package.
#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Analyzer>()?;
    m.add_class::<Index>()?;
    m.add("SaccadeError", py.get_type::<SaccadeError>())?;
    m.add("InputError", py.get_type::<InputError>())?;
    m.add("ModelError", py.get_type::<ModelError>())?;
    m.add("AnalysisError", py.get_type::<AnalysisError>())?;
    m.add_function(wrap_pyfunction!(pull_runtime, m)?)?;
    m.add_function(wrap_pyfunction!(pull_models, m)?)?;
    Ok(())
}
