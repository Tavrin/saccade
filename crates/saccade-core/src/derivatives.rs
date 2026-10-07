//! Declared derivative geometry and final-display-size text evidence.
use crate::{Error, Result, text_quality as tq, wave7::faces::FaceReport};
use image::{RgbaImage, imageops};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Bounded derivative declaration contract; never a measurement report.
pub const INPUT_SCHEMA: &str = "saccade-derivatives.v1";
/// Contact-sheet report contract, with report linkage from its first version.
pub const REPORT_SCHEMA: &str = "saccade-derivative-sheet.v1";
/// Largest number of derivatives in a sheet.
pub const MAX_DERIVATIVES: usize = 64;
/// Total admitted preview/contact-sheet pixels.
pub const MAX_SHEET_PIXELS: u64 = 16 * 1024 * 1024;

/// A manually protected source-pixel subject or text region.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// Printable label, inert data.
    pub label: String,
    /// Original source pixels [x, y, width, height].
    pub rect_px: [u32; 4],
}
/// A declared crop, size and optional delivered rendition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Derivative {
    /// Unique printable identity.
    pub id: String,
    /// Final display pixels, independent of encoded rendition resolution.
    pub display_size: [u32; 2],
    /// Source-pixel crop; omitted means the full source image.
    #[serde(default)]
    pub crop_px: Option<[u32; 4]>,
    /// Delivered file relative to the declaration; absent generates a preview.
    #[serde(default)]
    pub path: Option<String>,
}
/// Input declares known subjects/text; absence never establishes safety.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// Must equal [`INPUT_SCHEMA`].
    pub schema: String,
    /// Known protected subjects in source coordinates.
    #[serde(default)]
    pub subjects: Vec<Region>,
    /// Text regions in source coordinates, including background margins.
    #[serde(default)]
    pub text_regions: Vec<Region>,
    /// One review row per declaration, including missing files.
    pub derivatives: Vec<Derivative>,
}
fn label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
impl Declaration {
    /// Validate all geometry and allocation bounds before rendering any preview.
    pub fn validate(&self, size: [u32; 2]) -> Result<()> {
        if self.schema != INPUT_SCHEMA
            || self.derivatives.is_empty()
            || self.derivatives.len() > MAX_DERIVATIVES
            || self.subjects.len() > 64
            || self.text_regions.len() > 64
        {
            return Err(Error::Config(
                "invalid derivative schema or declaration count".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        let mut pixels = 0u64;
        for region in self.subjects.iter().chain(&self.text_regions) {
            if !label(&region.label) {
                return Err(Error::Config(
                    "region labels need 1..256 printable bytes".into(),
                ));
            }
            tq::validate_rect(size, region.rect_px)?;
        }
        for d in &self.derivatives {
            if !label(&d.id) || !ids.insert(&d.id) {
                return Err(Error::Config(
                    "derivative IDs must be unique printable labels".into(),
                ));
            }
            if d.display_size.iter().any(|v| *v == 0 || *v > 4096)
                || d.path.as_ref().is_some_and(|p| {
                    p.is_empty() || p.len() > 4096 || p.chars().any(char::is_control)
                })
            {
                return Err(Error::Config(
                    "invalid display size (1..4096) or rendition path".into(),
                ));
            }
            tq::validate_rect(size, crop(d, size))?;
            pixels += u64::from(d.display_size[0]) * u64::from(d.display_size[1]);
        }
        if pixels > MAX_SHEET_PIXELS {
            return Err(Error::Config(
                "derivative preview pixel budget exceeded".into(),
            ));
        }
        Ok(())
    }
}
/// Effective source crop; callers validate declarations before use.
pub fn crop(d: &Derivative, size: [u32; 2]) -> [u32; 4] {
    d.crop_px.unwrap_or([0, 0, size[0], size[1]])
}
/// Render a generated derivative using the exact declared crop and display size.
pub fn preview(source: &RgbaImage, d: &Derivative) -> Result<RgbaImage> {
    let size = [source.width(), source.height()];
    let r = crop(d, size);
    tq::validate_rect(size, r)?;
    if d.display_size.iter().any(|v| *v == 0 || *v > 4096) {
        return Err(Error::Config("invalid derivative display size".into()));
    }
    let cropped = imageops::crop_imm(source, r[0], r[1], r[2], r[3]);
    Ok(imageops::resize(
        &cropped.to_image(),
        d.display_size[0],
        d.display_size[1],
        imageops::FilterType::Lanczos3,
    ))
}
fn status(c: [f64; 4], b: [f64; 4]) -> &'static str {
    if b[0] >= c[0] && b[1] >= c[1] && b[0] + b[2] <= c[0] + c[2] && b[1] + b[3] <= c[1] + c[3] {
        "included"
    } else if b[0] < c[0] + c[2] && b[1] < c[1] + c[3] && b[0] + b[2] > c[0] && b[1] + b[3] > c[1] {
        "cut"
    } else {
        "excluded"
    }
}
fn projected(b: [f64; 4], c: [u32; 4], size: [u32; 2]) -> Option<[f64; 4]> {
    let c = c.map(f64::from);
    let x = b[0].max(c[0]);
    let y = b[1].max(c[1]);
    let endx = (b[0] + b[2]).min(c[0] + c[2]);
    let endy = (b[1] + b[3]).min(c[1] + c[3]);
    if endx <= x || endy <= y || c[2] <= 0. || c[3] <= 0. {
        return None;
    }
    let sx = f64::from(size[0]) / c[2];
    let sy = f64::from(size[1]) / c[3];
    Some([
        (x - c[0]) * sx,
        (y - c[1]) * sy,
        (endx - x) * sx,
        (endy - y) * sy,
    ])
}
/// Map a wholly retained source box to display pixels, rounding outwards.
pub fn mapped(r: [u32; 4], c: [u32; 4], size: [u32; 2]) -> Option<[u32; 4]> {
    if c[2] == 0
        || c[3] == 0
        || size.contains(&0)
        || status(c.map(f64::from), r.map(f64::from)) != "included"
    {
        return None;
    }
    let x = u64::from(r[0] - c[0]) * u64::from(size[0]) / u64::from(c[2]);
    let y = u64::from(r[1] - c[1]) * u64::from(size[1]) / u64::from(c[3]);
    let endx = (u64::from(r[0] - c[0] + r[2]) * u64::from(size[0])).div_ceil(u64::from(c[2]));
    let endy = (u64::from(r[1] - c[1] + r[3]) * u64::from(size[1])).div_ceil(u64::from(c[3]));
    Some([x as u32, y as u32, (endx - x) as u32, (endy - y) as u32])
}
/// Per-derivative evidence with no inference of undeclared subject completeness.
pub fn row(
    source: &RgbaImage,
    d: &Derivative,
    declaration: &Declaration,
    faces: Option<&FaceReport>,
    display: Option<&RgbaImage>,
    error: Option<&str>,
) -> Result<Value> {
    let c = crop(d, [source.width(), source.height()]);
    let mut subjects: Vec<Value> = declaration.subjects.iter().map(|s| json!({"label":s.label,"origin":"declared","rect_px":s.rect_px,"display_rect_px":projected(s.rect_px.map(f64::from), c, d.display_size),"state":status(c.map(f64::from), s.rect_px.map(f64::from))})).collect();
    if let Some(faces) = faces {
        subjects.extend(faces.faces.iter().enumerate().map(|(index, f)| {
            let b = [f.bbox.x, f.bbox.y, f.bbox.width, f.bbox.height];
            json!({"label":format!("face {}", index + 1),"origin":"detected_face","rect_px":b,"display_rect_px":projected(b.map(f64::from), c, d.display_size),"score":f.score,"state":status(c.map(f64::from), b.map(f64::from))})
        }));
    }
    let crop_state = if subjects.is_empty() {
        "not_established"
    } else if subjects.iter().all(|s| s["state"] == "included") {
        "preserved"
    } else {
        "unsafe"
    };
    let mut texts = Vec::new();
    let policy = tq::Policy::default();
    for (index, t) in declaration.text_regions.iter().enumerate() {
        let baseline = tq::legibility(source, t.rect_px, index, policy)?;
        let rect = mapped(t.rect_px, c, d.display_size);
        let measured = display
            .zip(rect)
            .map(|(im, r)| tq::legibility(im, r, index, policy))
            .transpose()?;
        let state = if rect.is_none() {
            "illegible"
        } else if display.is_none() {
            "unavailable"
        } else {
            match measured.as_ref().map(|m| m.state) {
                Some(tq::State::Illegible) => "illegible",
                Some(tq::State::Legible) if baseline.state == tq::State::Legible => "legible",
                _ => "not_established",
            }
        };
        texts.push(json!({"label":t.label,"source_rect_px":t.rect_px,"display_rect_px":rect,"state":state,"reason":if rect.is_none() {"text region cut or excluded by declared crop"} else {"pixel thresholds at declared display size; not human readability"},"baseline":baseline,"display":measured}));
    }
    let text_state = if texts.iter().any(|t| t["state"] == "illegible") {
        "illegible"
    } else if texts.is_empty() || texts.iter().any(|t| t["state"] == "not_established") {
        "not_established"
    } else if texts.iter().any(|t| t["state"] == "unavailable") {
        "unavailable"
    } else {
        "legible"
    };
    // Delivered content is not established by an asserted source crop alone.
    let preservation = if display.is_none() {
        "unavailable"
    } else if crop_state == "unsafe" {
        "unsafe"
    } else if d.path.is_some() {
        "not_established"
    } else {
        crop_state
    };
    let verdict = if error.is_some() || crop_state == "unsafe" || text_state == "illegible" {
        "fail"
    } else if preservation == "preserved" && text_state == "legible" {
        "pass"
    } else {
        "not_established"
    };
    Ok(
        json!({"id":d.id,"display_size":d.display_size,"crop_px":c,"rendition":if d.path.is_some() {"delivered"} else {"generated"},"verdict":verdict,"subject_preservation":{"state":preservation,"subjects":subjects,"basis":if d.path.is_some() {"declared crop only; delivered subject content unverified"} else {"generated crop geometry"}},"crop_safety":{"state":crop_state,"scope":"known subjects only; no detection never establishes safety"},"text_legibility":{"state":text_state,"policy":policy,"regions":texts},"ocr":{"state":"unavailable","reason":"OCR not executed; exact text correctness requires text or critical-text"},"error":error} ),
    )
}
/// Arrange previews at their actual display sizes in input order; no further scaling.
pub fn contact_sheet(
    previews: &[Option<RgbaImage>],
    sizes: &[[u32; 2]],
    labels: &[String],
) -> Result<(RgbaImage, Vec<[u32; 4]>)> {
    if labels.len() != sizes.len()
        || previews.len() != sizes.len()
        || sizes.is_empty()
        || sizes.len() > MAX_DERIVATIVES
        || sizes.iter().flatten().any(|v| *v == 0 || *v > 4096)
    {
        return Err(Error::Config("invalid contact-sheet sizes".into()));
    }
    let width = sizes.iter().map(|s| s[0]).max().unwrap_or(1).max(320) + 16;
    let height: u32 = sizes.iter().map(|s| s[1] + 32).sum();
    if u64::from(width) * u64::from(height) > MAX_SHEET_PIXELS {
        return Err(Error::Config("contact-sheet pixel budget exceeded".into()));
    }
    let mut sheet = RgbaImage::from_pixel(width, height, image::Rgba([240, 240, 240, 255]));
    let mut placements = Vec::new();
    let mut y = 24;
    for (index, (preview, size)) in previews.iter().zip(sizes).enumerate() {
        let mut header = image::RgbImage::from_pixel(width, 24, image::Rgb([240, 240, 240]));
        crate::explain::draw_text(
            &mut header,
            8,
            8,
            &labels[index],
            1,
            image::Rgb([20, 20, 20]),
        );
        let header = image::DynamicImage::ImageRgb8(header).to_rgba8();
        imageops::overlay(&mut sheet, &header, 0, i64::from(y - 24));
        placements.push([8, y, size[0], size[1]]);
        if let Some(preview) = preview {
            if [preview.width(), preview.height()] != *size {
                return Err(Error::Config("contact-sheet preview size mismatch".into()));
            }
            imageops::overlay(&mut sheet, preview, 8, i64::from(y));
        } else {
            for yy in y..y + size[1] {
                for x in 8..8 + size[0] {
                    sheet.put_pixel(x, yy, image::Rgba([180, 60, 60, 255]));
                }
            }
        }
        y += size[1] + 32;
    }
    Ok((sheet, placements))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn declaration() -> Declaration {
        serde_json::from_value(json!({"schema":INPUT_SCHEMA,"subjects":[],"text_regions":[],"derivatives":[{"id":"tile","display_size":[64,48],"crop_px":[40,20,80,60]}]})).unwrap()
    }
    #[test]
    fn crop_relative_mapping_rounds_outward_and_refuses_cut_or_excluded_regions() {
        assert_eq!(
            mapped([50, 30, 20, 10], [40, 20, 80, 60], [64, 48]),
            Some([8, 8, 16, 8])
        );
        assert_eq!(
            mapped([41, 21, 1, 1], [40, 20, 80, 60], [64, 48]),
            Some([0, 0, 2, 2])
        );
        assert_eq!(mapped([30, 30, 20, 10], [40, 20, 80, 60], [64, 48]), None);
        assert_eq!(mapped([0, 0, 10, 10], [40, 20, 80, 60], [64, 48]), None);
        assert_eq!(mapped([0, 0, 1, 1], [0, 0, 0, 1], [64, 48]), None);
    }
    #[test]
    fn declaration_and_sheet_allocation_budgets_are_enforced() {
        let mut d = declaration();
        d.validate([160, 120]).unwrap();
        d.derivatives.push(d.derivatives[0].clone());
        assert!(d.validate([160, 120]).is_err());
        let mut d = declaration();
        d.derivatives[0].crop_px = Some([u32::MAX, 0, 2, 2]);
        assert!(d.validate([160, 120]).is_err());
        assert!(
            contact_sheet(&[None, None], &[[4096, 4096]; 2], &["1".into(), "2".into()]).is_err()
        );
    }
    #[test]
    fn previews_use_exact_crop_and_sheet_never_rescales_them() {
        let mut d = declaration();
        d.derivatives[0].display_size = [80, 60];
        let image = RgbaImage::from_fn(160, 120, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        let p = preview(&image, &d.derivatives[0]).unwrap();
        assert_eq!(*p.get_pixel(0, 0), *image.get_pixel(40, 20));
        let (sheet, positions) =
            contact_sheet(&[Some(p.clone())], &[[80, 60]], &["1 pass 80x60".into()]).unwrap();
        let [x, y, w, h] = positions[0];
        assert_eq!(imageops::crop_imm(&sheet, x, y, w, h).to_image(), p);
    }
    #[test]
    fn no_separable_text_or_known_subjects_never_certifies_safety() {
        let image = RgbaImage::from_pixel(160, 120, image::Rgba([255; 4]));
        let mut d = declaration();
        d.derivatives[0].crop_px = None;
        d.text_regions.push(Region {
            label: "declared but blank".into(),
            rect_px: [10, 10, 40, 40],
        });
        d.validate([160, 120]).unwrap();
        let display = preview(&image, &d.derivatives[0]).unwrap();
        let r = row(&image, &d.derivatives[0], &d, None, Some(&display), None).unwrap();
        assert_eq!(r["crop_safety"]["state"], "not_established");
        assert_eq!(r["text_legibility"]["state"], "not_established");
        assert_eq!(r["verdict"], "not_established");
    }
}
