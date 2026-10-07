//! One operator-owned model and runtime configuration.
//!
//! Every surface (CLI, MCP, Python, library) resolves the model cache, the
//! pinned registry, the ONNX runtime library and the default embedding contract
//! from this module, so an operator provisions a model once and every command
//! finds the same pinned artifact.
//!
//! Precedence, highest first:
//!
//! 1. an explicit per-command flag or Python argument (deprecated spellings,
//!    still honoured, with a notice);
//! 2. environment: `SACCADE_MODELS_DIR`, `SACCADE_MODELS_REGISTRY`,
//!    `SACCADE_MODELS_RUNTIME_LIBRARY`, `SACCADE_MODELS_EMBEDDING_CONTRACT`
//!    (`SACCADE_MODEL_CACHE` is read as an alias of the first);
//! 3. the `[models]` table of the config file (`$SACCADE_CONFIG`, else
//!    `$XDG_CONFIG_HOME/saccade/config.toml`, else `~/.config/saccade/config.toml`);
//! 4. defaults (`$XDG_CACHE_HOME/saccade/models`, no registry file, no library).
//!
//! Requests received over MCP never supply any of these values: a server reads
//! only the operator configuration and refuses a request-supplied location that
//! differs from it ([`ModelConfig::check_request_location`](crate::model_config::ModelConfig::check_request_location)). Downloads happen
//! only through `saccade models pull` (or the deprecated per-command flags on
//! the CLI/Python side), never from a request.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::error::{Error, Result};

/// Schema of the resolved-configuration report.
pub const SCHEMA: &str = "saccade-model-config.v1";
/// Model cache directory.
pub const ENV_DIR: &str = "SACCADE_MODELS_DIR";
/// Older spelling of [`ENV_DIR`], still read.
pub const ENV_DIR_LEGACY: &str = "SACCADE_MODEL_CACHE";
/// Pinned registry document.
pub const ENV_REGISTRY: &str = "SACCADE_MODELS_REGISTRY";
/// Dynamic ONNX runtime library.
pub const ENV_RUNTIME: &str = "SACCADE_MODELS_RUNTIME_LIBRARY";
/// Default embedding contract.
pub const ENV_EMBEDDING: &str = "SACCADE_MODELS_EMBEDDING_CONTRACT";
/// Config file location override.
pub const ENV_CONFIG: &str = "SACCADE_CONFIG";
/// Earliest release in which the deprecated spellings may be removed.
pub const REMOVAL_NOT_BEFORE: &str = "0.4.0";

/// Where a resolved value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Built-in default.
    Default,
    /// The `[models]` table of this config file.
    File(PathBuf),
    /// This environment variable.
    Env(String),
    /// A deprecated per-command flag or Python argument.
    Flag,
}

impl Source {
    fn json(&self) -> Value {
        match self {
            Source::Default => json!({"kind":"default"}),
            Source::File(p) => json!({"kind":"config_file","path":p}),
            Source::Env(v) => json!({"kind":"env","name":v}),
            Source::Flag => json!({"kind":"deprecated_flag"}),
        }
    }
}

/// Deprecated per-command overrides, in the form each spelling supplies them.
#[derive(Debug, Default, Clone)]
pub struct Overrides<'a> {
    /// `--cache`, `--model-cache`, `--model-dir`, `--api-model-dir`, `model_dir=`.
    pub dir: Option<&'a Path>,
    /// `--registry`, `--model-registry`, `registry=`.
    pub registry: Option<&'a Path>,
    /// `--runtime-library`, `--library`.
    pub runtime_library: Option<&'a Path>,
    /// `--model` (embedding contract).
    pub embedding_contract: Option<&'a Path>,
}

/// The resolved operator configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelConfig {
    /// Content-addressed model cache.
    pub dir: PathBuf,
    /// Pinned registry document, when one is configured.
    pub registry: Option<PathBuf>,
    /// Dynamic ONNX runtime library, when one is configured.
    pub runtime_library: Option<PathBuf>,
    /// Default embedding contract, when one is configured.
    pub embedding_contract: Option<PathBuf>,
    /// Config file that was read, if any.
    pub config_file: Option<PathBuf>,
    sources: BTreeMap<&'static str, Source>,
}

type EnvFn<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn home_with(env: EnvFn<'_>) -> PathBuf {
    env("HOME")
        .or_else(|| env("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn abs_env(env: EnvFn<'_>, var: &str) -> Option<PathBuf> {
    env(var).map(PathBuf::from).filter(|p| p.is_absolute())
}

fn default_dir(env: EnvFn<'_>) -> PathBuf {
    abs_env(env, "XDG_CACHE_HOME")
        .unwrap_or_else(|| home_with(env).join(".cache"))
        .join("saccade")
        .join("models")
}

fn config_path(env: EnvFn<'_>) -> PathBuf {
    if let Some(p) = env(ENV_CONFIG).filter(|v| !v.is_empty()) {
        return PathBuf::from(p);
    }
    abs_env(env, "XDG_CONFIG_HOME")
        .unwrap_or_else(|| home_with(env).join(".config"))
        .join("saccade")
        .join("config.toml")
}

impl ModelConfig {
    /// Resolve from the process environment and the config file.
    pub fn resolve() -> Result<Self> {
        Self::resolve_with(&|k| std::env::var_os(k))
    }

    /// Resolve with an explicit environment lookup (used by tests).
    pub fn resolve_with(env: EnvFn<'_>) -> Result<Self> {
        let path = config_path(env);
        let mut table = toml::Table::new();
        let mut config_file = None;
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let doc: toml::Table = text
                    .parse()
                    .map_err(|e| Error::Config(format!("config file {}: {e}", path.display())))?;
                if let Some(models) = doc.get("models") {
                    table = models.as_table().cloned().ok_or_else(|| {
                        Error::Config(format!("{}: [models] must be a table", path.display()))
                    })?;
                    const KNOWN: [&str; 4] =
                        ["dir", "registry", "runtime_library", "embedding_contract"];
                    if let Some(k) = table.keys().find(|k| !KNOWN.contains(&k.as_str())) {
                        return Err(Error::Config(format!(
                            "{}: unknown [models] key `{k}`; expected one of {}",
                            path.display(),
                            KNOWN.join(", ")
                        )));
                    }
                }
                config_file = Some(path.clone());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::Config(format!(
                    "reading config file {}: {e}",
                    path.display()
                )));
            }
        }
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let from_file = |key: &str| -> Result<Option<PathBuf>> {
            match table.get(key) {
                None => Ok(None),
                Some(v) => {
                    let s = v.as_str().ok_or_else(|| {
                        Error::Config(format!("[models].{key} must be a path string"))
                    })?;
                    let p = PathBuf::from(s);
                    Ok(Some(if p.is_absolute() { p } else { base.join(p) }))
                }
            }
        };
        let mut sources = BTreeMap::new();
        let mut pick = |name: &'static str, key: &str, vars: &[&str]| -> Result<Option<PathBuf>> {
            for var in vars {
                if let Some(v) = env(var).filter(|v| !v.is_empty()) {
                    sources.insert(name, Source::Env((*var).to_owned()));
                    return Ok(Some(PathBuf::from(v)));
                }
            }
            if let Some(p) = from_file(key)? {
                sources.insert(name, Source::File(path.clone()));
                return Ok(Some(p));
            }
            sources.insert(name, Source::Default);
            Ok(None)
        };
        let dir = pick("dir", "dir", &[ENV_DIR, ENV_DIR_LEGACY])?;
        let registry = pick("registry", "registry", &[ENV_REGISTRY])?;
        let runtime_library = pick("runtime_library", "runtime_library", &[ENV_RUNTIME])?;
        let embedding_contract =
            pick("embedding_contract", "embedding_contract", &[ENV_EMBEDDING])?;
        Ok(Self {
            dir: dir.unwrap_or_else(|| default_dir(env)),
            registry,
            runtime_library,
            embedding_contract,
            config_file,
            sources,
        })
    }

    /// Apply deprecated per-command overrides; each used spelling prints one
    /// deprecation notice per process.
    #[must_use]
    pub fn with_overrides(mut self, o: Overrides<'_>) -> Self {
        if let Some(p) = o.dir {
            deprecated(
                "per-command model directory (--cache, --model-cache, --model-dir, --api-model-dir, model_dir=)",
                "SACCADE_MODELS_DIR or [models].dir in the saccade config file",
            );
            self.dir = p.to_path_buf();
            self.sources.insert("dir", Source::Flag);
        }
        if let Some(p) = o.registry {
            deprecated(
                "per-command model registry (--registry, --model-registry, registry=)",
                "SACCADE_MODELS_REGISTRY or [models].registry",
            );
            self.registry = Some(p.to_path_buf());
            self.sources.insert("registry", Source::Flag);
        }
        if let Some(p) = o.runtime_library {
            deprecated(
                "per-command runtime library (--runtime-library, --library)",
                "SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library",
            );
            self.runtime_library = Some(p.to_path_buf());
            self.sources.insert("runtime_library", Source::Flag);
        }
        if let Some(p) = o.embedding_contract {
            deprecated(
                "per-command embedding contract (--model)",
                "SACCADE_MODELS_EMBEDDING_CONTRACT or [models].embedding_contract",
            );
            self.embedding_contract = Some(p.to_path_buf());
            self.sources.insert("embedding_contract", Source::Flag);
        }
        self
    }

    /// Where `key` (`dir`, `registry`, `runtime_library`, `embedding_contract`) came from.
    pub fn source(&self, key: &str) -> Source {
        self.sources.get(key).cloned().unwrap_or(Source::Default)
    }

    /// Machine-readable report of the resolved values and their sources.
    pub fn to_json(&self) -> Value {
        let entry = |key: &'static str, v: Option<&PathBuf>| json!({"value": v, "source": self.source(key).json()});
        json!({
            "schema": SCHEMA,
            "config_file": self.config_file,
            "dir": entry("dir", Some(&self.dir)),
            "registry": entry("registry", self.registry.as_ref()),
            "runtime_library": entry("runtime_library", self.runtime_library.as_ref()),
            "embedding_contract": entry("embedding_contract", self.embedding_contract.as_ref()),
            "downloads": "only `saccade models pull` provisions artifacts; MCP requests can never select a path or trigger a download",
        })
    }

    /// MCP/server guard: a request may only restate the operator-configured
    /// location; any other value is refused.
    pub fn check_request_location(&self, key: &str, requested: &Path) -> Result<()> {
        let configured = match key {
            "dir" | "cache" => Some(&self.dir),
            "registry" => self.registry.as_ref(),
            "library" | "runtime_library" => self.runtime_library.as_ref(),
            "embedding_contract" => self.embedding_contract.as_ref(),
            _ => None,
        };
        let same = configured.is_some_and(|c| {
            let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            canon(c) == canon(requested)
        });
        if same {
            Ok(())
        } else {
            Err(Error::Config(format!(
                "model location `{key}` is operator configuration ({ENV_DIR}, {ENV_REGISTRY}, {ENV_RUNTIME} or [models] in the saccade config file); a request cannot choose it"
            )))
        }
    }
}

static NOTICED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Print one deprecation notice per spelling per process; behaviour is unchanged.
pub fn deprecated(spelling: &str, replacement: &str) {
    let first = NOTICED.lock().is_ok_and(|mut seen| {
        if seen.iter().any(|s| s == spelling) {
            false
        } else {
            seen.push(spelling.to_owned());
            true
        }
    });
    if first {
        eprintln!(
            "saccade: deprecated: {spelling} still works but is superseded by {replacement}; it will not be removed before {REMOVAL_NOT_BEFORE}"
        );
    }
}

/// The deprecation text for the legacy download flags
/// (`--download-model`, `--allow-download`, `allow_download=`).
pub fn deprecated_download_flag(flag: &str) {
    deprecated(
        flag,
        "`saccade models pull <id>` (the one provisioning verb)",
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn defaults_follow_xdg_cache() {
        let base = std::env::temp_dir();
        let (home, cache) = (base.join("nowhere-home"), base.join("nowhere-cache"));
        let c = ModelConfig::resolve_with(&env_of(&[
            ("HOME", home.to_str().unwrap()),
            ("XDG_CACHE_HOME", cache.to_str().unwrap()),
        ]))
        .unwrap();
        assert_eq!(c.dir, cache.join("saccade").join("models"));
        assert_eq!(c.source("dir"), Source::Default);
        assert!(c.registry.is_none() && c.config_file.is_none());
    }

    #[test]
    fn file_then_env_then_flag_precedence() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        let abs = d.path().join("abs-models.json");
        // A TOML literal string keeps Windows backslashes intact.
        std::fs::write(
            &cfg,
            format!(
                "[models]\ndir = \"store\"\nregistry = '{}'\n",
                abs.display()
            ),
        )
        .unwrap();
        let cfg_s = cfg.to_str().unwrap();
        let from_file = ModelConfig::resolve_with(&env_of(&[(ENV_CONFIG, cfg_s)])).unwrap();
        assert_eq!(from_file.dir, d.path().join("store"));
        assert_eq!(from_file.source("dir"), Source::File(cfg.clone()));
        assert_eq!(from_file.registry, Some(abs.clone()));

        let from_env = ModelConfig::resolve_with(&env_of(&[
            (ENV_CONFIG, cfg_s),
            (ENV_DIR_LEGACY, "/legacy/env"),
        ]))
        .unwrap();
        assert_eq!(from_env.dir, PathBuf::from("/legacy/env"));
        assert_eq!(from_env.source("dir"), Source::Env(ENV_DIR_LEGACY.into()));
        let preferred = ModelConfig::resolve_with(&env_of(&[
            (ENV_CONFIG, cfg_s),
            (ENV_DIR_LEGACY, "/legacy/env"),
            (ENV_DIR, "/new/env"),
        ]))
        .unwrap();
        assert_eq!(preferred.dir, PathBuf::from("/new/env"));

        let flagged = from_env.with_overrides(Overrides {
            dir: Some(Path::new("/flag")),
            ..Overrides::default()
        });
        assert_eq!(flagged.dir, PathBuf::from("/flag"));
        assert_eq!(flagged.source("dir"), Source::Flag);
        assert_eq!(flagged.registry, Some(abs.clone()));
    }

    #[test]
    fn unknown_keys_and_bad_types_are_errors() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        for body in ["[models]\ncache = \"x\"\n", "[models]\ndir = 3\n"] {
            std::fs::write(&cfg, body).unwrap();
            assert!(
                ModelConfig::resolve_with(&env_of(&[(ENV_CONFIG, cfg.to_str().unwrap())])).is_err()
            );
        }
    }

    #[test]
    fn requests_may_only_restate_the_operator_location() {
        let c = ModelConfig::resolve_with(&env_of(&[(ENV_DIR, "/operator/models")])).unwrap();
        assert!(
            c.check_request_location("cache", Path::new("/operator/models"))
                .is_ok()
        );
        let err = c
            .check_request_location("cache", Path::new("/elsewhere"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("operator configuration"), "{err}");
        assert!(
            c.check_request_location("registry", Path::new("/operator/models"))
                .is_err()
        );
    }

    #[test]
    fn python_and_cli_defaults_agree() {
        // The media analyzer (Python default) and every CLI command call the same resolver.
        let resolved = ModelConfig::resolve().unwrap().dir;
        assert_eq!(resolved, crate::media::default_model_dir());
    }
}
