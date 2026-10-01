//! Exercise the Action's write boundary offline, with mocked git/gh/saccade.
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
    let dispatch = "workflow_dispatch";
    for (event, fork, dirty, push_deny, gh_deny, path_case, expect) in [
        (
            "pull_request",
            "true",
            "false",
            "false",
            "false",
            "alias",
            2,
        ),
        (dispatch, "true", "false", "false", "false", "alias", 2),
        (dispatch, "false", "true", "false", "false", "alias", 2),
        (dispatch, "false", "false", "true", "false", "alias", 1),
        (dispatch, "false", "false", "false", "true", "alias", 1),
        (dispatch, "false", "false", "false", "false", "alias", 0),
        (dispatch, "false", "false", "false", "false", "relative", 0),
        (dispatch, "false", "false", "false", "false", "outside", 2),
        (
            dispatch,
            "false",
            "false",
            "false",
            "false",
            "symlink-outside",
            2,
        ),
        (dispatch, "false", "false", "false", "false", "sibling", 2),
        (
            dispatch,
            "false",
            "false",
            "false",
            "false",
            "pruned-outside",
            2,
        ),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("workspace");
        let baseline = root.join("baselines");
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&baseline).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let alias = tmp.path().join("workspace-alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, baseline.join("escape")).unwrap();
        std::fs::create_dir_all(root.join("baselines-other")).unwrap();
        let trace = tmp.path().join("trace");
        let staged = tmp.path().join("staged");
        let fixture = tmp.path().join("approve.json");
        let copied = match path_case {
            "relative" => std::path::PathBuf::from("baselines/a.png"),
            "outside" => alias.join("baselines/../a.png"),
            "symlink-outside" => baseline.join("escape/a.png"),
            "sibling" => root.join("baselines-other/a.png"),
            _ => alias.join("baselines/a.png"),
        };
        // Removed files are intentionally absent: only their parent can be
        // canonicalised. Check containment for pruned paths as well as copies.
        let pruned = if path_case == "pruned-outside" {
            outside.join("removed.png")
        } else {
            alias.join("baselines/removed.png")
        };
        std::fs::write(&fixture, serde_json::to_string(&serde_json::json!({"schema":"saccade-approve.v1", "copied":[{"name":"a.png", "from":"capture/a.png", "to":copied}], "pruned":[pruned]})).unwrap()).unwrap();
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
                "saccade",
                r#"#!/bin/bash
printf 'saccade %s\n' "$*" >> "$TRACE"
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
                ("BRANCH_PREFIX", "saccade/update-baselines"),
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
            assert!(!log.contains("saccade approve") && !log.contains("git push"));
        } else if expect == 2 {
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains("approve returned a path outside the baseline directory")
            );
            assert!(!log.contains("git push") && !log.contains("gh pr create"));
            if path_case != "pruned-outside" {
                assert!(!log.contains("git --literal-pathspecs add"));
            }
        } else {
            assert!(
                log.contains("--prune-missing")
                    && log.contains("--all-failing report/saccade-report.v1.json")
            );
            assert!(log.contains("git --literal-pathspecs add"));
            for name in ["a.png", "removed.png"] {
                let changed = baseline.canonicalize().unwrap().join(name);
                assert!(log.contains(&format!(
                    "git --literal-pathspecs add -- {}",
                    changed.display()
                )));
            }
            assert!(log.contains("git push https://github.com/owner/repo.git HEAD:refs/heads/saccade/update-baselines-123"));
            if push_deny == "true" {
                assert!(!log.contains("gh pr create"));
            }
            if expect == 0 {
                assert!(log.contains("--body-file summary.md"));
            }
        }
    }
}
