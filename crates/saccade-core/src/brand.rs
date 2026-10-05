//! One source-backed brand/theme/accessibility packet, without a combined score.
use crate::{Error, Result, color, evidence::canonical::Digest};
use moxcms::{ColorProfile, Layout, RenderingIntent, ToneReprCurve, TransformOptions};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
};

/// Policy read from `[brand]` in saccade.toml.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Named, hash-pinned external RGB matrix/shaper ICC profiles.
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
    /// Expected named colours and independently declared tolerances.
    #[serde(default)]
    pub swatches: Vec<Swatch>,
    /// Declared chart pairs to check under Machado simulation.
    #[serde(default)]
    pub cvd: Vec<CvdPair>,
}
/// A pinned profile, resolved relative to the project configuration.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// ICC file path.
    pub path: PathBuf,
    /// SHA-256 of exact profile bytes.
    pub sha256: Digest,
}
/// Explicit encoding for a source or target colour.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colour {
    /// Normalized encoded RGB, not linear values. PQ is absolute BT.2020/ST 2084.
    pub rgb: [f64; 3],
    /// srgb, display_p3, bt2020_pq, or a configured profile name.
    pub profile: String,
}
/// Target colour and tolerances, chosen by the maintainer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Swatch {
    /// Stable token name, e.g. logo_red.
    pub name: String,
    /// Expected physical colour.
    pub colour: Colour,
    /// CIEDE2000 threshold; only relative SDR profiles.
    pub delta_e2000: Option<f64>,
    /// BT.2124 threshold; only absolute BT.2020 PQ colours.
    pub delta_e_itp: Option<f64>,
}
/// A pair of named colours with an explicitly selected simulation severity.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CvdPair {
    /// Names of supplied chart-series swatches.
    pub names: [String; 2],
    /// protan, deutan or tritan; anomaly is indicated by severity below one.
    pub kind: String,
    /// Machado severity from zero to one.
    pub severity: f64,
    /// Report confusability below this ΔE2000, with no accessibility guarantee.
    pub minimum_delta_e2000: f64,
}
/// Source-provided sample with a locator for the evidence reader.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// Stable swatch name.
    pub name: String,
    /// Producer-supplied encoded colour.
    pub colour: Colour,
    /// CSS token, selector or sampling region description in the source artifact.
    pub source: String,
}
/// Source text and layout facts; a screenshot estimate is not accepted here.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    /// Stable node identifier.
    pub id: String,
    /// Locator in the DOM/layout source artifact.
    pub source: String,
    /// Effective sRGB foreground, including alpha (0..1).
    pub foreground: [f64; 4],
    /// Opaque sRGB background samples. Multiple samples do not prove gradient coverage.
    pub backgrounds: Vec<[f64; 3]>,
    /// Actual CSS font size in px, not screenshot-inferred.
    pub font_size_px: f64,
    /// Actual CSS font weight (100..900).
    pub font_weight: u16,
    /// Actual selected font, when known.
    pub font: Option<String>,
    /// Required selected font, when declared.
    pub expected_font: Option<String>,
    /// Source-established glyph ink bounds, x,y,width,height, in capture pixels.
    pub ink_box: Option<[f64; 4]>,
    /// Effective ancestor clipping intersection in capture pixels.
    pub clip_box: Option<[f64; 4]>,
    /// Source-established missing-glyph count, when known.
    pub missing_glyphs: Option<u32>,
}
/// Capture-bound values exported by a DOM/layout or image-sampling producer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// saccade-brand-source.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-brand-source.v1")))]
    pub schema: String,
    /// Artifact containing the source facts, resolved relative to this document.
    pub source_artifact: PathBuf,
    /// Exact bytes of the producer artifact.
    pub source_sha256: Digest,
    /// ICC relative_colorimetric; other intents are rejected in this version.
    pub rendering_intent: String,
    /// bradford_icc_pcs; the RGB profiles carry D50-adapted colorants.
    pub chromatic_adaptation: String,
    /// Supplied named samples.
    pub samples: Vec<Sample>,
    /// Optional source-backed text facts.
    #[serde(default)]
    pub text: Vec<Text>,
}
/// A bounded finding with its source and numerical evidence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Swatch, node or pair name.
    pub subject: String,
    /// Rule or algorithm.
    pub rule: String,
    /// pass, fail, review, or unavailable; never human approval.
    pub status: String,
    /// Numeric facts and applicability retained without threshold rounding.
    pub evidence: serde_json::Value,
    /// Source locators.
    pub sources: Vec<String>,
}
/// One review packet for colour, contrast, simulation and typography.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// saccade-brand-review.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-brand-review.v1")))]
    pub schema: String,
    /// Source evidence and all policy values retained verbatim.
    pub source: Evidence,
    /// Hash of the evidence document's semantic content.
    pub source_identity: Digest,
    /// Effective tolerances.
    pub policy: Policy,
    /// Profile identities, including builtins pinned to moxcms 0.7.11.
    pub profile_identities: BTreeMap<String, Digest>,
    /// Independent findings, without a universal score.
    pub findings: Vec<Finding>,
    /// Capability and evidence limits.
    pub limits: Vec<String>,
}
fn invalid(message: impl Into<String>) -> Error {
    Error::Config(message.into())
}
fn unit(values: &[f64]) -> bool {
    values
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
}
fn colour(c: &Colour) -> Result<()> {
    if !unit(&c.rgb) || c.profile.is_empty() {
        return Err(invalid(
            "colour requires finite normalized RGB and a named profile",
        ));
    }
    Ok(())
}
impl Policy {
    /// Reject ambiguous names, unbounded tolerances and unsupported metric encodings.
    pub fn validate(&self) -> Result<()> {
        let mut names = BTreeSet::new();
        for s in &self.swatches {
            colour(&s.colour)?;
            if s.name.is_empty()
                || !names.insert(&s.name)
                || (s.delta_e2000.is_none() && s.delta_e_itp.is_none())
                || [s.delta_e2000, s.delta_e_itp]
                    .into_iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v < 0.0)
                || (s.delta_e2000.is_some() && s.colour.profile == "bt2020_pq")
                || (s.delta_e_itp.is_some() && s.colour.profile != "bt2020_pq")
            {
                return Err(invalid("invalid swatch name, tolerance or metric encoding"));
            }
        }
        for name in self.profiles.keys() {
            if name.is_empty() || ["srgb", "display_p3", "bt2020_pq"].contains(&name.as_str()) {
                return Err(invalid("ICC profile name conflicts with a pinned builtin"));
            }
        }
        for c in &self.cvd {
            kind(&c.kind)?;
            if !unit(&[c.severity])
                || !c.minimum_delta_e2000.is_finite()
                || c.minimum_delta_e2000 < 0.0
                || c.names[0] == c.names[1]
            {
                return Err(invalid("invalid CVD severity, pair or tolerance"));
            }
        }
        Ok(())
    }
}
fn kind(name: &str) -> Result<usize> {
    match name {
        "protan" => Ok(0),
        "deutan" => Ok(1),
        "tritan" => Ok(2),
        _ => Err(invalid("CVD kind must be protan, deutan or tritan")),
    }
}
fn profile(name: &str, p: &Policy, root: &Path) -> Result<(ColorProfile, Digest)> {
    let builtin = match name {
        "srgb" => Some(ColorProfile::new_srgb()),
        "display_p3" => Some(ColorProfile::new_display_p3()),
        _ => None,
    };
    if let Some(c) = builtin {
        let bytes = c.encode().map_err(|e| invalid(e.to_string()))?;
        return Ok((c, Digest::of_bytes(&bytes)));
    }
    let pinned = p
        .profiles
        .get(name)
        .ok_or_else(|| invalid(format!("unsupported or undeclared ICC profile {name}")))?;
    let path = root.join(&pinned.path);
    const MAX_ICC_BYTES: u64 = 4 * 1024 * 1024;
    // Inspect the path before opening so a FIFO cannot block waiting for a writer.
    let metadata = std::fs::metadata(&path).map_err(|source| Error::Io {
        context: format!("inspecting ICC {}", path.display()),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAX_ICC_BYTES {
        return Err(invalid("ICC profile must be a regular file <=4 MiB"));
    }
    let mut file = std::fs::File::open(&path).map_err(|source| Error::Io {
        context: format!("opening ICC {}", path.display()),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| Error::Io {
        context: format!("inspecting ICC {}", path.display()),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAX_ICC_BYTES {
        return Err(invalid("ICC profile must be a regular file <=4 MiB"));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_ICC_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| Error::Io {
            context: format!("reading ICC {}", path.display()),
            source,
        })?;
    if bytes.len() as u64 > MAX_ICC_BYTES {
        return Err(invalid("ICC profile must be a regular file <=4 MiB"));
    }
    if Digest::of_bytes(&bytes) != pinned.sha256 {
        return Err(invalid(
            "ICC profile hash does not match the pinned contract",
        ));
    }
    // Reject all LUT transform tags, including ones the pinned parser may ignore.
    let count = bytes
        .get(128..132)
        .ok_or_else(|| invalid("truncated ICC tag table"))?;
    let count = u32::from_be_bytes([count[0], count[1], count[2], count[3]]) as usize;
    let end = count
        .checked_mul(12)
        .and_then(|n| n.checked_add(132))
        .ok_or_else(|| invalid("invalid ICC tag table"))?;
    let tags = bytes
        .get(132..end)
        .ok_or_else(|| invalid("truncated ICC tag table"))?;
    if tags.as_chunks::<12>().0.iter().any(|tag| {
        matches!(
            &tag[..4],
            b"A2B0"
                | b"A2B1"
                | b"A2B2"
                | b"B2A0"
                | b"B2A1"
                | b"B2A2"
                | b"D2B0"
                | b"D2B1"
                | b"D2B2"
                | b"D2B3"
                | b"B2D0"
                | b"B2D1"
                | b"B2D2"
                | b"B2D3"
                | b"pre0"
                | b"pre1"
                | b"pre2"
                | b"gamt"
        )
    }) {
        return Err(invalid(
            "brand review excludes ICC LUT tags, including mixed matrix/LUT profiles",
        ));
    }
    let c = ColorProfile::new_from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    if c.color_space != moxcms::DataColorSpace::Rgb
        || c.pcs != moxcms::DataColorSpace::Xyz
        || !matches!(
            c.profile_class,
            moxcms::ProfileClass::InputDevice | moxcms::ProfileClass::DisplayDevice
        )
        || !c.is_matrix_shaper()
    {
        return Err(invalid(
            "brand review supports input/display RGB matrix/shaper ICC profiles with XYZ PCS only",
        ));
    }
    Ok((c, pinned.sha256.clone()))
}
fn managed(
    c: &Colour,
    p: &Policy,
    root: &Path,
    ids: &mut BTreeMap<String, Digest>,
) -> Result<[f64; 3]> {
    colour(c)?;
    let (source, hash) = profile(&c.profile, p, root)?;
    ids.insert(c.profile.clone(), hash);
    let mut dest = ColorProfile::new_srgb();
    dest.cicp = None;
    dest.red_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
    dest.green_trc = dest.red_trc.clone();
    dest.blue_trc = dest.red_trc.clone();
    let options = TransformOptions {
        rendering_intent: RenderingIntent::RelativeColorimetric,
        allow_use_cicp_transfer: false,
        prefer_fixed_point: false,
        allow_extended_range_rgb_xyz: true,
        ..Default::default()
    };
    let transform = source
        .create_transform_f64(Layout::Rgb, &dest, Layout::Rgb, options)
        .map_err(|e| invalid(e.to_string()))?;
    let mut out = [0.0; 3];
    transform
        .transform(&c.rgb, &mut out)
        .map_err(|e| invalid(e.to_string()))?;
    if out.iter().any(|v| !v.is_finite()) {
        return Err(invalid("nonfinite ICC transform"));
    }
    Ok(out)
}
/// BT.2124 ΔE-ITP from ICtCp (T = Ct/2 exactly once), no spatial weighting.
pub fn delta_e_itp(a: [f64; 3], b: [f64; 3]) -> f64 {
    720.0 * ((a[0] - b[0]).powi(2) + 0.25 * (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn pq_decode(v: f64) -> f64 {
    let p = v.powf(32.0 / 2523.0);
    ((p - 3424.0 / 4096.0).max(0.0) / (2413.0 / 128.0 - 2392.0 / 128.0 * p)).powf(16384.0 / 2610.0)
}
fn pq_encode(v: f64) -> f64 {
    let p = v.max(0.0).powf(2610.0 / 16384.0);
    ((3424.0 / 4096.0 + 2413.0 / 128.0 * p) / (1.0 + 2392.0 / 128.0 * p)).powf(2523.0 / 32.0)
}
/// BT.2100 PQ RGB → ICtCp, ST 2084 normalized to 10000 cd/m².
pub fn pq_ictcp(rgb: [f64; 3]) -> Result<[f64; 3]> {
    if !unit(&rgb) {
        return Err(invalid("PQ RGB must be in [0,1]"));
    }
    let [r, g, b] = rgb.map(pq_decode);
    let l = pq_encode((1688.0 * r + 2146.0 * g + 262.0 * b) / 4096.0);
    let m = pq_encode((683.0 * r + 2951.0 * g + 462.0 * b) / 4096.0);
    let s = pq_encode((99.0 * r + 309.0 * g + 3688.0 * b) / 4096.0);
    Ok([
        (2048.0 * l + 2048.0 * m) / 4096.0,
        (6610.0 * l - 13613.0 * m + 7003.0 * s) / 4096.0,
        (17933.0 * l - 17390.0 * m - 543.0 * s) / 4096.0,
    ])
}
fn finding(
    subject: &str,
    rule: &str,
    status: &str,
    evidence: serde_json::Value,
    sources: Vec<String>,
) -> Finding {
    Finding {
        subject: subject.into(),
        rule: rule.into(),
        status: status.into(),
        evidence,
        sources,
    }
}
/// Review verified producer facts. The caller verifies source_artifact bytes before use.
pub fn review(p: &Policy, e: &Evidence, config_dir: &Path) -> Result<Report> {
    use serde_json::json;
    p.validate()?;
    if e.schema != "saccade-brand-source.v1"
        || e.rendering_intent != "relative_colorimetric"
        || e.chromatic_adaptation != "bradford_icc_pcs"
    {
        return Err(invalid(
            "brand source requires v1, relative_colorimetric and bradford_icc_pcs",
        ));
    }
    if p.swatches.is_empty() && p.cvd.is_empty() && e.text.is_empty() {
        return Err(invalid(
            "brand review requires swatches, chart pairs or source text",
        ));
    }
    let mut samples = BTreeMap::new();
    let mut ids = BTreeMap::new();
    for s in &e.samples {
        colour(&s.colour)?;
        if s.name.is_empty() || s.source.is_empty() || samples.insert(s.name.as_str(), s).is_some()
        {
            return Err(invalid("empty or duplicate source sample"));
        }
    }
    let mut findings = Vec::new();
    for s in &p.swatches {
        let Some(sample) = samples.get(s.name.as_str()) else {
            findings.push(finding(
                &s.name,
                "named_swatch",
                "unavailable",
                json!({"reason":"source swatch missing"}),
                vec![],
            ));
            continue;
        };
        for (metric, tolerance) in [
            ("delta_e2000", s.delta_e2000),
            ("delta_e_itp", s.delta_e_itp),
        ] {
            let Some(tolerance) = tolerance else {
                continue;
            };
            let measured = if metric == "delta_e2000" {
                if sample.colour.profile == "bt2020_pq" {
                    return Err(invalid("SDR swatch cannot compare against absolute PQ"));
                }
                color::delta_e(
                    color::lab(managed(&s.colour, p, config_dir, &mut ids)?),
                    color::lab(managed(&sample.colour, p, config_dir, &mut ids)?),
                )
            } else {
                if sample.colour.profile != "bt2020_pq" {
                    return Err(invalid("ITP requires BT.2020 PQ for both colours"));
                }
                ids.insert(
                    "bt2020_pq".into(),
                    Digest::of_bytes(
                        b"ITU-R BT.2100-2 PQ RGB/ICtCp; ST2084 10000 cd/m2; BT.2124-0",
                    ),
                );
                delta_e_itp(pq_ictcp(s.colour.rgb)?, pq_ictcp(sample.colour.rgb)?)
            };
            findings.push(finding(&s.name, metric, if measured <= tolerance {"pass"} else {"fail"}, json!({"measured":measured,"tolerance":tolerance,"expected":s.colour,"observed":sample.colour,"lab_white":"D65","k_lch":[1,1,1]}), vec![sample.source.clone()]));
        }
    }
    for pair in &p.cvd {
        let subject = pair.names.join(" / ");
        let (Some(a), Some(b)) = (
            samples.get(pair.names[0].as_str()),
            samples.get(pair.names[1].as_str()),
        ) else {
            findings.push(finding(
                &subject,
                "machado_2009",
                "unavailable",
                json!({"reason":"chart series missing"}),
                vec![],
            ));
            continue;
        };
        let a_rgb = managed(&a.colour, p, config_dir, &mut ids)?;
        let b_rgb = managed(&b.colour, p, config_dir, &mut ids)?;
        let original = color::delta_e(color::lab(a_rgb), color::lab(b_rgb));
        let simulated = color::delta_e(
            color::lab(color::simulate_severity(
                a_rgb,
                kind(&pair.kind)?,
                pair.severity,
            )?),
            color::lab(color::simulate_severity(
                b_rgb,
                kind(&pair.kind)?,
                pair.severity,
            )?),
        );
        findings.push(finding(&subject,"machado_2009",if simulated < pair.minimum_delta_e2000 {"review"} else {"pass"},json!({"kind":pair.kind,"severity":pair.severity,"original_delta_e2000":original,"simulated_delta_e2000":simulated,"minimum_delta_e2000":pair.minimum_delta_e2000,"out_of_gamut_clipped":true}),vec![a.source.clone(),b.source.clone()]));
    }
    let mut text_ids = BTreeSet::new();
    for t in &e.text {
        if t.id.is_empty()
            || !text_ids.insert(&t.id)
            || t.source.is_empty()
            || !unit(&t.foreground)
            || t.backgrounds.is_empty()
            || t.backgrounds.iter().any(|v| !unit(v))
            || !t.font_size_px.is_finite()
            || t.font_size_px <= 0.0
            || !(100..=900).contains(&t.font_weight)
        {
            return Err(invalid(
                "source-backed contrast requires unique nodes, sRGB colours, alpha, size, weight and opaque backgrounds",
            ));
        }
        // Alpha compositing is in encoded sRGB, matching the producer's CSS contract.
        let alpha = t.foreground[3];
        let ratios: Vec<f64> = t
            .backgrounds
            .iter()
            .map(|bg| {
                let fg = std::array::from_fn(|i| {
                    color::linear(t.foreground[i] * alpha + bg[i] * (1.0 - alpha))
                });
                let y1 = color::luminance(fg);
                let y2 = color::luminance(bg.map(color::linear));
                (y1.max(y2) + 0.05) / (y1.min(y2) + 0.05)
            })
            .collect();
        let minimum = ratios.iter().copied().fold(f64::INFINITY, f64::min);
        let large =
            t.font_size_px >= 24.0 || (t.font_size_px >= 56.0 / 3.0 && t.font_weight >= 700);
        let threshold = if large { 3.0 } else { 4.5 };
        // A failing observed sample proves a failure. Passing extrema cannot establish
        // the minimum over a continuous gradient or other varying background.
        let status = if minimum < threshold {
            "fail"
        } else if t.backgrounds.len() > 1 {
            "unavailable"
        } else {
            "pass"
        };
        let coverage = if t.backgrounds.len() > 1 {
            "sampled_backgrounds_only"
        } else {
            "declared_uniform_background"
        };
        findings.push(finding(&t.id,"wcag_2_2_1_4_3",status,json!({"coverage":coverage,"minimum_ratio":minimum,"ratios":ratios,"required_ratio":threshold,"font_size_px":t.font_size_px,"font_weight":t.font_weight,"large_text":large,"foreground":t.foreground,"backgrounds":t.backgrounds,"applicability":"declared non-exempt text"}),vec![t.source.clone()]));
        if let (Some(ink), Some(clip)) = (t.ink_box, t.clip_box) {
            for box_ in [ink, clip] {
                if box_.iter().any(|v| !v.is_finite()) || box_[2] < 0.0 || box_[3] < 0.0 {
                    return Err(invalid("invalid ink/clip box"));
                }
            }
            let clipped = ink[0] < clip[0]
                || ink[1] < clip[1]
                || ink[0] + ink[2] > clip[0] + clip[2]
                || ink[1] + ink[3] > clip[1] + clip[3];
            findings.push(finding(
                &t.id,
                "source_ink_clipping",
                if clipped { "fail" } else { "pass" },
                json!({"ink_box":ink,"clip_box":clip}),
                vec![t.source.clone()],
            ));
        }
        if t.expected_font.is_some() || t.missing_glyphs.is_some() {
            let wrong = t
                .expected_font
                .as_ref()
                .zip(t.font.as_ref())
                .is_some_and(|(a, b)| a != b)
                || t.missing_glyphs.is_some_and(|n| n > 0);
            let status = if wrong {
                "fail"
            } else if t.expected_font.is_some() && t.font.is_none() {
                "unavailable"
            } else {
                "pass"
            };
            findings.push(finding(&t.id,"source_typography",status,json!({"font":t.font,"expected_font":t.expected_font,"missing_glyphs":t.missing_glyphs}),vec![t.source.clone()]));
        }
    }
    findings.push(finding("text contrast","apca_wcag_3_draft","unavailable",json!({"reason":"APCA implementation excluded: reviewed licence has field-of-use/commercial restrictions; WCAG 3 is an incomplete draft, not an adopted compliance requirement"}),vec![]));
    Ok(Report { schema:"saccade-brand-review.v1".into(), source:e.clone(), source_identity:crate::evidence::canonical::digest(e).map_err(|v|invalid(v.to_string()))?, policy:p.clone(),profile_identities:ids,findings,limits:vec!["Producer source facts are hash-bound, not independently recaptured; screenshot clustering is not source-backed contrast.".into(),"ICC relative-colorimetric RGB matrix/shaper profiles only; D50 PCS/Bradford to linear sRGB D65, no physical display calibration or CMYK proofing.".into(),"Gradient/background coverage and glyph ink bounds are producer assertions; absence of a typography finding does not prove legibility.".into(),"Machado simulations and swatch thresholds do not establish individual task success, compliance or human approval.".into()] })
}
