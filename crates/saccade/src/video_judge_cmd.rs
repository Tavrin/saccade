//! Credential-free preparation of reviewed video judge schedules.
use crate::agent::CliError;
use saccade_core::assist::{self, video};
use serde_json::json;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Absolute user-owned rubric JSON.
    #[arg(long)]
    pub(crate) rubric: PathBuf,
    /// Frame maps (one clip or A/B pair).
    #[arg(long, num_args = 1..=2)]
    pub(crate) frame_map: Vec<PathBuf>,
    /// Direct PNG still candidates; may be combined with a motion frame map.
    #[arg(long, num_args = 1..=2)]
    pub(crate) image: Vec<PathBuf>,
    /// Optional PNG reference, outside anonymous A/B candidate slots.
    #[arg(long)]
    pub(crate) reference: Option<PathBuf>,
    /// Optional sampled-clip reference, exclusive with a still reference.
    #[arg(long, conflicts_with = "reference")]
    pub(crate) reference_frame_map: Option<PathBuf>,
    /// Stable view ids in frame-map then image order; default is content identity.
    #[arg(long)]
    pub(crate) view_id: Vec<String>,
    /// Build a timestamped sheet from at most eight uniformly selected frames.
    #[arg(long)]
    pub(crate) contact_sheet: bool,
    /// One or more pinned model identities; unpriced identities are refused.
    #[arg(long, required = true)]
    pub(crate) model: Vec<String>,
    /// Fresh repetitions per item/model/order; replays never count as samples.
    #[arg(long, default_value_t = 1)]
    pub(crate) repeats: u32,
    /// Required provider-returned revision pin, one per model.
    #[arg(long, required = true)]
    pub(crate) revision: Vec<String>,
    #[arg(long, default_value_t = 1.0)]
    pub(crate) fps: f64,
    #[arg(long, default_value_t = 256)]
    pub(crate) max_edge: u32,
    #[arg(long, default_value = "2")]
    pub(crate) max_spend_usd: String,
    /// Fresh output directory, containing reviewed requests and exact cost plan.
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) experimental: bool,
}
pub(crate) fn run(a: Args, json_output: bool) -> Result<u8, CliError> {
    let plan = execute(a)?;
    if json_output {
        println!("{}", serde_json::to_string(&plan)?);
    } else {
        println!(
            "Prepared {} requests; reservation ${:.9}",
            plan["rows"].as_array().map_or(0, Vec::len),
            plan["reservation_nano_usd"].as_u64().unwrap_or(0) as f64 / 1e9
        );
    }
    Ok(0)
}
pub(crate) fn execute(a: Args) -> Result<serde_json::Value, CliError> {
    let prepare = || -> assist::Result<serde_json::Value> {
        if !(1..=32).contains(&a.repeats)
            || a.model
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != a.model.len()
            || a.model.is_empty()
            || a.model.len() > 16
            || !(1..=2).contains(&(a.frame_map.len() + a.image.len()))
            || a.view_id
                .iter()
                .any(|id| id.trim().is_empty() || id.len() > 128)
            || (!a.view_id.is_empty() && a.view_id.len() != a.frame_map.len() + a.image.len())
            || (a.reference.is_some() && a.reference_frame_map.is_some())
            || !a.experimental
            || a.model.len() != a.revision.len()
            || a.revision.iter().any(|r| r.is_empty() || r.len() > 128)
        {
            return Err(assist::Error::Invalid(
                "video experimental flag or model revision pins",
            ));
        }
        let cap = assist::openrouter::decimal_amount(&a.max_spend_usd, false)
            .filter(|n| {
                *n > 0
                    && *n <= 2_000_000_000
                    && Some(*n) == assist::openrouter::decimal_amount(&a.max_spend_usd, true)
            })
            .ok_or(assist::Error::Invalid(
                "video cap must be positive and at most 2 USD",
            ))?;
        let rubric = video::rubric(&a.rubric)?;
        let mut clips = vec![];
        let mut media = vec![];
        for path in &a.frame_map {
            let (clip, bytes) = video::load_frames(path, a.fps, a.max_edge, a.contact_sheet)?;
            clips.push(clip);
            media.push(bytes);
        }
        for path in &a.image {
            let (clip, bytes) = video::still(path, a.max_edge)?;
            clips.push(clip);
            media.push(bytes);
        }
        for (i, clip) in clips.iter_mut().enumerate() {
            clip.view_id = a
                .view_id
                .get(i)
                .cloned()
                .unwrap_or_else(|| clip.source.as_str().to_string());
        }
        let (references, reference_media) = if let Some(path) = &a.reference {
            let (clip, bytes) = video::still(path, a.max_edge)?;
            (vec![clip], bytes)
        } else if let Some(path) = &a.reference_frame_map {
            let (clip, bytes) = video::load_frames(path, a.fps, a.max_edge, a.contact_sheet)?;
            (vec![clip], bytes)
        } else {
            (vec![], vec![])
        };
        let mut rows = vec![];
        let mut costs = vec![];
        let mut total = 0u64;
        for (arm, (m, revision)) in a.model.iter().zip(&a.revision).enumerate() {
            for repeat in 0..a.repeats {
                for reverse in 0..if clips.len() == 2 { 2 } else { 1 } {
                    let mut clips = clips.clone();
                    for clip in &mut clips {
                        clip.sample_id = format!("{}-repeat-{repeat}", clip.sample_id);
                    }
                    let mut media = media.clone();
                    if reverse == 1 {
                        clips.reverse();
                        media.reverse();
                    }
                    let packet = video::Packet {
                        schema: video::VERSION.into(),
                        rubric: rubric.clone(),
                        clips,
                        references: references.clone(),
                    };
                    let media: Vec<_> = media
                        .into_iter()
                        .flatten()
                        .chain(reference_media.clone())
                        .collect();
                    let payload = video::request(&packet, &media, m)?;
                    let bound = assist::openrouter::admission(
                        &serde_json::to_vec(&payload).map_err(|_| assist::Error::Storage)?,
                        m,
                    )?;
                    total = total
                        .checked_add(bound.reservation)
                        .ok_or(assist::Error::Invalid("video reservation overflow"))?;
                    let root = format!("model-{arm}-repeat-{repeat}-order-{reverse}");
                    costs.push(json!({"root":root,"model":m,"order":reverse,"repeat":repeat,"image_table":assist::price::openrouter_image_table(m),"schema_projection":assist::structured_output::projection_policy(m),"input_bound":bound.bounds.input,"output_bound":bound.bounds.output,"reservation_nano_usd":bound.reservation,"request_hash":assist::digest(&packet)?}));
                    rows.push(json!({"root":root,"model":m,"revision":revision,"payload":payload}));
                }
            }
        }
        if total > cap {
            return Err(assist::Error::Policy("video schedule exceeds cap"));
        }
        let schedule_bytes =
            serde_json::to_vec_pretty(&rows).map_err(|_| assist::Error::Storage)?;
        if schedule_bytes.len() > 32 * 1024 * 1024 {
            return Err(assist::Error::Invalid(
                "video schedule exceeds runner file limit",
            ));
        }
        // Build and admit everything before publishing a fresh plan.
        std::fs::create_dir(&a.out).map_err(|_| assist::Error::Storage)?;
        assist::write(&a.out.join("requests.json"), &rows)?;
        let plan = json!({"schema":video::PLAN_SCHEMA,"authority":"advisory; offline plan, no qualification","price_id":assist::price::OPENROUTER_PRICE_ID,"price_source":assist::price::OPENROUTER_PRICE_SOURCE,"price_date":assist::price::OPENROUTER_PRICE_DATE,"frame_bound":assist::price::OPENROUTER_IMAGE_TABLE,"safety_factor":2,"max_spend_nano_usd":cap,"reservation_nano_usd":total,"rows":costs,"native_mp4":"unavailable: no pinned video pricing/token rule or calibrated video receipt","scores_format":"saccade-video-judge-scores.v1 JSONL; scripts/assist/video_scores.py"});
        assist::write(&a.out.join("plan.json"), &plan)?;
        Ok(plan)
    };
    prepare().map_err(|e| CliError::usage(e.code()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn args(dir: &std::path::Path) -> Args {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        std::fs::write(dir.join("frame.png"), &png).unwrap();
        let frame = saccade_core::frame_map::Frame {
            index: 0,
            timestamp_s: 0.0,
            file: "frame.png".into(),
            sha256: Some(
                saccade_core::evidence::canonical::Digest::of_bytes(&png).as_str()[7..].into(),
            ),
        };
        for name in ["one", "two"] {
            let mut frame = frame.clone();
            if name == "two" {
                frame.index = 1;
            }
            assist::write(
                &dir.join(format!("{name}.json")),
                &saccade_core::frame_map::FrameMap {
                    schema: saccade_core::frame_map::SCHEMA.into(),
                    nominal_fps: Some(1.0),
                    frames: vec![frame],
                },
            )
            .unwrap();
        }
        std::fs::write(
            dir.join("rubric.json"),
            include_bytes!("../../../examples/rubrics/motion-naturalness.json"),
        )
        .unwrap();
        Args {
            rubric: dir.join("rubric.json"),
            frame_map: vec![dir.join("one.json"), dir.join("two.json")],
            image: vec![],
            reference: None,
            reference_frame_map: None,
            view_id: vec![],
            contact_sheet: false,
            repeats: 1,
            model: vec![assist::price::OPENROUTER_MODEL.into()],
            revision: vec!["absent".into()],
            fps: 1.0,
            max_edge: 8,
            max_spend_usd: "2".into(),
            out: dir.join("plan"),
            experimental: true,
        }
    }
    #[test]
    fn video_judge_cli_plan_both_orders_exact_cost_schema_and_cap_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let input = args(dir.path());
        let plan = execute(input).unwrap();
        assert_eq!(plan["rows"].as_array().unwrap().len(), 2);
        let rows: Vec<serde_json::Value> =
            assist::decode(&std::fs::read(dir.path().join("plan/requests.json")).unwrap()).unwrap();
        let first = video::packet(&rows[0]["payload"]).unwrap();
        let second = video::packet(&rows[1]["payload"]).unwrap();
        assert_eq!(first.clips[0].source, second.clips[1].source);
        assert_eq!(first.clips[1].source, second.clips[0].source);
        assert_eq!(
            plan["reservation_nano_usd"].as_u64().unwrap(),
            plan["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["reservation_nano_usd"].as_u64().unwrap())
                .sum::<u64>()
        );
        let schema: serde_json::Value =
            serde_json::from_str(saccade_core::schema_catalog::get(video::PLAN_SCHEMA).unwrap())
                .unwrap();
        assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&plan));
        let mut input = args(dir.path());
        input.out = dir.path().join("refused");
        input.max_spend_usd = "0.000000001".into();
        assert!(execute(input).is_err());
        assert!(!dir.path().join("refused").exists());
    }
    #[test]
    fn independent_arms_repeats_are_fresh_and_fully_reserved() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = args(dir.path());
        input.model.push(assist::price::OPENROUTER_GPT_MODEL.into());
        input.revision.push("absent".into());
        input.repeats = 3;
        let plan = execute(input).unwrap();
        assert_eq!(plan["rows"].as_array().unwrap().len(), 12);
        let rows: Vec<serde_json::Value> =
            assist::decode(&std::fs::read(dir.path().join("plan/requests.json")).unwrap()).unwrap();
        let mut payloads = std::collections::BTreeSet::new();
        for row in &rows {
            assert!(payloads.insert(serde_json::to_string(&row["payload"]).unwrap()));
            let bound = assist::openrouter::admission(
                &serde_json::to_vec(&row["payload"]).unwrap(),
                row["model"].as_str().unwrap(),
            )
            .unwrap();
            assert!(bound.reservation > 0);
        }
        for arm in 0..2 {
            for repeat in 0..3 {
                let first = video::packet(&rows[arm * 6 + repeat * 2]["payload"]).unwrap();
                let reverse = video::packet(&rows[arm * 6 + repeat * 2 + 1]["payload"]).unwrap();
                assert_eq!(first.clips[0].sample_id, reverse.clips[1].sample_id);
                if repeat > 0 {
                    let previous =
                        video::packet(&rows[arm * 6 + (repeat - 1) * 2]["payload"]).unwrap();
                    assert_ne!(first.clips[0].sample_id, previous.clips[0].sample_id);
                }
            }
        }
        assert_ne!(
            rows[0]["payload"]["response_format"],
            rows[6]["payload"]["response_format"]
        );
        let schema: serde_json::Value =
            serde_json::from_str(saccade_core::schema_catalog::get(video::PLAN_SCHEMA).unwrap())
                .unwrap();
        assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&plan));
        let mut duplicate = args(dir.path());
        duplicate.model.push(duplicate.model[0].clone());
        duplicate.revision.push("absent".into());
        duplicate.out = dir.path().join("duplicate");
        assert!(execute(duplicate).is_err());
        let mut bad = args(dir.path());
        bad.repeats = 0;
        assert!(execute(bad).is_err());
    }
    #[test]
    fn judge_parity_cli_mixed_inputs_reference_and_stable_ids() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = args(dir.path());
        input.frame_map.truncate(1);
        input.image = vec![dir.path().join("frame.png")];
        input.reference = Some(dir.path().join("frame.png"));
        input.view_id = vec!["sequence".into(), "image".into()];
        execute(input).unwrap();
        let rows: Vec<serde_json::Value> =
            assist::decode(&std::fs::read(dir.path().join("plan/requests.json")).unwrap()).unwrap();
        let first = video::packet(&rows[0]["payload"]).unwrap();
        let reverse = video::packet(&rows[1]["payload"]).unwrap();
        assert_eq!(first.clips[0].kind, "motion");
        assert_eq!(first.clips[1].kind, "still");
        assert_eq!(first.clips[0].view_id, reverse.clips[1].view_id);
        assert_eq!(first.clips[0].sample_id, reverse.clips[1].sample_id);
        assert_eq!(first.references[0].source, reverse.references[0].source);
        let mut single = args(dir.path());
        single.frame_map.clear();
        single.image = vec![dir.path().join("frame.png")];
        single.out = dir.path().join("single");
        assert_eq!(
            execute(single).unwrap()["rows"].as_array().unwrap().len(),
            1
        );
    }
}
