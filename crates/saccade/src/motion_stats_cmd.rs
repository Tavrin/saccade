//! Frame-map intake and constructed-negative CLI; external extraction remains producer-owned.
use crate::agent::CliError;
use saccade_core::{frame_map, localized::digest, motion_stats as ms};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(clap::Args)]
pub(crate) struct StatsArgs {
    /// Reference saccade-frame-map.v1; image paths relative to map directory.
    reference: PathBuf,
    /// Candidate saccade-frame-map.v1.
    candidate: PathBuf,
    /// Static binary inclusion region, shared mask:PATH grammar.
    #[arg(long)]
    mask: Option<String>,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
#[derive(clap::Args)]
pub(crate) struct CalibrationArgs {
    /// Independent positive frame maps; repeat this flag for multiple sources.
    #[arg(long, required = true)]
    positive: Vec<PathBuf>,
    /// Comma-separated strengths in (0,1].
    #[arg(long, value_delimiter = ',', default_value = "0.5,1")]
    strengths: Vec<f64>,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    /// Required lower exact 95% strict-win bound.
    #[arg(long, default_value_t = 0.8)]
    threshold: f64,
    /// Optional higher-is-better external scores, bound to this generated manifest SHA256.
    #[arg(long)]
    scores: Option<PathBuf>,
    /// New output directory; refuses to overwrite generated evidence.
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>, CliError> {
    Ok(saccade_core::evidence_quality::read(path, limit)?)
}
fn image(path: &Path) -> Result<(image::RgbaImage, String), CliError> {
    let b = bytes(path, 16 << 20)?;
    let mut r = image::ImageReader::new(std::io::Cursor::new(&b))
        .with_guessed_format()
        .map_err(|e| CliError::io(e.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(256);
    limits.max_image_height = Some(256);
    limits.max_alloc = Some(1 << 20);
    r.limits(limits);
    let im = r.decode().map_err(|e| CliError::usage(e.to_string()))?;
    if !matches!(
        im.color(),
        image::ColorType::Rgb8
            | image::ColorType::Rgba8
            | image::ColorType::L8
            | image::ColorType::La8
    ) {
        return Err(CliError::usage("motion input requires 8-bit SDR"));
    }
    Ok((im.to_rgba8(), digest(&b)))
}
fn load(path: &Path) -> Result<(ms::Clip, serde_json::Value), CliError> {
    let b = bytes(path, 1 << 20)?;
    let mut map: frame_map::FrameMap = serde_json::from_slice(&b)?;
    frame_map::check(&map, None, frame_map::CheckPolicy::default(), None)?;
    if !(4..=128).contains(&map.frames.len()) {
        return Err(CliError::usage("requires 4..128 frames"));
    }
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| CliError::io(e.to_string()))?;
    let mut frames = Vec::new();
    let mut hashes = Vec::new();
    for f in &mut map.frames {
        let p = root
            .join(&f.file)
            .canonicalize()
            .map_err(|e| CliError::io(e.to_string()))?;
        if !p.starts_with(&root) {
            return Err(CliError::usage("frame escapes map root"));
        }
        let (im, hash) = image(&p)?;
        if f.sha256.as_ref().is_some_and(|h| *h != hash) {
            return Err(CliError::usage("frame SHA256 mismatch"));
        }
        f.sha256 = Some(hash.clone());
        hashes.push(hash);
        frames.push(im);
    }
    let c = ms::Clip { frames, map };
    ms::validate(&c)?;
    Ok((
        c,
        serde_json::json!({"frame_map_sha256":digest(&b),"frame_sha256":hashes}),
    ))
}
pub(crate) fn stats(a: StatsArgs) -> Result<u8, CliError> {
    let (r, rpin) = load(&a.reference)?;
    let (c, cpin) = load(&a.candidate)?;
    if r.frames[0].dimensions() != c.frames[0].dimensions() {
        return Err(CliError::usage(
            "reference/candidate dimensions must match; no implicit resize",
        ));
    }
    let mut mask_pin = None;
    let mask = a
        .mask
        .as_deref()
        .map(|s| -> Result<Vec<bool>, CliError> {
            let spec = saccade_core::mask_spec::parse_effect_spec(s)?;
            let saccade_core::mask_spec::EffectSelection::Mask { image: path } = spec.selection
            else {
                return Err(CliError::usage("motion scope supports mask:PATH only"));
            };
            let b = bytes(Path::new(&path), 16 << 20)?;
            mask_pin = Some(digest(&b));
            let mut reader = image::ImageReader::new(std::io::Cursor::new(&b))
                .with_guessed_format()
                .map_err(|e| CliError::io(e.to_string()))?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(256);
            limits.max_image_height = Some(256);
            limits.max_alloc = Some(1 << 20);
            reader.limits(limits);
            let im = reader
                .decode()
                .map_err(|e| CliError::usage(e.to_string()))?;
            let image::DynamicImage::ImageLuma8(im) = im else {
                return Err(CliError::usage("mask must be binary L8 PNG"));
            };
            if im.dimensions() != r.frames[0].dimensions()
                || im.as_raw().iter().any(|v| !matches!(v, 0 | 255))
            {
                return Err(CliError::usage("mask dimensions or binary values invalid"));
            }
            if (im.as_raw().iter().filter(|&&v| v == 255).count() as u64) < spec.min_pixels {
                return Err(CliError::usage("mask below declared minimum inclusion"));
            }
            Ok(im.as_raw().iter().map(|v| *v == 255).collect())
        })
        .transpose()?;
    let report = ms::compare(
        ms::measure(&r, mask.as_deref())?,
        ms::measure(&c, mask.as_deref())?,
    );
    let mut value = serde_json::to_value(report)?;
    value["sources"] =
        serde_json::json!({"reference":rpin,"candidate":cpin,"mask_sha256":mask_pin});
    finish(&a.out, &value, a.json)?;
    Ok(0)
}
fn finish(path: &Path, value: &serde_json::Value, json: bool) -> Result<(), CliError> {
    crate::local_cmd::write_value(path, value)?;
    if json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(value)?))?;
    } else {
        crate::emit(&format!("{}: {}\n", value["schema"], path.display()))?;
    }
    Ok(())
}
fn save(c: &ms::Clip, dir: &Path) -> Result<(PathBuf, serde_json::Value), CliError> {
    std::fs::create_dir_all(dir).map_err(|e| CliError::io(e.to_string()))?;
    let mut map = c.map.clone();
    for (i, (im, f)) in c.frames.iter().zip(&mut map.frames).enumerate() {
        f.file = format!("{i:04}.png");
        let p = dir.join(&f.file);
        im.save(&p).map_err(|e| CliError::io(e.to_string()))?;
        f.sha256 = Some(digest(&bytes(&p, 16 << 20)?));
    }
    let path = dir.join("frames.json");
    crate::local_cmd::write_value(&path, &serde_json::to_value(&map)?)?;
    let pin = digest(&bytes(&path, 1 << 20)?);
    Ok((
        path,
        serde_json::json!({"frame_map_sha256":pin,"frames":map.frames}),
    ))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct External {
    schema: String,
    manifest_sha256: String,
    scores: Vec<ExternalScore>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalScore {
    id: String,
    scorer: String,
    positive: f64,
    negative: f64,
}
pub(crate) fn calibration(a: CalibrationArgs) -> Result<u8, CliError> {
    if a.positive.len() > 64
        || a.strengths.is_empty()
        || a.strengths.len() > 8
        || a.strengths
            .iter()
            .any(|s| !s.is_finite() || *s <= 0.0 || *s > 1.0)
        || !a.threshold.is_finite()
        || !(0.5..=1.0).contains(&a.threshold)
    {
        return Err(CliError::usage(
            "maximum 64 positive maps, 8 strengths in (0,1], threshold [0.5,1]",
        ));
    }
    let unique: std::collections::BTreeSet<_> = a.strengths.iter().map(|s| s.to_bits()).collect();
    if unique.len() != a.strengths.len() {
        return Err(CliError::usage("duplicate strengths"));
    }
    if a.out.exists() {
        return Err(CliError::usage(
            "calibration output must be a new directory",
        ));
    }
    let mut positives = Vec::new();
    let mut source_ids = std::collections::BTreeSet::new();
    for p in &a.positive {
        let (c, pin) = load(p)?;
        if c.frames.len() < 8 {
            return Err(CliError::usage(
                "calibration positives require 8..128 frames so dropped negatives retain four frames",
            ));
        }
        let identity = digest(&serde_json::to_vec(&pin["frame_sha256"])?);
        if !source_ids.insert(identity) {
            return Err(CliError::usage("duplicate positive sequence"));
        }
        positives.push((c, pin));
    }
    std::fs::create_dir_all(&a.out).map_err(|e| CliError::io(e.to_string()))?;
    let mut entries = Vec::new();
    let mut scores = Vec::new();
    let mut keys = BTreeMap::new();
    for (source, (c, _pin)) in positives.iter().enumerate() {
        let reference = ms::measure(c, None)?;
        let baseline = ms::compare(reference.clone(), reference.clone()).distances;
        for class in ms::CLASSES {
            for (si, &strength) in a.strengths.iter().enumerate() {
                let id = format!("p{source}-{class}-s{si}");
                let seed = a.seed.wrapping_add(source as u64);
                let negative = ms::degrade(c, class, strength, seed)?;
                let (_, generated) = save(&negative, &a.out.join(&id))?;
                let report = ms::compare(reference.clone(), ms::measure(&negative, None)?);
                crate::local_cmd::write_value(
                    &a.out.join(&id).join("motion-stats.json"),
                    &serde_json::to_value(&report)?,
                )?;
                for (metric, &distance) in &report.distances {
                    if let Some(&positive) = baseline.get(metric) {
                        let ratio = metric.starts_with("spectral_band_");
                        let negative = if ratio {
                            -(distance - 1.0).abs()
                        } else {
                            -distance
                        };
                        // Roundoff alone cannot qualify a constructed separator.
                        let negative = if negative.abs() <= 1e-12 {
                            0.0
                        } else {
                            negative
                        };
                        scores.push(ms::Score {
                            id: id.clone(),
                            scorer: metric.clone(),
                            class: (*class).into(),
                            strength,
                            positive: if ratio {
                                -(positive - 1.0).abs()
                            } else {
                                -positive
                            },
                            negative,
                        });
                    }
                }
                keys.insert(id.clone(), ((*class).to_string(), strength));
                entries.push(serde_json::json!({"id":id,"positive_source":source,"class":class,"strength":strength,"seed":seed,"generated":generated}));
            }
        }
    }
    let manifest = serde_json::json!({"schema":"saccade-motion-degradations.v1","generator":"saccade-motion-degradations-v1","seed":a.seed,"positive_sources":positives.iter().map(|(_,p)|p).collect::<Vec<_>>(),"entries":entries});
    let manifest_path = a.out.join("manifest.json");
    crate::local_cmd::write_value(&manifest_path, &manifest)?;
    let manifest_hash = digest(&bytes(&manifest_path, 16 << 20)?);
    if let Some(path) = a.scores {
        let external: External = serde_json::from_slice(&bytes(&path, 16 << 20)?)?;
        if external.schema != "saccade-motion-scores.v1"
            || external.manifest_sha256 != manifest_hash
        {
            return Err(CliError::usage(
                "external scores schema/manifest hash mismatch",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut scorers = std::collections::BTreeSet::new();
        for s in external.scores {
            let Some((class, strength)) = keys.get(&s.id) else {
                return Err(CliError::usage("unknown external score id"));
            };
            if s.scorer.is_empty()
                || !s.positive.is_finite()
                || !s.negative.is_finite()
                || !seen.insert((s.id.clone(), s.scorer.clone()))
            {
                return Err(CliError::usage("invalid or duplicate external score"));
            }
            let scorer = format!("external:{}", s.scorer);
            scorers.insert(scorer.clone());
            scores.push(ms::Score {
                id: s.id,
                scorer,
                class: class.clone(),
                strength: *strength,
                positive: s.positive,
                negative: s.negative,
            });
        }
        if scorers.is_empty()
            || scorers
                .iter()
                .any(|s| scores.iter().filter(|v| &v.scorer == s).count() != keys.len())
        {
            return Err(CliError::usage(
                "external scorer must cover every generated negative",
            ));
        }
    }
    let mut report = ms::calibrate(&scores, a.threshold)?;
    // A scorer must cover every independent source at every declared strength.
    let rows = report["rows"]
        .as_array_mut()
        .ok_or_else(|| CliError::usage("invalid calibration rows"))?;
    for row in rows.iter_mut() {
        if row["pairs"].as_u64() != Some(positives.len() as u64) {
            row["trusted"] = false.into();
            row["incomplete_support"] = true.into();
        }
    }
    let invalid: std::collections::BTreeSet<(String, String)> = rows
        .iter()
        .filter(|r| !r["trusted"].as_bool().unwrap_or(false))
        .map(|r| {
            (
                r["scorer"].as_str().unwrap_or("").to_string(),
                r["class"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let counts: BTreeMap<(String, String), usize> =
        rows.iter().fold(BTreeMap::new(), |mut m, r| {
            *m.entry((
                r["scorer"].as_str().unwrap_or("").to_string(),
                r["class"].as_str().unwrap_or("").to_string(),
            ))
            .or_default() += 1;
            m
        });
    if let Some(scorers) = report["scorers"].as_array_mut() {
        for scorer in scorers {
            let name = scorer["scorer"].as_str().unwrap_or("").to_string();
            if let Some(classes) = scorer["trusted_for"].as_array_mut() {
                classes.retain(|c| {
                    let key = (name.clone(), c.as_str().unwrap_or("").to_string());
                    !invalid.contains(&key) && counts.get(&key) == Some(&a.strengths.len())
                });
            }
        }
    }
    report["independent_sources"] = positives.len().into();
    report["builtin_numerical_tie_floor_native_units"] = 1e-12.into();
    report["unavailable_evidence"] = "Missing per-source or per-strength metric support forbids trust; omitted observations are unavailable, never wins.".into();

    report["manifest_sha256"] = manifest_hash.into();
    report["score_orientation"] =
        "higher is better; built-in scores = negative reference distance".into();
    crate::local_cmd::write_value(&a.out.join("scores.json"), &serde_json::to_value(&scores)?)?;
    finish(&a.out.join("calibration.json"), &report, a.json)?;
    Ok(0)
}
