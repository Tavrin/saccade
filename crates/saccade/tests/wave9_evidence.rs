//! Coordinator-run end-to-end mirrors; all image fixtures are generated.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .unwrap()
}
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn value(o: Output, code: i32) -> Value {
    assert_eq!(
        o.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap()
}
fn fixtures(root: &Path) -> std::path::PathBuf {
    std::fs::create_dir_all(root).unwrap();
    for name in ["first.png", "second.png"] {
        image::RgbImage::from_fn(32, 32, |x, y| {
            image::Rgb([((x * 31 + y * 17) % 255) as u8; 3])
        })
        .save(root.join(name))
        .unwrap();
    }
    let plan = json!({"schema":"saccade-visual-trial-plan.v1","objective":"Prefer clearer detail","metrics":["flip","detail_energy"],"spatial_policy":saccade_core::evidence_quality::spatial::Policy::default(),"pairs":[{"id":"p","first":"first.png","second":"second.png","mask":null}],"seed":17});
    let p = root.join("plan.json");
    std::fs::write(&p, serde_json::to_vec(&plan).unwrap()).unwrap();
    p
}
#[test]
#[ignore = "heavy: wave9-cli"]
fn reference_and_trial_commands_preserve_evidence_and_refuse_changed_plan() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("inputs");
    let plan = fixtures(&root);
    let out = temp.path().join("trial");
    let result = value(
        run(&[
            "experiment",
            "reference",
            path(&root.join("first.png")),
            path(&root.join("second.png")),
            "--json",
        ]),
        0,
    );
    let reference: saccade_core::evidence_quality::reference::Report =
        serde_json::from_value(result).unwrap();
    assert_eq!(reference.schema, "saccade-reference-evidence.v1");
    assert_eq!(reference.input_sha256.len(), 2);
    assert_eq!(reference.alignment.exposure_scale, 1.0);
    assert_eq!(
        value(
            run(&[
                "review",
                "trial",
                "register",
                path(&plan),
                "--out",
                path(&out),
                "--json"
            ]),
            0
        )["state"],
        "registered"
    );
    assert_eq!(
        value(
            run(&[
                "review",
                "trial",
                "start",
                path(&plan),
                "--out",
                path(&out),
                "--json"
            ]),
            0
        )["state"],
        "inspection_started"
    );
    let items = saccade_core::judge_vote::read_run(&out).unwrap();
    let id = &items.items[0].id;
    assert_eq!(
        value(
            run(&[
                "review",
                "trial",
                "vote",
                path(&plan),
                "--out",
                path(&out),
                "--voter",
                "human",
                "--item",
                id,
                "--answer",
                "P1",
                "--json"
            ]),
            0
        )["votes"],
        1
    );
    let judgments = temp.path().join("judgments.json");
    std::fs::write(
        &judgments,
        serde_json::to_vec(&json!({"votes":[{"item":id,"answer":"P2"}]})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        value(
            run(&[
                "review",
                "trial",
                "import",
                path(&plan),
                path(&judgments),
                "--out",
                path(&out),
                "--voter",
                "second",
                "--json"
            ]),
            0
        )["votes"],
        2
    );
    let mut p: Value = serde_json::from_slice(&std::fs::read(&plan).unwrap()).unwrap();
    p["seed"] = json!(18);
    std::fs::write(&plan, serde_json::to_vec(&p).unwrap()).unwrap();
    let changed = value(
        run(&[
            "review",
            "trial",
            "start",
            path(&plan),
            "--out",
            path(&out),
            "--json",
        ]),
        2,
    );
    assert!(changed.to_string().contains("trial_plan_changed"));
}
#[cfg(feature = "mcp")]
#[test]
#[ignore = "heavy: wave9-cli"]
fn mcp_mirrors_are_discoverable_local_and_root_contained() {
    use std::io::Write;
    use std::process::Stdio;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("input");
    let plan = fixtures(&root);
    let out = temp.path().join("output");
    let outside = temp.path().join("outside.png");
    image::RgbImage::new(8, 8).save(&outside).unwrap();
    let request = |id, arguments| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"saccade_measure","arguments":arguments}});
    let requests = [
        json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}),
        request(
            1,
            json!({"operation":"reference_compare","render":"first.png","reference":"second.png"}),
        ),
        request(
            2,
            json!({"operation":"trial_register","plan":plan,"out":"trial"}),
        ),
        request(
            3,
            json!({"operation":"trial_start","plan":plan,"out":"trial"}),
        ),
        request(
            4,
            json!({"operation":"reference_compare","render":outside,"reference":"second.png"}),
        ),
        request(
            5,
            json!({"operation":"trial_register","plan":plan,"out":"../escape"}),
        ),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["mcp", "--root", path(&root), "--out-root", path(&out)])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies.len(), 6);
    let tools = replies[0]["result"]["tools"].to_string();
    for op in [
        "reference_compare",
        "trial_register",
        "trial_start",
        "trial_vote",
        "trial_import",
    ] {
        assert!(tools.contains(op));
    }
    for reply in &replies[1..4] {
        assert_eq!(reply["result"]["isError"], false, "{reply}");
    }
    for reply in &replies[4..] {
        assert_eq!(reply["result"]["isError"], true, "{reply}");
    }
    assert!(out.join("trial/public/index.html").exists());
}
