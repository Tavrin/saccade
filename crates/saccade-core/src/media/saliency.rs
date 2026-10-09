//! Deterministic bounded colour-surround saliency with a weak centre/edge prior.
use serde::{Deserialize, Serialize};
/// Small map and original-pixel focal point. Not an object detector.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Saliency {
    /// Map resolution, at most 64 by 64.
    pub size: [u32; 2],
    /// Row-major normalized saliency in `[0,1]`.
    pub map: Vec<f32>,
    /// Original-pixel saliency-weighted centroid, or centre for a flat image.
    pub focal_point: [f32; 2],
    /// False for a flat image where the centre fallback is used.
    pub informative: bool,
}
/// Colour difference from a broad surround, suppressing the outermost edge gently.
pub fn compute(image: &image::RgbaImage) -> Saliency {
    let w = image.width().clamp(1, 64);
    let h = image.height().clamp(1, 64);
    let rgb = crate::compare::flatten_over(image, 255);
    let small = image::imageops::resize(&rgb, w, h, image::imageops::FilterType::Triangle);
    let surround = image::imageops::blur(&small, (w.min(h) as f32 / 6.).max(1.));
    let mut map = Vec::with_capacity((w * h) as usize);
    let mut mass = 0.;
    let mut sum = [0.; 2];
    let mut maximum: f32 = 0.;
    for (x, y, p) in small.enumerate_pixels() {
        let q = surround.get_pixel(x, y);
        let difference = (0..3)
            .map(|i| (f32::from(p[i]) - f32::from(q[i])).powi(2))
            .sum::<f32>();
        let nx = (x as f32 + 0.5) / w as f32;
        let ny = (y as f32 + 0.5) / h as f32;
        let prior = 0.75 + nx.min(1. - nx).min(ny.min(1. - ny)) * 0.5;
        let weight = difference * prior;
        maximum = maximum.max(weight);
        mass += weight;
        sum[0] += weight * nx;
        sum[1] += weight * ny;
        map.push(weight);
    }
    let informative = maximum > 1. && mass > 0.;
    if maximum > 0. {
        for v in &mut map {
            *v /= maximum;
        }
    }
    let focal_point = if informative {
        [
            sum[0] / mass * image.width() as f32,
            sum[1] / mass * image.height() as f32,
        ]
    } else {
        [image.width() as f32 / 2., image.height() as f32 / 2.]
    };
    Saliency {
        size: [w, h],
        map,
        focal_point,
        informative,
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn offcentre_object_guides_declared_portrait_crop() {
        let im = image::RgbaImage::from_fn(120, 80, |x, y| {
            image::Rgba(if (86..100).contains(&x) && (32..48).contains(&y) {
                [250, 30, 10, 255]
            } else {
                [60, 60, 60, 255]
            })
        });
        let a = super::compute(&im);
        assert!(a.focal_point[0] > 85. && a.focal_point[0] < 102.);
        assert!(a.map.iter().any(|v| *v > 0.9));
        assert!(
            a.map
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        );
        let invisible = image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 0]));
        assert!(!super::compute(&invisible).informative);
    }
    #[test]
    fn single_objects_move_focal_point_and_flat_uses_centre() {
        for (cx, cy) in [(20, 24), (76, 70), (50, 45)] {
            let im = image::RgbaImage::from_fn(100, 100, |x, y| {
                image::Rgba(if x.abs_diff(cx) < 7 && y.abs_diff(cy) < 7 {
                    [240, 20, 30, 255]
                } else {
                    [40, 40, 40, 255]
                })
            });
            let a = super::compute(&im);
            assert!(a.informative);
            assert!(
                (a.focal_point[0] - cx as f32).abs() < 5.
                    && (a.focal_point[1] - cy as f32).abs() < 5.,
                "{:?}",
                a.focal_point
            );
            assert_eq!(a.map, super::compute(&im).map);
        }
        let a = super::compute(&image::RgbaImage::from_pixel(
            40,
            30,
            image::Rgba([20, 20, 20, 255]),
        ));
        assert!(!a.informative);
        assert_eq!(a.focal_point, [20., 15.]);
    }
}
