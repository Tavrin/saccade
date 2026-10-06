//! Git revision bisection using an external capture command and local reports.
use crate::agent::CliError;
use clap::Args;
use saccade_core::perf::{Comparability, FrameChange};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Args)]
pub(crate) struct BisectArgs {
    /// Known good revision in the current repository.
    #[arg(long)]
    good: Option<String>,
    /// Known bad revision descended from --good.
    #[arg(long)]
    bad: Option<String>,
    /// Shell capture command; write images to SACCADE_CAPTURE_DIR (sh on Unix, cmd on Windows).
    #[arg(long)]
    capture: String,
    /// Stable baseline directory, copied before Git changes revisions.
    #[arg(long)]
    baseline: PathBuf,
    /// Require qualified performance evidence and count a slower frame as bad.
    #[arg(long)]
    perf: bool,
    /// Evidence directory outside the repository; defaults to a new sibling.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Print bounded JSON.
    #[arg(long)]
    json: bool,
    /// Internal callback invoked by git bisect run.
    #[arg(long, hide = true)]
    step: bool,
}

fn git(repo: &Path, args: &[&str]) -> Result<Output, CliError> {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| CliError::io(e.to_string()))
}

fn checked_git(repo: &Path, args: &[&str]) -> Result<String, CliError> {
    let output = git(repo, args)?;
    if !output.status.success() {
        return Err(CliError::usage(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), CliError> {
    std::fs::create_dir_all(to).map_err(|e| CliError::io(e.to_string()))?;
    for entry in std::fs::read_dir(from).map_err(|e| CliError::io(e.to_string()))? {
        let entry = entry.map_err(|e| CliError::io(e.to_string()))?;
        let kind = entry.file_type().map_err(|e| CliError::io(e.to_string()))?;
        if kind.is_symlink() {
            return Err(CliError::usage(
                "baseline symlinks are not copied for bisect",
            ));
        }
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target).map_err(|e| CliError::io(e.to_string()))?;
        }
    }
    Ok(())
}

struct Reset<'a> {
    repo: &'a Path,
    active: bool,
}
impl Drop for Reset<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = git(self.repo, &["bisect", "reset"]);
        }
    }
}

fn step(args: &BisectArgs, repo: &Path, out: &Path) -> Result<u8, CliError> {
    let revision = checked_git(repo, &["rev-parse", "HEAD"])?;
    let dir = out.join("steps").join(&revision);
    let capture_dir = dir.join("capture");
    std::fs::create_dir_all(&capture_dir).map_err(|e| CliError::io(e.to_string()))?;
    #[cfg(windows)]
    let mut capture = {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new("cmd");
        // cmd does not understand the C-runtime escaping used by Command::arg.
        // /S strips this outer pair of quotes while preserving the user's
        // shell syntax, including quoted paths and command separators.
        command
            .args(["/D", "/S", "/C"])
            .raw_arg(format!("\"{}\"", args.capture));
        command
    };
    #[cfg(not(windows))]
    let mut capture = {
        let mut command = Command::new("sh");
        command.args(["-c", &args.capture]);
        command
    };
    let output = capture
        .current_dir(repo)
        .env("SACCADE_CAPTURE_DIR", &capture_dir)
        .output()
        .map_err(|e| CliError::io(e.to_string()))?;
    std::fs::write(dir.join("capture.stdout.log"), &output.stdout)
        .map_err(|e| CliError::io(e.to_string()))?;
    std::fs::write(dir.join("capture.stderr.log"), &output.stderr)
        .map_err(|e| CliError::io(e.to_string()))?;
    let dirty = !checked_git(repo, &["status", "--porcelain", "--untracked-files=all"])?.is_empty();
    let (verdict, reason, code) = if dirty {
        ("aborted", "capture command dirtied the repository", 128)
    } else if !output.status.success() {
        ("skip", "capture command failed", 125)
    } else {
        let report = saccade_core::run::run(
            &out.join("baseline"),
            &capture_dir,
            &dir.join("report"),
            &saccade_core::config::RunConfig::default(),
        )?;
        if report.is_empty_run()
            || report.totals.error > 0
            || report.totals.missing > 0
            || report.totals.new > 0
        {
            ("skip", "comparison has incomplete image evidence", 125)
        } else if args.perf
            && report.perf_diff.as_ref().is_none_or(|p| {
                p.comparability != Comparability::Qualified
                    || p.noise_comparability != Comparability::Qualified
                    || p.frame_change == FrameChange::Unknown
            })
        {
            ("skip", "performance evidence is not qualified", 125)
        } else if report.is_regression()
            || (args.perf
                && report
                    .perf_diff
                    .as_ref()
                    .is_some_and(|p| p.frame_change == FrameChange::Slower))
        {
            ("bad", "image or qualified frame regression", 1)
        } else {
            ("good", "comparison passed", 0)
        }
    };
    crate::local_cmd::write_value(
        &dir.join("step.json"),
        &json!({"revision":revision,"verdict":verdict,"reason":reason,"capture_exit":output.status.code(),"report":if dir.join("report/saccade-report.v1.json").is_file(){Some("report/saccade-report.v1.json")}else{None}}),
    )?;
    println!("saccade bisect: {revision} {verdict} ({reason})");
    Ok(code)
}

pub(crate) fn run(args: BisectArgs) -> Result<u8, CliError> {
    let repo = std::env::current_dir().map_err(|e| CliError::io(e.to_string()))?;
    if args.step {
        let out = args
            .out
            .as_ref()
            .ok_or_else(|| CliError::usage("internal bisect step needs --out"))?;
        return step(&args, &repo, out);
    }
    let good = args
        .good
        .as_deref()
        .ok_or_else(|| CliError::usage("bisect requires --good"))?;
    let bad = args
        .bad
        .as_deref()
        .ok_or_else(|| CliError::usage("bisect requires --bad"))?;
    let root = checked_git(&repo, &["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(root);
    if !checked_git(&root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err(CliError::usage("bisect refuses a dirty repository"));
    }
    let bisect_log = checked_git(&root, &["rev-parse", "--git-path", "BISECT_LOG"])?;
    let bisect_log = Path::new(&bisect_log);
    let bisect_log = if bisect_log.is_absolute() {
        bisect_log.to_path_buf()
    } else {
        root.join(bisect_log)
    };
    if bisect_log.exists() {
        return Err(CliError::usage("repository already has a bisect session"));
    }
    let original = checked_git(&root, &["rev-parse", "HEAD"])?;
    let good = checked_git(
        &root,
        &["rev-parse", "--verify", &format!("{good}^{{commit}}")],
    )?;
    let bad = checked_git(
        &root,
        &["rev-parse", "--verify", &format!("{bad}^{{commit}}")],
    )?;
    if !git(&root, &["merge-base", "--is-ancestor", &good, &bad])?
        .status
        .success()
        || good == bad
    {
        return Err(CliError::usage("--good must be an ancestor of --bad"));
    }
    let baseline =
        std::fs::canonicalize(&args.baseline).map_err(|e| CliError::io(e.to_string()))?;
    if !baseline.is_dir() {
        return Err(CliError::usage("--baseline must be a directory"));
    }
    let out = args.out.clone().unwrap_or_else(|| {
        root.parent().unwrap_or(Path::new(".")).join(format!(
            "{}-saccade-bisect",
            root.file_name().unwrap_or_default().to_string_lossy()
        ))
    });
    let out = if out.is_absolute() {
        out
    } else {
        repo.join(out)
    };
    if out.starts_with(&root)
        || baseline.starts_with(&out)
        || out.starts_with(&baseline)
        || out.exists()
    {
        return Err(CliError::usage(
            "bisect output must be new and outside the repository and baseline",
        ));
    }
    copy_tree(&baseline, &out.join("baseline"))?;
    std::fs::create_dir_all(out.join("steps")).map_err(|e| CliError::io(e.to_string()))?;
    // A disposable local clone keeps the original HEAD intact even if a
    // capture command writes tracked files or Git refuses a checkout.
    let work = out.join("repository");
    let clone = Command::new("git")
        .args(["clone", "--shared", "--quiet", "--"])
        .arg(&root)
        .arg(&work)
        .output()
        .map_err(|e| CliError::io(e.to_string()))?;
    if !clone.status.success() {
        return Err(CliError::usage(format!(
            "git clone: {}",
            String::from_utf8_lossy(&clone.stderr).trim()
        )));
    }
    let mut reset = Reset {
        repo: &work,
        active: true,
    };
    checked_git(&work, &["bisect", "start", &bad, &good])?;
    let exe = std::env::current_exe().map_err(|e| CliError::io(e.to_string()))?;
    let mut command = Command::new("git");
    command
        .current_dir(&work)
        .args(["bisect", "run"])
        .arg(exe)
        // Reports belong beside the evidence, outside the disposable Git clone.
        .arg("--report-index")
        .arg(out.join("reports/index.jsonl"))
        .arg("bisect")
        .arg("--step")
        .arg("--capture")
        .arg(&args.capture)
        .arg("--baseline")
        .arg(out.join("baseline"))
        .arg("--out")
        .arg(&out);
    if args.perf {
        command.arg("--perf");
    }
    let run = command.output().map_err(|e| CliError::io(e.to_string()))?;
    std::fs::write(
        out.join("git-bisect.log"),
        [&run.stdout[..], &run.stderr[..]].concat(),
    )
    .map_err(|e| CliError::io(e.to_string()))?;
    let first_bad = if run.status.success() {
        Some(checked_git(&work, &["rev-parse", "refs/bisect/bad"])?)
    } else {
        None
    };
    checked_git(&work, &["bisect", "reset"])?;
    reset.active = false;
    std::fs::remove_dir_all(&work).map_err(|e| CliError::io(e.to_string()))?;
    if checked_git(&root, &["rev-parse", "HEAD"])? != original {
        return Err(CliError::new(
            "restore_failed",
            "git bisect reset did not restore original HEAD",
        ));
    }
    let mut steps = Vec::<Value>::new();
    for entry in std::fs::read_dir(out.join("steps")).map_err(|e| CliError::io(e.to_string()))? {
        let entry = entry.map_err(|e| CliError::io(e.to_string()))?;
        let file = entry.path().join("step.json");
        if file.is_file() {
            steps.push(crate::local_cmd::read_value(&file)?);
        }
    }
    steps.sort_by(|a, b| a["revision"].as_str().cmp(&b["revision"].as_str()));
    let full = json!({"schema":"saccade-git-bisect.v1","first_bad":first_bad,"original_head":original,"steps":steps,"git_bisect_exit":run.status.code()});
    let result = out.join("saccade-git-bisect.v1.json");
    crate::local_cmd::write_value(&result, &full)?;
    if args.json {
        let mut value = crate::local_cmd::base_result("bisect");
        value["data"] = json!({"first_bad":first_bad,"steps":steps.len(),"status":if first_bad.is_some(){"found"}else{"inconclusive"}});
        value["artifact"] = crate::local_cmd::reference(&result)?;
        value["measurement"] = json!(if first_bad.is_some() {
            "regression"
        } else {
            "unknown"
        });
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&crate::local_cmd::bounded(value, 4096)?)?
        ))?;
    } else {
        crate::emit(&format!(
            "bisect: {}; first bad: {}; {} steps; evidence: {}\n",
            if first_bad.is_some() {
                "found"
            } else {
                "inconclusive"
            },
            first_bad.as_deref().unwrap_or("unknown"),
            steps.len(),
            result.display()
        ))?;
    }
    Ok(if first_bad.is_some() { 1 } else { 2 })
}
