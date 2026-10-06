//! Immutable preregistration around the existing blind vote and presentation primitives.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
/// Trial plan discriminator.
pub const PLAN_SCHEMA: &str = "saccade-visual-trial-plan.v1";
/// Frozen registration discriminator.
pub const FROZEN_SCHEMA: &str = "saccade-visual-trial-frozen.v1";
/// Trial transport receipt discriminator.
pub const RECEIPT_SCHEMA: &str = "saccade-visual-trial-receipt.v1";
/// One pair, roles kept in the private preregistration file.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// Private pair name.
    pub id: String,
    /// Relative first image file.
    pub first: String,
    /// Relative second image file.
    pub second: String,
    /// Optional white-pixel inclusion PNG, exact image dimensions.
    pub mask: Option<String>,
}
/// Declared metric, display scope, pairs and randomization chosen before inspection starts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Must equal PLAN_SCHEMA.
    pub schema: String,
    /// Neutral judging criterion; no implementation labels.
    pub objective: String,
    /// Metrics chosen before inspection; no scoring policy can change later.
    pub metrics: Vec<String>,
    /// Exact numerical comparison policy, hashed with the plan.
    pub spatial_policy: super::spatial::Policy,
    /// Fixed pairs and inclusion masks.
    pub pairs: Vec<Pair>,
    /// Seed for both pair order and within-pair permutation.
    pub seed: u64,
}
/// Private registration; never distribute this to blind judges.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frozen {
    /// Discriminator.
    pub schema: String,
    /// Immutable declared plan.
    pub plan: Plan,
    /// Canonical plan SHA-256.
    pub plan_sha256: String,
    /// Plan plus transitive inputs identity.
    pub registration_id: String,
    /// Absolute input root provenance, private only.
    pub root: String,
    /// Raw file identities, including masks.
    pub inputs: BTreeMap<String, String>,
}
/// Bounded state receipt, without seed, roles or original file paths.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    /// Receipt version.
    pub schema: String,
    /// Frozen registration identity.
    pub registration_id: String,
    /// Canonical plan hash.
    pub plan_sha256: String,
    /// registered, inspection_started, or vote_recorded.
    pub state: String,
    /// Public blind gallery path when started.
    pub presentation: Option<String>,
    /// Persisted vote count.
    pub votes: usize,
}
fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    crate::evidence::canonical::bytes(value).map_err(|e| Error::Config(e.to_string()))
}
fn hash<T: Serialize>(value: &T) -> Result<String> {
    Ok(crate::localized::digest(&canonical(value)?))
}
/// Parse and validate a bounded plan without decoding or displaying its images.
pub fn read_plan(path: &Path) -> Result<Plan> {
    let plan: Plan = serde_json::from_slice(&super::read(path, 1 << 20)?)?;
    if plan.schema != PLAN_SCHEMA
        || plan.objective.trim().is_empty()
        || plan.objective.len() > 4096
        || plan.pairs.is_empty()
        || plan.pairs.len() > 128
        || plan.metrics.is_empty()
        || plan.metrics.iter().any(|m| {
            !matches!(
                m.as_str(),
                "flip" | "rgb_mad" | "detail_energy" | "tile_bias" | "coverage"
            )
        })
    {
        return Err(Error::Config("invalid preregistered trial plan".into()));
    }
    plan.spatial_policy.validate()?;
    if matches!(
        plan.spatial_policy.background,
        Some(super::spatial::Background::Selection { .. })
    ) {
        return Err(Error::Config(
            "trial background selections must use the frozen pair mask instead".into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for pair in &plan.pairs {
        if pair.id.is_empty() || !ids.insert(&pair.id) || pair.first == pair.second {
            return Err(Error::Config("invalid/duplicate trial pair".into()));
        }
    }
    Ok(plan)
}
fn inputs(plan: &Plan, root: &Path) -> Result<BTreeMap<String, String>> {
    let mut inputs = BTreeMap::new();
    for name in plan
        .pairs
        .iter()
        .flat_map(|p| [Some(&p.first), Some(&p.second), p.mask.as_ref()])
        .flatten()
    {
        let path = super::relative(root, name)?;
        if !inputs.contains_key(name) {
            inputs.insert(
                name.clone(),
                crate::localized::digest(&super::read(&path, 128 << 20)?),
            );
        }
    }
    Ok(inputs)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(crate::run::io_err(
            "creating immutable trial artifact".into(),
        ))?;
    file.write_all(bytes).map_err(crate::run::io_err(
        "writing immutable trial artifact".into(),
    ))
}
fn receipt(f: &Frozen, state: &str, votes: usize) -> Receipt {
    Receipt {
        schema: RECEIPT_SCHEMA.into(),
        registration_id: f.registration_id.clone(),
        plan_sha256: f.plan_sha256.clone(),
        state: state.into(),
        presentation: (state != "registered").then(|| "public/index.html".into()),
        votes,
    }
}
/// Register hashes before any pixel inspection. The output must be a new directory.
pub fn register(plan_file: &Path, out: &Path) -> Result<Receipt> {
    let plan = read_plan(plan_file)?;
    let root = crate::paths::canonicalize(plan_file.parent().unwrap_or(Path::new(".")))
        .map_err(crate::run::io_err("resolving trial inputs".into()))?;
    let inputs = inputs(&plan, &root)?;
    let plan_sha256 = hash(&plan)?;
    let registration_id = hash(&serde_json::json!({"plan":plan_sha256,"inputs":inputs}))?;
    let frozen = Frozen {
        schema: FROZEN_SCHEMA.into(),
        plan,
        plan_sha256,
        registration_id,
        root: root.to_string_lossy().into_owned(),
        inputs,
    };
    if out.exists() {
        return Err(Error::NotEmptyOutDir(
            "trial registration needs a new directory".into(),
        ));
    }
    std::fs::create_dir_all(out)
        .map_err(crate::run::io_err("creating preregistered trial".into()))?;
    let bytes = canonical(&frozen)?;
    write_new(&out.join("frozen.json"), &bytes)?;
    write_new(
        &out.join("registration.sha256"),
        crate::localized::digest(&bytes).as_bytes(),
    )?;
    Ok(receipt(&frozen, "registered", 0))
}
/// Verify the current plan and all transitive inputs against immutable registration.
pub fn validate(plan_file: &Path, out: &Path) -> Result<Frozen> {
    regular_tree(out)?;
    let bytes = super::read(&out.join("frozen.json"), 1 << 20)?;
    let frozen: Frozen = serde_json::from_slice(&bytes)?;
    let expected = super::read(&out.join("registration.sha256"), 128)?;
    if frozen.schema != FROZEN_SCHEMA
        || crate::localized::digest(&bytes).as_bytes() != expected.as_slice()
        || hash(&read_plan(plan_file)?)? != frozen.plan_sha256
        || hash(&frozen.plan)? != frozen.plan_sha256
        || inputs(&frozen.plan, Path::new(&frozen.root))? != frozen.inputs
        || hash(&serde_json::json!({"plan":frozen.plan_sha256,"inputs":frozen.inputs}))?
            != frozen.registration_id
    {
        return Err(Error::TrialPlanChanged);
    }
    if out.join("inspection-started.sha256").exists()
        && super::read(&out.join("inspection-started.sha256"), 128)? != expected
    {
        return Err(Error::TrialPlanChanged);
    }
    Ok(frozen)
}
fn frozen_image(frozen: &Frozen, root: &Path, name: &str) -> Result<image::DynamicImage> {
    let path = super::relative(root, name)?;
    let bytes = super::read(&path, 128 << 20)?;
    if frozen.inputs.get(name) != Some(&crate::localized::digest(&bytes)) {
        return Err(Error::TrialPlanChanged);
    }
    super::decode(&bytes, &path)
}
fn regular_tree(root: &Path) -> Result<()> {
    let mut todo = vec![(root.to_path_buf(), 0)];
    let mut count = 0;
    while let Some((path, depth)) = todo.pop() {
        count += 1;
        if count > 1024 || depth > 8 {
            return Err(Error::Config("trial artifact tree exceeds limit".into()));
        }
        let meta = std::fs::symlink_metadata(&path)
            .map_err(crate::run::io_err("reading trial artifact type".into()))?;
        if meta.is_dir() {
            for entry in std::fs::read_dir(path).map_err(crate::run::io_err(
                "reading trial artifact directory".into(),
            ))? {
                todo.push((
                    entry
                        .map_err(crate::run::io_err("reading trial artifact entry".into()))?
                        .path(),
                    depth + 1,
                ));
            }
        } else if !meta.file_type().is_file() {
            return Err(Error::Config(
                "trial artifacts must be regular files and directories".into(),
            ));
        }
    }
    Ok(())
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
/// Freeze inspection state, then decode pairs and emit randomized blind strips.
/// Reuses the core viewer permutation and core judge vote model/store.
pub fn start(plan_file: &Path, out: &Path) -> Result<Receipt> {
    let frozen = validate(plan_file, out)?;
    let pin = super::read(&out.join("registration.sha256"), 128)?;
    if !out.join("inspection-started.sha256").exists() {
        write_new(&out.join("inspection-started.sha256"), &pin)?;
    }
    if out.join("presentation.sha256").exists() {
        verify_presentation(out)?;
        return Ok(receipt(
            &frozen,
            "inspection_started",
            crate::judge_vote::votes(out).len(),
        ));
    }
    std::fs::create_dir_all(out.join("public/strips"))
        .map_err(crate::run::io_err("creating blind presentations".into()))?;
    let mut items = Vec::new();
    let mut private = Vec::new();
    let mut gallery = String::new();
    let mut artifacts = BTreeMap::new();
    let order =
        crate::view::shuffled_order(frozen.plan.seed, "trial-pairs", frozen.plan.pairs.len());
    for (position, index) in order.into_iter().enumerate() {
        let pair = &frozen.plan.pairs[index];
        let root = Path::new(&frozen.root);
        let a = frozen_image(&frozen, root, &pair.first)?.to_rgba8();
        let b = frozen_image(&frozen, root, &pair.second)?.to_rgba8();
        if a.dimensions() != b.dimensions() {
            return Err(Error::Config("blind trial pair dimensions differ".into()));
        }
        let (w, h) = a.dimensions();
        let mut first = crate::compare::flatten_over(&a, 0);
        let mut second = crate::compare::flatten_over(&b, 0);
        let excluded = if let Some(mask) = &pair.mask {
            let mask = frozen_image(&frozen, root, mask)?.to_luma8();
            if mask.dimensions() != (w, h) || !mask.pixels().any(|p| p[0] > 0) {
                return Err(Error::Config(
                    "trial inclusion mask is empty or wrong-sized".into(),
                ));
            }
            let excluded: Vec<_> = mask.pixels().map(|p| p[0] == 0).collect();
            for (i, (a, b)) in first.pixels_mut().zip(second.pixels_mut()).enumerate() {
                if excluded[i] {
                    *a = image::Rgb([128; 3]);
                    *b = *a;
                }
            }
            Some(excluded)
        } else {
            None
        };
        let permutation = crate::view::shuffled_order(frozen.plan.seed, &pair.id, 2);
        let swapped = permutation[0] == 1;
        let panels = if swapped {
            [&second, &first]
        } else {
            [&first, &second]
        };
        let mut strip = image::RgbImage::new(w * 2, h);
        for (panel, img) in panels.iter().enumerate() {
            image::imageops::replace(&mut strip, *img, panel as i64 * i64::from(w), 0);
        }
        let id = crate::localized::digest(format!("{}|{index}", frozen.registration_id).as_bytes());
        let id = format!("t_{}", &id[..24]);
        let name = format!("{id}.png");
        let mut png = std::io::Cursor::new(Vec::new());
        strip
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|source| Error::Encode {
                path: out.join("public/strips").join(&name),
                source,
            })?;
        let png = png.into_inner();
        std::fs::write(out.join("public/strips").join(&name), &png)
            .map_err(crate::run::io_err("writing blind strip".into()))?;
        // Also retain the core vote transport's expected strip location.
        std::fs::create_dir_all(out.join("strips"))
            .map_err(crate::run::io_err("creating vote strips".into()))?;
        std::fs::write(out.join("strips").join(&name), &png)
            .map_err(crate::run::io_err("writing vote strip".into()))?;
        artifacts.insert(
            format!("public/strips/{name}"),
            crate::localized::digest(&png),
        );
        artifacts.insert(format!("strips/{name}"), crate::localized::digest(&png));
        let choices = vec![
            crate::judge_vote::Choice {
                answer: "P1".into(),
                label: "Image 1".into(),
                key: "1".into(),
            },
            crate::judge_vote::Choice {
                answer: "P2".into(),
                label: "Image 2".into(),
                key: "2".into(),
            },
            crate::judge_vote::Choice {
                answer: "unsure".into(),
                label: "Unsure".into(),
                key: "u".into(),
            },
        ];
        let evidence_hash = hash(
            &serde_json::json!({"strip":crate::localized::digest(&png),"objective":frozen.plan.objective,"choices":choices,"registration":frozen.registration_id}),
        )?;
        items.push(crate::judge_vote::VoteItem {id:id.clone(),item:id.clone(),order:if swapped {"ba"} else {"ab"}.into(),question:"preference".into(),kind:"preference".into(),kind_limits:"Blind preference is a recorded judgment, not baseline approval or capture/performance qualification.".into(),text:"Which image better meets the preregistered criterion?".into(),context:frozen.plan.objective.clone(),choices,strip:name.clone(),pairwise:true,evidence_hash});
        // Preregistered numerical diagnostics stay private until judging is complete.
        let cmp = crate::compare::compare(&second, &first, &Default::default())?;
        let metric =
            crate::compare::masked_metrics(&cmp.error_map, excluded.as_deref(), w, h, [0, 0, w, h])
                .ok_or_else(|| Error::Config("empty trial scope".into()))?;
        let gaps = if let Some(bg) = &frozen.plan.spatial_policy.background {
            Some((
                super::spatial::background(bg, &a, root, root)?,
                super::spatial::background(bg, &b, root, root)?,
            ))
        } else {
            None
        };
        let spatial = super::spatial::analyze(
            &a,
            &b,
            &cmp.error_map,
            excluded.as_deref(),
            gaps.as_ref().map(|(a, b)| (a.as_slice(), b.as_slice())),
            &frozen.plan.spatial_policy,
            &Default::default(),
            crate::diagnostics::ChangeClass::LocalStructure,
        )?;
        private.push(serde_json::json!({"item":id,"pair":pair.id,"swapped":swapped,"metrics":frozen.plan.metrics,"flip":metric,"spatial":spatial}));
        gallery.push_str(&format!("<figure><figcaption>Trial {} — Image 1 | Image 2 — item {}</figcaption><img style=\"max-width:100%\" src=\"strips/{}\" alt=\"Blind pair\"><label>Judgment <select data-item=\"{}\"><option value=\"\">Unanswered</option><option>P1</option><option>P2</option><option>unsure</option></select></label></figure>",position+1,escape(&id),escape(&name),escape(&id)));
    }
    let html = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Blind visual trial</title><h1>Blind visual trial</h1><p>{}</p>{gallery}<button id=\"save\">Export judgments</button><script>document.querySelector('#save').onclick=()=>{{const votes=[...document.querySelectorAll('select')].filter(x=>x.value).map(x=>({{item:x.dataset.item,answer:x.value}}));const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([JSON.stringify({{votes}})],{{type:'application/json'}}));a.download='judgments.json';a.click();}};</script>",
        escape(&frozen.plan.objective)
    );
    std::fs::write(out.join("public/index.html"), html.as_bytes())
        .map_err(crate::run::io_err("writing blind gallery".into()))?;
    artifacts.insert(
        "public/index.html".into(),
        crate::localized::digest(html.as_bytes()),
    );
    let run = crate::judge_vote::VoteRun {
        schema: crate::judge_vote::VOTES_SCHEMA.into(),
        run_id: frozen.registration_id[..16].into(),
        created_at_unix: 0,
        items,
    };
    let run_bytes = canonical(&run)?;
    std::fs::write(out.join("run.json"), &run_bytes)
        .map_err(crate::run::io_err("writing core vote run".into()))?;
    artifacts.insert("run.json".into(), crate::localized::digest(&run_bytes));
    let key = canonical(&private)?;
    std::fs::write(out.join("private-key.json"), &key)
        .map_err(crate::run::io_err("writing private trial mapping".into()))?;
    artifacts.insert("private-key.json".into(), crate::localized::digest(&key));
    let bytes = canonical(&artifacts)?;
    write_new(&out.join("presentation.json"), &bytes)?;
    write_new(
        &out.join("presentation.sha256"),
        crate::localized::digest(&bytes).as_bytes(),
    )?;
    Ok(receipt(&frozen, "inspection_started", 0))
}
fn verify_presentation(out: &Path) -> Result<()> {
    let bytes = super::read(&out.join("presentation.json"), 1 << 20)?;
    let expected = super::read(&out.join("presentation.sha256"), 128)?;
    if crate::localized::digest(&bytes).as_bytes() != expected {
        return Err(Error::TrialPlanChanged);
    }
    let artifacts: BTreeMap<String, String> = serde_json::from_slice(&bytes)?;
    for (name, hash) in artifacts {
        let path = super::relative(out, &name)?;
        if crate::localized::digest(&super::read(&path, 128 << 20)?) != hash {
            return Err(Error::TrialPlanChanged);
        }
    }
    Ok(())
}
/// Record an explicit blind verdict using the existing core judge vote validation/store.
pub fn vote(
    plan_file: &Path,
    out: &Path,
    voter: &str,
    item: &str,
    answer: &str,
) -> Result<Receipt> {
    let frozen = validate(plan_file, out)?;
    verify_presentation(out)?;
    crate::judge_vote::record_vote(out, voter, item, answer).map_err(Error::Config)?;
    Ok(receipt(
        &frozen,
        "vote_recorded",
        crate::judge_vote::votes(out).len(),
    ))
}
/// Validate all imported choices before writing any votes; partial I/O failures remain explicit.
pub fn import(plan_file: &Path, out: &Path, voter: &str, bytes: &[u8]) -> Result<Receipt> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Judgments {
        votes: Vec<Judgment>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Judgment {
        item: String,
        answer: String,
    }
    let frozen = validate(plan_file, out)?;
    verify_presentation(out)?;
    let judgments: Judgments = serde_json::from_slice(bytes)?;
    let run = crate::judge_vote::read_run(out).map_err(Error::Config)?;
    if judgments.votes.is_empty() || judgments.votes.len() > 128 {
        return Err(Error::Config("empty or oversized judgments".into()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for v in &judgments.votes {
        if !seen.insert(&v.item)
            || !run
                .items
                .iter()
                .any(|i| i.id == v.item && i.choices.iter().any(|c| c.answer == v.answer))
        {
            return Err(Error::Config("invalid or duplicate blind judgment".into()));
        }
    }
    for v in judgments.votes {
        crate::judge_vote::record_vote(out, voter, &v.item, &v.answer).map_err(Error::Config)?;
    }
    Ok(receipt(
        &frozen,
        "vote_recorded",
        crate::judge_vote::votes(out).len(),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preregistration_locks_metrics_masks_pairs_and_seed_after_inspection() {
        let tmp = tempfile::tempdir().unwrap();
        image::RgbImage::from_pixel(32, 32, image::Rgb([90; 3]))
            .save(tmp.path().join("a.png"))
            .unwrap();
        image::RgbImage::from_pixel(32, 32, image::Rgb([100; 3]))
            .save(tmp.path().join("b.png"))
            .unwrap();
        let mut plan = Plan {
            schema: PLAN_SCHEMA.into(),
            objective: "Prefer the clearer image".into(),
            metrics: vec!["flip".into()],
            spatial_policy: Default::default(),
            pairs: vec![Pair {
                id: "pair-a".into(),
                first: "a.png".into(),
                second: "b.png".into(),
                mask: None,
            }],
            seed: 17,
        };
        let file = tmp.path().join("plan.json");
        std::fs::write(&file, serde_json::to_vec(&plan).unwrap()).unwrap();
        let out = tmp.path().join("trial");
        let registered = register(&file, &out).unwrap();
        assert_eq!(registered.state, "registered");
        assert!(!out.join("public").exists());
        let started = start(&file, &out).unwrap();
        assert_eq!(started.state, "inspection_started");
        let html = std::fs::read_to_string(out.join("public/index.html")).unwrap();
        assert!(!html.contains("a.png"));
        assert!(!html.contains("b.png"));
        assert!(!html.contains("seed"));
        let run = crate::judge_vote::read_run(&out).unwrap();
        vote(&file, &out, "reviewer", &run.items[0].id, "P1").unwrap();
        assert_eq!(crate::judge_vote::votes(&out).len(), 1);
        let original = plan.clone();
        for choice in 0..5 {
            plan = original.clone();
            match choice {
                0 => plan.seed += 1,
                1 => plan.metrics.push("rgb_mad".into()),
                2 => plan.pairs[0].mask = Some("a.png".into()),
                3 => {
                    let pair = &mut plan.pairs[0];
                    std::mem::swap(&mut pair.first, &mut pair.second)
                }
                _ => plan.spatial_policy.tile_size = 16,
            }
            std::fs::write(&file, serde_json::to_vec(&plan).unwrap()).unwrap();
            assert!(matches!(start(&file, &out), Err(Error::TrialPlanChanged)));
            assert!(matches!(
                vote(&file, &out, "reviewer", &run.items[0].id, "P2"),
                Err(Error::TrialPlanChanged)
            ));
        }
        std::fs::write(&file, serde_json::to_vec(&original).unwrap()).unwrap();
        assert!(
            import(
                &file,
                &out,
                "reviewer",
                br#"{"votes":[{"item":"unknown","answer":"P1"}]}"#
            )
            .is_err()
        );
        std::fs::write(out.join("public/index.html"), "changed presentation").unwrap();
        assert!(matches!(
            vote(&file, &out, "reviewer", &run.items[0].id, "P2"),
            Err(Error::TrialPlanChanged)
        ));
    }
    #[test]
    fn changed_input_is_refused_before_display() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["a.png", "b.png"] {
            image::RgbImage::new(8, 8)
                .save(tmp.path().join(name))
                .unwrap();
        }
        let plan = Plan {
            schema: PLAN_SCHEMA.into(),
            objective: "Prefer clarity".into(),
            metrics: vec!["flip".into()],
            spatial_policy: Default::default(),
            pairs: vec![Pair {
                id: "p".into(),
                first: "a.png".into(),
                second: "b.png".into(),
                mask: None,
            }],
            seed: 1,
        };
        let file = tmp.path().join("plan.json");
        std::fs::write(&file, serde_json::to_vec(&plan).unwrap()).unwrap();
        let out = tmp.path().join("trial");
        register(&file, &out).unwrap();
        std::fs::write(tmp.path().join("a.png"), b"changed").unwrap();
        assert!(matches!(start(&file, &out), Err(Error::TrialPlanChanged)));
    }
}
