//! Pinned text detection and EfficientSAM pipelines. Successful execution is not source parity.
use super::{
    models::{Result, VisionError, verify},
    runtime::{FloatOutput, OnnxModel, ort_error},
    vision::{Detection, Detector, MAX_PIXELS, Mask, Provenance, Rect, Segmenter, VisionImage},
};
use ort::value::Tensor;
use std::{collections::BTreeMap, path::Path};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

/// Pinned BERT or CLIP text-query detector; tokenizer bytes are verified with graph bytes.
pub struct TextDetector {
    graph: OnnxModel,
    tokenizer: Tokenizer,
}
impl TextDetector {
    /// Load the selected immutable graph and its JSON tokenizer, CPU only.
    pub fn load(
        model: &super::models::Model,
        cache: &Path,
        library: &Path,
        download: bool,
    ) -> Result<Self> {
        if !matches!(
            model.input.adapter.as_str(),
            "grounding-dino-v1" | "owlv2-v1"
        ) {
            return Err(VisionError::Invalid("text detector adapter".into()));
        }
        let graph = OnnxModel::load(model, cache, library, download)?;
        let a = model
            .artifacts
            .iter()
            .find(|a| a.role == "tokenizer-json")
            .ok_or_else(|| VisionError::Unavailable("pinned tokenizer required".into()))?;
        let mut tokenizer = Tokenizer::from_file(verify(cache, a)?)
            .map_err(|_| VisionError::Invalid("tokenizer JSON".into()))?;
        tokenizer.with_padding(if model.id == "owlv2-base" {
            Some(PaddingParams {
                strategy: PaddingStrategy::Fixed(16),
                pad_id: 0,
                pad_token: "!".into(),
                ..Default::default()
            })
        } else {
            None
        });
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: if model.id == "owlv2-base" { 16 } else { 256 },
                ..Default::default()
            }))
            .map_err(|_| VisionError::Invalid("tokenizer truncation".into()))?;
        Ok(Self { graph, tokenizer })
    }
}
fn sigmoid(x: f32) -> f32 {
    1. / (1. + (-x).exp())
}
fn output<'a>(outputs: &'a BTreeMap<String, FloatOutput>, name: &str) -> Result<&'a FloatOutput> {
    outputs
        .get(name)
        .ok_or_else(|| VisionError::Invalid(format!("missing graph output {name}")))
}
fn center_box(b: &[f32], size: [u32; 2], scale: [f32; 2]) -> Option<Rect> {
    let x0 = ((b[0] - b[2] / 2.) * scale[0]).clamp(0., size[0] as f32);
    let y0 = ((b[1] - b[3] / 2.) * scale[1]).clamp(0., size[1] as f32);
    let x1 = ((b[0] + b[2] / 2.) * scale[0]).clamp(0., size[0] as f32);
    let y1 = ((b[1] + b[3] / 2.) * scale[1]).clamp(0., size[1] as f32);
    (x1 > x0 && y1 > y0).then_some(Rect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}
/// Aspect-preserving OWLv2 square pad in rescaled RGB space, per-channel image mean.
fn owl_tensor(image: &VisionImage, model: &super::models::Model) -> Result<Tensor<f32>> {
    let [w, h] = image.size();
    let side = w.max(h) as usize;
    if w == 0 || h == 0 || side as u64 * side as u64 > MAX_PIXELS {
        return Err(VisionError::Invalid(
            "OWLv2 padded canvas exceeds pixel budget".into(),
        ));
    }
    // Pad before resize without quantizing the mean to uint8.
    let mut mean = [0f64; 3];
    for p in image.pixels.pixels() {
        for c in 0..3 {
            mean[c] += f64::from(p[c]) / 255.;
        }
    }
    let n = f64::from(w) * f64::from(h);
    let mean = mean.map(|v| (v / n) as f32);
    let target = 960usize;
    let mut data = vec![0f32; 3 * target * target];
    // Triangle filtering includes antialiasing when the padded source is larger.
    for c in 0..3 {
        let channel = image::ImageBuffer::<image::Luma<f32>, Vec<f32>>::from_fn(
            side as u32,
            side as u32,
            |x, y| {
                image::Luma([if x < w && y < h {
                    f32::from(image.pixels.get_pixel(x, y)[c]) / 255.
                } else {
                    mean[c]
                }])
            },
        );
        let resized =
            image::imageops::resize(&channel, 960, 960, image::imageops::FilterType::Triangle);
        for (i, p) in resized.pixels().enumerate() {
            data[c * target * target + i] = (p[0] - model.input.mean[c]) / model.input.std[c];
        }
    }
    Tensor::from_array(([1usize, 3, 960, 960], data)).map_err(ort_error)
}
fn dino_geometry(size: [u32; 2]) -> ([u32; 2], [f32; 2]) {
    let side = size[0].max(size[1]) as f32;
    let resized = size.map(|v| (v as f32 * 800. / side).round().max(1.) as u32);
    // DINO pred_boxes are normalized to the valid image extent supplied by pixel_mask,
    // unlike OWLv2's full square canvas. Undo resize on that valid extent.
    (resized, size.map(|v| v as f32))
}
fn dino_tensor(
    image: &VisionImage,
    model: &super::models::Model,
) -> Result<(Tensor<f32>, Tensor<i64>)> {
    let ([w, h], _) = dino_geometry(image.size());
    let resized =
        image::imageops::resize(&image.pixels, w, h, image::imageops::FilterType::Triangle);
    // Normalize valid pixels, then zero-pad bottom/right in normalized space.
    let mut data = vec![0.; 3 * 800 * 800];
    let mut mask = vec![0i64; 800 * 800];
    for y in 0..h {
        for x in 0..w {
            let i = y as usize * 800 + x as usize;
            let pixel = resized.get_pixel(x, y);
            mask[i] = 1;
            for c in 0..3 {
                data[c * 800 * 800 + i] = (f32::from(pixel[c]) * model.input.scale
                    - model.input.mean[c])
                    / model.input.std[c];
            }
        }
    }
    Ok((
        Tensor::from_array(([1usize, 3, 800, 800], data)).map_err(ort_error)?,
        Tensor::from_array(([1usize, 800, 800], mask)).map_err(ort_error)?,
    ))
}
impl Detector for TextDetector {
    fn detect(
        &mut self,
        image: &VisionImage,
        phrase: &str,
    ) -> Result<(Vec<Detection>, Provenance)> {
        if phrase.trim().is_empty() || phrase.len() > 4096 {
            return Err(VisionError::Invalid("text query length".into()));
        }
        let dino = self.graph.model.id == "grounding-dino-tiny";
        let query = if dino {
            let p = phrase.trim().to_lowercase();
            if p.ends_with('.') { p } else { format!("{p}.") }
        } else {
            phrase.trim().to_lowercase()
        };
        let encoding = self
            .tokenizer
            .encode(query, true)
            .map_err(|_| VisionError::Invalid("tokenizer encode".into()))?;
        let ids = encoding
            .get_ids()
            .iter()
            .map(|v| i64::from(*v))
            .collect::<Vec<_>>();
        let length = ids.len();
        let ids = Tensor::from_array(([1usize, length], ids)).map_err(ort_error)?;
        let attention = Tensor::from_array((
            [1usize, length],
            encoding
                .get_attention_mask()
                .iter()
                .map(|v| i64::from(*v))
                .collect::<Vec<_>>(),
        ))
        .map_err(ort_error)?;
        let (pixels, pixel_mask) = if dino {
            let (pixels, mask) = dino_tensor(image, &self.graph.model)?;
            (pixels, Some(mask))
        } else {
            (owl_tensor(image, &self.graph.model)?, None)
        };
        let mut inputs =
            ort::inputs!["pixel_values"=>pixels,"input_ids"=>ids,"attention_mask"=>attention];
        if dino {
            inputs.push((
                "token_type_ids".into(),
                Tensor::from_array((
                    [1usize, length],
                    encoding
                        .get_type_ids()
                        .iter()
                        .map(|v| i64::from(*v))
                        .collect::<Vec<_>>(),
                ))
                .map_err(ort_error)?
                .into(),
            ));
            inputs.push((
                "pixel_mask".into(),
                pixel_mask
                    .ok_or_else(|| VisionError::Invalid("DINO padding mask".into()))?
                    .into(),
            ));
        }
        let out = self
            .graph
            .run_named(inputs, &["logits", "pred_boxes"], 900 * 256 + 3600 * 4)?;
        let logits = output(&out, "logits")?;
        let boxes = output(&out, "pred_boxes")?;
        let count = if dino { 900 } else { 3600 };
        let columns = if dino { 256 } else { 1 };
        if logits.shape != [1, count as i64, columns as i64] || boxes.shape != [1, count as i64, 4]
        {
            return Err(VisionError::Invalid("text detector output shapes".into()));
        }
        let size = image.size();
        let side = size[0].max(size[1]) as f32;
        let scale = if dino {
            dino_geometry(size).1
        } else {
            [side, side]
        };
        let mut detections = vec![];
        for (i, b) in boxes.values.as_chunks::<4>().0.iter().enumerate() {
            let row = &logits.values[i * columns..(i + 1) * columns];
            // Restrict DINO logits to non-special tokens from this exact query, not padding.
            let score = if dino {
                row.iter()
                    .take(length)
                    .zip(encoding.get_special_tokens_mask())
                    .filter(|(_, special)| **special == 0)
                    .map(|(v, _)| sigmoid(*v))
                    .fold(0., f32::max)
            } else {
                sigmoid(row[0])
            };
            if score < if dino { 0.4 } else { 0.1 } {
                continue;
            }
            if let Some(bbox) = center_box(b, size, scale) {
                detections.push(Detection {
                    bbox,
                    score,
                    mask: None,
                });
            }
        }
        detections.sort_by(|a, b| b.score.total_cmp(&a.score));
        // Explicit deterministic NMS policy; independent of upstream example thresholds.
        let mut kept: Vec<Detection> = vec![];
        for d in detections {
            if kept.len() == 256 {
                break;
            }
            if kept.iter().all(|k| {
                let x = d.bbox.intersection(k.bbox);
                x / (d.bbox.width * d.bbox.height + k.bbox.width * k.bbox.height - x) <= 0.5
            }) {
                kept.push(d);
            }
        }
        let mut p = self.graph.provenance(image);
        p.resolution_handling = self.graph.model.input.preprocessing.clone()
            + "; detection thresholds DINO0.4 OWLv2 0.1; NMS0.5; whole query phrase";
        Ok((kept, p))
    }
}

/// EfficientSAM's verified split graphs; box corners are original-pixel prompts.
pub struct EfficientSam {
    encoder: OnnxModel,
    decoder: OnnxModel,
}
impl EfficientSam {
    /// Load both graphs and verify the complete artifact bundle.
    pub fn load(
        model: &super::models::Model,
        cache: &Path,
        library: &Path,
        download: bool,
    ) -> Result<Self> {
        if model.id != "efficientsam-ti" || model.input.adapter != "efficientsam-v1" {
            return Err(VisionError::Invalid("EfficientSAM contract".into()));
        }
        Ok(Self {
            encoder: OnnxModel::load_role(model, cache, library, download, "encoder")?,
            decoder: OnnxModel::load_role(model, cache, library, download, "decoder")?,
        })
    }
}
fn binary_runs(values: &[f32], size: [u32; 2]) -> Mask {
    let mut runs = vec![];
    let mut start = None;
    for (i, v) in values.iter().enumerate() {
        if *v > 0. {
            if start.is_none() {
                start = Some(i as u32);
            }
        } else if let Some(s) = start.take() {
            runs.push([s, i as u32 - s]);
        }
    }
    if let Some(s) = start {
        runs.push([s, values.len() as u32 - s]);
    }
    Mask { size, runs }
}
impl Segmenter for EfficientSam {
    fn segment(&mut self, image: &VisionImage, boxes: &[Rect]) -> Result<(Vec<Mask>, Provenance)> {
        let size = image.size();
        if boxes.len() > 256 {
            return Err(VisionError::Invalid("prompt count".into()));
        }
        for b in boxes {
            b.validate(size)?;
        }
        if boxes.is_empty() {
            return Ok((vec![], self.encoder.provenance(image)));
        }
        // Three candidates fit the bounded mask budget; avoid the full MAX_PIXELS allocation.
        if u64::from(size[0]) * u64::from(size[1]) > MAX_PIXELS / 4 {
            return Err(VisionError::Invalid(
                "EfficientSAM image exceeds candidate mask budget".into(),
            ));
        }
        let pixels = self.encoder.tensor(image)?;
        let out = self.encoder.run_named(
            ort::inputs!["batched_images"=>pixels],
            &["image_embeddings"],
            256 * 64 * 64,
        )?;
        let embedding = output(&out, "image_embeddings")?;
        if embedding.shape != [1, 256, 64, 64] {
            return Err(VisionError::Invalid("EfficientSAM embedding shape".into()));
        }
        let mut masks = vec![];
        for b in boxes {
            let coords = Tensor::from_array((
                [1usize, 1, 2, 2],
                vec![b.x, b.y, b.x + b.width, b.y + b.height],
            ))
            .map_err(ort_error)?;
            let labels =
                Tensor::from_array(([1usize, 1, 2], vec![2f32, 3f32])).map_err(ort_error)?;
            let emb = Tensor::from_array(([1usize, 256, 64, 64], embedding.values.clone()))
                .map_err(ort_error)?;
            let dims = Tensor::from_array(([2usize], vec![i64::from(size[1]), i64::from(size[0])]))
                .map_err(ort_error)?;
            let out=self.decoder.run_named(ort::inputs!["image_embeddings"=>emb,"batched_point_coords"=>coords,"batched_point_labels"=>labels,"orig_im_size"=>dims],&["output_masks","iou_predictions"],size[0] as usize*size[1] as usize*4+4)?;
            let values = output(&out, "output_masks")?;
            let scores = output(&out, "iou_predictions")?;
            if scores.shape.len() != 3
                || scores.shape[0..2] != [1, 1]
                || values.shape.len() != 5
                || values.shape[0..2] != [1, 1]
                || values.shape[2] != scores.shape[2]
                || values.shape[3..5] != [i64::from(size[1]), i64::from(size[0])]
                || scores.values.is_empty()
                || scores.values.len() > 4
            {
                return Err(VisionError::Invalid("EfficientSAM masks/IoU shapes".into()));
            }
            let best = scores
                .values
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(i, _)| i)
                .ok_or_else(|| VisionError::Invalid("mask selection".into()))?;
            let n = size[0] as usize * size[1] as usize;
            let mask = binary_runs(&values.values[best * n..(best + 1) * n], size);
            mask.validate(size)?;
            masks.push(mask);
        }
        Ok((masks, self.encoder.provenance(image)))
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn boxes_map_square_padding_to_original_and_clip() {
        let b = center_box(&[0.5, 0.25, 0.5, 0.5], [200, 100], [200., 200.]).unwrap();
        assert_eq!(
            b,
            Rect {
                x: 50.,
                y: 0.,
                width: 100.,
                height: 100.
            }
        );
        assert!(center_box(&[0.5, 0.9, 0.2, 0.1], [200, 100], [200., 200.]).is_none());
    }
    #[test]
    fn dino_padding_preserves_aspect_and_inverse_coordinates() {
        let (resized, scale) = dino_geometry([400, 200]);
        assert_eq!(resized, [800, 400]);
        assert_eq!(scale, [400., 200.]);
        let bbox = center_box(&[0.5, 0.5, 0.5, 1.0], [400, 200], scale).unwrap();
        assert_eq!(
            bbox,
            Rect {
                x: 100.,
                y: 0.,
                width: 200.,
                height: 200.
            }
        );
        assert!(center_box(&[0.5, 1.2, 0.1, 0.1], [400, 200], scale).is_none());
    }
    #[test]
    fn mask_threshold_is_positive_and_runs_preserve_original_pixels() {
        let m = binary_runs(&[-1., 0., 0.1, 1., -0.1, 2.], [3, 2]);
        assert_eq!(m.runs, vec![[2, 2], [5, 1]]);
        m.validate([3, 2]).unwrap();
    }
}
