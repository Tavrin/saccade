//! Exercise the Action's write boundary offline, with mocked git/gh/flipdiff.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn baseline_update_guards_forks_events_dirty_checkouts_and_permission_failures() {
    let action = include_str!("../../../action.yml");
    let step = action
        .split("    - name: Open baseline-update pull request")
        .nth(1)
        .unwrap()
        .split("    - name: Fail on regression")
        .next()
        .unwrap();
    assert!(step.contains("github.event_name == 'workflow_dispatch'"));
    assert!(step.contains("!github.event.pull_request.head.repo.fork"));
    let script = step
        .split("      run: |\n")
        .nth(1)
        .unwrap()
        .lines()
        .map(|line| line.strip_prefix("        ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!script.contains("${{"));
    for (event, fork, dirty, push_deny, gh_deny, expect) in [
        ("pull_request", "true", "false", "false", "false", 2),
        ("workflow_dispatch", "true", "false", "false", "false", 2),
        ("workflow_dispatch", "false", "true", "false", "false", 2),
        ("workflow_dispatch", "false", "false", "true", "false", 1),
        ("workflow_dispatch", "false", "false", "false", "true", 1),
        ("workflow_dispatch", "false", "false", "false", "false", 0),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("workspace");
        let baseline = root.join("baselines");
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&baseline).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let trace = tmp.path().join("trace");
        let staged = tmp.path().join("staged");
        let fixture = tmp.path().join("approve.json");
        std::fs::write(&fixture, serde_json::to_string(&serde_json::json!({"schema":"flipdiff-approve.v1", "copied":[{"name":"a.png", "from":"capture/a.png", "to":baseline.join("a.png")}], "pruned":[]})).unwrap()).unwrap();
        let programs = [
            (
                "git",
                r#"#!/bin/bash
printf 'git %s\n' "$*" >> "$TRACE"
if [ "$1" = diff ]; then
  [ "$DIRTY" = true ] && exit 1
  [ "$2" = --cached ] && [ -f "$STAGED" ] && exit 1
fi
[ "$1" = --literal-pathspecs ] && touch "$STAGED"
[ "$1" = push ] && [ "$PUSH_DENY" = true ] && exit 1
exit 0
"#,
            ),
            (
                "gh",
                r#"#!/bin/bash
printf 'gh %s\n' "$*" >> "$TRACE"
[ "$GH_DENY" = true ] && exit 1
printf 'https://example.invalid/pr/1\n'
"#,
            ),
            (
                "flipdiff",
                r#"#!/bin/bash
printf 'flipdiff %s\n' "$*" >> "$TRACE"
cat "$APPROVAL_FIXTURE"
"#,
            ),
        ];
        for (name, contents) in programs {
            let path = bin.join(name);
            std::fs::write(&path, contents).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
        let output = Command::new("bash")
            .arg("-c")
            .arg(&script)
            .envs([
                ("EVENT_NAME", event),
                ("FORK", fork),
                ("DIRTY", dirty),
                ("PUSH_DENY", push_deny),
                ("GH_DENY", gh_deny),
                ("REF_TYPE", "branch"),
                ("REPO", "owner/repo"),
                ("SERVER_URL", "https://github.com"),
                ("BASE_BRANCH", "main"),
                ("RUN_ID", "123"),
                ("BRANCH_PREFIX", "flipdiff/update-baselines"),
                ("BASELINE_DIR", "baselines"),
                ("CAPTURE_DIR", "captures"),
                ("REPORT_DIR", "report"),
                ("PRUNE_MISSING", "true"),
                ("SUMMARY_PATH", "summary.md"),
                ("GH_TOKEN", "mock-token"),
            ])
            .env("PATH", path)
            .env("GITHUB_WORKSPACE", &root)
            .env("RUNNER_TEMP", tmp.path())
            .env("TRACE", &trace)
            .env("STAGED", staged)
            .env("APPROVAL_FIXTURE", fixture)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(expect),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let log = std::fs::read_to_string(&trace).unwrap_or_default();
        if event != "workflow_dispatch" || fork == "true" {
            assert!(log.is_empty(), "untrusted event ran commands: {log}");
        } else if dirty == "true" {
            assert!(!log.contains("flipdiff approve") && !log.contains("git push"));
        } else {
            assert!(
                log.contains("--prune-missing")
                    && log.contains("--all-failing report/flipdiff-report.v1.json")
            );
            assert!(log.contains("git --literal-pathspecs add"));
            assert!(log.contains("git push https://github.com/owner/repo.git HEAD:refs/heads/flipdiff/update-baselines-123"));
            if push_deny == "true" {
                assert!(!log.contains("gh pr create"));
            }
            if expect == 0 {
                assert!(log.contains("--body-file summary.md"));
            }
        }
    }
}
