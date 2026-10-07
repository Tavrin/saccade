//! First-party print extension: preserve CMYK separations, apply ICC profiles,
//! and produce measured D50 Lab evidence. No press certification is implied.
mod input;
mod pdf;
use input::Raster;
use lcms2::{
    CIELab, CIELabExt, ColorSpaceSignature, GlobalContext, Intent, PixelFormat, Profile, Transform,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Versioned, report-links-compatible measurement schema.
pub const SCHEMA: &str = "saccade-print.v1";
/// Machine-readable contract shipped with the extension.
pub const JSON_SCHEMA: &str = include_str!("../schemas/saccade-print.v1.schema.json");
/// Typed print input/measurement failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The input does not contain CMYK separations.
    #[error("not a print input: {0}")]
    NotPrintInput(String),
    /// Encoded bytes or decoded pixels exceed the resource envelope.
    #[error("print input exceeds 128 MiB / 16 million pixels")]
    Limit,
    /// A required profile is absent or incompatible.
    #[error("ICC profile: {0}")]
    Profile(String),
    /// Container structure cannot be decoded.
    #[error("decode: {0}")]
    Decode(String),
    /// An explicit input contract is not supported.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Invalid comparison policy.
    #[error("configuration: {0}")]
    Config(String),
    /// Filesystem failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Core report decoration failure.
    #[error(transparent)]
    Core(#[from] saccade_core::Error),
    /// Artifact encoding failure.
    #[error(transparent)]
    Image(#[from] image::ImageError),
}
/// Print operation result.
pub type Result<T> = std::result::Result<T, Error>;
impl Error {
    /// Stable transport error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotPrintInput(_) => "not_print_input",
            Self::Limit => "print_limit",
            Self::Profile(_) => "print_profile",
            Self::Decode(_) => "print_decode",
            Self::Unsupported(_) => "print_unsupported",
            Self::Config(_) => "print_config",
            Self::Io(_) | Self::Image(_) => "io",
            Self::Core(_) => "print_report",
        }
    }
}
/// Declared comparison policy. Explicit input profiles override embedded profiles on both sides.
#[derive(Clone, Debug)]
pub struct Options {
    /// Optional CMYK input ICC override; otherwise each embedded profile is required.
    pub input_profile: Option<PathBuf>,
    /// Optional target CMYK output ICC for gamut diagnostics.
    pub output_profile: Option<PathBuf>,
    /// Maximum total ink coverage in percent, 0..400.
    pub tac_limit: f64,
    /// Physical raster density used by the small-mark heuristic.
    pub dpi: f64,
    /// Maximum text-like component height in points.
    pub small_text_points: f64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            input_profile: None,
            output_profile: None,
            tac_limit: 300.,
            dpi: 300.,
            small_text_points: 12.,
        }
    }
}

fn profile(bytes: &[u8]) -> Result<Profile> {
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(Error::Limit);
    }
    let p = Profile::new_icc(bytes).map_err(|e| Error::Profile(e.to_string()))?;
    if p.color_space() != ColorSpaceSignature::CmykData {
        return Err(Error::Profile(
            "requires CMYK ICC, not RGB or a guessed default".into(),
        ));
    }
    Ok(p)
}
fn lab_profile() -> Result<Profile> {
    Profile::new_lab4_context(
        GlobalContext::new(),
        &lcms2::CIExyY {
            x: 0.3457,
            y: 0.3585,
            Y: 1.,
        },
    )
    .map_err(|e| Error::Profile(e.to_string()))
}
fn convert(r: &Raster, override_bytes: Option<&[u8]>) -> Result<(Vec<[f64; 3]>, Value)> {
    let bytes = override_bytes
        .or(r.embedded.as_deref())
        .ok_or_else(|| Error::Profile("missing embedded ICC; supply --input-profile".into()))?;
    let p = profile(bytes)?;
    let lab = lab_profile()?;
    let transform: Transform<[f64; 4], [f64; 3]> = Transform::new(
        &p,
        PixelFormat::CMYK_DBL,
        &lab,
        PixelFormat::Lab_DBL,
        Intent::RelativeColorimetric,
    )
    .map_err(|e| Error::Profile(e.to_string()))?;
    let mut pixels = vec![[0.; 3]; r.ink.len()];
    transform.transform_pixels(&r.ink, &mut pixels);
    if pixels.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
        return Err(Error::Profile("non-finite Lab output".into()));
    }
    Ok((
        pixels,
        json!({"sha256":saccade_core::localized::digest(bytes),"source":if override_bytes.is_some(){"explicit"}else{"embedded"},"embedded_sha256":r.embedded.as_ref().map(|b|saccade_core::localized::digest(b))}),
    ))
}
fn stats(values: &[f64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n == 0 {
        return Value::Null;
    }
    let p = |q: f64| {
        sorted[((q * n as f64).ceil() as usize)
            .saturating_sub(1)
            .min(n - 1)]
    };
    json!({"mean":values.iter().sum::<f64>()/n as f64,"p50":p(0.5),"p95":p(0.95),"p99":p(0.99),"max":sorted[n-1]})
}
/// Compare CMYK rasters and write pixel maps into an existing empty output directory.
/// Callers must apply their output-path authorization/overwrite policy first.
/// Alpha is straightened and ink coverage is composited onto unprinted paper before
/// ICC conversion. Separation maps use the same effective ink coverage as the colour comparison.
pub fn compare(a: &Path, b: &Path, out: &Path, options: &Options) -> Result<Value> {
    if !options.tac_limit.is_finite()
        || !(0. ..=400.).contains(&options.tac_limit)
        || !options.dpi.is_finite()
        || !(36. ..=2400.).contains(&options.dpi)
        || !options.small_text_points.is_finite()
        || !(1. ..=72.).contains(&options.small_text_points)
    {
        return Err(Error::Config(
            "TAC 0..400, DPI 36..2400, small text 1..72 points required".into(),
        ));
    }
    let mut aa = input::load(a)?;
    let mut bb = input::load(b)?;
    if (aa.width, aa.height) != (bb.width, bb.height) {
        return Err(Error::Config(
            "raster dimensions must match; no automatic registration".into(),
        ));
    }
    for r in [&mut aa, &mut bb] {
        for (ink, alpha) in r.ink.iter_mut().zip(&r.alpha) {
            for v in ink {
                *v *= alpha;
            }
        }
    }
    let override_bytes = options
        .input_profile
        .as_ref()
        .map(|p| input::bytes(p, 4 * 1024 * 1024))
        .transpose()?;
    let (al, ap) = convert(&aa, override_bytes.as_deref())?;
    let (bl, bp) = convert(&bb, override_bytes.as_deref())?;
    let delta: Vec<_> = al.iter().zip(&bl).map(|(a, b)| delta_e(a, b)).collect();
    let n = delta.len();
    let heat: Vec<_> = delta.iter().map(|v| (v / 10.).min(1.)).collect();
    map(out, "delta-e2000.png", aa.width, aa.height, &heat)?;
    let mut separations = serde_json::Map::new();
    for (i, name) in ["c", "m", "y", "k"].iter().enumerate() {
        let diff: Vec<_> = aa
            .ink
            .iter()
            .zip(&bb.ink)
            .map(|(a, b)| (a[i] - b[i]).abs())
            .collect();
        map(
            out,
            &format!("separation-{name}.png"),
            aa.width,
            aa.height,
            &diff.iter().map(|v| v / 100.).collect::<Vec<_>>(),
        )?;
        separations.insert(name.to_string(), stats(&diff));
    }
    let mut sides = serde_json::Map::new();
    let output_bytes = options
        .output_profile
        .as_ref()
        .map(|p| input::bytes(p, 4 * 1024 * 1024))
        .transpose()?;
    for (side, r, lab) in [("reference", &aa, &al), ("candidate", &bb, &bl)] {
        let tac: Vec<f64> = r.ink.iter().map(|p| p.iter().sum()).collect();
        let over: Vec<_> = tac
            .iter()
            .map(|v| f64::from(*v > options.tac_limit))
            .collect();
        map(
            out,
            &format!("{side}-tac-over.png"),
            r.width,
            r.height,
            &over,
        )?;
        let marks = marks(r, options);
        let gamut = if let Some(bytes) = &output_bytes {
            let mask = gamut_mask(lab, bytes)?;
            map(
                out,
                &format!("{side}-out-of-gamut.png"),
                r.width,
                r.height,
                &mask,
            )?;
            json!({"state":"measured","method":"Little CMS relative-colorimetric proofing gamut check; two isolated alarm-code passes","share":mask.iter().sum::<f64>()/n as f64})
        } else {
            json!({"state":"not_requested"})
        };
        sides.insert(side.into(),json!({"tac_percent":stats(&tac),"over_limit_pixels":over.iter().sum::<f64>() as usize,"over_limit_share":over.iter().sum::<f64>()/n as f64,"out_of_gamut":gamut,"registration_sensitive_marks":marks}));
    }
    let value = json!({"schema":SCHEMA,"operation":"print_compare","verdict":"measured","width":aa.width,"height":aa.height,"pixels":n,"inputs":{"reference_sha256":aa.digest,"candidate_sha256":bb.digest},"profiles":{"reference":ap,"candidate":bp,"output_sha256":output_bytes.as_ref().map(|b|saccade_core::localized::digest(b))},"policy":{"comparison_space":"CIELAB D50","intent":"relative_colorimetric","black_point_compensation":false,"alpha":"ink coverage on unprinted paper","tac_limit_percent":options.tac_limit,"dpi":options.dpi,"small_text_points":options.small_text_points,"heatmap_saturation_delta_e2000":10.,"cmm":"Little CMS","cmm_version":lcms2::version()},"delta_e2000":stats(&delta),"separations_percent":separations,"sides":sides,"limitations":["Gamut share uses the ICC CMM model, not a physical press measurement.","Small four-colour components are text-like candidates, not recognized text; raster density is caller-declared.","No spot colours, overprint, trapping, finishing or PDF vector analysis."]});
    Ok(saccade_core::report_links::decorate(&value)?)
}
fn map(out: &Path, name: &str, w: u32, h: u32, values: &[f64]) -> Result<()> {
    let bytes: Vec<u8> = values
        .iter()
        .map(|v| (v.clamp(0., 1.) * 255.).round() as u8)
        .collect();
    let img = image::GrayImage::from_raw(w, h, bytes).ok_or(Error::Limit)?;
    img.save(out.join(name))?;
    Ok(())
}
fn marks(r: &Raster, o: &Options) -> Value {
    let mut seen = vec![false; r.ink.len()];
    let mut result = Vec::new();
    let rich = |i: usize| r.ink[i][3] >= 50. && r.ink[i][..3].iter().all(|v| *v >= 5.);
    let w = r.width as usize;
    for start in 0..seen.len() {
        if seen[start] || !rich(start) {
            continue;
        }
        let mut queue = vec![start];
        seen[start] = true;
        let mut cursor = 0;
        let (mut x0, mut y0, mut x1, mut y1) = (start % w, start / w, start % w, start / w);
        while cursor < queue.len() {
            let i = queue[cursor];
            cursor += 1;
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
            for next in [
                x.checked_sub(1).map(|x| y * w + x),
                (x + 1 < w).then_some(i + 1),
                y.checked_sub(1).map(|y| y * w + x),
                (y + 1 < r.height as usize).then_some(i + w),
            ]
            .into_iter()
            .flatten()
            {
                if !seen[next] && rich(next) {
                    seen[next] = true;
                    queue.push(next);
                }
            }
        }
        let height_points = (y1 - y0 + 1) as f64 * 72. / o.dpi;
        if height_points <= o.small_text_points && queue.len() >= 2 {
            result.push(json!({"rect":[x0,y0,x1-x0+1,y1-y0+1],"pixels":queue.len(),"height_points":height_points,"kind":"small_four_colour_mark","interpretation":"rich-black / registration-sensitive text candidate; heuristic"}));
            if result.len() >= 10000 {
                break;
            }
        }
    }
    json!({"candidates":result,"limit":10000,"truncated":result.len()>=10000,"ink_thresholds_percent":{"c":5,"m":5,"y":5,"k":50}})
}

fn delta_e(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let lab = |p: &[f64; 3]| CIELab {
        L: p[0],
        a: p[1],
        b: p[2],
    };
    lab(a).cie2000_delta_e(&lab(b), 1., 1., 1.)
}

fn gamut_mask(lab: &[[f64; 3]], bytes: &[u8]) -> Result<Vec<f64>> {
    // Two isolated alarm colours distinguish gamut alarms from legitimate output
    // values without changing global CMM state or depending on sentinel collisions.
    profile(bytes)?;
    let pass = |alarm: u16| -> Result<Vec<[u16; 3]>> {
        let mut context = lcms2::ThreadContext::new();
        context.set_alarm_codes([alarm; 16]);
        let target =
            Profile::new_icc_context(&context, bytes).map_err(|e| Error::Profile(e.to_string()))?;
        let lab_profile = Profile::new_lab4_context(
            &context,
            &lcms2::CIExyY {
                x: 0.3457,
                y: 0.3585,
                Y: 1.,
            },
        )
        .map_err(|e| Error::Profile(e.to_string()))?;
        let transform: Transform<[f64; 3], [u16; 3], lcms2::ThreadContext> =
            Transform::new_proofing_context(
                &context,
                &lab_profile,
                PixelFormat::Lab_DBL,
                &lab_profile,
                PixelFormat::Lab_16,
                &target,
                Intent::RelativeColorimetric,
                Intent::RelativeColorimetric,
                lcms2::Flags::SOFT_PROOFING | lcms2::Flags::GAMUT_CHECK,
            )
            .map_err(|e| Error::Profile(e.to_string()))?;
        let mut pixels = vec![[0; 3]; lab.len()];
        transform.transform_pixels(lab, &mut pixels);
        Ok(pixels)
    };
    let a = pass(0)?;
    let b = pass(u16::MAX)?;
    Ok(a.iter().zip(&b).map(|(a, b)| f64::from(a != b)).collect())
}
