//! Wave 7 hosted mappings bound to the wave 4 immutable evidence catalogue.
use super::{catalog::Catalog, schema::Role};
use crate::wave7::{
    models::Result,
    observation::{ImageInput, ObservationRequest, Task},
    providers::{Coordinates, Provider, ProviderAdapter},
};
/// Produce the bounded vendor request and optionally decode a recorded response.
/// No credential loading, egress, costs, approval or live qualification is implied.
pub fn mapped(
    catalog: &Catalog,
    pngs: &[(Role, Vec<u8>)],
    provider: Provider,
    data: &str,
    response: Option<&[u8]>,
) -> Result<serde_json::Value> {
    catalog
        .validate()
        .map_err(|e| crate::wave7::models::VisionError::Invalid(e.to_string()))?;
    let mut images = Vec::new();
    for (index, (role, png)) in pngs.iter().enumerate() {
        let image = catalog
            .images
            .iter()
            .find(|i| i.role == *role)
            .ok_or_else(|| {
                crate::wave7::models::VisionError::Invalid(
                    "provider image absent from catalog".into(),
                )
            })?;
        if crate::evidence::canonical::Digest::of_bytes(png) != image.encoded_sha256 {
            return Err(crate::wave7::models::VisionError::Integrity(
                "provider PNG differs from assist catalog".into(),
            ));
        }
        let dims = image.dimensions;
        // Wave 4 capture presentation can be resized. Do not guess a transform.
        if image.transform.crop != [0, 0, dims[0], dims[1]] {
            return Err(crate::wave7::models::VisionError::Invalid(
                "hosted mapping currently requires full-size catalog PNG".into(),
            ));
        }
        images.push(ImageInput {
            id: format!("image-{index}"),
            bytes: png.clone(),
            media_type: "image/png".into(),
            original_size: dims,
            presented_size: image.transform.encoded,
            scale: [
                image.transform.encoded[0] as f32 / dims[0] as f32,
                image.transform.encoded[1] as f32 / dims[1] as f32,
            ],
            offset: [0., 0.],
        });
    }
    let request = ObservationRequest {
        task: Task::Reasoning,
        data: data.into(),
        images,
        model: provider.model().into(),
        encoder_version: "assist-wave7-mapping/1".into(),
        max_output_tokens: 2048,
    };
    let adapter = ProviderAdapter {
        provider,
        coordinates: Coordinates::Pixels,
    };
    let observation = response
        .map(|bytes| adapter.decode(&request, bytes))
        .transpose()?;
    Ok(
        serde_json::json!({"schema":"saccade-assist-vision-provider.v1","authority":"advisory, fixture-only; deterministic measurements unchanged","catalog_sha256":super::digest(catalog).map_err(|e|crate::wave7::models::VisionError::Invalid(e.to_string()))?,"request_sha256":request.hash()?,"request":adapter.request(&request)?,"observation":observation,"cost":"unknown; no live transport or spend","live_qualification":false}),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn hosted_mapping_binds_catalog_pixels_and_retains_advisory_authority() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([120; 3])))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let digest = crate::evidence::canonical::Digest::of_bytes(&png);
        let catalog = Catalog {
            version: super::super::schema::CATALOG_VERSION.into(),
            images: vec![super::super::catalog::Image {
                role: Role::Single,
                sha256: digest.clone(),
                encoded_sha256: digest,
                dimensions: [8, 8],
                capture_scope: [0, 0, 8, 8],
                complete: true,
                original_pixels: true,
                transform: super::super::geometry::Transform {
                    crop: [0, 0, 8, 8],
                    encoded: [8, 8],
                },
            }],
            regions: vec![],
            exclusions: vec![],
            measurements: serde_json::json!({}),
            source_evidence: vec![],
            source_evidence_hashes: vec![],
        };
        let mut pngs = vec![(Role::Single, png)];
        for provider in [Provider::Claude, Provider::Gpt] {
            let result = mapped(&catalog, &pngs, provider, "visible label", None).unwrap();
            assert_eq!(result["live_qualification"], false);
            assert_eq!(result["observation"], serde_json::Value::Null);
            assert!(result["authority"].as_str().unwrap().contains("advisory"));
        }
        pngs[0].1[0] ^= 1;
        assert!(matches!(
            mapped(&catalog, &pngs, Provider::Claude, "visible label", None),
            Err(crate::wave7::models::VisionError::Integrity(_))
        ));
    }
}
