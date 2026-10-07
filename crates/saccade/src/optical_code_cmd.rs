//! Optical-code file transport; no provider calls or implicit document-page selection.
use crate::agent::CliError;
use saccade_core::general::{documents, input, optical_code as oc};
use std::path::PathBuf;

#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum Symbology {
    Qr,
    Code128,
    Code39,
    Ean13,
    Ean8,
    Upca,
    Upce,
    Itf,
    DataMatrix,
    Aztec,
    Pdf417,
}
impl From<Symbology> for oc::Symbology {
    fn from(s: Symbology) -> Self {
        match s {
            Symbology::Qr => Self::Qr,
            Symbology::Code128 => Self::Code128,
            Symbology::Code39 => Self::Code39,
            Symbology::Ean13 => Self::Ean13,
            Symbology::Ean8 => Self::Ean8,
            Symbology::Upca => Self::Upca,
            Symbology::Upce => Self::Upce,
            Symbology::Itf => Self::Itf,
            Symbology::DataMatrix => Self::DataMatrix,
            Symbology::Aztec => Self::Aztec,
            Symbology::Pdf417 => Self::Pdf417,
        }
    }
}
#[derive(clap::Args)]
pub(crate) struct Args {
    /// Final 8-bit SDR image, or PDF/SVG with explicit --page and --dpi.
    image: PathBuf,
    /// Select one symbology; use a region when multiple codes are present.
    #[arg(long, value_enum, default_value = "qr")]
    symbology: Symbology,
    /// Exact decoded Unicode payload (no normalization).
    #[arg(long, conflicts_with = "expect_pattern")]
    expect: Option<String>,
    /// Rust regex matched against the entire decoded payload (max 4096 bytes).
    #[arg(long)]
    expect_pattern: Option<String>,
    /// Capture-pixel rectangle x,y,width,height.
    #[arg(long,value_parser=rect)]
    region: Option<[u32; 4]>,
    /// One-based document page; must be explicit for documents.
    #[arg(long, requires = "dpi")]
    page: Option<usize>,
    /// Document render DPI in 36..600; must be explicit for documents.
    #[arg(long, requires = "page")]
    dpi: Option<f64>,
    /// Independent minimum QR module size in original pixels.
    #[arg(long)]
    minimum_module_px: Option<f64>,
    /// Independent minimum normalized QR luma contrast (0..1).
    #[arg(long)]
    minimum_contrast: Option<f64>,
    /// Require four clear sampled QR modules on every side.
    #[arg(long)]
    require_quiet_zone: bool,
    /// Empty evidence directory; JSON also emits the complete versioned report.
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}
fn rect(s: &str) -> Result<[u32; 4], String> {
    let parts = s
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()
        .map_err(|_| "region requires four unsigned integers")?;
    parts
        .try_into()
        .map_err(|_| "region requires x,y,width,height".into())
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    if !cfg!(feature = "optical-code") {
        return Err(CliError::new(
            "feature_unavailable",
            "optical-code requires the optical-code feature",
        ));
    }
    let bytes = input::bytes(&args.image, input::MAX_BYTES)?;
    let doc = documents::format(&bytes).is_some();
    let image = if doc {
        let page = args.page.ok_or_else(|| {
            CliError::usage("document optical-code requires explicit --page and --dpi")
        })?;
        let dpi = args
            .dpi
            .ok_or_else(|| CliError::usage("document optical-code requires explicit --dpi"))?;
        if page == 0 || page > documents::page_count(&bytes)? {
            return Err(CliError::usage("document page is out of range"));
        }
        documents::page(&bytes, dpi, page - 1)?
    } else {
        if args.page.is_some() || args.dpi.is_some() {
            return Err(CliError::usage("--page/--dpi require document input"));
        }
        input::decode(&bytes)?
    };
    let expected = args
        .expect
        .map(oc::Expected::Exact)
        .or_else(|| args.expect_pattern.map(oc::Expected::Pattern));
    let mut report = oc::verify(
        &image,
        oc::Options {
            symbology: args.symbology.into(),
            region: args.region,
            expected,
            minimum_module_px: args.minimum_module_px,
            minimum_contrast: args.minimum_contrast,
            require_quiet_zone: args.require_quiet_zone,
        },
    )?;
    report.input_sha256 = Some(saccade_core::localized::digest(&bytes));
    if doc {
        report.page = args.page;
        report.dpi = args.dpi;
    }
    let value = saccade_core::report_links::decorate(&serde_json::to_value(&report)?)?;
    if let Some(out) = &args.out {
        crate::general_cmd::prepare_out(out, &[&args.image])?;
        crate::general_cmd::persist_document(&value, out)?;
    }
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    } else {
        crate::emit(&format!(
            "optical-code: {} ({:?}); payload match: {:?}; failures: {}\n",
            report.verdict,
            report.state,
            report.payload_match,
            report.failures.join(", ")
        ))?;
    }
    Ok(u8::from(report.verdict == "fail"))
}
