//! Offline blind packets and advisory disagreement aggregation.
use crate::agent::CliError;
use saccade_core::review_board;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(clap::Subcommand)]
enum Operation {
    /// Generate separate blind forms/files; keep trial.json private.
    Prepare {
        plan: PathBuf,
        /// A new directory whose parent exists.
        #[arg(long)]
        out: PathBuf,
    },
    /// Collect offline ballots; output must be outside the prepared bundle.
    Collect {
        trial: PathBuf,
        /// Returned ballot file; repeat for each rater. Omitted raters stay missing.
        #[arg(long = "ballot")]
        ballots: Vec<PathBuf>,
        /// New operator-only board directory; never distribute before voting closes.
        #[arg(long)]
        out: PathBuf,
    },
}
pub(crate) fn run(args: Args, json: bool) -> Result<u8, CliError> {
    let (mode, out, data) = match args.operation {
        Operation::Prepare { plan, out } => {
            let value = review_board::prepare(
                &review_board::read(&plan, review_board::PLAN_SCHEMA)?,
                &plan,
                &out,
            )?;
            ("review.board.prepare", out, value)
        }
        Operation::Collect {
            trial,
            ballots,
            out,
        } => {
            if ballots.len() > 32 {
                return Err(CliError::usage("at most 32 ballot files"));
            }
            let ballots = ballots
                .iter()
                .map(|p| review_board::read(p, review_board::BALLOT_SCHEMA))
                .collect::<Result<Vec<_>, _>>()?;
            let board = review_board::collect(&trial, &ballots, &out)?;
            let data = serde_json::json!({"trial_id":board["trial_id"],"report_id":board["report_id"],"items":board["items"].as_array().map_or(0, Vec::len),"raters":board["raters"].as_array().map_or(0, Vec::len),"agreement":board["agreement"],"approval_authority":false});
            ("review.board.collect", out, data)
        }
    };
    if json {
        let mut result = crate::local_cmd::base_result(mode);
        let artifact = out.join(if mode == "review.board.prepare" {
            "trial.json"
        } else {
            "saccade-review-board.v2.json"
        });
        result["artifact"] = crate::local_cmd::reference(&artifact)?;
        result["data"] = data;
        result["verdict"] = serde_json::json!("advisory");
        result["limits"] = serde_json::json!([
            "Votes and agreement statistics grant no baseline approval authority."
        ]);
        crate::emit(&format!("{}\n", serde_json::to_string(&result)?))?;
    } else {
        crate::emit(&format!(
            "{mode}: {} — advisory only; no baseline approval\n",
            out.display()
        ))?;
    }
    Ok(0)
}
