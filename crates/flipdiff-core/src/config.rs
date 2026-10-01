//! Run configuration and the `flipdiff.toml` file format.

use std::path::Path;

use globset::{Glob, GlobBuilder};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::report::Metric;

/// A per-path override of the default threshold and/or metric.
#[derive(Debug, Clone, PartialEq)]
pub struct Override {
    /// Glob matched, case-insensitively, against the `/`-separated image name.
    /// `*` does not cross `/`; `**` does.
    pub glob: String,
    /// Replacement threshold, if any.
    pub threshold: Option<f64>,
    /// Replacement metric, if any.
    pub metric: Option<Metric>,
}

/// Settings for [`crate::run::run`].
#[derive(Debug, Clone, PartialEq)]
pub struct RunConfig {
    /// Default pass threshold.
    pub default_threshold: f64,
    /// Default deciding metric.
    pub default_metric: Metric,
    /// FLIP observer setting (pixels per degree).
    pub pixels_per_degree: f32,
    /// Whether a `new` entry counts as a regression.
    pub fail_on_new: bool,
    /// Globs of image names to skip entirely.
    pub ignore: Vec<String>,
    /// Per-path overrides; the first matching one wins, field by field.
    pub overrides: Vec<Override>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            default_threshold: 0.01,
            default_metric: Metric::Mean,
            pixels_per_degree: nv_flip::DEFAULT_PIXELS_PER_DEGREE,
            fail_on_new: false,
            ignore: Vec::new(),
            overrides: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    threshold: Option<f64>,
    metric: Option<Metric>,
    fail_on_new: Option<bool>,
    ppd: Option<f32>,
    #[serde(default)]
    ignore: Vec<String>,
    #[serde(default, rename = "override")]
    overrides: Vec<FileOverride>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileOverride {
    glob: String,
    threshold: Option<f64>,
    metric: Option<Metric>,
}

pub(crate) fn compile_glob(pattern: &str) -> Result<globset::GlobMatcher> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .case_insensitive(true)
        .build()
        .map(|g: Glob| g.compile_matcher())
        .map_err(|e| Error::Config(format!("invalid glob {pattern:?}: {e}")))
}

impl RunConfig {
    /// Parses a `flipdiff.toml` file (see `SPEC.md`). Unknown keys are errors.
    pub fn from_toml_file(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            context: format!("reading config {}", path.display()),
            source,
        })?;
        Self::from_toml_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display())))
    }

    /// Parses `flipdiff.toml` contents.
    pub fn from_toml_str(text: &str) -> Result<Self> {
        let file: FileConfig = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        let mut cfg = Self::default();
        if let Some(v) = file.threshold {
            cfg.default_threshold = v;
        }
        if let Some(v) = file.metric {
            cfg.default_metric = v;
        }
        if let Some(v) = file.fail_on_new {
            cfg.fail_on_new = v;
        }
        if let Some(v) = file.ppd {
            cfg.pixels_per_degree = v;
        }
        cfg.ignore = file.ignore;
        cfg.overrides = file
            .overrides
            .into_iter()
            .map(|o| Override {
                glob: o.glob,
                threshold: o.threshold,
                metric: o.metric,
            })
            .collect();
        cfg.validate()?;
        Ok(cfg)
    }

    /// Checks that every glob compiles, thresholds are finite and pixels per
    /// degree is finite and positive.
    pub fn validate(&self) -> Result<()> {
        if !self.default_threshold.is_finite() {
            return Err(Error::Config("threshold must be finite".into()));
        }
        crate::compare::check_ppd(self.pixels_per_degree)?;
        for g in &self.ignore {
            compile_glob(g)?;
        }
        for o in &self.overrides {
            compile_glob(&o.glob)?;
            if o.threshold.is_some_and(|t| !t.is_finite()) {
                return Err(Error::Config(format!(
                    "override {:?}: threshold must be finite",
                    o.glob
                )));
            }
        }
        Ok(())
    }

    /// Effective `(metric, threshold)` for an image name: the first matching
    /// override wins, field by field over the defaults.
    pub fn effective_for(&self, name: &str) -> (Metric, f64) {
        let mut metric = self.default_metric;
        let mut threshold = self.default_threshold;
        let hit = self
            .overrides
            .iter()
            .find(|o| compile_glob(&o.glob).is_ok_and(|m| m.is_match(name)));
        if let Some(o) = hit {
            if let Some(m) = o.metric {
                metric = m;
            }
            if let Some(t) = o.threshold {
                threshold = t;
            }
        }
        (metric, threshold)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn first_matching_override_wins_field_by_field() {
        let cfg = RunConfig::from_toml_str(
            r#"
            threshold = 0.02
            metric = "mean"
            [[override]]
            glob = "terrain/**"
            threshold = 0.03
            [[override]]
            glob = "terrain/**"
            threshold = 0.5
            metric = "max"
            [[override]]
            glob = "**/*.png"
            metric = "p95"
            "#,
        )
        .expect("parse");
        // First match supplies the threshold only; metric stays the default.
        assert_eq!(cfg.effective_for("terrain/a.png"), (Metric::Mean, 0.03));
        assert_eq!(cfg.effective_for("ui/a.png"), (Metric::P95, 0.02));
        assert_eq!(cfg.effective_for("ui/a.jpg"), (Metric::Mean, 0.02));
    }

    #[test]
    fn globs_ignore_case() {
        let cfg = RunConfig::from_toml_str(
            "ignore = [\"**/*.png\"]\n[[override]]\nglob = \"UI/*.PNG\"\nthreshold = 0.5\n",
        )
        .expect("parse");
        assert_eq!(cfg.effective_for("ui/a.png").1, 0.5);
        assert!(compile_glob("**/*.png").expect("glob").is_match("x/Y.PNG"));
    }
}
