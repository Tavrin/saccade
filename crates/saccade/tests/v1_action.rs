//! Offline Action contracts; GitHub and git writes are mocked.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

fn step(name: &str) -> &'static str {
    include_str!("../../../action.yml")
        .split(&format!("    - name: {name}\n"))
        .nth(1)
        .unwrap()
        .split("\n    - name:")
        .next()
        .unwrap()
}
#[test]
fn fork_workflow_has_read_only_permissions_and_stable_outputs() {
    let usage = include_str!("../../../.github/workflows/example-usage.yml");
    let update = include_str!("../../../.github/workflows/example-update-baselines.yml");
    let action = include_str!("../../../action.yml");
    assert!(
        usage.contains("  pull_request:\n") && usage.contains("permissions:\n  contents: read\n")
    );
    assert!(usage.contains("persist-credentials: false"));
    assert!(!usage.contains(": write") && !usage.contains("secrets."));
    assert!(!action.contains("API_KEY") && !action.contains("review --run"));
    assert!(action.contains("default: binary"));
    assert!(step("Pull request comment").contains("!github.event.pull_request.head.repo.fork"));
    assert!(
        step("Pull request comment")
            .contains("github.event.pull_request.head.repo.full_name == github.repository")
    );
    assert!(update.contains("permissions:\n  contents: read\n"));
    assert!(update.contains("github.ref_name == github.event.repository.default_branch"));
    assert!(update.contains("artifact-ids: ${{ inputs.evidence-artifact-id }}"));
    assert!(update.contains("run-id: ${{ inputs.evidence-run-id }}"));
    assert!(!update.contains("pull_request_target"));
    for field in ["verdict", "exit-code", "report-url", "artifact-id"] {
        assert!(action.contains(&format!("  {field}:\n")));
    }
    assert!(!action.contains("--all-failing") && !action.contains("--auto"));
    let inline = step("Publish inline PR images");
    assert!(inline.contains("inputs.inline-images == 'true'"));
    assert!(inline.contains("!github.event.pull_request.head.repo.fork"));
    assert!(inline.contains("github.event.pull_request.head.repo.full_name == github.repository"));
    assert!(inline.contains("scripts/pr-inline-assets.py"));
    assert!(
        action.find("- name: Upload report").unwrap()
            < action.find("- name: Publish inline PR images").unwrap()
    );
    assert!(
        action.find("- name: Publish inline PR images").unwrap()
            < action.find("- name: Pull request comment").unwrap()
    );
}
#[cfg(unix)]
#[path = "support/approval.rs"]
mod support;
#[cfg(unix)]
mod execution {
    use super::{step, support};
    use image::{Rgb, RgbImage};
    use saccade_core::{evidence::canonical::Digest, paths};
    use std::{
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command, Output},
    };
    struct Fixture {
        temp: tempfile::TempDir,
        root: PathBuf,
        bin: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("workspace");
            let bin = temp.path().join("bin");
            for p in [&root, &bin] {
                std::fs::create_dir_all(p).unwrap();
            }
            for n in ["output", "path"] {
                std::fs::write(temp.path().join(n), "").unwrap();
            }
            Self { temp, root, bin }
        }
        fn stub(&self, name: &str, script: &str) {
            let file = self.bin.join(name);
            std::fs::write(&file, format!("#!/bin/bash\n{script}\n")).unwrap();
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        fn run(&self, name: &str, env: &[(&str, &str)]) -> Output {
            let script = step(name)
                .split("      run: |\n")
                .nth(1)
                .unwrap()
                .lines()
                .map(|l| l.strip_prefix("        ").unwrap_or(l))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!script.contains("${{"));
            Command::new("bash")
                .args(["-c", &script])
                .current_dir(&self.root)
                .env(
                    "PATH",
                    format!("{}:{}", self.bin.display(), std::env::var("PATH").unwrap()),
                )
                .env("RUNNER_TEMP", self.temp.path())
                .env("GITHUB_WORKSPACE", &self.root)
                .env("GITHUB_OUTPUT", self.temp.path().join("output"))
                .env("GITHUB_PATH", self.temp.path().join("path"))
                .env("GITHUB_STEP_SUMMARY", self.temp.path().join("summary"))
                .env("TRACE", self.temp.path().join("trace"))
                .env("STAGED", self.temp.path().join("staged"))
                .env("FIXTURES", self.temp.path())
                .env("SACCADE_BIN", env!("CARGO_BIN_EXE_saccade"))
                .env(
                    "ACTION_PATH",
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
                )
                .envs([
                    ("INSTALL_MODE", "binary"),
                    ("INPUT_VERSION", ""),
                    ("ACTION_REF", "v1"),
                    ("ACTION_REPO", "owner/saccade"),
                    ("RUNNER_OS", "Linux"),
                    ("RUNNER_ARCH", "X64"),
                    ("BASELINE_DIR", "baselines"),
                    ("CAPTURE_DIR", "captures"),
                    ("REPORT_DIR", "report"),
                    ("THRESHOLD", ""),
                    ("METRIC", ""),
                    ("CONFIG", ""),
                    ("FAIL_ON_NEW", ""),
                    ("UPDATE_BASELINES", "false"),
                    ("EVENT_NAME", "workflow_dispatch"),
                    ("FORK", "false"),
                    ("REF_TYPE", "branch"),
                    ("BASE_BRANCH", "main"),
                    ("DEFAULT_BRANCH", "main"),
                    ("REPO", "owner/repo"),
                    ("SERVER_URL", "https://github.com"),
                    ("RUN_ID", "123"),
                    ("BRANCH_PREFIX", "saccade/update-baselines"),
                    ("PRUNE_MISSING", "false"),
                    ("UPDATE_MANIFEST", "plan/manifest.json"),
                    ("MANIFEST_SHA256", ""),
                    ("SUMMARY_PATH", "summary.md"),
                    ("GH_TOKEN", "mock-token"),
                    ("COMMENT_KEY", ""),
                    ("ARTIFACT_URL", "https://example.invalid/artifact/1"),
                    ("RUN_URL", "https://example.invalid/run/1"),
                    ("DIRTY", "false"),
                    ("PUSH_DENY", "false"),
                    ("GH_DENY", "false"),
                    ("INSTALL_OUTCOME", "success"),
                    ("COMPARE_OUTCOME", "success"),
                    ("JUNIT_OUTCOME", "success"),
                    ("UPLOAD_OUTCOME", "success"),
                    ("SUMMARY_OUTCOME", "success"),
                    ("UPDATE_OUTCOME", "skipped"),
                ])
                .envs(env.iter().copied())
                .output()
                .unwrap()
        }
        fn log(&self) -> String {
            std::fs::read_to_string(self.temp.path().join("trace")).unwrap_or_default()
        }
        fn images(&self, n: usize) {
            self.stub(
                "saccade",
                "printf 'saccade %s\\n' \"$*\" >> \"$TRACE\"\nexec \"$SACCADE_BIN\" \"$@\"",
            );
            for dir in ["baselines", "captures"] {
                std::fs::create_dir_all(self.root.join(dir)).unwrap();
                for i in 0..n {
                    RgbImage::from_pixel(8, 8, Rgb([if dir == "baselines" { 10 } else { 200 }; 3]))
                        .save(self.root.join(format!("{dir}/{i}.png")))
                        .unwrap();
                }
            }
            code(&self.run("Compare images", &[]), 0);
        }
        fn hash(&self) -> String {
            Digest::of_bytes(&std::fs::read(self.root.join("plan/manifest.json")).unwrap())
                .as_str()
                .strip_prefix("sha256:")
                .unwrap()
                .to_owned()
        }
        fn plan(&self, entries: &[&str], prune: bool) -> String {
            support::draft(
                &self.root.join("report/saccade-report.v1.json"),
                &self.root.join("plan"),
                entries,
                prune,
            );
            self.stub(
                "git",
                r#"printf 'git %s\n' "$*" >> "$TRACE"
if [ "$1" = diff ]; then
  [ "$DIRTY" = true ] && exit 1
  [ "$2" = --cached ] && [ -f "$STAGED" ] && exit 1
fi
[ "$1" != --literal-pathspecs ] || touch "$STAGED"
[ "$1" != push ] || [ "$PUSH_DENY" != true ] || exit 1
exit 0"#,
            );
            self.stub("gh", "printf 'gh %s\\n' \"$*\" >> \"$TRACE\"\n[ \"$GH_DENY\" != true ] || exit 1\necho https://example.invalid/pr/1");
            self.hash()
        }
        fn install(&self) {
            std::fs::write(self.temp.path().join("archive"), b"release fixture").unwrap();
            std::fs::write(
                self.temp.path().join("checksum"),
                Digest::of_bytes(b"release fixture")
                    .as_str()
                    .strip_prefix("sha256:")
                    .unwrap(),
            )
            .unwrap();
            std::fs::write(self.temp.path().join("exe"), "#!/bin/bash\nexit 0\n").unwrap();
            self.stub(
                "gh",
                r#"printf 'gh %s\n' "$*" >> "$TRACE"
[ "${DOWNLOAD_DENY:-false}" != true ] || exit 1
while [ "$#" -gt 0 ]; do
  case "$1" in --dir) dl=$2; shift ;; --pattern) [[ "$2" = *.sha256 ]] || asset=$2; shift ;; esac
  shift
done
cp "$FIXTURES/archive" "$dl/$asset"
[ "${NO_CHECKSUM:-false}" = true ] || cp "$FIXTURES/checksum" "$dl/$asset.sha256""#,
            );
            self.stub(
                "cargo",
                r#"printf 'cargo %s\n' "$*" >> "$TRACE"
while [ "$1" != --root ]; do shift; done
mkdir -p "$2/bin"
cp "$FIXTURES/exe" "$2/bin/saccade"
chmod +x "$2/bin/saccade""#,
            );
            self.stub("tar", "printf 'tar %s\\n' \"$*\" >> \"$TRACE\"\ncp \"$FIXTURES/exe\" \"$4/saccade\"\nchmod +x \"$4/saccade\"");
            self.stub("unzip", "printf 'unzip %s\\n' \"$*\" >> \"$TRACE\"\ncp \"$FIXTURES/exe\" \"$6/saccade.exe\"\nchmod +x \"$6/saccade.exe\"");
        }
    }
    fn code(out: &Output, wanted: i32) {
        assert_eq!(
            out.status.code(),
            Some(wanted),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    #[test]
    fn binary_and_source_selection_are_explicit() {
        for (mode, version, os, arch, wanted) in [
            ("binary", "", "Linux", "X64", 0),
            ("binary", "v0.1.0", "Linux", "ARM64", 0),
            ("binary", "v0.1.0", "macOS", "ARM64", 0),
            ("binary", "v0.1.0", "Windows", "X64", 0),
            ("binary", "", "macOS", "X64", 2),
            ("source", "", "Linux", "X64", 0),
            ("source", "abcdef", "Linux", "X64", 0),
            ("auto", "", "Linux", "X64", 2),
        ] {
            let f = Fixture::new();
            f.install();
            code(
                &f.run(
                    "Install saccade",
                    &[
                        ("INSTALL_MODE", mode),
                        ("INPUT_VERSION", version),
                        ("RUNNER_OS", os),
                        ("RUNNER_ARCH", arch),
                    ],
                ),
                wanted,
            );
            assert_eq!(f.log().contains("cargo install"), mode == "source");
            if mode == "source" {
                assert!(!f.log().contains("gh release"));
                assert!(f.log().contains("--locked"));
                assert!(f.log().contains(if version.is_empty() {
                    "--path"
                } else {
                    "--rev abcdef"
                }));
            } else if wanted == 0 {
                assert!(f.log().contains(&format!(
                    "gh release download {}",
                    if version.is_empty() { "v1" } else { version }
                )));
                assert!(f.log().contains(".sha256"));
                assert!(
                    !std::fs::read_to_string(f.temp.path().join("path"))
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }
    #[test]
    fn checksum_and_installation_failures_never_build_source() {
        for problem in [
            "mismatch",
            "missing",
            "download",
            "no-version",
            "executable",
        ] {
            let f = Fixture::new();
            f.install();
            if problem == "mismatch" {
                std::fs::write(f.temp.path().join("checksum"), "0".repeat(64)).unwrap();
            }
            if problem == "executable" {
                std::fs::write(f.temp.path().join("exe"), "#!/bin/bash\nexit 1\n").unwrap();
            }
            code(
                &f.run(
                    "Install saccade",
                    &[
                        (
                            "NO_CHECKSUM",
                            if problem == "missing" {
                                "true"
                            } else {
                                "false"
                            },
                        ),
                        (
                            "DOWNLOAD_DENY",
                            if problem == "download" {
                                "true"
                            } else {
                                "false"
                            },
                        ),
                        (
                            "ACTION_REF",
                            if problem == "no-version" { "" } else { "v1" },
                        ),
                    ],
                ),
                2,
            );
            assert!(!f.log().contains("cargo"));
            if problem != "executable" {
                assert!(!f.log().contains("tar "));
            }
            assert!(
                std::fs::read_to_string(f.temp.path().join("path"))
                    .unwrap()
                    .is_empty()
            );
        }
    }
    #[test]
    fn failed_report_upload_summary_and_outputs_survive_the_gate() {
        let f = Fixture::new();
        f.images(1);
        assert!(
            std::fs::read_to_string(f.temp.path().join("output"))
                .unwrap()
                .contains("exit-code=1\nverdict=fail\nreport-written=true")
        );
        assert!(f.root.join("report/.saccade-run").exists());
        // Both documented contracts work, and an unknown successor still fails.
        let path = f.root.join("report/saccade-report.v1.json");
        let mut report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (schema, expected) in [
            ("saccade-report.v1", "exit-code=1\nverdict=fail"),
            ("saccade-report.v2", "exit-code=1\nverdict=fail"),
            ("saccade-report.v99", "exit-code=2\nverdict=command-error"),
        ] {
            report["schema"] = serde_json::json!(schema);
            std::fs::write(&path, serde_json::to_vec(&report).unwrap()).unwrap();
            std::fs::write(f.temp.path().join("output"), "").unwrap();
            code(
                &f.run(
                    "Compare images",
                    &[
                        ("UPDATE_BASELINES", "true"),
                        ("EVENT_NAME", "workflow_dispatch"),
                    ],
                ),
                0,
            );
            assert!(
                std::fs::read_to_string(f.temp.path().join("output"))
                    .unwrap()
                    .contains(expected)
            );
        }
        report["schema"] = serde_json::json!("saccade-report.v2");
        std::fs::write(&path, serde_json::to_vec(&report).unwrap()).unwrap();
        let upload = step("Upload report");
        assert!(upload.contains("always() && steps.compare.outputs.report-written == 'true'"));
        assert!(
            upload.contains("path: ${{ inputs.report-dir }}")
                && upload.contains("include-hidden-files: true")
        );
        code(&f.run("Write job summary", &[]), 0);
        assert!(
            std::fs::read_to_string(f.temp.path().join("summary"))
                .unwrap()
                .contains("https://example.invalid/artifact/1")
        );
        for exit in ["1", "2", ""] {
            code(
                &f.run("Fail on regression", &[("EXIT_CODE", exit)]),
                if exit == "1" { 1 } else { 2 },
            );
        }
        code(
            &f.run(
                "Fail on regression",
                &[("EXIT_CODE", "1"), ("JUNIT_OUTCOME", "failure")],
            ),
            2,
        );
        code(&f.run("Compare images", &[]), 0);
        assert!(
            std::fs::read_to_string(f.temp.path().join("output"))
                .unwrap()
                .contains("exit-code=2\nverdict=command-error\nreport-written=false")
        );
    }
    #[test]
    fn junit_is_in_the_report_before_upload_and_failure() {
        let f = Fixture::new();
        f.images(1);
        code(&f.run("Export JUnit", &[]), 0);
        let xml = std::fs::read_to_string(f.root.join("report/junit.xml")).unwrap();
        assert!(xml.contains("<testsuite") && xml.contains("<failure"));
        let a = include_str!("../../../action.yml");
        assert!(a.find("- name: Export JUnit").unwrap() < a.find("- name: Upload report").unwrap());
        assert!(
            a.find("- name: Upload report").unwrap()
                < a.find("- name: Fail on regression").unwrap()
        );
    }
    #[test]
    fn update_boundary_rejects_forks_untrusted_events_and_write_errors() {
        for (key, value, wanted) in [
            ("EVENT_NAME", "pull_request", 2),
            ("FORK", "true", 2),
            ("REF_TYPE", "tag", 2),
            ("BASE_BRANCH", "feature", 2),
            ("DIRTY", "true", 2),
            ("PUSH_DENY", "true", 1),
            ("GH_DENY", "true", 1),
        ] {
            let f = Fixture::new();
            f.images(1);
            let hash = f.plan(&["0.png"], false);
            std::fs::write(f.temp.path().join("trace"), "").unwrap();
            code(
                &f.run(
                    "Open baseline-update pull request",
                    &[("MANIFEST_SHA256", &hash), (key, value)],
                ),
                wanted,
            );
            if ["EVENT_NAME", "FORK", "REF_TYPE", "BASE_BRANCH"].contains(&key) {
                assert!(f.log().is_empty());
            }
            if key == "DIRTY" {
                assert!(!f.log().contains("saccade approve"));
            }
            if key != "GH_DENY" {
                assert!(!f.log().contains("gh pr create"));
            }
        }
    }
    #[test]
    fn selected_immutable_manifest_checks_stale_content_and_stages_every_entry() {
        for problem in [
            "none",
            "manifest",
            "hash",
            "candidate",
            "baseline",
            "report",
            "decision",
            "expanded-scope",
            "wrong-plan",
        ] {
            let f = Fixture::new();
            f.images(8);
            let before = std::fs::read(f.root.join("baselines/7.png")).unwrap();
            let mut hash = f.plan(
                &[
                    "0.png", "1.png", "2.png", "3.png", "4.png", "5.png", "6.png",
                ],
                false,
            );
            match problem {
                "hash" => hash = "0".repeat(64),
                "manifest" => {
                    let p = f.root.join("plan/manifest.json");
                    let mut b = std::fs::read(&p).unwrap();
                    b.push(b' ');
                    std::fs::write(p, b).unwrap();
                }
                "candidate" | "baseline" => {
                    let dir = if problem == "candidate" {
                        "captures"
                    } else {
                        "baselines"
                    };
                    RgbImage::from_pixel(8, 8, Rgb([50; 3]))
                        .save(f.root.join(format!("{dir}/0.png")))
                        .unwrap();
                }
                "report" | "decision" => {
                    let p = f.root.join(if problem == "report" {
                        "report/saccade-report.v1.json"
                    } else {
                        "plan/decision.json"
                    });
                    let mut v: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                    v["schema"] = "changed".into();
                    std::fs::write(p, serde_json::to_vec(&v).unwrap()).unwrap();
                }
                "expanded-scope" | "wrong-plan" => {
                    let p = f.root.join("plan/manifest.json");
                    let mut v: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                    if problem == "expanded-scope" {
                        v["entries"][0]["entry_id"] = "7.png".into();
                    } else {
                        v["entries"][0]["after"] = format!("sha256:{}", "0".repeat(64)).into();
                    }
                    std::fs::write(p, serde_json::to_vec(&v).unwrap()).unwrap();
                    hash = f.hash();
                }
                _ => {}
            }
            code(
                &f.run(
                    "Open baseline-update pull request",
                    &[("MANIFEST_SHA256", &hash)],
                ),
                if problem == "none" { 0 } else { 2 },
            );
            assert!(!f.log().contains("--all-failing"));
            assert_eq!(
                std::fs::read(f.root.join("baselines/7.png")).unwrap(),
                before
            );
            if problem == "none" {
                for n in 0..7 {
                    assert_eq!(
                        std::fs::read(f.root.join(format!("baselines/{n}.png"))).unwrap(),
                        std::fs::read(f.root.join(format!("captures/{n}.png"))).unwrap()
                    );
                    assert!(f.log().contains(&paths::portable(
                        &paths::canonicalize(f.root.join(format!("baselines/{n}.png"))).unwrap()
                    )));
                }
                assert!(f.log().contains("git push https://github.com/owner/repo.git HEAD:refs/heads/saccade/update-baselines-123"));
                assert!(
                    f.log().contains("gh pr create") && f.log().contains("--body-file summary.md")
                );
            } else {
                assert!(
                    !f.log().contains("git --literal-pathspecs add")
                        && !f.log().contains("git push")
                );
            }
        }
    }
    #[test]
    fn reviewed_report_is_preserved_and_deletions_need_opt_in() {
        for allow in [false, true] {
            let f = Fixture::new();
            f.images(1);
            std::fs::remove_dir_all(f.root.join("report")).unwrap();
            std::fs::remove_file(f.root.join("captures/0.png")).unwrap();
            code(&f.run("Compare images", &[]), 0);
            let report = std::fs::read(f.root.join("report/saccade-report.v1.json")).unwrap();
            let hash = f.plan(&["0.png"], true);
            std::fs::write(f.temp.path().join("trace"), "").unwrap();
            code(&f.run("Compare images", &[("UPDATE_BASELINES", "true")]), 0);
            assert!(f.log().is_empty());
            assert_eq!(
                report,
                std::fs::read(f.root.join("report/saccade-report.v1.json")).unwrap()
            );
            code(
                &f.run(
                    "Open baseline-update pull request",
                    &[
                        ("MANIFEST_SHA256", &hash),
                        ("PRUNE_MISSING", if allow { "true" } else { "false" }),
                    ],
                ),
                if allow { 0 } else { 2 },
            );
            assert_eq!(f.root.join("baselines/0.png").exists(), !allow);
        }
    }
}
