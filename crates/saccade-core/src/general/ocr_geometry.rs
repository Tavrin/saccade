//! DB quad postprocessing and perspective crops. Parameters from pinned inference.yml.
use crate::{Error, Result};
type Quad = [[f32; 2]; 4];
fn length(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn ordered(mut p: Quad) -> Quad {
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    if p[0][1] > p[1][1] {
        p.swap(0, 1);
    }
    if p[2][1] > p[3][1] {
        p.swap(2, 3);
    }
    [p[0], p[2], p[3], p[1]]
}
fn inside(p: [f32; 2], q: Quad) -> bool {
    let cross: [f32; 4] = std::array::from_fn(|i| {
        let a = q[i];
        let b = q[(i + 1) % 4];
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    });
    cross.iter().all(|v| *v >= -0.01) || cross.iter().all(|v| *v <= 0.01)
}
/// OpenCV INTER_LINEAR convention: pixel-centre coordinates, bilinear sampling,
/// replicated edge values and no downsampling antialias filter.
pub(super) fn resize(rgb: &image::RgbImage, w: u32, h: u32) -> Result<image::RgbImage> {
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 2 * 1024 * 1024 {
        return Err(Error::Config("OCR resize bound".into()));
    }
    Ok(image::RgbImage::from_fn(w, h, |x, y| {
        let sx = ((x as f64 + 0.5) * f64::from(rgb.width()) / f64::from(w) - 0.5)
            .clamp(0., f64::from(rgb.width() - 1));
        let sy = ((y as f64 + 0.5) * f64::from(rgb.height()) / f64::from(h) - 0.5)
            .clamp(0., f64::from(rgb.height() - 1));
        let ix = sx.floor() as u32;
        let iy = sy.floor() as u32;
        let fx = sx - f64::from(ix);
        let fy = sy - f64::from(iy);
        let a = rgb.get_pixel(ix, iy);
        let b = rgb.get_pixel((ix + 1).min(rgb.width() - 1), iy);
        let c = rgb.get_pixel(ix, (iy + 1).min(rgb.height() - 1));
        let d = rgb.get_pixel(
            (ix + 1).min(rgb.width() - 1),
            (iy + 1).min(rgb.height() - 1),
        );
        image::Rgb(std::array::from_fn(|i| {
            ((1. - fy) * ((1. - fx) * f64::from(a[i]) + fx * f64::from(b[i]))
                + fy * ((1. - fx) * f64::from(c[i]) + fx * f64::from(d[i])))
            .round() as u8
        }))
    }))
}
/// Threshold .3, rectangle mean >=.6, <=1000 contours, min-side3, unclip1.5, expanded min-side5.
pub(super) fn boxes(map: &[f32], w: u32, h: u32, size: (u32, u32)) -> Result<Vec<Quad>> {
    if map.len() != w as usize * h as usize || w == 0 || h == 0 || map.len() > 2 * 1024 * 1024 {
        return Err(Error::Config("DB bitmap bounds".into()));
    }
    let bitmap = image::GrayImage::from_fn(w, h, |x, y| {
        image::Luma([u8::from(map[y as usize * w as usize + x as usize] > 0.3) * 255])
    });
    let contours = imageproc::contours::find_contours::<i32>(&bitmap);
    let mut out = Vec::new();
    for contour in contours.iter().take(1000) {
        if contour.points.len() < 3 {
            continue;
        }
        let r = imageproc::geometry::min_area_rect(&contour.points);
        let q = ordered(r.map(|p| [p.x as f32, p.y as f32]));
        let a = length(q[0], q[1]);
        let b = length(q[0], q[3]);
        if a.min(b) < 3. {
            continue;
        }
        let xmin = q
            .iter()
            .map(|p| p[0])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let xmax = q
            .iter()
            .map(|p| p[0])
            .fold(0., f32::max)
            .ceil()
            .min((w - 1) as f32) as u32;
        let ymin = q
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let ymax = q
            .iter()
            .map(|p| p[1])
            .fold(0., f32::max)
            .ceil()
            .min((h - 1) as f32) as u32;
        let mut sum = 0f64;
        let mut count = 0;
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                if inside([x as f32, y as f32], q) {
                    sum += f64::from(map[y as usize * w as usize + x as usize]);
                    count += 1;
                }
            }
        }
        if count == 0 || sum / f64::from(count) < 0.6 {
            continue;
        }
        // Minimum rectangle of the round offset of a rectangle expands each side by
        // area * unclip_ratio / perimeter; no general polygon offset is needed for quad mode.
        let d = a * b * 1.5 / (2. * (a + b));
        if a.min(b) + 2. * d < 5. {
            continue;
        }
        let u = [(q[1][0] - q[0][0]) / a, (q[1][1] - q[0][1]) / a];
        let v = [(q[3][0] - q[0][0]) / b, (q[3][1] - q[0][1]) / b];
        let signs = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]];
        let mut expanded = q;
        for i in 0..4 {
            for c in 0..2 {
                expanded[i][c] += d * (signs[i][0] * u[c] + signs[i][1] * v[c]);
            }
            expanded[i][0] = (expanded[i][0] / w as f32 * size.0 as f32)
                .round()
                .clamp(0., size.0.saturating_sub(1) as f32);
            expanded[i][1] = (expanded[i][1] / h as f32 * size.1 as f32)
                .round()
                .clamp(0., size.1.saturating_sub(1) as f32);
        }
        if length(expanded[0], expanded[1]) >= 1. && length(expanded[0], expanded[3]) >= 1. {
            out.push(expanded);
        }
    }
    out.sort_by(|a, b| {
        a[0][1]
            .total_cmp(&b[0][1])
            .then(a[0][0].total_cmp(&b[0][0]))
    });
    // Match upstream adjacent-line x ordering with a ten-pixel y tolerance.
    for i in 1..out.len() {
        let mut j = i;
        while j > 0
            && (out[j][0][1] - out[j - 1][0][1]).abs() < 10.
            && out[j][0][0] < out[j - 1][0][0]
        {
            out.swap(j, j - 1);
            j -= 1;
        }
    }
    Ok(out)
}
/// Bilinear perspective rectification; long vertical crops rotate 90 degrees like upstream.
pub(super) fn rectify(rgb: &image::RgbImage, q: Quad) -> Result<image::RgbImage> {
    use imageproc::geometric_transformations::{Interpolation, Projection, warp_into};
    let w = length(q[0], q[1]).max(length(q[3], q[2])) as u32;
    let h = length(q[0], q[3]).max(length(q[1], q[2])) as u32;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16 * 1024 * 1024 {
        return Err(Error::Config("OCR crop bound".into()));
    }
    let from = q.map(|p| (p[0], p[1]));
    let to = [
        (0., 0.),
        (w as f32, 0.),
        (w as f32, h as f32),
        (0., h as f32),
    ];
    let projection = Projection::from_control_points(from, to)
        .ok_or_else(|| Error::Config("OCR crop degenerate geometry".into()))?;
    let mut crop = image::RgbImage::new(w, h);
    warp_into(
        rgb,
        &projection,
        Interpolation::Bilinear,
        image::Rgb([255; 3]),
        &mut crop,
    );
    Ok(if h as f32 / w as f32 >= 1.5 {
        image::imageops::rotate270(&crop)
    } else {
        crop
    })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn db_threshold_score_minimum_and_unclip_have_effect() {
        let mut map = vec![0.; 64 * 32];
        for y in 10..20 {
            for x in 12..44 {
                map[y * 64 + x] = 0.9;
            }
        }
        let b = boxes(&map, 64, 32, (64, 32)).unwrap();
        assert_eq!(b.len(), 1);
        assert!(b[0][0][0] < 12. && b[0][0][1] < 10.);
        for v in &mut map {
            if *v > 0. {
                *v = 0.4;
            }
        }
        assert!(boxes(&map, 64, 32, (64, 32)).unwrap().is_empty());
        let rgb = image::RgbImage::from_pixel(64, 32, image::Rgb([40, 80, 120]));
        let crop = rectify(&rgb, [[12., 10.], [44., 10.], [44., 20.], [12., 20.]]).unwrap();
        assert_eq!(crop.dimensions(), (32, 10));
        assert_eq!(crop.get_pixel(5, 5).0, [40, 80, 120]);
    }
    #[test]
    fn linear_downscale_samples_centres_without_antialiasing() {
        let image = image::RgbImage::from_fn(8, 1, |x, _| {
            image::Rgb([if x == 3 || x == 4 { 255 } else { 0 }; 3])
        });
        let resized = resize(&image, 2, 1).unwrap();
        assert_eq!(resized.get_pixel(0, 0).0, [0; 3]);
        assert_eq!(resized.get_pixel(1, 0).0, [0; 3]);
        let tiny = image::RgbImage::from_pixel(1, 1, image::Rgb([17, 31, 55]));
        assert_eq!(resize(&tiny, 4, 3).unwrap().get_pixel(3, 2).0, [17, 31, 55]);
    }
}
