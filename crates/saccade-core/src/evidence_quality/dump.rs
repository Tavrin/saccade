//! Bounded generic screen-space instance/material dumps rasterized into native IDs.
use crate::{Error, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
/// One generic screen-space object. Coordinates are capture pixels; boxes are x/y/width/height.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    /// Nonzero integer native label, at most 65535.
    pub id: u32,
    /// Producer name.
    pub name: String,
    /// Pixel boxes; sample centers define membership.
    #[serde(default, alias = "bounding_boxes")]
    pub boxes: Vec<[f64; 4]>,
    /// Screen-space polygons; sample centers use even-odd membership.
    #[serde(default)]
    pub polygons: Vec<Vec<[f64; 2]>>,
}
/// Rasterize an explicitly supplied dump. Later objects win overlapping pixels;
/// this is declared geometry with lower confidence than an actual ID buffer.
pub fn rasterize(
    bytes: &[u8],
    dimensions: (u32, u32),
) -> Result<(image::DynamicImage, BTreeMap<u32, String>)> {
    if bytes.len() > 1 << 20 {
        return Err(Error::Config("instance dump exceeds byte budget".into()));
    }
    let instances: Vec<Instance> = serde_json::from_slice(bytes)?;
    let (w, h) = dimensions;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16_777_216 || instances.len() > 1024 {
        return Err(Error::Config(
            "instance dump exceeds object/pixel budget".into(),
        ));
    }
    let mut image = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(w, h);
    let mut names = BTreeMap::new();
    let mut work = 0u64;
    for instance in instances {
        if instance.id == 0
            || instance.id > 65535
            || instance.name.trim().is_empty()
            || names.insert(instance.id, instance.name).is_some()
            || instance.boxes.len() + instance.polygons.len() > 1024
        {
            return Err(Error::Config(
                "invalid/duplicate dump id/name/geometry".into(),
            ));
        }
        for rect in instance.boxes {
            let [x, y, rw, rh] = rect;
            if rect.iter().any(|v| !v.is_finite()) || rw <= 0.0 || rh <= 0.0 {
                return Err(Error::Config("invalid dump box".into()));
            }
            let x0 = x.floor().clamp(0.0, f64::from(w)) as u32;
            let x1 = (x + rw).ceil().clamp(0.0, f64::from(w)) as u32;
            let y0 = y.floor().clamp(0.0, f64::from(h)) as u32;
            let y1 = (y + rh).ceil().clamp(0.0, f64::from(h)) as u32;
            work = work.saturating_add(u64::from(x1 - x0) * u64::from(y1 - y0));
            if work > 64 * 1024 * 1024 {
                return Err(Error::Config(
                    "dump rasterization exceeds work budget".into(),
                ));
            }
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let (px, py) = (f64::from(xx) + 0.5, f64::from(yy) + 0.5);
                    if px >= x && px < x + rw && py >= y && py < y + rh {
                        image.put_pixel(xx, yy, image::Luma([instance.id as u16]));
                    }
                }
            }
        }
        for polygon in instance.polygons {
            if !(3..=1024).contains(&polygon.len())
                || polygon.iter().flatten().any(|v| !v.is_finite())
            {
                return Err(Error::Config("invalid dump polygon".into()));
            }
            let x0 = polygon
                .iter()
                .map(|p| p[0])
                .fold(f64::INFINITY, f64::min)
                .floor()
                .clamp(0.0, f64::from(w)) as u32;
            let x1 = polygon
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .clamp(0.0, f64::from(w)) as u32;
            let y0 = polygon
                .iter()
                .map(|p| p[1])
                .fold(f64::INFINITY, f64::min)
                .floor()
                .clamp(0.0, f64::from(h)) as u32;
            let y1 = polygon
                .iter()
                .map(|p| p[1])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .clamp(0.0, f64::from(h)) as u32;
            work =
                work.saturating_add(u64::from(x1 - x0) * u64::from(y1 - y0) * polygon.len() as u64);
            if work > 64 * 1024 * 1024 {
                return Err(Error::Config("dump polygon work budget exceeded".into()));
            }
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let (px, py) = (f64::from(xx) + 0.5, f64::from(yy) + 0.5);
                    let mut inside = false;
                    let mut previous = polygon[polygon.len() - 1];
                    for point in &polygon {
                        if (point[1] > py) != (previous[1] > py)
                            && px
                                < (previous[0] - point[0]) * (py - point[1])
                                    / (previous[1] - point[1])
                                    + point[0]
                        {
                            inside = !inside;
                        }
                        previous = *point;
                    }
                    if inside {
                        image.put_pixel(xx, yy, image::Luma([instance.id as u16]));
                    }
                }
            }
        }
    }
    Ok((image::DynamicImage::ImageLuma16(image), names))
}
#[cfg(test)]
mod tests {
    #[test]
    fn boxes_polygons_overlap_and_malformed_geometry() {
        let bytes=br#"[{"id":7,"name":"panel","boxes":[[0,0,3,4]]},{"id":9,"name":"triangle","polygons":[[[1,0],[4,0],[4,3]]]}]"#;
        let (image, names) = super::rasterize(bytes, (4, 4)).unwrap();
        let ids = super::super::field::ids(&image).unwrap();
        assert_eq!(ids[0], 7);
        assert_eq!(ids[3], 9);
        assert_eq!(ids[15], 0);
        assert_eq!(names[&9], "triangle");
        assert!(
            super::rasterize(br#"[{"id":1,"name":"bad","boxes":[[0,0,-1,1]]}]"#, (4, 4)).is_err()
        );
    }
}
