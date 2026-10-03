//! R13 maintained documentation, demo and generation contracts.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use saccade_core::evidence::canonical::Digest;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    saccade_core::paths::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}
fn text(path: &str) -> String {
    std::fs::read_to_string(repo().join(path)).unwrap()
}

#[cfg(all(feature = "ai", feature = "mcp"))]
#[test]
fn copyable_commands_execute_with_documented_verdicts() {
    let temp = tempfile::tempdir().unwrap();
    for side in ["baseline", "capture"] {
        let dest = temp.path().join("examples").join(side);
        std::fs::create_dir_all(&dest).unwrap();
        for entry in std::fs::read_dir(repo().join("examples").join(side)).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().is_some_and(|e| e == "png") {
                std::fs::copy(entry.path(), dest.join(entry.file_name())).unwrap();
            }
        }
    }
    let commands: Vec<serde_json::Value> =
        serde_json::from_str(&text("scripts/docs-smoke.json")).unwrap();
    for command in commands {
        let args: Vec<_> = command["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let spelling = command["spelling"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("saccade {}", args.join(" ")));
        assert!(
            text(command["document"].as_str().unwrap()).contains(&spelling),
            "undocumented smoke command: {spelling}"
        );
        let out = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(&args)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert_eq!(
            out.status.code().map(i64::from),
            command["exit"].as_i64(),
            "{spelling}: {out:?}"
        );
    }
}

#[test]
fn generated_entry_packs_match_the_shared_source_and_budget() {
    let guide = text("integrations/agent-guide.md");
    for pack in [
        "integrations/codex/AGENTS.saccade.md",
        "integrations/claude-code/skills/saccade/SKILL.md",
    ] {
        let body = text(pack);
        assert!(body.ends_with(&guide), "{pack} drifted from shared source");
        assert!(body.len() <= 4800, "{pack} exceeds estimated 1200 tokens");
    }
}

#[test]
fn generated_indexes_match_shipped_outputs() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let out = Command::new(python)
        .args(["scripts/gen-docs.py", "--check"])
        .env("LC_ALL", "C")
        .env("PYTHONUTF8", "0")
        .env("PYTHONCOERCECLOCALE", "0")
        .current_dir(repo())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let capabilities = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["inspect", "capabilities", "--json"])
        .output()
        .unwrap();
    let data: serde_json::Value = serde_json::from_slice(&capabilities.stdout).unwrap();
    let reference = text("docs/cli.md");
    for operation in data["data"]["operations"].as_array().unwrap() {
        assert!(reference.contains(&format!("## saccade {}\n", operation.as_str().unwrap())));
    }
}

#[test]
fn gallery_redaction_preserves_case_identity_and_exact_report_references() {
    use saccade_core::evidence::{Artifact, Document};
    let temp = tempfile::tempdir().unwrap();
    for (side, value) in [("base", 20), ("cap", 200)] {
        std::fs::create_dir(temp.path().join(side)).unwrap();
        image::RgbImage::from_pixel(16, 16, image::Rgb([value, value, value]))
            .save(temp.path().join(side).join("scene.png"))
            .unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["compare", "base", "cap", "--out", "raw"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let original = Document::read(&temp.path().join("raw/evidence.json")).unwrap();
    let Artifact::Case(original) = original.artifact else {
        panic!("case");
    };
    let python = if cfg!(windows) { "python" } else { "python3" };
    // This test exercises publication metadata only; Pillow is not needed in CI.
    let code = "import importlib.util,sys,types\nfrom pathlib import Path\nsys.modules['PIL']=types.SimpleNamespace(Image=None)\nspec=importlib.util.spec_from_file_location('gallery',sys.argv[1])\ngallery=importlib.util.module_from_spec(spec)\nspec.loader.exec_module(gallery)\ngallery.copy_report(Path(sys.argv[2]),Path(sys.argv[3]),lambda s:s.replace('../base','../published-base'))\n";
    let copied = temp.path().join("published");
    let output = Command::new(python)
        .arg("-c")
        .arg(code)
        .env("LC_ALL", "C")
        .env("PYTHONUTF8", "0")
        .env("PYTHONCOERCECLOCALE", "0")
        .arg(repo().join("docs/showcase/build.py"))
        .arg(temp.path().join("raw"))
        .arg(&copied)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let document = copied.join("evidence.json");
    let Artifact::Case(case) = Document::read(&document).unwrap().artifact else {
        panic!("case");
    };
    assert_eq!(case.case_id, original.case_id);
    assert_eq!(
        case.measurement.semantic_sha256,
        original.measurement.semantic_sha256
    );
    assert_ne!(
        case.measurement.report.sha256,
        original.measurement.report.sha256
    );
    case.measurement.report.verify(&document).unwrap();
    for fact in &case.facts {
        fact.artifact.verify(&document).unwrap();
    }
}

#[test]
fn procedural_assets_and_illustrative_timings_are_labeled() {
    for entry in std::fs::read_dir(repo().join("showcases")).unwrap() {
        let dir = entry.unwrap().path();
        if dir.join("commands.json").is_file() {
            let readme = std::fs::read_to_string(dir.join("README.md"))
                .unwrap()
                .to_lowercase();
            assert!(
                readme.contains("procedural") || readme.contains("synthetic"),
                "{}",
                dir.display()
            );
            if dir.file_name().unwrap() == "perf-identity" {
                assert!(readme.contains("not benchmark measurements"));
            }
        }
    }
    assert!(text("showcases/README.md").contains("--features prechecks"));
    assert!(text("docs/showcase/template.html").contains("unvalidated"));
}

#[test]
fn rendering_example_has_license_permission_and_exact_provenance() {
    let root = repo();
    let record: serde_json::Value =
        serde_json::from_str(&text("crates/saccade/assets/demo/provenance.json")).unwrap();
    assert_eq!(record["license"], "MIT OR Apache-2.0");
    assert!(
        record["publication_permission"]
            .as_str()
            .unwrap()
            .contains("Repository license")
    );
    for (base, key) in [
        (root.clone(), "sources"),
        (root.join("crates/saccade/assets/demo"), "assets"),
    ] {
        for (name, hash) in record[key].as_object().unwrap() {
            assert_eq!(
                serde_json::to_value(Digest::of_bytes(&std::fs::read(base.join(name)).unwrap()))
                    .unwrap(),
                *hash,
                "{name}"
            );
        }
    }
    let base = root.join("crates/saccade/assets/demo/identity/baseline/sphere.png");
    let cap = root.join("crates/saccade/assets/demo/identity/capture/sphere.png");
    assert_ne!(std::fs::read(&base).unwrap(), std::fs::read(&cap).unwrap());
    assert_eq!(
        image::open(base).unwrap().to_rgb8(),
        image::open(cap).unwrap().to_rgb8()
    );
}

#[test]
fn maintained_instructions_do_not_invoke_removed_commands() {
    let mut sources = vec![
        "README.md",
        "docs/design.md",
        "docs/captures.md",
        "docs/identity-and-performance.md",
        "docs/review.md",
        "docs/agents.md",
        "docs/ci.md",
        "docs/contracts.md",
        "docs/evaluation.md",
        "docs/cli.md",
        "docs/experimental.md",
        "docs/safety-a11y.md",
        "integrations/README.md",
        "integrations/agent-guide.md",
        "integrations/codex/AGENTS.saccade.md",
        "integrations/claude-code/skills/saccade/SKILL.md",
        "integrations/claude-code/commands/saccade.md",
        "docs/showcase/template.html",
    ];
    let showcase_readmes: Vec<_> = std::fs::read_dir(repo().join("showcases"))
        .unwrap()
        .map(|e| e.unwrap().path().join("README.md"))
        .filter(|p| p.is_file())
        .collect();
    let showcase_names: Vec<_> = showcase_readmes
        .iter()
        .map(|p| {
            p.strip_prefix(repo())
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    sources.extend(showcase_names.iter().map(String::as_str));
    let removed = [
        "summary",
        "entries",
        "explain",
        "snapshot",
        "decision-request",
        "decide",
        "ask",
        "judge",
        "ablate",
        "rank",
        "sequence",
        "bisect",
        "safety",
        "a11y",
        "watch",
        "runs",
        "unblind",
        "config",
    ];
    for source in sources {
        let content = text(source);
        for suffix in content.split("saccade ").skip(1) {
            let word = suffix
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                .next()
                .unwrap();
            assert!(
                !removed.contains(&word),
                "removed command in {source}: {word}"
            );
        }
        assert!(
            !content.contains("--json="),
            "removed JSON switch in {source}"
        );
        assert!(
            !content.contains("/home/") && !content.contains("/mnt/"),
            "local path in {source}"
        );
    }
}

#[cfg(feature = "ai")]
#[test]
fn demo_review_records_disagreement_and_scoped_illustrative_resolution() {
    use saccade_core::evidence::{Artifact, Document};
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["demo", "--out", "demo"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let path = temp.path().join("demo/review/report/evidence.json");
    let doc = Document::read(&path).unwrap();
    let Artifact::Case(case) = doc.artifact else {
        panic!("case");
    };
    assert!(!case.intent.value().unwrap().criteria.is_empty());
    assert_eq!(case.proposals.len(), 2);
    assert_ne!(
        case.proposals[0].response.answer,
        case.proposals[1].response.answer
    );
    assert!(
        case.proposals
            .iter()
            .all(|p| p.provider.identity.provider == "offline-fixture")
    );
    assert_eq!(case.human_decisions.len(), 1);
    assert!(case.human_decisions[0].note.contains("Illustrative"));
    assert_eq!(case.human_decisions[0].binding.case_id, case.case_id);
    case.measurement.report.verify(&path).unwrap();
    for input in &case.inputs {
        input.content.verify(&path).unwrap();
    }
}

#[cfg(not(feature = "prechecks"))]
#[test]
fn showcase_validation_explains_missing_prechecks_before_running_cases() {
    if cfg!(windows) {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let binary = Path::new(env!("CARGO_BIN_EXE_saccade"));
    let path = std::env::join_paths(
        std::iter::once(binary.parent().unwrap().to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let out = Command::new("bash")
        .arg("scripts/run-showcases.sh")
        .current_dir(repo())
        .env("PATH", path)
        .env("SACCADE_SHOWCASE_REPORTS", temp.path().join("reports"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--features prechecks"),
        "{out:?}"
    );
    assert!(!temp.path().join("reports").exists());
}
