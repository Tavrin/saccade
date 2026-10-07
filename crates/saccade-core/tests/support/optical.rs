//! Procedural fixtures: project MIT OR Apache-2.0, rxing encoder Apache-2.0.
#![allow(dead_code, missing_docs, clippy::unwrap_used)]
use image::{Rgba, RgbaImage};
use rxing::{BarcodeFormat, MultiFormatWriter, Writer};
pub fn qr(payload: &str, module: u32, dark: u8, light: u8) -> RgbaImage {
    let bits = MultiFormatWriter
        .encode(payload, &BarcodeFormat::QR_CODE, 0, 0)
        .unwrap();
    RgbaImage::from_fn(
        bits.getWidth() * module,
        bits.getHeight() * module,
        |x, y| {
            let v = if bits.get(x / module, y / module) {
                dark
            } else {
                light
            };
            Rgba([v, v, v, 255])
        },
    )
}
pub fn barcode(payload: &str, format: BarcodeFormat) -> RgbaImage {
    let bits = MultiFormatWriter
        .encode(payload, &format, 360, 120)
        .unwrap();
    RgbaImage::from_fn(bits.getWidth(), bits.getHeight(), |x, y| {
        let v = if bits.get(x, y) { 0 } else { 255 };
        Rgba([v, v, v, 255])
    })
}
pub fn scene(code: &RgbaImage) -> RgbaImage {
    let mut page = RgbaImage::from_pixel(640, 480, Rgba([245, 245, 245, 255]));
    for y in 0..60 {
        for x in 0..640 {
            page.put_pixel(x, y, Rgba([40, 60, 80, 255]));
        }
    }
    for y in 85..100 {
        for x in 40..350 {
            page.put_pixel(x, y, Rgba([90, 90, 90, 255]));
        }
    }
    image::imageops::replace(&mut page, code, 80, 120);
    page
}
pub fn pdf(code: &RgbaImage) -> Vec<u8> {
    // A standalone vector PDF page: draw final module rectangles, not an embedded PNG shortcut.
    let mut stream = String::from("1 g 0 0 640 480 re f\n0 g\n");
    for y in 0..code.height() {
        for x in 0..code.width() {
            if code.get_pixel(x, y)[0] < 128 {
                stream.push_str(&format!("{} {} 1 1 re f\n", 80 + x, 480 - 120 - y - 1));
            }
        }
    }
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 640 480] /Resources << >> /Contents 4 0 R >>"
            .into(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, object));
    }
    let xref = pdf.len();
    pdf.push_str("xref\n0 5\n0000000000 65535 f \n");
    for offset in &offsets[1..] {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    pdf.into_bytes()
}
