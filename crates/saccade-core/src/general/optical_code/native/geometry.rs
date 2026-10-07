//! Project sampled QR grid coordinates back to original capture pixels.
use super::{Quality, decoder_error};
use rxing::{
    Point,
    common::{DetectorRXingResult, PerspectiveTransform, Quadrilateral},
    point,
};

pub(super) fn point_box(points: &[Point], r: [u32; 4]) -> Option<[u32; 4]> {
    if points.is_empty() || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return None;
    }
    let minx = points
        .iter()
        .map(|p| p.x)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.) as u32;
    let miny = points
        .iter()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.) as u32;
    let maxx = points
        .iter()
        .map(|p| p.x)
        .fold(0., f32::max)
        .ceil()
        .min(r[2] as f32) as u32;
    let maxy = points
        .iter()
        .map(|p| p.y)
        .fold(0., f32::max)
        .ceil()
        .min(r[3] as f32) as u32;
    Some([
        r[0] + minx.min(r[2] - 1),
        r[1] + miny.min(r[3] - 1),
        maxx.saturating_sub(minx).max(1),
        maxy.saturating_sub(miny).max(1),
    ])
}
pub(super) fn qr(
    grid: &impl DetectorRXingResult,
    luma: &[u8],
    r: [u32; 4],
) -> crate::Result<([u32; 4], Quality)> {
    let points = grid.getPoints();
    if points.len() < 3 {
        return Err(crate::Error::Config(
            "QR finder geometry unavailable".into(),
        ));
    }
    let [bl, tl, tr] = [points[0], points[1], points[2]];
    let n = grid.getBits().getWidth() as f32;
    let (br, end) = if let Some(p) = points.get(3) {
        (*p, n - 6.5)
    } else {
        (point(tr.x + bl.x - tl.x, tr.y + bl.y - tl.y), n - 3.5)
    };
    let transform = PerspectiveTransform::quadrilateralToQuadrilateral(
        Quadrilateral::new(
            point(3.5, 3.5),
            point(n - 3.5, 3.5),
            point(end, end),
            point(3.5, n - 3.5),
        ),
        Quadrilateral::new(tl, tr, br, bl),
    )
    .map_err(decoder_error)?;
    let project = |x: f32, y: f32| {
        let mut p = [point(x, y)];
        transform.transform_points_single(&mut p);
        p[0]
    };
    let sample = |x: f32, y: f32| -> Option<f64> {
        let p = project(x, y);
        if !p.x.is_finite()
            || !p.y.is_finite()
            || p.x < 0.
            || p.y < 0.
            || p.x >= r[2] as f32
            || p.y >= r[3] as f32
        {
            return None;
        }
        Some(f64::from(luma[(p.y as u32 * r[2] + p.x as u32) as usize]))
    };
    let corners = [
        project(0., 0.),
        project(n, 0.),
        project(n, n),
        project(0., n),
    ];
    let bbox = point_box(&corners, r)
        .ok_or_else(|| crate::Error::Config("nonfinite QR geometry".into()))?;
    let mut module = f64::INFINITY;
    for (x, y) in [
        (0.5, 0.5),
        (n - 1.5, 0.5),
        (0.5, n - 1.5),
        (n - 1.5, n - 1.5),
    ] {
        let a = project(x, y);
        for b in [project(x + 1., y), project(x, y + 1.)] {
            module = module.min(f64::from((a.x - b.x).hypot(a.y - b.y)));
        }
    }
    let mut q = Quality {
        module_size_px: module.is_finite().then_some(module),
        ..Default::default()
    };
    if corners
        .iter()
        .any(|p| p.x < 0. || p.y < 0. || p.x > r[2] as f32 || p.y > r[3] as f32)
    {
        q.reasons
            .push("projected QR bounding envelope is clipped to the declared region".into());
    }
    let (mut dark, mut light, mut nd, mut nl) = (0., 0., 0u32, 0u32);
    let mut clipped = false;
    for y in 0..n as u32 {
        for x in 0..n as u32 {
            if let Some(v) = sample(x as f32 + 0.5, y as f32 + 0.5) {
                if grid.getBits().get(x, y) {
                    dark += v;
                    nd += 1;
                } else {
                    light += v;
                    nl += 1;
                }
            } else {
                clipped = true;
            }
        }
    }
    if nd > 0 && nl > 0 && !clipped {
        let d = dark / f64::from(nd);
        let l = light / f64::from(nl);
        q.contrast = Some(((l - d) / 255.).max(0.));
        // Demand light-like pixels using the decoded grid's light/dark midpoint.
        let threshold = (d + l) / 2.;
        let mut clear = [0; 4];
        let mut missing = false;
        for (side, count) in clear.iter_mut().enumerate() {
            for distance in 0..4 {
                let mut clean = true;
                for i in 0..n as u32 {
                    let along = i as f32 + 0.5;
                    let offset = distance as f32 + 0.5;
                    let (x, y) = match side {
                        0 => (along, -offset),
                        1 => (n + offset, along),
                        2 => (along, n + offset),
                        _ => (-offset, along),
                    };
                    match sample(x, y) {
                        Some(v) if v > threshold && l > d => {}
                        Some(_) => clean = false,
                        None => {
                            missing = true;
                            clean = false;
                        }
                    }
                }
                if !clean {
                    break;
                }
                *count += 1;
            }
        }
        q.quiet_zone_modules = Some(clear);
        q.quiet_zone_clear = if clear == [4; 4] {
            Some(true)
        } else if missing {
            None
        } else {
            Some(false)
        };
        if missing {
            q.reasons
                .push("quiet zone extends beyond the declared region or raster".into());
        }
    } else {
        q.reasons.push(
            "module contrast/quiet zone unavailable: clipped or missing light/dark samples".into(),
        );
    }
    Ok((bbox, q))
}
