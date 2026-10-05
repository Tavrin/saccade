//! Document renderer boundary; renderer licence/source verification is deferred.
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
/// Generic renderer, to be implemented only after source/licence verification.
/// Implementations must disable SVG external resources/scripts and enforce PDF raster limits.
pub trait Renderer {
    /// Rasterizes exactly one declared page without network, script execution or unbounded allocation.
    fn page(&mut self, encoded: &[u8], request: Request) -> crate::Result<image::RgbaImage>;
}
/// Validates the byte/DPI/page request and the renderer's output dimensions.
/// This boundary does not imply a renderer is installed or comparison routing is implemented.
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
            feature: "documents (renderer source/licence verification pending)",
        })
    }
}
