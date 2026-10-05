//! Bounded compression/resampling observations; no attribution or authenticity verdict.
use serde_json::{Value, json};
fn luma(image: &image::RgbaImage, x: u32, y: u32) -> f64 {
    let p = image.get_pixel(x, y);
    let a = f64::from(p[3]) / 255.;
    (0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])) * a
        + 255. * (1. - a)
}
fn periodicity(values: &[f64]) -> Vec<Value> {
    if values.len() < 32 {
        return Vec::new();
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    (2..=16).map(|lag| {
        let mut aa=0.;let mut bb=0.;let mut ab=0.;
        for i in lag..values.len(){let a=values[i]-mean;let b=values[i-lag]-mean;aa+=a*a;bb+=b*b;ab+=a*b;}
        json!({"lag_pixels":lag,"correlation":if aa*bb>1e-12{Some(ab/(aa*bb).sqrt())}else{None}})
    }).collect()
}
/// Report DCT histogram gaps and raster derivative periodicity as unqualified observations.
/// The decoded-pixel DCT is approximate; it cannot recover the original compressed coefficients.
pub fn indicators(image: &image::RgbaImage, tables: &Value) -> crate::Result<Value> {
    if image.width() == 0
        || image.height() == 0
        || u64::from(image.width()) * u64::from(image.height()) > super::input::MAX_PIXELS
    {
        return Err(crate::Error::Config("forensic raster bounds".into()));
    }
    let table = tables
        .as_array()
        .and_then(|t| t.iter().find(|t| t["id"] == 0))
        .and_then(|t| t["zigzag_values"].as_array());
    let mut dct = Vec::new();
    if let Some(table) = table {
        let blocks_x = image.width() / 8;
        let blocks_y = image.height() / 8;
        let count = (blocks_x * blocks_y).min(4096);
        for (u, v, zigzag) in [(1, 0, 1), (0, 1, 2), (1, 1, 4), (2, 0, 5)] {
            let Some(q) = table
                .get(zigzag)
                .and_then(Value::as_f64)
                .filter(|&v| v > 0.)
            else {
                continue;
            };
            let mut histogram = vec![0u32; 257];
            for block in 0..count {
                let stride = (blocks_x * blocks_y / count.max(1)).max(1);
                let block = block * stride;
                let (bx, by) = ((block % blocks_x) * 8, (block / blocks_x) * 8);
                let mut coefficient = 0.;
                for y in 0..8 {
                    for x in 0..8 {
                        coefficient += (luma(image, bx + x, by + y) - 128.)
                            * ((f64::from(2 * x + 1) * f64::from(u) * std::f64::consts::PI) / 16.)
                                .cos()
                            * ((f64::from(2 * y + 1) * f64::from(v) * std::f64::consts::PI) / 16.)
                                .cos();
                    }
                }
                coefficient *= 0.25
                    * if u == 0 || v == 0 {
                        std::f64::consts::FRAC_1_SQRT_2
                    } else {
                        1.
                    };
                let quantized = (coefficient / q).round().clamp(-128., 128.) as i32;
                histogram[(quantized + 128) as usize] += 1;
            }
            let first = histogram.iter().position(|&v| v > 0).unwrap_or(0);
            let last = histogram.iter().rposition(|&v| v > 0).unwrap_or(0);
            let gaps = histogram[first..=last].iter().filter(|&&v| v == 0).count();
            dct.push(json!({"frequency":[u,v],"current_quantizer":q,"blocks":count,"coefficient_range":[first as i32-128,last as i32-128],"empty_bins_between_extrema":gaps,"histogram":histogram}));
        }
    }
    let (w, h) = (image.width().min(512), image.height().min(512));
    let (ox, oy) = ((image.width() - w) / 2, (image.height() - h) / 2);
    let mut columns = vec![0.; w.saturating_sub(2) as usize];
    let mut rows = vec![0.; h.saturating_sub(2) as usize];
    for y in 0..h {
        for x in 1..w.saturating_sub(1) {
            columns[(x - 1) as usize] += (luma(image, ox + x - 1, oy + y)
                - 2. * luma(image, ox + x, oy + y)
                + luma(image, ox + x + 1, oy + y))
            .abs()
                / f64::from(h);
        }
    }
    for x in 0..w {
        for y in 1..h.saturating_sub(1) {
            rows[(y - 1) as usize] += (luma(image, ox + x, oy + y - 1)
                - 2. * luma(image, ox + x, oy + y)
                + luma(image, ox + x, oy + y + 1))
            .abs()
                / f64::from(w);
        }
    }
    Ok(
        json!({"encoder_signatures":{"family":"conventional_Annex_K_scaling_compatibility","compatible_quality":tables.as_array().and_then(|t|t.iter().find(|t|t["id"]==0)).and_then(|t|t.get("conventional_luminance_quality")),"encoder_identity":"unknown","cannot_show":"many encoders share these tables; no exact encoder attribution"},"double_compression":{"status":"observed_unqualified","approximate_decoded_dct":dct,"can_show":"low-frequency coefficient histogram gaps after current-table requantization","cannot_show":"content/single compression also cause gaps; equal or misaligned double compression can be missed; no history verdict"},"resampling":{"status":"observed_unqualified","sample_rect":[ox,oy,w,h],"horizontal":periodicity(&columns),"vertical":periodicity(&rows),"can_show":"periodicity in second-derivative energy at 2..16 pixel lags","cannot_show":"textures, JPEG blocks and graphics also repeat; no inferred resize factor or editing verdict"},"qualification":"constructed positive/negative history discrimination remains pending; raw observations only"}),
    )
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn constant_is_uninformative_and_periodic_edges_are_observed() {
        let flat = image::RgbaImage::from_pixel(64, 64, image::Rgba([100, 100, 100, 255]));
        let report = indicators(&flat, &json!([])).unwrap();
        assert_eq!(
            report["resampling"]["horizontal"][0]["correlation"],
            Value::Null
        );
        let periodic = image::RgbaImage::from_fn(64, 64, |x, _| {
            image::Rgba(if x % 4 < 1 { [0, 0, 0, 255] } else { [255; 4] })
        });
        let report = indicators(&periodic, &json!([])).unwrap();
        assert!(
            report["resampling"]["horizontal"][2]["correlation"]
                .as_f64()
                .unwrap()
                > 0.99
        );
        assert_eq!(report["encoder_signatures"]["encoder_identity"], "unknown");
    }
    #[test]
    #[ignore = "heavy: forensics"]
    fn constructed_single_double_and_resize_histories_remain_observations() {
        let source = image::RgbaImage::from_fn(128, 128, |x, y| {
            image::Rgba([
                ((x * 37 + y * 13) % 256) as u8,
                ((y * 29 + x * 7) % 256) as u8,
                ((x * y) % 256) as u8,
                255,
            ])
        });
        let encode = |image: &image::RgbaImage, q| {
            let mut b = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut b, q)
                .encode_image(image)
                .unwrap();
            b
        };
        let once = encode(&source, 75);
        let decoded = super::super::input::decode(&once).unwrap();
        let twice = encode(&decoded, 90);
        let resize =
            image::imageops::resize(&source, 192, 192, image::imageops::FilterType::Triangle);
        let mut results = Vec::new();
        for bytes in [once, twice, encode(&resize, 90)] {
            let image = super::super::input::decode(&bytes).unwrap();
            let header = super::super::integrity::headers(&bytes, &image, false).unwrap();
            results
                .push(indicators(&image, &header["compression"]["quantisation_tables"]).unwrap());
        }
        assert_ne!(
            results[0]["double_compression"]["approximate_decoded_dct"],
            results[1]["double_compression"]["approximate_decoded_dct"]
        );
        assert_ne!(
            results[0]["resampling"]["horizontal"],
            results[2]["resampling"]["horizontal"]
        );
        // This checks sensitivity, not specificity or a qualified history classifier.
        assert!(
            results
                .iter()
                .all(|r| r["qualification"].as_str().unwrap().contains("pending"))
        );
    }
}
