//! Feature boundaries and offline evidence contracts.
#![allow(clippy::unwrap_used, missing_docs)]

use saccade_core::Report;
use serde_json::json;

#[test]
fn core_dependency_graph_excludes_transports_and_watchers() {
    for (extra, server) in [
        (Vec::<&str>::new(), false),
        (vec!["--no-default-features"], false),
        (
            vec!["--no-default-features", "--features", "workbench"],
            true,
        ),
        (
            vec!["--no-default-features", "--features", "prechecks"],
            false,
        ),
    ] {
        let output = std::process::Command::new(env!("CARGO"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args([
                "tree",
                "--offline",
                "--locked",
                "-p",
                "saccade-core",
                "--edges",
                "normal",
                "--prefix",
                "none",
                "--format",
                "{p}",
            ])
            .args(extra)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let graph = String::from_utf8(output.stdout).unwrap();
        let packages: Vec<_> = graph
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        for forbidden in ["ureq", "notify", "inotify", "mio", "rustls", "http"] {
            assert!(
                !packages.contains(&forbidden),
                "unexpected {forbidden}: {graph}"
            );
        }
        assert_eq!(packages.contains(&"tiny_http"), server, "{graph}");
        assert!(packages.contains(&"rustfft"));
    }
}

#[test]
fn optional_evidence_is_readable_without_its_producer() {
    let value = json!({
        "schema":"saccade-report.v1", "tool_version":"0.1.0", "generated_at_unix":0,
        "config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},
        "totals":{"pass":0,"fail":0,"error":0,"missing":0,"new":0,"total":0}, "entries":[],
        "perf_diff":{
            "schema":"saccade-perf-diff.v1","unit":"ms","noise_k":3.0,
            "frame":{"before":10.0,"after":9.0,"delta":-1.0,"delta_pct":-10.0,"noise_floor":null,"beyond_noise":null,"status":"paired"},
            "unattributed_before":0.0,"unattributed_after":0.0,"terms":[],"warnings":[]
        }, "combined_verdict":"INCONCLUSIVE"
    });
    let report: Report = serde_json::from_value(value).unwrap();
    assert_eq!(report.perf_diff.as_ref().unwrap().frame.delta, Some(-1.0));
    assert!(
        saccade_core::render::render_markdown(&report, &Default::default())
            .contains("INCONCLUSIVE")
    );
    let round_trip: Report =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    assert_eq!(round_trip, report);
    let buffer: saccade_core::buffer::BufferResult = serde_json::from_value(json!({
        "kind":"depth","encoding":"r32f","unit":"depth","scale":1.0,
        "metric":"mean","value":0.0,"threshold":0.01,"heatmap_max":0.01,
        "stats":{"mean":0.0,"max":0.0,"p95":0.0,"p99":0.0}
    }))
    .unwrap();
    assert_eq!(buffer.unit, "depth");
    let mut label: saccade_core::labels::Label = serde_json::from_value(json!({
        "entry":"scene.png","question":"triage","answer":"noise","evidence_hash":"",
        "state":{},"images":[],"sha256":[],"hotspots":[],"provenance":[{"source":"human"}]
    }))
    .unwrap();
    label.evidence_hash = saccade_core::labels::identity(&label);
    let labels = saccade_core::labels::Labels {
        schema: saccade_core::labels::SCHEMA.into(),
        items: vec![label],
    };
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("labels.json");
    std::fs::write(&file, serde_json::to_vec(&labels).unwrap()).unwrap();
    assert_eq!(
        saccade_core::labels::Labels::read(&file).unwrap().items[0].answer,
        "noise"
    );
    let votes: saccade_core::judge_vote::VoteRun = serde_json::from_value(json!({"schema":"saccade-judge-votes.v1","run_id":"0123456789abcdef","created_at_unix":0,"items":[]})).unwrap();
    assert!(votes.items.is_empty());
}

#[cfg(not(feature = "graphics"))]
#[test]
fn missing_graphics_computation_names_the_required_feature_before_writing() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    let cfg = saccade_core::config::RunConfig::default();
    let operations = [
        saccade_core::sequence::run_sequence(tmp.path(), tmp.path(), &out, "*", &cfg).map(|_| ()),
        saccade_core::rank::run_rank(
            tmp.path(),
            &[],
            None,
            saccade_core::Metric::Mean,
            &out,
            &cfg,
        )
        .map(|_| ()),
        saccade_core::ablate::run(tmp.path(), &[], &out, &cfg, 3).map(|_| ()),
        saccade_core::bisect::runs(&[], None, &out, &Default::default()).map(|_| ()),
        saccade_core::perf::pair(tmp.path(), tmp.path(), &Default::default()).map(|_| ()),
        saccade_core::perf::noise(&[], saccade_core::perf::DEFAULT_PERF_NAME).map(|_| ()),
    ];
    for result in operations {
        assert!(matches!(
            result,
            Err(saccade_core::Error::FeatureUnavailable {
                feature: "graphics"
            })
        ));
    }
    assert!(!out.exists());
}
