//! In-process Rust sections. Unsupported flags remain explicit partial rows;
//! workers never fall back to a CLI or silently ignore requested measurements.
use super::{invalid, probe, read};
use crate::{Result, media};
use serde_json::Value;
use std::path::Path;

pub(super) fn invoke(analyzer: &media::Analyzer, args: &[String]) -> Result<(i32, Value)> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(invalid("empty batch worker command"));
    };
    if command == "compare" {
        return compare(args);
    }
    let result = match command {
        "batch-probe" => probe(Path::new(&args[1]), Path::new(&args[2])),
        "analyze-media" => analyze(analyzer, args),
        "mask-metrics" => mask_metrics(args),
        "watermark" => watermark(args),
        "inspect-image" => inspect(args),
        "tofu" => tofu(args),
        "text-legibility" => legibility(args),
        _ => Err(invalid(format!(
            "batch section {command} is unavailable in-process; no CLI fallback"
        ))),
    }?;
    let exit_code = match result["state"].as_str() {
        Some("candidates" | "illegible") => 1,
        Some("insufficient_evidence" | "unavailable") => 4,
        _ => 0,
    };
    Ok((exit_code, result))
}
fn analyze(analyzer: &media::Analyzer, args: &[String]) -> Result<Value> {
    let mut strict = false;
    let mut profile = None;
    let mut sizes = Vec::new();
    let mut options = media::Options::default();
    let mut options_seen = false;
    let mut flags = args[2..].iter();
    while let Some(flag) = flags.next() {
        let (name, inline) = flag
            .split_once('=')
            .map_or((flag.as_str(), None), |(n, v)| (n, Some(v)));
        if name == "--json" && inline.is_none() {
            continue;
        }
        if name == "--strict" && inline.is_none() {
            strict = true;
            continue;
        }
        if !matches!(name, "--profile" | "--options" | "--output-size") {
            return Err(invalid(format!(
                "batch analyze-media flag {name} is unavailable in-process; use the shared model configuration"
            )));
        }
        let value = inline
            .or_else(|| flags.next().map(String::as_str))
            .ok_or_else(|| invalid(format!("batch flag {name} needs a value")))?;
        match name {
            "--profile" => {
                profile = Some(match value {
                    "cpu-lite" => media::Profile::CpuLite,
                    "cpu-full" => media::Profile::CpuFull,
                    "gpu" => media::Profile::Gpu,
                    _ => return Err(invalid("unknown media profile")),
                });
            }
            "--options" => {
                if options_seen {
                    return Err(invalid("repeated media options file"));
                }
                options_seen = true;
                let bytes = read(Path::new(value))?;
                if bytes.len() > 65536 {
                    return Err(invalid("media options exceed 64 KiB"));
                }
                options = serde_json::from_slice(&bytes)?;
            }
            "--output-size" => {
                let (w, h) = value
                    .split_once('x')
                    .ok_or_else(|| invalid("output size must be WxH"))?;
                sizes.push([
                    w.parse().map_err(|_| invalid("invalid output width"))?,
                    h.parse().map_err(|_| invalid("invalid output height"))?,
                ]);
            }
            _ => unreachable!(),
        }
    }
    if options.description {
        return Err(invalid("batch cannot dispatch description providers"));
    }
    options.strict |= strict;
    options.output_sizes.extend(sizes);
    // As in the CLI, an options profile overrides the analyzer's preset.
    options.profile = options.profile.or(profile);
    let bytes = read(Path::new(&args[1]))?;
    let result = analyzer
        .analyze_bytes(&bytes, &options)
        .map_err(|e| invalid(e.to_string()))?;
    Ok(serde_json::to_value(result)?)
}

struct Flags(std::collections::BTreeMap<String, Vec<String>>);
impl Flags {
    fn parse(args: &[String], start: usize, booleans: &[&str]) -> Result<Self> {
        let mut flags = std::collections::BTreeMap::<String, Vec<String>>::new();
        let mut args = args[start..].iter();
        while let Some(flag) = args.next() {
            if flag == "--json" {
                continue;
            }
            let (name, inline) = flag
                .split_once('=')
                .map_or((flag.as_str(), None), |(n, v)| (n, Some(v)));
            if !name.starts_with("--") {
                return Err(invalid("batch flags must use --name value or --name=value"));
            }
            if booleans.contains(&name) && inline.is_some() {
                return Err(invalid(format!("boolean flag {name} takes no value")));
            }
            let value = if booleans.contains(&name) {
                "true"
            } else {
                inline
                    .or_else(|| args.next().map(String::as_str))
                    .ok_or_else(|| invalid(format!("{name} needs a value")))?
            };
            flags.entry(name.into()).or_default().push(value.into());
        }
        Ok(Self(flags))
    }
    fn many(&mut self, name: &str) -> Vec<String> {
        self.0.remove(name).unwrap_or_default()
    }
    fn one(&mut self, name: &str) -> Result<Option<String>> {
        let mut values = self.many(name);
        if values.len() > 1 {
            return Err(invalid(format!("repeated batch flag {name}")));
        }
        Ok(values.pop())
    }
    fn number<T: std::str::FromStr>(&mut self, name: &str, default: T) -> Result<T> {
        self.one(name)?
            .map(|v| v.parse().map_err(|_| invalid(format!("invalid {name}"))))
            .unwrap_or(Ok(default))
    }
    fn finish(self) -> Result<()> {
        if let Some(flag) = self.0.keys().next() {
            Err(invalid(format!(
                "batch flag {flag} is unavailable in-process (no CLI fallback)"
            )))
        } else {
            Ok(())
        }
    }
}
fn compare(args: &[String]) -> Result<(i32, Value)> {
    let mut flags = Flags::parse(
        args,
        3,
        &["--allow-empty", "--fail-on-new", "--require-matching-meta"],
    )?;
    let mut config = if let Some(path) = flags.one("--config")? {
        crate::config::RunConfig::from_toml_file(Path::new(&path))?
    } else {
        crate::config::RunConfig::default()
    };
    if let Some(threshold) = flags.one("--threshold")? {
        config.default_threshold = threshold
            .parse()
            .map_err(|_| invalid("invalid threshold"))?;
        config.explicit_tolerances = true;
    }
    config.pixels_per_degree = flags.number("--ppd", config.pixels_per_degree)?;
    if let Some(metric) = flags.one("--metric")? {
        config.explicit_tolerances = true;
        config.default_metric = match metric.as_str() {
            "mean" => crate::Metric::Mean,
            "p95" => crate::Metric::P95,
            "p99" => crate::Metric::P99,
            "max" => crate::Metric::Max,
            _ => return Err(invalid("metric must be mean, p95, p99 or max")),
        };
    }
    config.allow_empty |= flags.one("--allow-empty")?.is_some();
    config.fail_on_new |= flags.one("--fail-on-new")?.is_some();
    config.meta.required |= flags.one("--require-matching-meta")?.is_some();
    let out = flags
        .one("--out")?
        .ok_or_else(|| invalid("missing comparison output"))?;
    flags.finish()?;
    // Same report producer as CLI comparison, retaining original sidecar paths.
    let report = crate::run::run(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&out),
        &config,
    )?;
    let exit_code = i32::from(report.is_regression());
    Ok((exit_code, serde_json::to_value(report)?))
}
fn mask_metrics(args: &[String]) -> Result<Value> {
    use crate::mask_metrics::{self as mask, ClassSelection, ClassSpec, Policy};
    let mut flags = Flags::parse(args, 3, &["--each-label"])?;
    let named = flags.many("--class");
    let each = flags.one("--each-label")?.is_some();
    if each && !named.is_empty() {
        return Err(invalid("each-label conflicts with class"));
    }
    let classes = if each {
        ClassSelection::EachLabel
    } else if named.is_empty() {
        ClassSelection::Foreground
    } else {
        ClassSelection::Named(
            named
                .iter()
                .map(|s| ClassSpec::parse(s))
                .collect::<Result<_>>()?,
        )
    };
    let policy = Policy {
        classes,
        void: flags
            .one("--void")?
            .as_deref()
            .map(ClassSpec::parse)
            .transpose()?,
        boundary_tolerance_px: flags.number("--boundary-px", 2.)?,
    };
    flags.finish()?;
    Ok(serde_json::to_value(mask::evaluate(
        &mask::read_labels(Path::new(&args[1]))?,
        &mask::read_labels(Path::new(&args[2]))?,
        &policy,
    )?)?)
}
fn watermark(args: &[String]) -> Result<Value> {
    use crate::wave7::{
        vision::VisionImage,
        watermark::{self as wm, DwtConfig},
    };
    let mut flags = Flags::parse(args, 2, &[])?;
    let payload = flags.one("--expected-payload")?;
    let quantization_step = flags.number("--quantization-step", 36.)?;
    let minimum_agreement = flags.number("--minimum-agreement", 0.9)?;
    flags.finish()?;
    let legacy = payload
        .map(|s| {
            if s.is_empty()
                || s.len() > 128
                || !s.len().is_multiple_of(2)
                || !s.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(invalid("payload needs 1..64 hex-encoded bytes"));
            }
            let expected_payload = (0..s.len())
                .step_by(2)
                .map(|i| {
                    u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| invalid("invalid hex payload"))
                })
                .collect::<Result<_>>()?;
            Ok(DwtConfig {
                expected_payload,
                quantization_step,
                minimum_agreement,
            })
        })
        .transpose()?;
    let image = VisionImage::load(Path::new(&args[1])).map_err(|e| invalid(e.to_string()))?;
    Ok(serde_json::to_value(
        wm::inspect(&image, None, legacy.as_ref()).map_err(|e| invalid(e.to_string()))?,
    )?)
}
fn inspect(args: &[String]) -> Result<Value> {
    use crate::general::{assessment, input, integrity, registration};
    let mut flags = Flags::parse(args, 2, &["--include-gps"])?;
    let include_gps = flags.one("--include-gps")?.is_some();
    let out = flags
        .one("--out")?
        .ok_or_else(|| invalid("missing inspection output"))?;
    flags.finish()?;
    let bytes = read(Path::new(&args[1]))?;
    let image = input::decode(&bytes)?;
    let mut headers = integrity::headers(&bytes, &image, include_gps)?;
    headers["compression"]["history_indicators"] = crate::general::forensics::indicators(
        &image,
        &headers["compression"]["quantisation_tables"],
    )?;
    let copies = registration::copy_move(&image).map_err(|e| invalid(e.to_string()))?;
    let credentials = crate::general::credentials::inspect(&bytes)?;
    std::fs::create_dir_all(&out).map_err(super::io)?;
    let rgb = crate::compare::flatten_over(&image, 255);
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 90)
        .encode_image(&rgb)
        .map_err(|e| invalid(e.to_string()))?;
    let recompressed = crate::compare::flatten_over(&input::decode(&encoded)?, 255);
    let layer = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let (a, b) = (rgb.get_pixel(x, y), recompressed.get_pixel(x, y));
        image::Rgb(std::array::from_fn(|c| {
            a[c].abs_diff(b[c]).saturating_mul(16)
        }))
    });
    layer
        .save(Path::new(&out).join("error-level-analysis.png"))
        .map_err(|e| invalid(e.to_string()))?;
    let report = serde_json::json!({"schema":integrity::SCHEMA,"operation":"inspect_image","verdict":"unknown","counts":{"copy_move_candidates":copies.len(),"publication_outputs":0},"input":{"sha256":crate::localized::digest(&bytes),"dimensions":[image.width(),image.height()],"orientation_policy":"raw raster dimensions; EXIF orientation not applied"},"headers":headers,"copy_move":{"candidates":copies,"can_show":"reciprocal FAST/oriented BRIEF self-matches clustered by translation","cannot_show":"natural repetition can match; rotated/projective copies and textureless edits may be missed; candidates do not establish manipulation"},"error_level_analysis":{"artifact":"error-level-analysis.png","quality":90,"display_gain":16,"assurance":"weak visual layer only","can_show":"local difference from one recompression with the current Rust JPEG encoder","cannot_show":"does not identify edited regions, authenticity or AI generation; depends on content and compression"},"near_duplicates":null,"publication":{"outputs":[],"quality":assessment::assess(&image)?,"text_legibility":{"status":"unavailable","reason":"no image-bound OCR/source observations supplied"},"can_show":"raw pixel adequacy for declared output/crop and content-dependent quality measures","cannot_show":"no publication approval; no human readability or provenance proof"},"ai_generation":credentials["ai_generation"],"credentials":credentials,"limitations":["heuristics never produce real/fake or AI-generation verdicts","GPS is unsigned metadata and only emitted with include-gps","optional source/OCR/archive flags are unavailable in-process"]});
    crate::report_links::write(
        &Path::new(&out).join(format!("{}.json", integrity::SCHEMA)),
        &report,
    )?;
    crate::manifest::write(Path::new(&out), Default::default())?;
    Ok(report)
}
#[cfg(not(feature = "text-quality"))]
fn tofu(_: &[String]) -> Result<Value> {
    Err(invalid("requires text-quality"))
}
#[cfg(not(feature = "text-quality"))]
fn legibility(_: &[String]) -> Result<Value> {
    Err(invalid("requires text-quality"))
}
#[cfg(feature = "text-quality")]
fn tofu(args: &[String]) -> Result<Value> {
    use crate::{general::input, text_quality as tq};
    let mut flags = Flags::parse(args, 2, &[])?;
    let mask = flags
        .one("--mask")?
        .map(|p| read(Path::new(&p)))
        .transpose()?;
    let expected = flags.one("--expected-text")?;
    if expected.as_ref().is_some_and(|s| s.len() > 65536) {
        return Err(invalid("expected text exceeds 64 KiB"));
    }
    flags.finish()?;
    let bytes = read(Path::new(&args[1]))?;
    let image = input::decode(&bytes)?;
    let mask_image = mask.as_deref().map(input::decode).transpose()?;
    Ok(serde_json::to_value(tq::tofu(
        &image,
        mask_image.as_ref(),
        crate::localized::digest(&bytes),
        mask.as_ref().map(|b| crate::localized::digest(b)),
        expected,
        tq::unavailable_ocr("OCR not requested"),
    )?)?)
}
#[cfg(feature = "text-quality")]
fn legibility(args: &[String]) -> Result<Value> {
    use crate::{general::input, text_quality as tq};
    let mut flags = Flags::parse(args, 3, &[])?;
    let policy = tq::Policy {
        minimum_contrast: flags.number("--minimum-contrast", 4.5)?,
        minimum_x_height_px: flags.number("--minimum-x-height-px", 8.)?,
        minimum_sharpness: flags.number("--minimum-sharpness", 0.35)?,
        minimum_stroke_px: flags.number("--minimum-stroke-px", 1.)?,
    };
    let rects = flags.many("--region");
    flags.finish()?;
    if rects.is_empty() || rects.len() > 64 {
        return Err(invalid("text-quality requires 1..64 regions"));
    }
    let (a, b) = (read(Path::new(&args[1]))?, read(Path::new(&args[2]))?);
    let (baseline_image, variant_image) = (input::decode(&a)?, input::decode(&b)?);
    let size = [baseline_image.width(), baseline_image.height()];
    let dim = [variant_image.width(), variant_image.height()];
    let mut baseline = Vec::new();
    let mut regions = Vec::new();
    for (index, r) in rects.iter().enumerate() {
        let r: [u32; 4] = r
            .split(',')
            .map(|v| v.parse().map_err(|_| invalid("region requires integers")))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid("region requires four integers"))?;
        let base = tq::legibility(&baseline_image, r, index, policy)?;
        let x = (u64::from(r[0]) * u64::from(dim[0]) / u64::from(size[0])) as u32;
        let y = (u64::from(r[1]) * u64::from(dim[1]) / u64::from(size[1])) as u32;
        let endx = (u64::from(r[0] + r[2]) * u64::from(dim[0])).div_ceil(u64::from(size[0])) as u32;
        let endy = (u64::from(r[1] + r[3]) * u64::from(dim[1])).div_ceil(u64::from(size[1])) as u32;
        let mut variant =
            tq::legibility(&variant_image, [x, y, endx - x, endy - y], index, policy)?;
        if base.state != tq::State::Legible && variant.state == tq::State::Legible {
            variant.state = tq::State::InsufficientEvidence;
            variant
                .reasons
                .push("baseline did not establish legible pixel evidence".into());
        }
        baseline.push(base);
        regions.push(variant);
    }
    let states: Vec<_> = baseline
        .iter()
        .chain(regions.iter())
        .map(|r| r.state)
        .collect();
    let state = if states.contains(&tq::State::Illegible) {
        tq::State::Illegible
    } else if states.iter().all(|s| *s == tq::State::Legible) {
        tq::State::Legible
    } else {
        tq::State::InsufficientEvidence
    };
    Ok(serde_json::to_value(tq::LegibilityReport { report_id:None, source_refs:Vec::new(), schema:tq::LEGIBILITY_SCHEMA.into(), state, baseline_sha256:crate::localized::digest(&a), baseline_dimensions:size, policy, baseline, variants:vec![tq::Variant { index:0,image_sha256:crate::localized::digest(&b),dimensions:dim,regions }], limitations:vec!["declared text regions and proportional dimension mapping required; layout changes need separately aligned captures".into(),"OCR unavailable without imported observations; pixel measurements do not certify human readability".into()] })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_sections_keep_core_schemas_and_artifacts() -> Result<()> {
        let t = tempfile::tempdir().map_err(super::super::io)?;
        let path = t.path().join("image.png");
        image::RgbImage::from_pixel(40, 30, image::Rgb([20, 40, 70]))
            .save(&path)
            .map_err(|e| invalid(e.to_string()))?;
        let path = super::super::path_string(&path)?;
        let analyzer =
            media::Analyzer::new(media::Profile::CpuLite, t.path().join("models"), false)
                .map_err(|e| invalid(e.to_string()))?;
        let (code, record) = invoke(
            &analyzer,
            &["analyze-media".into(), path.clone(), "--json".into()],
        )?;
        assert_eq!(code, 0);
        assert_eq!(record["schema"], media::SCHEMA);
        let (code, mask) = invoke(
            &analyzer,
            &[
                "mask-metrics".into(),
                path.clone(),
                path.clone(),
                "--each-label".into(),
                "--boundary-px=0".into(),
                "--json".into(),
            ],
        )?;
        assert_eq!(code, 0);
        assert_eq!(mask["schema"], crate::mask_metrics::SCHEMA);
        assert_eq!(mask["summary"]["macro_iou"], 1.);
        let (_, watermark) = invoke(
            &analyzer,
            &["watermark".into(), path.clone(), "--json".into()],
        )?;
        assert_eq!(
            watermark["schema"],
            crate::wave7::watermark::WATERMARK_SCHEMA
        );
        let out = t.path().join("inspect");
        let (_, inspection) = invoke(
            &analyzer,
            &[
                "inspect-image".into(),
                path.clone(),
                "--out".into(),
                super::super::path_string(&out)?,
                "--include-gps".into(),
                "--json".into(),
            ],
        )?;
        assert_eq!(inspection["schema"], crate::general::integrity::SCHEMA);
        assert!(out.join("error-level-analysis.png").is_file());
        assert!(crate::manifest::verify(&out)?.is_empty());
        #[cfg(feature = "text-quality")]
        {
            let (_, report) = invoke(
                &analyzer,
                &[
                    "tofu".into(),
                    path.clone(),
                    "--expected-text".into(),
                    "hello".into(),
                    "--json".into(),
                ],
            )?;
            assert_eq!(report["schema"], crate::text_quality::TOFU_SCHEMA);
            let (_, report) = invoke(
                &analyzer,
                &[
                    "text-legibility".into(),
                    path.clone(),
                    path,
                    "--region".into(),
                    "0,0,40,30".into(),
                    "--json".into(),
                ],
            )?;
            assert_eq!(report["schema"], crate::text_quality::LEGIBILITY_SCHEMA);
            assert_eq!(report["baseline"].as_array().map(Vec::len), Some(1));
            assert_eq!(
                report["variants"][0]["regions"].as_array().map(Vec::len),
                Some(1)
            );
        }
        Ok(())
    }
    #[test]
    fn native_flags_refuse_silent_changes() -> Result<()> {
        let mut flags = Flags::parse(
            &[
                "cmd".into(),
                "--ppd=42".into(),
                "--registration".into(),
                "auto".into(),
            ],
            1,
            &[],
        )?;
        assert_eq!(flags.number("--ppd", 67.)?, 42.);
        assert!(flags.finish().is_err());
        assert!(
            Flags::parse(
                &["cmd".into(), "--each-label=false".into()],
                1,
                &["--each-label"]
            )
            .is_err()
        );
        Ok(())
    }
}
