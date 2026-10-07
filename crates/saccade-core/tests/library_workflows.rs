//! Direct Rust command workflows preserve refusal, artifacts and independent policies.
use saccade_core::{
    config::RunConfig,
    report::Mode,
    workflows::{self, CompareRun, IntentOptions, signed_approval::Policy},
};
use std::path::Path;

fn image(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir(dir)?;
    image::RgbImage::from_pixel(32, 32, image::Rgb([30, 50, 90])).save(dir.join("page.png"))?;
    Ok(())
}

#[test]
fn explicit_policies_do_not_install_or_weaken_each_other() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    let base = fixture.path().join("base");
    let cap = fixture.path().join("capture");
    image(&base)?;
    image(&cap)?;
    let required = Policy {
        require_signed_approval: true,
        ..Default::default()
    };
    let off = Policy::default();
    let config = RunConfig::default();
    let intent = IntentOptions::default();
    let out = fixture.path().join("report");
    let options = CompareRun {
        baseline: &base,
        capture: &cap,
        out: &out,
        config: &config,
        policy: &required,
        approved: false,
        verified: None,
        junit: None,
        intent: &intent,
    };
    let refused = workflows::run_compare(&options)
        .err()
        .ok_or("required policy accepted unsigned baseline")?;
    assert_eq!(refused.code, "approval_signature_required");
    assert!(!out.exists());
    let completed = workflows::run_compare(&CompareRun {
        policy: &off,
        ..options
    })?;
    assert!(!completed.is_regression());
    assert!(out.join("evidence.json").is_file());
    assert!(out.join("index.html").is_file());
    // A previous policy-off run does not turn the required policy off.
    assert_eq!(
        workflows::run_compare(&options)
            .err()
            .ok_or("policy changed")?
            .code,
        "approval_signature_required"
    );
    let mut exact = config.clone();
    exact.mode = Mode::Identity;
    let proof_dir = fixture.path().join("proof");
    let proof = workflows::run_prove(&CompareRun {
        config: &exact,
        out: &proof_dir,
        policy: &off,
        ..options
    })?;
    assert_eq!(proof.report.entries[0].bit_identical, Some(true));
    assert!(
        workflows::run_prove(&CompareRun {
            policy: &off,
            ..options
        })
        .is_err()
    );
    Ok(())
}

#[test]
#[cfg(any(unix, windows))]
fn history_and_manifests_are_callable_without_cli() -> Result<(), Box<dyn std::error::Error>> {
    use workflows::{approval, history, manifest};
    let fixture = tempfile::tempdir()?;
    let base = fixture.path().join("base");
    let cap = fixture.path().join("capture");
    image(&base)?;
    image(&cap)?;
    let out = fixture.path().join("report");
    let options = CompareRun {
        baseline: &base,
        capture: &cap,
        out: &out,
        config: &RunConfig::default(),
        policy: &Policy::default(),
        approved: false,
        verified: None,
        junit: None,
        intent: &IntentOptions::default(),
    };
    workflows::run_compare(&options)?;
    let report = out.join(saccade_core::report::REPORT_FILE_NAME);
    let store = fixture.path().join("history");
    assert!(history::record_report(&report, &store, None)?.recorded);
    let duplicate = history::record_report(&report, &store, None)?;
    assert!(!duplicate.recorded);
    let expected = serde_json::json!({"schema":"saccade-history.v1","operation":"record","recorded":false,"reason":"report_already_recorded","report_sha256":duplicate.report_sha256});
    assert_eq!(
        serde_json::to_vec(&serde_json::to_value(&duplicate)?)?,
        serde_json::to_vec(&expected)?
    );
    let result = history::analyze_history(&history::AnalyzeOptions {
        store: &store,
        entry: None,
        drift: true,
        out: None,
        limit: 10,
    })?;
    assert_eq!(result.witness["groups"], 1);
    assert_eq!(result.preview()?["policy_changed"], false);
    assert!(history::analyze_onset(&store, 9).is_err());
    let manifest = manifest::build(&manifest::BuildOptions {
        dir: &out,
        approved_anchor: None,
        last_good: None,
        cases: None,
        policy: options.policy,
    })?;
    manifest::verify(&manifest.path, options.policy)?;
    let bytes = std::fs::read(base.join("page.png"))?;
    let plan_dir = fixture.path().join("approval");
    let plan = approval::run(
        &report,
        &cap,
        &base,
        None,
        approval::Options {
            names: vec!["page.png".into()],
            dry_run: true,
            out: Some(plan_dir),
            ..Default::default()
        },
    )?;
    assert!(plan.dry_run);
    assert_eq!(plan.applied.len(), 1);
    assert_eq!(std::fs::read(base.join("page.png"))?, bytes);
    assert!(plan.out.join("decision.json").is_file());
    // A stale retained object fails hash verification before advice can be returned.
    let row: serde_json::Value =
        serde_json::from_str(std::fs::read_to_string(store.join("index.jsonl"))?.trim())?;
    std::fs::write(
        store.join("objects").join(format!(
            "{}.json",
            row["report_sha256"].as_str().ok_or("hash")?
        )),
        b"changed",
    )?;
    assert!(
        history::analyze_history(&history::AnalyzeOptions {
            store: &store,
            entry: None,
            drift: false,
            out: None,
            limit: 10
        })
        .is_err()
    );
    Ok(())
}

#[test]
#[cfg(any(unix, windows))]
fn signed_batch_refuses_before_intake_or_worker_execution() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    let policy = Policy {
        require_signed_approval: true,
        ..Default::default()
    };
    let options = saccade_core::batch::Options::default();
    let error = workflows::batch::run_batch(&workflows::batch::BatchRun {
        source: &fixture.path().join("missing"),
        reference_dir: None,
        out: &fixture.path().join("out"),
        options: &options,
        executable: Path::new("absent-worker"),
        policy: &policy,
    })
    .err()
    .ok_or("signed subprocess batch accepted")?;
    assert_eq!(error.code, "approval_consumer_unsupported");
    assert!(!fixture.path().join("out").exists());
    Ok(())
}

#[test]
fn empty_run_warning_uses_the_callers_input_path() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let base = fixture.path().join("baseline");
    std::fs::create_dir(&base)?;
    let capture = fixture.path().join("capture");
    image(&capture)?;
    let out = fixture.path().join("report");
    let result = workflows::run_compare(&CompareRun {
        baseline: &base,
        capture: &capture,
        out: &out,
        config: &RunConfig::default(),
        policy: &Policy::default(),
        approved: false,
        verified: None,
        junit: None,
        intent: &IntentOptions::default(),
    })?;
    assert_eq!(
        result.warnings,
        vec![format!(
            "saccade: warning: nothing compared: the baseline directory {} has no images, so every capture is new; empty comparisons are not evidence",
            base.display()
        )]
    );
    Ok(())
}

#[test]
fn approval_observer_failure_precedes_baseline_mutation() -> Result<(), Box<dyn std::error::Error>>
{
    use workflows::approval;
    let fixture = tempfile::tempdir()?;
    let base = fixture.path().join("base");
    let capture = fixture.path().join("capture");
    image(&base)?;
    image(&capture)?;
    image::RgbImage::from_pixel(32, 32, image::Rgb([90, 50, 90])).save(capture.join("page.png"))?;
    let before = std::fs::read(base.join("page.png"))?;
    let report_dir = fixture.path().join("report");
    workflows::run_compare(&CompareRun {
        baseline: &base,
        capture: &capture,
        out: &report_dir,
        config: &RunConfig::default(),
        policy: &Policy::default(),
        approved: false,
        verified: None,
        junit: None,
        intent: &IntentOptions::default(),
    })?;
    let report = report_dir.join(saccade_core::report::REPORT_FILE_NAME);
    let plan = approval::run(
        &report,
        &capture,
        &base,
        None,
        approval::Options {
            names: vec!["page.png".into()],
            dry_run: true,
            out: Some(fixture.path().join("plan")),
            ..Default::default()
        },
    )?;
    let decision = plan.out.join("decision.json");
    let apply_dir = fixture.path().join("apply");
    let refused = approval::run_with_plan_observer(
        &report,
        &capture,
        &base,
        Some(&decision),
        approval::Options {
            out: Some(apply_dir.clone()),
            ..Default::default()
        },
        |plan| {
            assert!(!plan.dry_run);
            assert!(plan.out.join("manifest.json").is_file());
            assert_ne!(plan.entries[0].before, plan.entries[0].after);
            Err(workflows::CommandError::io("caller transport failed"))
        },
    )
    .err()
    .ok_or("observer failure did not abort application")?;
    assert_eq!(refused.code, "io");
    assert_eq!(std::fs::read(base.join("page.png"))?, before);
    Ok(())
}
