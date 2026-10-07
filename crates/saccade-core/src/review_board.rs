//! Offline categorical blind review. Packets disclose only anonymous pixels;
//! aggregation records disagreement and missingness, with no approval authority.
use crate::{Error, Result, general::input};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
    path::{Path, PathBuf},
};

/// Operator-only source plan schema.
pub const PLAN_SCHEMA: &str = "saccade-review-board-plan.v1";
/// Operator-only registered trial schema.
pub const TRIAL_SCHEMA: &str = "saccade-review-board-trial.v1";
/// Blind packet schema (contains no source mapping or verdict).
pub const PACKET_SCHEMA: &str = "saccade-review-board-packet.v1";
/// Individual ballot schema.
pub const BALLOT_SCHEMA: &str = "saccade-review-board-ballot.v1";
/// Unlinked board contract; linked output uses its v2 successor.
pub const BOARD_SCHEMA: &str = "saccade-review-board.v1";
const MAX_JSON: u64 = 4 * 1024 * 1024;
const QUESTION: &str = "Which image do you prefer visually? Judge only the displayed pixels.";
const CHOICES: [&str; 4] = ["first", "second", "tie", "unsure"];

/// A rater identity; tool raters may fill the same offline ballot as humans.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rater {
    /// Operator's rater ID, never shown to other raters.
    pub id: String,
    /// `human` or `tool`; no inference of independence is made.
    pub kind: String,
}
/// A pair with stable operator-side labels.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// Stable item ID, absent from blind packets.
    pub id: String,
    /// Operator grouping, absent from blind packets.
    pub group: String,
    /// First source image, relative to the plan file.
    pub first: PathBuf,
    /// Second source image, relative to the plan file.
    pub second: PathBuf,
}
/// Source plan for a single categorical preference question across all pairs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Always [`PLAN_SCHEMA`].
    pub schema: String,
    /// At least two, at most 32 unique raters.
    pub raters: Vec<Rater>,
    /// One to 256 unique pairs.
    pub pairs: Vec<Pair>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Presentation {
    id: String,
    evidence_hash: String,
    first: String,
    second: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    schema: String,
    trial_id: String,
    rater_key: String,
    question: String,
    choices: Vec<String>,
    items: Vec<Presentation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisteredItem {
    pair: Pair,
    first_hash: String,
    second_hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisteredRater {
    rater: Rater,
    key: String,
    packet: String,
    packet_hash: String,
    reversed: Vec<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Trial {
    schema: String,
    trial_id: String,
    items: Vec<RegisteredItem>,
    raters: Vec<RegisteredRater>,
}
/// One optional response. Null means missing; `unsure` is an explicit category.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    /// Opaque presentation ID from the rater's packet.
    pub id: String,
    /// Exact question and displayed PNG binding.
    pub evidence_hash: String,
    /// `first`, `second`, `tie`, `unsure`, or null.
    pub answer: Option<String>,
    /// Rater's explanation (at most 2048 bytes), treated only as data.
    pub note: String,
}
/// A returned individual ballot. This self-declaration does not authenticate a person.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ballot {
    /// Always [`BALLOT_SCHEMA`].
    pub schema: String,
    /// Exact trial identity.
    pub trial_id: String,
    /// Assigned opaque rater key.
    pub rater_key: String,
    /// True only if the rater did not see peer votes or a tool verdict before voting.
    pub blind_confirmed: bool,
    /// Responses; omitted items remain missing.
    pub responses: Vec<Response>,
}
fn invalid(code: &'static str, message: impl Into<String>) -> Error {
    Error::ReviewBoard {
        code,
        message: message.into(),
    }
}
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(invalid("invalid_board_input", message))
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn binding(parts: &impl Serialize) -> Result<String> {
    Ok(hash(&serde_json::to_vec(parts)?))
}
fn printable(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control)
}
fn validate_plan(plan: &Plan) -> Result<()> {
    require(plan.schema == PLAN_SCHEMA, "unsupported plan schema")?;
    require((2..=32).contains(&plan.raters.len()), "need 2..32 raters")?;
    require((1..=256).contains(&plan.pairs.len()), "need 1..256 pairs")?;
    let mut ids = BTreeSet::new();
    for rater in &plan.raters {
        require(
            printable(&rater.id) && ids.insert(&rater.id),
            "invalid or duplicate rater ID",
        )?;
        require(
            matches!(rater.kind.as_str(), "human" | "tool"),
            "rater kind must be human or tool",
        )?;
    }
    ids.clear();
    for pair in &plan.pairs {
        require(
            printable(&pair.id) && ids.insert(&pair.id) && printable(&pair.group),
            "invalid or duplicate pair ID/group",
        )?;
    }
    Ok(())
}
/// Read a bounded, strict versioned board input; successors require a reader upgrade.
pub fn read<T: serde::de::DeserializeOwned>(path: &Path, schema: &'static str) -> Result<T> {
    let bytes = input::bytes(path, MAX_JSON)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if value["schema"] != schema {
        let actual = value["schema"].as_str().unwrap_or("missing");
        if let Some((prefix, version)) = schema.rsplit_once(".v")
            && actual
                .strip_prefix(&format!("{prefix}.v"))
                .and_then(|v| v.parse::<u32>().ok())
                > version.parse::<u32>().ok()
        {
            return Err(Error::VersionSkew {
                actual: actual.into(),
                supported: schema,
            });
        }
        return Err(invalid(
            "invalid_board_input",
            format!("expected {schema}, found {actual}"),
        ));
    }
    // Deserialize original bytes so duplicate fields cannot disappear in a Value projection.
    serde_json::from_slice(&bytes).map_err(|e| invalid("invalid_board_input", e.to_string()))
}
fn write(path: &Path, value: &impl Serialize) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)
        .map_err(crate::run::io_err("writing board input".into()))
}
fn png(path: &Path) -> Result<Vec<u8>> {
    let bytes = input::bytes(path, input::MAX_BYTES)?;
    require(
        matches!(
            image::guess_format(&bytes),
            Ok(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ),
        "blind board supports only 8-bit PNG/JPEG rasters",
    )?;
    let pixels = input::decode(&bytes)?;
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| invalid("invalid_board_input", e.to_string()))?;
    Ok(out.into_inner())
}
fn staging(out: &Path) -> Result<tempfile::TempDir> {
    require(
        std::fs::symlink_metadata(out).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "output must be a new directory",
    )?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    tempfile::Builder::new()
        .prefix(".review-board-")
        .tempdir_in(parent)
        .map_err(crate::run::io_err("staging review board".into()))
}
fn publish(stage: tempfile::TempDir, out: &Path) -> Result<()> {
    std::fs::rename(stage.path(), out).map_err(crate::run::io_err("publishing review board".into()))
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn script_data(v: &impl Serialize) -> Result<String> {
    Ok(serde_json::to_string(v)?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026"))
}
fn form(packet: &Packet, ballot: &Ballot) -> Result<String> {
    Ok(include_str!("../assets/review-board-form.html")
        .replace("__PACKET__", &script_data(packet)?)
        .replace("__BALLOT__", &script_data(ballot)?))
}
/// Prepare a new operator bundle with one separately distributable blind packet per rater.
/// Source paths/IDs, order mapping and rater roster remain only in `trial.json`.
/// Pixels are re-encoded as anonymous PNGs without source metadata.
pub fn prepare(plan: &Plan, source: &Path, out: &Path) -> Result<Value> {
    validate_plan(plan)?;
    let stage = staging(out)?;
    let base = source.parent().unwrap_or(Path::new("."));
    let mut items = Vec::new();
    // Retain only two images at a time; on-disk private staging is removed on failure.
    let pixels = stage.path().join("pixels");
    std::fs::create_dir(&pixels).map_err(crate::run::io_err("staging pixels".into()))?;
    for (i, pair) in plan.pairs.iter().enumerate() {
        let first = png(&base.join(&pair.first))?;
        let second = png(&base.join(&pair.second))?;
        items.push(RegisteredItem {
            pair: pair.clone(),
            first_hash: hash(&first),
            second_hash: hash(&second),
        });
        for (side, bytes) in [("a", first), ("b", second)] {
            std::fs::write(pixels.join(format!("{i}-{side}.png")), bytes)
                .map_err(crate::run::io_err("staging pixels".into()))?;
        }
    }
    let trial_id = binding(&(plan, &items, QUESTION, CHOICES))?;
    let mut raters = Vec::new();
    for (r, rater) in plan.raters.iter().enumerate() {
        let key = binding(&(&trial_id, r, &rater.id))?;
        let dir_name = format!("rater-{:03}", r + 1);
        let packet_dir = stage.path().join(&dir_name);
        std::fs::create_dir(&packet_dir)
            .map_err(crate::run::io_err("creating blind packet".into()))?;
        let mut presentations = Vec::new();
        let mut reversed = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let id = binding(&(&key, i))?;
            let reverse = Sha256::digest(id.as_bytes())[0] & 1 == 1;
            let (a, b) = if reverse { ("b", "a") } else { ("a", "b") };
            let first = format!("pair-{:03}-1.png", i + 1);
            let second = format!("pair-{:03}-2.png", i + 1);
            for (side, name) in [(a, &first), (b, &second)] {
                std::fs::copy(
                    pixels.join(format!("{i}-{side}.png")),
                    packet_dir.join(name),
                )
                .map_err(crate::run::io_err("copying blind pixels".into()))?;
            }
            let hashes = if reverse {
                (&item.second_hash, &item.first_hash)
            } else {
                (&item.first_hash, &item.second_hash)
            };
            presentations.push(Presentation {
                id,
                evidence_hash: binding(&(QUESTION, CHOICES, hashes))?,
                first,
                second,
            });
            reversed.push(reverse);
        }
        let packet = Packet {
            schema: PACKET_SCHEMA.into(),
            trial_id: trial_id.clone(),
            rater_key: key.clone(),
            question: QUESTION.into(),
            choices: CHOICES.iter().map(|s| (*s).into()).collect(),
            items: presentations,
        };
        let ballot = Ballot {
            schema: BALLOT_SCHEMA.into(),
            trial_id: trial_id.clone(),
            rater_key: key.clone(),
            blind_confirmed: false,
            responses: packet
                .items
                .iter()
                .map(|p| Response {
                    id: p.id.clone(),
                    evidence_hash: p.evidence_hash.clone(),
                    answer: None,
                    note: String::new(),
                })
                .collect(),
        };
        write(&packet_dir.join("packet.json"), &packet)?;
        write(&packet_dir.join("ballot.json"), &ballot)?;
        std::fs::write(packet_dir.join("index.html"), form(&packet, &ballot)?)
            .map_err(crate::run::io_err("writing blind form".into()))?;
        raters.push(RegisteredRater {
            rater: rater.clone(),
            key,
            packet: dir_name,
            packet_hash: binding(&packet)?,
            reversed,
        });
    }
    std::fs::remove_dir_all(pixels)
        .map_err(crate::run::io_err("removing private staging".into()))?;
    write(
        &stage.path().join("trial.json"),
        &Trial {
            schema: TRIAL_SCHEMA.into(),
            trial_id: trial_id.clone(),
            items,
            raters,
        },
    )?;
    publish(stage, out)?;
    Ok(
        json!({"trial_id":trial_id,"raters":plan.raters.len(),"items":plan.pairs.len(),"approval_authority":false}),
    )
}

/// Nominal Krippendorff alpha, coincidence-weighted for unequal item vote counts.
/// Items with fewer than two votes do not contribute; nothing is imputed.
pub fn nominal_alpha(rows: &[Vec<Option<String>>]) -> Value {
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut n = 0usize;
    let mut disagreement = 0.0;
    let mut comparable_items = 0;
    for row in rows {
        let mut local = BTreeMap::<&str, usize>::new();
        for answer in row.iter().flatten() {
            *local.entry(answer).or_default() += 1;
        }
        let m: usize = local.values().sum();
        if m < 2 {
            continue;
        }
        comparable_items += 1;
        n += m;
        let squares: usize = local.values().map(|c| c * c).sum();
        disagreement += (m * m - squares) as f64 / (m - 1) as f64;
        for (answer, count) in local {
            *counts.entry(answer).or_default() += count;
        }
    }
    let squares: usize = counts.values().map(|c| c * c).sum();
    let expected = if n >= 2 {
        (n * n - squares) as f64 / (n * (n - 1)) as f64
    } else {
        0.0
    };
    let observed = if n > 0 { disagreement / n as f64 } else { 0.0 };
    let reason = if n < 2 {
        Some("fewer than two votes on every item")
    } else if expected == 0.0 {
        Some("only one category in comparable votes; expected disagreement is zero")
    } else {
        None
    };
    json!({"method":"krippendorff_alpha_nominal","value":if reason.is_none() { Some(1.0-observed/expected) } else { None },"unavailable_reason":reason,"comparable_items":comparable_items,"coincidence_votes":n,"observed_disagreement":if n>0 {Some(observed)} else {None},"expected_disagreement":if n>=2 {Some(expected)} else {None}})
}
fn verify_trial(trial: &Trial, base: &Path) -> Result<Vec<Packet>> {
    let plan = Plan {
        schema: PLAN_SCHEMA.into(),
        pairs: trial.items.iter().map(|i| i.pair.clone()).collect(),
        raters: trial.raters.iter().map(|r| r.rater.clone()).collect(),
    };
    validate_plan(&plan)?;
    require(
        binding(&(&plan, &trial.items, QUESTION, CHOICES))? == trial.trial_id,
        "trial registration changed",
    )?;
    let mut packets = Vec::new();
    for (r, registered) in trial.raters.iter().enumerate() {
        require(
            registered.packet == format!("rater-{:03}", r + 1)
                && registered.key == binding(&(&trial.trial_id, r, &registered.rater.id))?,
            "rater registration changed",
        )?;
        let packet: Packet = read(
            &base.join(&registered.packet).join("packet.json"),
            PACKET_SCHEMA,
        )?;
        require(
            binding(&packet)? == registered.packet_hash
                && packet.trial_id == trial.trial_id
                && packet.rater_key == registered.key
                && packet.items.len() == trial.items.len()
                && registered.reversed.len() == trial.items.len()
                && packet.question == QUESTION
                && packet.choices == CHOICES,
            "blind packet changed",
        )?;
        for (i, presentation) in packet.items.iter().enumerate() {
            let expected_id = binding(&(&registered.key, i))?;
            let reverse = Sha256::digest(expected_id.as_bytes())[0] & 1 == 1;
            require(
                reverse == registered.reversed[i]
                    && presentation.id == expected_id
                    && presentation.first == format!("pair-{:03}-1.png", i + 1)
                    && presentation.second == format!("pair-{:03}-2.png", i + 1),
                "presentation mapping changed",
            )?;
            let first = hash(&input::bytes(
                &base.join(&registered.packet).join(&presentation.first),
                input::MAX_BYTES,
            )?);
            let second = hash(&input::bytes(
                &base.join(&registered.packet).join(&presentation.second),
                input::MAX_BYTES,
            )?);
            let item = &trial.items[i];
            let expected = if reverse {
                (&item.second_hash, &item.first_hash)
            } else {
                (&item.first_hash, &item.second_hash)
            };
            if (&first, &second) != expected
                || binding(&(QUESTION, CHOICES, (&first, &second)))? != presentation.evidence_hash
            {
                return Err(invalid(
                    "board_evidence_changed",
                    "displayed pixels changed after preparation",
                ));
            }
        }
        // The generated form itself must still be the blind template, never an aggregate.
        let blank = Ballot {
            schema: BALLOT_SCHEMA.into(),
            trial_id: trial.trial_id.clone(),
            rater_key: registered.key.clone(),
            blind_confirmed: false,
            responses: packet
                .items
                .iter()
                .map(|p| Response {
                    id: p.id.clone(),
                    evidence_hash: p.evidence_hash.clone(),
                    answer: None,
                    note: String::new(),
                })
                .collect(),
        };
        require(
            input::bytes(&base.join(&registered.packet).join("index.html"), MAX_JSON)?
                == form(&packet, &blank)?.as_bytes(),
            "blind form changed",
        )?;
        packets.push(packet);
    }
    Ok(packets)
}
/// Collect separately returned ballots into a new operator-only board directory.
/// Reject duplicate raters/items, foreign/stale bindings and exposed ballots.
/// Output must be outside the prepared bundle so packets cannot gain peer votes.
pub fn collect(trial_path: &Path, ballots: &[Ballot], out: &Path) -> Result<Value> {
    let trial: Trial = read(trial_path, TRIAL_SCHEMA)?;
    let base = trial_path.parent().unwrap_or(Path::new("."));
    let base_abs =
        crate::paths::canonicalize(base).map_err(crate::run::io_err("resolving trial".into()))?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent_abs = crate::paths::canonicalize(parent)
        .map_err(crate::run::io_err("resolving board output".into()))?;
    require(
        !parent_abs.starts_with(base_abs),
        "board output must be outside the prepared bundle",
    )?;
    let stage = staging(out)?;
    let packets = verify_trial(&trial, base)?;
    let mut matrix = vec![vec![None; trial.raters.len()]; trial.items.len()];
    let mut notes = vec![vec![String::new(); trial.raters.len()]; trial.items.len()];
    let mut seen = BTreeSet::new();
    for ballot in ballots {
        require(
            ballot.schema == BALLOT_SCHEMA && ballot.trial_id == trial.trial_id,
            "foreign ballot or unsupported schema",
        )?;
        let r = trial
            .raters
            .iter()
            .position(|r| r.key == ballot.rater_key)
            .ok_or_else(|| invalid("invalid_board_ballot", "unknown rater key"))?;
        require(
            seen.insert(r),
            "duplicate rater ballot; revisions must be selected explicitly",
        )?;
        if !ballot.blind_confirmed {
            return Err(invalid(
                "board_blind_protocol",
                "rater must confirm no prior peer votes or tool verdict exposure",
            ));
        }
        require(
            ballot.responses.len() <= trial.items.len(),
            "too many responses",
        )?;
        let mut answered = BTreeSet::new();
        for response in &ballot.responses {
            let i = packets[r]
                .items
                .iter()
                .position(|p| p.id == response.id)
                .ok_or_else(|| invalid("invalid_board_ballot", "unknown presentation ID"))?;
            require(
                answered.insert(i) && response.evidence_hash == packets[r].items[i].evidence_hash,
                "duplicate or stale response",
            )?;
            require(response.note.len() <= 2048, "note exceeds 2048 bytes")?;
            if let Some(answer) = &response.answer {
                require(
                    CHOICES.contains(&answer.as_str()),
                    "unsupported answer category",
                )?;
                let stable = match (answer.as_str(), trial.raters[r].reversed[i]) {
                    ("first", true) => "second",
                    ("second", true) => "first",
                    _ => answer.as_str(),
                };
                matrix[i][r] = Some(stable.to_owned());
            }
            notes[i][r] = response.note.clone();
        }
    }
    let items: Vec<Value> = trial.items.iter().enumerate().map(|(i, item)| {
        let mut counts = BTreeMap::<&str, usize>::new();
        for a in matrix[i].iter().flatten() { *counts.entry(a).or_default() += 1; }
        let count = matrix[i].iter().flatten().count();
        let votes: Vec<Value> = trial.raters.iter().enumerate().map(|(r, rater)| json!({"rater":rater.rater.id,"answer":matrix[i][r],"note":notes[i][r],"status":if matrix[i][r].is_some(){"recorded"}else{"missing"}})).collect();
        json!({"id":item.pair.id,"group":item.pair.group,"votes":votes,"counts":counts,"recorded":count,"missing":trial.raters.len()-count,"agreement":if count<2 {"insufficient"} else if counts.len()==1 {"unanimous"} else {"disagreement"}})
    }).collect();
    let raters: Vec<Value> = trial.raters.iter().enumerate().map(|(r, rater)| {
        let mut matched=0; let mut compared=0;
        for row in &matrix {
            if let Some(own) = &row[r] {
                for (peer, vote) in row.iter().enumerate() {
                    if peer != r && let Some(vote) = vote { compared += 1; matched += usize::from(own == vote); }
                }
            }
        }
        let recorded = matrix.iter().filter(|row| row[r].is_some()).count();
        json!({"id":rater.rater.id,"kind":rater.rater.kind,"ballot_received":seen.contains(&r),"recorded":recorded,"missing":matrix.len()-recorded,"peer_comparisons":compared,"peer_matches":matched,"peer_disagreements":compared-matched,"peer_agreement":if compared>0 {Some(matched as f64/compared as f64)} else {None}})
    }).collect();
    let groups: BTreeSet<_> = trial.items.iter().map(|i| &i.pair.group).collect();
    let by_group: BTreeMap<_, _> = groups
        .into_iter()
        .map(|group| {
            let rows: Vec<_> = trial
                .items
                .iter()
                .zip(&matrix)
                .filter(|(i, _)| &i.pair.group == group)
                .map(|(_, row)| row.clone())
                .collect();
            (group, nominal_alpha(&rows))
        })
        .collect();
    let report = json!({"schema":BOARD_SCHEMA,"trial_id":trial.trial_id,"verdict":"advisory","approval_authority":false,"question":QUESTION,"categories":CHOICES,"items":items,"raters":raters,"agreement":nominal_alpha(&matrix),"agreement_by_group":by_group,"limits":["Preference votes and aggregates never approve a baseline or override a deterministic failure.","Null and omitted responses are missing, never imputed; unsure and tie are explicit nominal categories.","Alpha excludes singleton items; per-rater peer agreement is descriptive, not accuracy or reliability.","Blindness is enforced by packet content and a rater exposure declaration; no person authentication or independence claim.","Share only the assigned rater directory before voting; keep trial mappings, source context and this board private until voting closes."]});
    let report = crate::report_links::decorate(&report)?;
    write(&stage.path().join("saccade-review-board.v2.json"), &report)?;
    std::fs::write(stage.path().join("index.html"), board_html(&report)?)
        .map_err(crate::run::io_err("writing disagreement board".into()))?;
    publish(stage, out)?;
    // Index the published path; staging paths would be stale after rename.
    crate::report_links::index(&out.join("saccade-review-board.v2.json"), &report)?;
    Ok(report)
}
fn board_html(report: &Value) -> Result<String> {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Review disagreement board</title><style>body{font:16px system-ui;margin:2rem;max-width:1100px}table{border-collapse:collapse;width:100%;margin:1rem 0}td,th{border:1px solid #888;padding:.5rem;text-align:left;vertical-align:top}small{display:block;white-space:pre-wrap}.scroll{overflow:auto}.missing{background:#eee;color:#333}</style><h1>Review disagreement board</h1><p>Advisory only. Votes never approve a baseline. Null means missing.</p>",
    );
    html.push_str(&format!("<h2>Agreement statistic</h2><pre>{}</pre><h2>Items and notes</h2><div class=\"scroll\"><table><tr><th>Item / group</th><th>Agreement / missing</th>", esc(&serde_json::to_string_pretty(&report["agreement"])?)));
    if let Some(raters) = report["raters"].as_array() {
        for r in raters {
            html.push_str(&format!(
                "<th>{}</th>",
                esc(r["id"].as_str().unwrap_or_default())
            ));
        }
    }
    html.push_str("</tr>");
    if let Some(items) = report["items"].as_array() {
        for item in items {
            html.push_str(&format!(
                "<tr><th>{}<small>{}</small></th><td>{}<small>Missing: {}</small></td>",
                esc(item["id"].as_str().unwrap_or_default()),
                esc(item["group"].as_str().unwrap_or_default()),
                esc(item["agreement"].as_str().unwrap_or_default()),
                item["missing"]
            ));
            if let Some(votes) = item["votes"].as_array() {
                for vote in votes {
                    html.push_str(&format!(
                        "<td class=\"{}\">{}<small>{}</small></td>",
                        if vote["answer"].is_null() {
                            "missing"
                        } else {
                            "recorded"
                        },
                        esc(vote["answer"].as_str().unwrap_or("Missing")),
                        esc(vote["note"].as_str().unwrap_or_default())
                    ));
                }
            }
            html.push_str("</tr>");
        }
    }
    html.push_str("</table></div><h2>Per-rater agreement and missingness</h2><div class=\"scroll\"><table><tr><th>Rater / kind</th><th>Recorded / missing</th><th>Matches / disagreements / peer comparisons</th><th>Peer agreement</th></tr>");
    if let Some(raters) = report["raters"].as_array() {
        for r in raters {
            html.push_str(&format!(
                "<tr><th>{}<small>{}</small></th><td>{} / {}</td><td>{} / {} / {}</td><td>{}</td></tr>",
                esc(r["id"].as_str().unwrap_or_default()),
                esc(r["kind"].as_str().unwrap_or_default()),
                r["recorded"],
                r["missing"],
                r["peer_matches"],
                r["peer_disagreements"],
                r["peer_comparisons"],
                r["peer_agreement"]
            ));
        }
    }
    html.push_str("</table></div><h2>Limits</h2><ul>");
    if let Some(limits) = report["limits"].as_array() {
        for limit in limits {
            html.push_str(&format!(
                "<li>{}</li>",
                esc(limit.as_str().unwrap_or_default())
            ));
        }
    }
    html.push_str("</ul></html>");
    Ok(html)
}
