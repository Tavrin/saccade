//! `flipdiff runs REF_DIR RUN_DIR... [--json] [--out DIR]`: the run overview
//! of `flipdiff serve`, without a server.

use std::path::{Path, PathBuf};

use flipdiff_core::runs::{
    NoAssets, Pairing, RunInput, RunsModel, RunsOptions, overview, unique_labels, write_static,
};

use crate::agent::CliError;

/// Where `flipdiff runs` writes the static page by default.
pub const DEFAULT_OUT: &str = "runs";

/// What `flipdiff runs` was asked.
pub struct RunsRequest {
    /// The reference run.
    pub ref_dir: PathBuf,
    /// The runs compared against it.
    pub run_dirs: Vec<PathBuf>,
    /// Labels, reference first.
    pub labels: Option<Vec<String>>,
    /// Print the `flipdiff-runs.v1` JSON instead of text.
    pub json: bool,
    /// Directory of the static page.
    pub out: Option<PathBuf>,
    /// Pair the images of every run with the reference's by sorted position
    /// instead of by name.
    pub by_position: bool,
    /// Measurement settings.
    pub opts: RunsOptions,
}

fn inputs(req: &RunsRequest) -> Result<(RunInput, Vec<RunInput>), CliError> {
    let dirs: Vec<PathBuf> = std::iter::once(&req.ref_dir)
        .chain(&req.run_dirs)
        .cloned()
        .collect();
    let labels = match &req.labels {
        Some(l) if l.len() != dirs.len() => {
            return Err(CliError::usage(format!(
                "--labels takes one label per directory ({}), got {}",
                dirs.len(),
                l.len()
            )));
        }
        Some(l) => l.clone(),
        None => {
            let names: Vec<PathBuf> = dirs
                .iter()
                .map(|d| flipdiff_core::explain::absolute(d))
                .collect();
            unique_labels(&names)
        }
    };
    let mut all: Vec<RunInput> = dirs
        .into_iter()
        .zip(labels)
        .map(|(dir, label)| RunInput {
            display: dir.display().to_string(),
            dir,
            label,
            pairing: Pairing::Name,
        })
        .collect();
    let reference = all.remove(0);
    if req.by_position {
        for r in &mut all {
            r.pairing = Pairing::Position;
        }
    }
    Ok((reference, all))
}

/// The cache directory shared with `flipdiff serve`.
fn cache_dir() -> PathBuf {
    flipdiff_core::serve::default_cache_dir().join("runs")
}

/// The text summary: one line per run.
fn text(model: &RunsModel, page: Option<&Path>) -> String {
    let mut out = format!(
        "reference {} ({} images)\n",
        model.reference.label,
        model.reference.images.len()
    );
    for r in &model.runs {
        out.push_str(&format!("  {}: {}\n", r.label, r.summary));
        if r.no_visible_effect {
            out.push_str("    No visible effect: this run changed nothing.\n");
        }
        if r.mismatch && r.pairing == "name" {
            out.push_str(&format!(
                "    {} of {} file names match; try --pair-by-position\n",
                r.name_matches,
                model.reference.images.len()
            ));
        }
    }
    if let Some(p) = page {
        out.push_str(&format!("wrote {}\n", p.join("index.html").display()));
    }
    out
}

/// The model as lean JSON for agents: no thumbnails or heatmaps exist, so the
/// fields that would be `null` are left out.
pub fn lean_value(model: &RunsModel) -> Result<serde_json::Value, CliError> {
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                m.retain(|_, x| !x.is_null());
                m.values_mut().for_each(strip);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(model)?;
    strip(&mut value);
    Ok(value)
}

/// The text summary of an overview (one line per run).
pub fn summary_text(model: &RunsModel) -> String {
    text(model, None)
}

/// Runs the command; the exit code is 0 (an overview is not a verdict).
pub fn run(req: &RunsRequest) -> Result<u8, CliError> {
    let (reference, runs) = inputs(req)?;
    let cache = cache_dir();
    // JSON alone writes no files; an explicit `--out` also writes the page.
    let write_page = !req.json || req.out.is_some();
    let model = if write_page {
        let out = req
            .out
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
        write_static(&reference, &runs, &req.opts, &cache, &out)?
    } else {
        overview(&reference, &runs, &req.opts, Some(&cache), &NoAssets)?
    };
    if req.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&model)?))?;
    } else {
        let out = req
            .out
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
        crate::emit(&text(&model, Some(&out)))?;
    }
    Ok(0)
}
