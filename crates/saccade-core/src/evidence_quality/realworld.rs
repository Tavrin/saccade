//! Opt-in, generic read-only pair inventory evaluator; source data stays outside the repo.
use super::*;
use serde::Deserialize;
use std::path::PathBuf;
#[derive(Deserialize)]
struct Pair {
    id: String,
    baseline: PathBuf,
    candidate: PathBuf,
    expected: Option<String>,
    #[serde(default)]
    background: bool,
}
#[derive(Deserialize)]
struct Inventory {
    pairs: Vec<Pair>,
    out: PathBuf,
}
#[test]
fn read_only_checks() {
    let Some(path) = std::env::var_os("WAVE9_READONLY_INVENTORY") else {
        return;
    };
    let inventory: Inventory =
        serde_json::from_slice(&read(std::path::Path::new(&path), 1 << 20).unwrap()).unwrap();
    assert!(
        !inventory.out.exists(),
        "read-only output must be a new scratch directory"
    );
    std::fs::create_dir_all(&inventory.out).unwrap();
    let mut results = Vec::new();
    let mut matches = 0;
    for (index, pair) in inventory.pairs.iter().enumerate() {
        let before_b = crate::localized::digest(&read(&pair.baseline, 128 << 20).unwrap());
        let before_c = crate::localized::digest(&read(&pair.candidate, 128 << 20).unwrap());
        let policy = spatial::Policy {
            background: pair
                .background
                .then_some(spatial::Background::Luminance { max: 0.001 }),
            ..Default::default()
        };
        let config = crate::config::RunConfig {
            spatial: Some(policy),
            diagnostics: crate::diagnostics::DiagnosticsConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let out = inventory.out.join(format!("pair-{index:03}"));
        let report = crate::run::run(&pair.baseline, &pair.candidate, &out, &config).unwrap();
        crate::render::render_html(&report, &out).unwrap();
        let entry = &report.entries[0];
        let spatial = entry
            .spatial
            .as_ref()
            .unwrap_or_else(|| panic!("pair {}: {:?}", pair.id, entry.error));
        let passed = pair
            .expected
            .as_ref()
            .is_none_or(|expected| match expected.as_str() {
                "benign" => matches!(
                    spatial.class,
                    crate::diagnostics::ChangeClass::TextureNoiseOnly
                        | crate::diagnostics::ChangeClass::Identical
                ),
                "genuine" => !matches!(
                    spatial.class,
                    crate::diagnostics::ChangeClass::TextureNoiseOnly
                        | crate::diagnostics::ChangeClass::Identical
                ),
                _ => false,
            });
        matches += usize::from(passed);
        results.push(serde_json::json!({"id":pair.id,"expected":pair.expected,"label_match":passed,"class":spatial.class,"scales":spatial.scales,"detail_energy_ratio":spatial.detail_energy_ratio,"absolute_shifted_tiles":spatial.absolute_shifted_tiles,"absolute_shifted_share":spatial.absolute_shifted_share,"relative_shifted_share":spatial.relative_shifted_share,"coverage_ratio":spatial.coverage_ratio,"silhouette_disagreement":spatial.silhouette_disagreement,"regions":spatial.regions,"gallery":entry.gallery,"baseline_sha256":before_b,"candidate_sha256":before_c,"report":out.join(crate::report::REPORT_FILE_NAME)}));
        assert_eq!(
            before_b,
            crate::localized::digest(&read(&pair.baseline, 128 << 20).unwrap())
        );
        assert_eq!(
            before_c,
            crate::localized::digest(&read(&pair.candidate, 128 << 20).unwrap())
        );
    }
    std::fs::write(inventory.out.join("summary.json"),serde_json::to_vec_pretty(&serde_json::json!({"policy_version":spatial::POLICY_VERSION,"thresholds":spatial::Policy::default(),"pairs":results,"label_matches":matches,"total":inventory.pairs.len()})).unwrap()).unwrap();
    assert_eq!(
        matches,
        inventory.pairs.len(),
        "held-out label mismatch: inspect summary.json; do not retune thresholds"
    );
}
