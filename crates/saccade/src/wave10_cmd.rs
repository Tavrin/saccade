//! Field-feedback command boundary, keeping core evidence reusable.
use crate::agent::CliError;
use std::path::PathBuf;
pub(crate) const RENDER_SCHEMA: &str = "saccade-render-evidence.v1";
#[derive(clap::Args, Default)]
#[group(id = "FieldEvidenceArgs")]
pub(crate) struct CompareArgs {
    /// Export native FLIP and tile grids as float32 NPY/EXR with a JSON index.
    #[arg(long)]
    pub export_maps: bool,
    /// Require an actual ID layer or nonempty mask scope.
    #[arg(long)]
    pub require_scope: bool,
    /// Same-arm repeat files or run directories (2..32); enables noise-aware deciding evidence.
    #[arg(long,num_args=2..,value_name="REPEAT")]
    pub noise_from: Vec<PathBuf>,
    /// Generic screen-space dump filename relative to each capture; used when no layer manifest exists.
    #[arg(long)]
    #[arg(alias = "mask-from-dump")]
    pub mask_dump: Option<String>,
    /// Named layer and native predicate, NAME=id=1,2 or NAME=label=pattern.
    #[arg(long, allow_hyphen_values = true)]
    pub mask_layer: Option<String>,
    /// Required occupancy from NAME=predicate[:MIN_PIXELS] or mask:FILE[:MIN_PIXELS].
    #[arg(long = "require-effect")]
    pub require_effect: Vec<String>,
    /// Per-ID rows and diagnostic crops retained, at most 32.
    #[arg(long)]
    pub id_top: Option<usize>,
    /// Declared normalized luminance threshold for colour per-ID statistics.
    #[arg(long)]
    pub id_threshold: Option<f64>,
}
impl CompareArgs {
    pub(crate) fn requested(&self) -> bool {
        self.export_maps
            || self.require_scope
            || !self.noise_from.is_empty()
            || self.mask_layer.is_some()
            || !self.require_effect.is_empty()
            || self.mask_dump.is_some()
            || self.id_top.is_some()
            || self.id_threshold.is_some()
    }
    pub(crate) fn apply(&self, cfg: &mut saccade_core::config::RunConfig) -> Result<(), CliError> {
        cfg.field.export_maps |= self.export_maps;
        cfg.field.require_scope |= self.require_scope;
        if !self.noise_from.is_empty() {
            cfg.field.noise_from.clone_from(&self.noise_from);
        }
        if let Some(top) = self.id_top {
            cfg.field.top = top;
        }
        if let Some(threshold) = self.id_threshold {
            cfg.field.threshold = threshold;
        }
        if let Some(dump) = &self.mask_dump {
            let p = cfg.layers.get_or_insert_with(Default::default);
            p.dump = Some(dump.clone());
            p.scope = Some(saccade_core::evidence_quality::layers::LayerScope {
                layer: "derived_instances".into(),
                predicate: saccade_core::evidence_quality::layers::Predicate::Mask,
                mode: Default::default(),
            });
            cfg.mask_mode = saccade_core::compare::MaskMode::Neutralize;
        }
        crate::wave11_cmd::apply_masks(cfg, self.mask_layer.as_deref(), &self.require_effect)?;
        if (cfg.field.export_maps || !cfg.field.noise_from.is_empty()) && cfg.buffers.is_empty() {
            cfg.spatial.get_or_insert_with(Default::default);
        }
        if !cfg.field.noise_from.is_empty()
            && let Some(p) = &mut cfg.spatial
        {
            p.decide = true;
        }
        cfg.field.validate()?;
        Ok(())
    }
}
#[derive(clap::Args)]
pub(crate) struct RenderArgs {
    baseline: PathBuf,
    candidate: PathBuf,
    #[arg(long, default_value = "render-evidence")]
    out: PathBuf,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    strict: crate::arms_cmd::StrictArgs,
    #[arg(long = "intended-variable")]
    intended: Vec<String>,
    #[command(flatten)]
    field: CompareArgs,
}
pub(crate) fn render(args: RenderArgs) -> Result<u8, CliError> {
    let mut cfg = crate::load_config(args.config.as_deref())?;
    args.strict.apply(&mut cfg.meta);
    cfg.meta.intended.extend(args.intended);
    args.field.apply(&mut cfg)?;
    if cfg.buffers.is_empty() {
        cfg.spatial.get_or_insert_with(Default::default).decide = true;
    }
    let report = saccade_core::run::run(&args.baseline, &args.candidate, &args.out, &cfg)?;
    if args.json {
        let scopes = report
            .entries
            .iter()
            .filter_map(|e| e.field_evidence.as_ref().map(|f| f.scope.as_str()))
            .collect::<std::collections::BTreeSet<_>>();
        let value = serde_json::json!({"schema":RENDER_SCHEMA,"scope":if scopes.len()==1{scopes.first().copied().unwrap_or("unknown")}else{"mixed"},"verdict":if report.is_regression(){"fail"}else{"pass"},"counts":report.totals,"allowed_unreached":report.config.meta.arm_validation.as_ref().map(|a|a.allowed_unreached.iter().take(4).collect::<Vec<_>>()),"artifact":crate::local_cmd::reference(&args.out.join(saccade_core::report::REPORT_FILE_NAME))?,"entries":report.entries.iter().take(3).map(|e|serde_json::json!({"name":crate::local_cmd::short(&e.name,120),"status":e.status,"scope":e.field_evidence.as_ref().map(|f|&f.scope),"class":e.field_evidence.as_ref().map(|f|&f.class),"top_ids":e.field_evidence.as_ref().map(|f|f.per_id.iter().take(3).map(|r|serde_json::json!({"id":r.id,"pixels":r.pixels,"shift":r.shift,"relative":r.relative})).collect::<Vec<_>>()),"error":e.error})).collect::<Vec<_>>()});
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&crate::local_cmd::bounded(value, 4096)?)?
        ))?;
    } else {
        crate::emit_run(&report, &args.out, false, false)?;
    }
    Ok(u8::from(report.is_regression()))
}
#[derive(clap::Args)]
pub(crate) struct NoiseBuildArgs {
    /// 2..32 repeats from the same arm, files or run directories.
    #[arg(required=true,num_args=2..)]
    pub repeats: Vec<PathBuf>,
    #[arg(long, default_value = "repeat-noise.json")]
    pub out: PathBuf,
    #[arg(long, default_value_t = 32)]
    pub tile_size: u32,
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub strict: crate::arms_cmd::StrictArgs,
}
pub(crate) fn noise_build(args: NoiseBuildArgs) -> Result<u8, CliError> {
    let mut cfg = saccade_core::config::RunConfig::default();
    args.strict.apply(&mut cfg.meta);
    for repeat in args.repeats.iter().skip(1) {
        saccade_core::arms::enforce(&args.repeats[0], repeat, &cfg)?;
    }
    let report =
        saccade_core::evidence_quality::repeat_noise::build_runs(&args.repeats, args.tile_size)?;
    crate::general_cmd::write_new(&args.out, &serde_json::to_value(&report)?)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&report)?))?;
    } else {
        crate::emit(&format!(
            "repeat noise: {} entries; method {}; wrote {}\n",
            report.entries.len(),
            report.method,
            args.out.display()
        ))?;
    }
    Ok(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use clap::Parser;
    #[test]
    fn field_command_arguments_parse_without_changing_legacy_noise() {
        for args in [
            vec!["saccade", "noise", "a", "b", "--json"],
            vec!["saccade", "noise", "build", "a", "b", "--json"],
            vec![
                "saccade",
                "render-evidence",
                "a",
                "b",
                "--noise-from",
                "r1",
                "r2",
                "--allow-unreached",
                "warmup",
                "--json",
            ],
            vec![
                "saccade",
                "compare",
                "a",
                "b",
                "--export-maps",
                "--require-scope",
                "--json",
            ],
        ] {
            assert!(crate::Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
        }
    }
    #[test]
    #[cfg(not(feature = "local-models"))]
    fn missing_model_dependency_precedes_image_read() {
        let cli = crate::Cli::try_parse_from([
            "saccade",
            "locate",
            "nonexistent-image.png",
            "foreground",
        ])
        .unwrap();
        let err = crate::dispatch(cli.command, false).unwrap_err();
        assert!(err.message.contains("fix:"));
        assert!(err.message.contains("local-models"));
    }
    #[test]
    #[cfg(not(feature = "ocr"))]
    fn missing_ocr_dependency_precedes_image_read() {
        let cli = crate::Cli::try_parse_from([
            "saccade",
            "text",
            "nonexistent-a.png",
            "nonexistent-b.png",
        ])
        .unwrap();
        let err = crate::dispatch(cli.command, false).unwrap_err();
        assert!(err.message.contains("fix:"));
        assert!(err.message.contains("ocr build"));
    }
}
