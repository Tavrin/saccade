use super::*;
fn load(
    root: &Path,
    pin: &ImagePin,
    dimensions: [u32; 2],
) -> std::result::Result<image::DynamicImage, String> {
    if !valid_hash(&pin.sha256) {
        return Err("invalid image pin".into());
    }
    let path = contained(root, &pin.path).map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "missing render file".into()
        } else {
            e.to_string()
        }
    })?;
    if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err("image must be a regular file <=64 MiB".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if crate::localized::digest(&bytes) != pin.sha256 {
        return Err("image byte hash mismatch".into());
    }
    let actual = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    if [actual.0, actual.1] != dimensions {
        return Err("image dimensions differ from context".into());
    }
    image::load_from_memory(&bytes).map_err(|e| e.to_string())
}
fn bounds(indices: impl Iterator<Item = usize>, w: u32) -> Option<[u32; 4]> {
    let mut b = [u32::MAX, u32::MAX, 0, 0];
    let mut any = false;
    for i in indices {
        let x = i as u32 % w;
        let y = i as u32 / w;
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
        any = true;
    }
    any.then(|| [b[0], b[1], b[2] - b[0] + 1, b[3] - b[1] + 1])
}
fn pair(manifest: &Manifest, v: &View, root: &Path, context_hash: &str) -> Measurement {
    let camera_hash = hash(&v.camera).unwrap_or_default();
    let mut m = Measurement {
        id: v.id.clone(),
        camera_sha256: camera_hash.clone(),
        state: "missing_render".into(),
        issues: Vec::new(),
        flip: None,
        error_map: Vec::new(),
        high_error_bounds: None,
        silhouette: None,
        regression: None,
    };
    let (Some(a), Some(b)) = (&v.reference, &v.candidate) else {
        m.issues.push("required render receipt absent".into());
        return m;
    };
    m.state = "rejected_render".into();
    for (i, r) in [a, b].into_iter().enumerate() {
        let asset = &manifest.assets[i];
        if r.camera_sha256 != camera_hash
            || r.context_sha256 != context_hash
            || r.asset_document_sha256 != asset.document_sha256
            || r.asset_geometry_sha256 != asset.geometry_sha256
            || !valid_hash(&r.materials_sha256)
            || r.textures_sha256.len() > 256
            || r.textures_sha256.iter().any(|h| !valid_hash(h))
            || manifest.context.material_mode == "override_materials"
                && Some(&r.materials_sha256) != manifest.context.override_material_sha256.as_ref()
        {
            m.issues.push(format!(
                "{} camera/context/asset/material identity mismatch",
                if i == 0 { "reference" } else { "candidate" }
            ));
        }
    }
    if !m.issues.is_empty() {
        return m;
    }
    let images = [
        load(root, &a.image, manifest.context.dimensions),
        load(root, &b.image, manifest.context.dimensions),
    ];
    for (i, result) in images.iter().enumerate() {
        if let Err(e) = result {
            if e == "missing render file" {
                m.state = "missing_render".into();
            }
            m.issues.push(format!(
                "{}: {e}",
                if i == 0 { "reference" } else { "candidate" }
            ));
        }
    }
    if !m.issues.is_empty() {
        return m;
    }
    let [Ok(a_image), Ok(b_image)] = images else {
        return m;
    };
    let dimensions = manifest.context.dimensions;
    for image in [&a_image, &b_image] {
        if [image.width(), image.height()] != dimensions
            || !matches!(
                image.color(),
                image::ColorType::Rgb8
                    | image::ColorType::Rgba8
                    | image::ColorType::L8
                    | image::ColorType::La8
            )
        {
            m.issues
                .push("render dimensions/encoded SDR format differ from context".into());
        }
    }
    if a.silhouette.is_some() != b.silhouette.is_some() {
        m.issues.push("silhouette evidence is one-sided".into());
    }
    if !m.issues.is_empty() {
        return m;
    }
    if let (Some(a_mask), Some(b_mask)) = (&a.silhouette, &b.silhouette) {
        let masks = [
            load(root, a_mask, dimensions),
            load(root, b_mask, dimensions),
        ];
        if masks.iter().any(|r| {
            !r.as_ref().is_ok_and(|i| {
                i.color() == image::ColorType::L8 && [i.width(), i.height()] == dimensions
            })
        }) {
            m.issues
                .push("silhouette masks require matching hash-bound L8 dimensions".into());
            return m;
        }
        let [Ok(a_mask), Ok(b_mask)] = masks else {
            return m;
        };
        let a = a_mask.to_luma8();
        let b = b_mask.to_luma8();
        let n = a.len() as f64 * 255.0;
        m.silhouette = Some(Silhouette {
            reference_coverage: a.as_raw().iter().map(|&v| v as f64).sum::<f64>() / n,
            candidate_coverage: b.as_raw().iter().map(|&v| v as f64).sum::<f64>() / n,
            changed_fraction: a
                .as_raw()
                .iter()
                .zip(b.as_raw())
                .map(|(&a, &b)| a.abs_diff(b) as f64)
                .sum::<f64>()
                / n,
            change_bounds: bounds(
                a.as_raw()
                    .iter()
                    .zip(b.as_raw())
                    .enumerate()
                    .filter_map(|(i, (a, b))| (a != b).then_some(i)),
                dimensions[0],
            ),
        });
    }
    let comparison = crate::compare::compare_rgba(
        &b_image.to_rgba8(),
        &a_image.to_rgba8(),
        &crate::compare::CompareOptions {
            pixels_per_degree: manifest.pixels_per_degree,
            ..Default::default()
        },
    );
    match comparison {
        Ok(c) => {
            m.state = "measured".into();
            m.regression = Some(
                c.metrics.mean > manifest.maximum_mean_flip
                    || m.silhouette
                        .as_ref()
                        .is_some_and(|s| s.changed_fraction > manifest.maximum_silhouette_change),
            );
            m.flip = Some(c.metrics);
            m.high_error_bounds = bounds(
                c.error_map
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &e)| (e as f64 > manifest.maximum_mean_flip).then_some(i)),
                dimensions[0],
            );
            m.error_map = c.error_map;
        }
        Err(e) => m.issues.push(e.to_string()),
    }
    m
}
pub(super) fn measure(manifest: Manifest, root: &Path) -> Result<Report> {
    let manifest_sha256 = hash(&manifest)?;
    let context_sha256 = hash(&manifest.context)?;
    let views: Vec<_> = manifest
        .views
        .iter()
        .map(|v| pair(&manifest, v, root, &context_sha256))
        .collect();
    let measured_views = views.iter().filter(|v| v.state == "measured").count();
    let coverage = Coverage {
        declared_views: views.len(),
        supplied_pairs: manifest
            .views
            .iter()
            .filter(|v| v.reference.is_some() && v.candidate.is_some())
            .count(),
        measured_views,
        missing_views: views.iter().filter(|v| v.state == "missing_render").count(),
        rejected_views: views
            .iter()
            .filter(|v| v.state == "rejected_render")
            .count(),
        fraction: measured_views as f64 / views.len() as f64,
    };
    let worst_view = views
        .iter()
        .filter_map(|v| v.flip.map(|m| (v.id.clone(), m)))
        .max_by(|a, b| a.1.mean.total_cmp(&b.1.mean).then_with(|| b.0.cmp(&a.0)))
        .map(|(id, m)| WorstView {
            id,
            mean: m.mean,
            maximum: m.max,
        });
    let passed = coverage.measured_views == coverage.declared_views
        && views.iter().all(|v| v.regression == Some(false));
    Ok(Report{schema:"saccade-asset-view-report.v1".into(),manifest,manifest_sha256,context_sha256,views,worst_view,coverage,passed,
        limitations:vec!["Finite cameras establish sampled visibility only, not angular/surface coverage or a bound over all viewpoints and LOD transitions.".into(),"Supplied image/mesh identities are checked; camera matrices, renderer executable/source/environment and render-to-asset receipts are producer assertions, not a recapture attestation.".into(),"Raw per-view FLIP remains unaligned. The worst view is selected by mean, so inspect maxima and local error maps for tiny defects.".into(),"Geometry measurements remain sampled surface evidence. Material, normal-map, alpha coverage and renderer appearance are separate supplied render observations, not exact attribute identity.".into(),"Missing/rejected views cannot pass. Without paired masks, object silhouette coverage is unknown; camera coverage always uses the full declared view set.".into(),"No renderer execution, runtime cost, live LOD transition, temporal popping or human perception qualification. Encoded SDR images only, no implicit resize or tone mapping.".into()]})
}
