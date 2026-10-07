//! Bounded optional resvg/usvg and hayro document rasterization.
/// Declared vector/document type.
#[derive(Clone, Copy, Debug)]
pub enum Format {
    /// SVG raster input.
    Svg,
    /// A PDF page.
    Pdf,
}
/// Bounded single-page raster request; process multipage documents one page at a time.
#[derive(Clone, Copy, Debug)]
pub struct Request {
    /// Original input format.
    pub format: Format,
    /// Declared DPI in 36..600; never guessed from a display viewport.
    pub dpi: f64,
    /// Zero-based page index in 0..500 (SVG only allows page zero).
    pub page: usize,
}
/// Generic page renderer; optional adapters are reviewed from registry sources.
/// Implementations must disable SVG external resources/scripts and enforce PDF raster limits.
pub trait Renderer {
    /// Rasterizes one declared page without network or scripts; adapters must bound raster output.
    /// Custom implementations own their isolation; the shipped adapter uses a capped worker.
    fn page(&mut self, encoded: &[u8], request: Request) -> crate::Result<image::RgbaImage>;
}
/// Validates the byte/DPI/page request and the renderer's output dimensions.
/// The caller declares DPI/page explicitly; upstream renderer semantics remain limited.
pub fn rasterize(
    renderer: &mut impl Renderer,
    encoded: &[u8],
    request: Request,
) -> crate::Result<image::RgbaImage> {
    if encoded.len() as u64 > super::input::MAX_BYTES
        || !request.dpi.is_finite()
        || !(36.0..=600.0).contains(&request.dpi)
        || request.page >= 500
        || matches!(request.format, Format::Svg) && request.page != 0
    {
        return Err(crate::Error::Config(
            "invalid document byte/DPI/page request".into(),
        ));
    }
    let image = renderer.page(encoded, request)?;
    if image.width() == 0
        || image.height() == 0
        || u64::from(image.width()) * u64::from(image.height()) > super::input::MAX_PIXELS
    {
        return Err(crate::Error::Config(
            "document raster exceeds pixel limits".into(),
        ));
    }
    Ok(image)
}
/// Explicit unavailable backend; never replaces a document with blank pixels.
pub struct Unavailable;
impl Renderer for Unavailable {
    fn page(&mut self, _encoded: &[u8], _request: Request) -> crate::Result<image::RgbaImage> {
        Err(crate::Error::FeatureUnavailable {
            feature: "documents",
        })
    }
}

/// Fixed default raster density for input adapters (CSS SVG pixels are 96/inch).
pub const DEFAULT_DPI: f64 = 96.;
/// Page-by-page comparison evidence schema.
pub const SCHEMA: &str = "saccade-documents.v3";
/// Complete, declared page correspondence.
pub mod page_map;
#[cfg(feature = "documents")]
mod preflight;
/// Resource-isolated renderer protocol and fixed caps.
pub mod worker;
/// Detects a document from bytes, independent of the supplied filename.
pub fn format(encoded: &[u8]) -> Option<Format> {
    if encoded.starts_with(b"%PDF-") {
        Some(Format::Pdf)
    } else {
        let head = &encoded[..encoded.len().min(4096)];
        std::str::from_utf8(head)
            .ok()
            .filter(|s| s.contains("<svg"))
            .map(|_| Format::Svg)
    }
}
/// Offline Rust renderers. No file/network resources or host font discovery.
#[cfg(feature = "documents")]
pub struct Native;
#[cfg(feature = "documents")]
impl Renderer for Native {
    fn page(&mut self, encoded: &[u8], request: Request) -> crate::Result<image::RgbaImage> {
        if !matches!(
            (format(encoded), request.format),
            (Some(Format::Svg), Format::Svg) | (Some(Format::Pdf), Format::Pdf)
        ) {
            return Err(worker::error("document_invalid_request"));
        }
        worker::page(encoded, request.dpi, request.page)
    }
}
#[cfg(feature = "documents")]
struct Local;
#[cfg(feature = "documents")]
fn dimensions(w: f64, h: f64) -> crate::Result<(u32, u32)> {
    if !w.is_finite()
        || !h.is_finite()
        || w <= 0.
        || h <= 0.
        || w > 16384.
        || h > 16384.
        || w.ceil() * h.ceil() > super::input::MAX_PIXELS as f64
    {
        return Err(worker::error("document_dimensions_limit"));
    }
    Ok((w.ceil() as u32, h.ceil() as u32))
}
#[cfg(feature = "documents")]
impl Renderer for Local {
    fn page(&mut self, encoded: &[u8], request: Request) -> crate::Result<image::RgbaImage> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> crate::Result<image::RgbaImage> {
            let (w, h, mut pixels) = match request.format {
                Format::Svg => {
                    let text = std::str::from_utf8(encoded).map_err(|_| crate::Error::Config("SVG must be UTF-8".into()))?;
                    let doc = roxmltree::Document::parse(text).map_err(|e| crate::Error::Config(format!("SVG XML: {e}")))?;
                    if doc.descendants().count() > 100000 {
                        return Err(crate::Error::Config("SVG node limit exceeded".into()));
                    }
                    // Fail rather than silently omit text, active content or external images.
                    for node in doc.descendants().filter(|n| n.is_element()) {
                        if matches!(node.tag_name().name(), "text" | "image" | "script" | "foreignObject" | "animate" | "animateTransform" | "set")
                            || node.attributes().any(|a| a.name().starts_with("on") || a.name() == "href" && !a.value().starts_with('#')) {
                            return Err(crate::Error::Config("SVG requires static paths, inline shapes and fragment references; outline text and embed images as paths".into()));
                        }
                    }
                    let mut options = resvg::usvg::Options { dpi: DEFAULT_DPI as f32, ..Default::default() };
                    options.image_href_resolver = resvg::usvg::ImageHrefResolver {
                        resolve_data: Box::new(|_, _, _| None), resolve_string: Box::new(|_, _| None),
                    };
                    let tree = resvg::usvg::Tree::from_data(encoded, &options).map_err(|e| crate::Error::Config(format!("SVG parse: {e}")))?;
                    let scale = (request.dpi / DEFAULT_DPI) as f32;
                    let (w,h) = dimensions(f64::from(tree.size().width()) * f64::from(scale), f64::from(tree.size().height()) * f64::from(scale))?;
                    let mut map = resvg::tiny_skia::Pixmap::new(w,h).ok_or_else(|| crate::Error::Config("SVG raster allocation failed".into()))?;
                    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale,scale), &mut map.as_mut());
                    (w, h, map.take())
                }
                Format::Pdf => {
                    let pdf = hayro::Pdf::new(std::sync::Arc::new(encoded.to_vec())).map_err(|e| crate::Error::Config(format!("PDF parse: {e:?}")))?;
                    if pdf.pages().len() > 500 || pdf.len() > 100000 {
                        return Err(crate::Error::Config("PDF page/object limit exceeded".into()));
                    }
                    let page = pdf.pages().get(request.page).ok_or_else(|| crate::Error::Config("PDF page unavailable".into()))?;
                    let scale = (request.dpi / 72.) as f32;
                    let (pw, ph) = page.render_dimensions();
                    let (w,h) = dimensions(f64::from(pw)*f64::from(scale), f64::from(ph)*f64::from(scale))?;
                    let warnings = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                    let sink = warnings.clone();
                    let missing_font = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let missing = missing_font.clone();
                        let settings = hayro::InterpreterSettings {
                            font_resolver: std::sync::Arc::new(move |_| { missing.store(true,std::sync::atomic::Ordering::Relaxed); None }),
                        warning_sink: std::sync::Arc::new(move |warning| { if let Ok(mut out) = sink.lock() && out.len() < 32 { out.push(format!("{warning:?}")); } }),
                    };
                    let map = hayro::render(page, &settings, &hayro::RenderSettings { x_scale:scale, y_scale:scale, width:Some(w as u16), height:Some(h as u16) });
                    let warnings = warnings.lock().map_err(|_| crate::Error::Config("PDF warning state failed".into()))?;
                    if missing_font.load(std::sync::atomic::Ordering::Relaxed) || !warnings.is_empty() { return Err(crate::Error::Config(format!("unsupported PDF rendering: {}", warnings.join("; ")))); }
                    (w,h,map.take_u8())
                }
            };
            // Both renderers return premultiplied RGBA; image expects straight alpha.
            for p in pixels.as_chunks_mut::<4>().0 { if p[3] > 0 && p[3] < 255 { for c in 0..3 { p[c] = ((u32::from(p[c])*255 + u32::from(p[3])/2)/u32::from(p[3])).min(255) as u8; } } }
            image::RgbaImage::from_raw(w,h,pixels).ok_or_else(|| crate::Error::Config("document raster length mismatch".into()))
        })).map_err(|_| crate::Error::Config("document renderer failed".into()))?
    }
}
/// Counts pages in the capped worker; non-document images contain one page.
pub fn page_count(encoded: &[u8]) -> crate::Result<usize> {
    if encoded.len() as u64 > super::input::MAX_BYTES {
        return Err(worker::error("document_encoded_limit"));
    }
    if format(encoded).is_none() {
        return Ok(1);
    }
    #[cfg(feature = "documents")]
    {
        worker::count(encoded)
    }
    #[cfg(not(feature = "documents"))]
    {
        Err(crate::Error::FeatureUnavailable {
            feature: "documents",
        })
    }
}
/// Loads one document page in the capped worker with declared density.
pub fn page(encoded: &[u8], dpi: f64, page: usize) -> crate::Result<image::RgbaImage> {
    #[cfg(feature = "documents")]
    {
        worker::page(encoded, dpi, page)
    }
    #[cfg(not(feature = "documents"))]
    {
        let format = format(encoded)
            .ok_or_else(|| crate::Error::Config("not a supported document".into()))?;
        rasterize(&mut Unavailable, encoded, Request { format, dpi, page })
    }
}
#[cfg(all(test, feature = "documents"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn svg_density_pixels_and_resource_refusal() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1in" height="1in"><rect width="96" height="96" fill="red"/></svg>"#;
        let render = |data: &[u8], dpi, page| {
            rasterize(
                &mut Local,
                data,
                Request {
                    format: Format::Svg,
                    dpi,
                    page,
                },
            )
        };
        assert!(matches!(
            page_count(&vec![0; super::super::input::MAX_BYTES as usize + 1]),
            Err(crate::Error::Document {
                code: "document_encoded_limit"
            })
        ));
        let image = render(svg, 192., 0).expect("render");
        assert_eq!(image.dimensions(), (192, 192));
        assert_eq!(image.get_pixel(96, 96).0, [255, 0, 0, 255]);
        assert!(render(svg, f64::NAN, 0).is_err());
        assert!(render(svg, 96., 1).is_err());
        let external = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="file:///tmp/private.png"/></svg>"#;
        assert!(render(external, 96., 0).is_err());
        let text = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><text>café</text></svg>"#.as_bytes();
        assert!(render(text, 96., 0).is_err());
    }
}
