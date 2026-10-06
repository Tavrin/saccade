//! Independent-repeat timing spread and baseline shift for ablation tables.
use crate::{
    Result,
    paired_stats::{Generator, quantile},
};
use serde::{Deserialize, Serialize};
/// Per-arm repeat evidence. Unpaired intervals never imply matched acquisition.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// All accepted repeat timings in milliseconds.
    pub samples_ms: Vec<f64>,
    /// Median timing.
    pub median_ms: f64,
    /// Interquartile range.
    pub iqr_ms: f64,
    /// Independent-sample Hodges–Lehmann shift against baseline.
    pub hl_delta_ms: f64,
    /// Seeded independent-arm bootstrap CI, unavailable without two repeats per arm.
    pub interval_ms: Option<[f64; 2]>,
    /// Explicit method and independence limitation.
    pub method: String,
}
fn shift(a: &[f64], b: &[f64]) -> Result<f64> {
    let mut diffs = a
        .iter()
        .flat_map(|x| b.iter().map(move |y| x - y))
        .collect::<Vec<_>>();
    diffs.sort_by(f64::total_cmp);
    quantile(&diffs, 0.5)
}
/// Summarize own repeats and bootstrap each arm separately against the baseline.
pub fn summarize(base: &[f64], arm: &[f64]) -> Result<Summary> {
    if base.is_empty()
        || arm.is_empty()
        || base.len() > 128
        || arm.len() > 128
        || base.iter().chain(arm).any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err(crate::Error::Config(
            "ablation timings need 1..128 positive finite repeats per arm".into(),
        ));
    }
    let mut sorted = arm.to_vec();
    sorted.sort_by(f64::total_cmp);
    let interval_ms = if base.len() > 1 && arm.len() > 1 {
        let mut rng = Generator::new(11);
        let mut draws = Vec::new();
        for _ in 0..2048 {
            let a = (0..arm.len())
                .map(|_| rng.index(arm.len()).map(|i| arm[i]))
                .collect::<Result<Vec<_>>>()?;
            let b = (0..base.len())
                .map(|_| rng.index(base.len()).map(|i| base[i]))
                .collect::<Result<Vec<_>>>()?;
            draws.push(shift(&a, &b)?);
        }
        draws.sort_by(f64::total_cmp);
        Some([quantile(&draws, 0.025)?, quantile(&draws, 0.975)?])
    } else {
        None
    };
    Ok(Summary{samples_ms:arm.to_vec(),median_ms:quantile(&sorted,0.5)?,iqr_ms:quantile(&sorted,0.75)?-quantile(&sorted,0.25)?,hl_delta_ms:shift(arm,base)?,interval_ms,method:"unpaired_two_sample_HL_cross_differences; independent_repeat_percentile_bootstrap_95pct_seed11_2048; acquisition independence is external; one-repeat CI unavailable".into()})
}
/// Collect all complete repeats; missing timing means no summary, never a partial spread.
pub fn collect(paths: &[std::path::PathBuf], name: &str) -> Result<Option<Vec<f64>>> {
    let mut times = Vec::new();
    for p in paths {
        let Some(perf) = crate::perf::CapturePerf::read(p, name)? else {
            return Ok(None);
        };
        times.push(perf.frame.value);
    }
    Ok(Some(times))
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn four_arm_lab_processing_effects_rank_and_have_own_spread() {
        let base = [10., 10.2, 9.8];
        let mut rows = [
            vec![9., 9.2, 8.8],
            vec![6., 6.1, 5.9],
            vec![11., 11.4, 10.6],
            vec![10., 10.2, 9.8],
        ]
        .iter()
        .map(|a| summarize(&base, a).unwrap())
        .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.hl_delta_ms.total_cmp(&b.hl_delta_ms));
        assert_eq!(
            rows.iter().map(|r| r.median_ms).collect::<Vec<_>>(),
            vec![6., 9., 10., 11.]
        );
        assert!(rows[0].interval_ms.unwrap()[1] < -3.);
        assert!(rows[3].interval_ms.unwrap()[0] > 0.);
        assert!(rows[3].iqr_ms > rows[0].iqr_ms);
    }
}

#[cfg(all(test, feature = "graphics"))]
#[allow(clippy::unwrap_used)]
mod pipeline_tests {
    use serde_json::json;
    #[test]
    fn four_arm_repeat_table_uses_every_repeat_and_strict_lab_fingerprints() {
        let tmp = tempfile::tempdir().unwrap();
        let map = tmp.path().join("map.toml");
        std::fs::write(&map,"compare='mapped_only'\nabsent='value'\n[fields.'run.mode']\npath='mode'\n[subtrees.'run.env']\npath='options'\n[[readiness]]\nname='sample_ready'\nreached={path='ready'}\nobserved={path='ready'}\n").unwrap();
        let capture = |label: &str, variant: u32, value: f64, color: u8| {
            let path = tmp.path().join(label);
            std::fs::create_dir(&path).unwrap();
            image::RgbaImage::from_pixel(32, 32, image::Rgba([color, color, color, 255]))
                .save(path.join("specimen.png"))
                .unwrap();
            std::fs::write(
                path.join(crate::meta::DEFAULT_META_NAME),
                serde_json::to_vec(
                    &json!({"mode":"fixed","options":{"algorithm":variant},"ready":true}),
                )
                .unwrap(),
            )
            .unwrap();
            std::fs::write(path.join("saccade-perf.json"),serde_json::to_vec(&json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":value,"samples":2,"stat":"mean"},"terms":[],"counters":{}})).unwrap()).unwrap();
            path
        };
        let bases = [
            capture("reference-1", 0, 10., 90),
            capture("reference-2", 0, 10.2, 90),
        ];
        let groups = [
            ("slow", 11., 1, 100),
            ("fast", 6., 2, 120),
            ("null", 10., 3, 90),
            ("moderate", 9., 4, 95),
        ]
        .into_iter()
        .map(|(label, ms, v, c)| {
            (
                label.into(),
                vec![
                    capture(&format!("{label}-1"), v, ms, c),
                    capture(&format!("{label}-2"), v, ms + 0.2, c),
                ],
            )
        })
        .collect::<Vec<_>>();
        let mut cfg = crate::config::RunConfig::default();
        cfg.meta.require_valid_arms = true;
        cfg.meta.fingerprint_map = Some(map);
        cfg.meta.intended = vec!["run.env.algorithm".into()];
        cfg.perf.gpu_clocks_not_applicable = true;
        cfg.spatial = Some(Default::default());
        let out = tmp.path().join("table");
        let table = crate::ablate::run_repeats(&bases, &groups, &out, &cfg, 3).unwrap();
        assert_eq!(
            table
                .arms
                .iter()
                .map(|a| a.label.as_str())
                .collect::<Vec<_>>(),
            vec!["fast", "moderate", "null", "slow"]
        );
        assert_eq!(
            table.arms[0].timing.as_ref().unwrap().samples_ms,
            vec![6., 6.2]
        );
        assert_eq!(
            table.base_timing.as_ref().unwrap().samples_ms,
            vec![10., 10.2]
        );
        assert!(table.arms[0].timing.as_ref().unwrap().interval_ms.unwrap()[1] < -3.);
        assert_eq!(
            table.arms[2].image_effect,
            crate::ablate::ImageEffect::Identical
        );
        assert!(!table.arms[0].image_classes.is_empty());
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join(crate::ablate::ABLATE_FILE)).unwrap())
                .unwrap();
        assert!(report["report_id"].as_str().unwrap().starts_with("sha256:"));
        assert!(out.join("reports/index.jsonl").is_file());
        assert!(
            std::fs::read_to_string(out.join("ablation.md"))
                .unwrap()
                .contains("IQR ms")
        );
        assert!(
            std::fs::read_to_string(out.join("index.html"))
                .unwrap()
                .contains("Median ms")
        );
        let invalid = &groups[0].1[1];
        std::fs::write(
            invalid.join(crate::meta::DEFAULT_META_NAME),
            serde_json::to_vec(&json!({"mode":"fixed","options":{"algorithm":1},"ready":false}))
                .unwrap(),
        )
        .unwrap();
        let err = crate::ablate::run_repeats(&bases, &groups, &tmp.path().join("invalid"), &cfg, 3)
            .unwrap_err();
        assert!(matches!(err, crate::Error::InvalidComparison(_)));
        assert!(!tmp.path().join("invalid").exists());
    }
}

#[cfg(all(test, feature = "graphics"))]
#[allow(clippy::unwrap_used)]
mod external_inventory_tests {
    use serde::Deserialize;
    /// Supplied entirely outside the source tree; no partner vocabulary is compiled in.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Inventory {
        baseline: Vec<std::path::PathBuf>,
        arms: Vec<Arm>,
        fingerprint_map: std::path::PathBuf,
        intended: Vec<String>,
        out: std::path::PathBuf,
        source_refs: Vec<String>,
        #[serde(default)]
        subtree_mappings: std::collections::BTreeMap<String, crate::arms::Source>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Arm {
        label: String,
        repeats: Vec<std::path::PathBuf>,
        valid: bool,
        expected_delta_ms: Option<f64>,
        tolerance_ms: f64,
    }
    #[test]
    #[ignore = "external: read-only source-object coverage supplied explicitly"]
    fn read_only_real_capture_subtrees_list_all_producer_configuration_keys() {
        let path = std::env::var_os("SACCADE_W11_REAL_INVENTORY")
            .map(std::path::PathBuf::from)
            .unwrap();
        let inventory: Inventory =
            serde_json::from_slice(&crate::evidence_quality::read(&path, 1 << 20).unwrap())
                .unwrap();
        assert!(
            !inventory.subtree_mappings.is_empty(),
            "declare external subtree mappings"
        );
        let mut map = crate::arms::FingerprintMap::read(&inventory.fingerprint_map).unwrap();
        for (prefix, mut source) in inventory.subtree_mappings {
            map.fields
                .retain(|key, _| key != &prefix && !key.starts_with(&format!("{prefix}.")));
            source.subtree = true;
            map.fields.insert(prefix, source);
        }
        std::fs::create_dir_all(&inventory.out).unwrap();
        let mapping = inventory.out.join("subtree-map.json");
        std::fs::write(&mapping, serde_json::to_vec_pretty(&map).unwrap()).unwrap();
        let options = crate::meta::MetaOptions {
            fingerprint_map: Some(mapping),
            ..Default::default()
        };
        let check =
            crate::arms::check_paths(&inventory.baseline[0], &inventory.baseline[0], &options)
                .unwrap();
        assert_eq!(check.exit_code, 0);
        assert!(
            check
                .mapped_keys
                .iter()
                .any(|key| key.starts_with("run.env."))
        );
        crate::report_links::write(&inventory.out.join("real-subtree-check.json"), &check).unwrap();
    }
    #[test]
    #[ignore = "external: read-only capture inventory supplied explicitly"]
    fn read_only_real_capture_inventory_has_the_declared_ranking_and_refusals() {
        let path = std::env::var_os("SACCADE_W11_REAL_INVENTORY")
            .map(std::path::PathBuf::from)
            .unwrap();
        let inventory: Inventory =
            serde_json::from_slice(&crate::evidence_quality::read(&path, 1 << 20).unwrap())
                .unwrap();
        assert!(inventory.baseline.len() >= 2 && inventory.arms.len() >= 4);
        let mut cfg = crate::config::RunConfig::default();
        cfg.meta.require_valid_arms = true;
        cfg.meta.fingerprint_map = Some(inventory.fingerprint_map);
        cfg.meta.intended = inventory.intended;
        cfg.spatial = Some(Default::default());
        let _links = crate::report_links::scope(
            inventory.source_refs,
            Some(inventory.out.join("reports/index.jsonl")),
        )
        .unwrap();
        let mut refusals = Vec::new();
        let mut groups = Vec::new();
        for arm in &inventory.arms {
            assert!(!arm.repeats.is_empty());
            for repeat in &arm.repeats {
                let result =
                    crate::arms::check_paths(&inventory.baseline[0], repeat, &cfg.meta).unwrap();
                assert_eq!(
                    result.exit_code == 0,
                    arm.valid,
                    "{} readiness/identity",
                    arm.label
                );
                if !arm.valid {
                    refusals.push(serde_json::json!({"label":arm.label,"check":result}));
                }
            }
            if arm.valid {
                groups.push((arm.label.clone(), arm.repeats.clone()));
            }
        }
        std::fs::create_dir_all(&inventory.out).unwrap();
        let table = crate::ablate::run_repeats(
            &inventory.baseline,
            &groups,
            &inventory.out.join("table"),
            &cfg,
            8,
        )
        .unwrap();
        let mut comparison = Vec::new();
        for row in &table.arms {
            let expected = inventory
                .arms
                .iter()
                .find(|a| a.label == row.label)
                .unwrap();
            let timing = row.timing.as_ref().unwrap();
            if let Some(delta) = expected.expected_delta_ms {
                assert!(
                    (timing.hl_delta_ms - delta).abs() <= expected.tolerance_ms,
                    "{}: got {} vs {}",
                    row.label,
                    timing.hl_delta_ms,
                    delta
                );
            }
            comparison.push(serde_json::json!({"label":row.label,"saccade_delta_ms":timing.hl_delta_ms,"reference_delta_ms":expected.expected_delta_ms,"median_ms":timing.median_ms,"iqr_ms":timing.iqr_ms,"ci_ms":timing.interval_ms,"descriptive_rank":row.timing_rank,"image_classes":row.image_classes,"performance_comparability":row.perf_diff.as_ref().map(|p|p.comparability)}));
        }
        assert!(
            table
                .arms
                .windows(2)
                .all(|w| w[0].timing.as_ref().unwrap().hl_delta_ms
                    <= w[1].timing.as_ref().unwrap().hl_delta_ms)
        );
        std::fs::write(inventory.out.join("acceptance.json"),serde_json::to_vec_pretty(&serde_json::json!({"inventory_sha256":crate::run::sha256_file(&path).unwrap(),"comparison":comparison,"refusals":refusals})).unwrap()).unwrap();
    }
}
