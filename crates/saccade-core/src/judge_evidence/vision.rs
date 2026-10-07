//! Bounded anonymous pixels. Source paths, intent and model proposals stay local.
use super::*;
use crate::evidence::case::ArtifactRef;
use crate::evidence::human::PresentationMap;
use serde::Deserialize;

/// Maximum long edge of each full-frame view.
pub const FULL_FRAME_EDGE: u32 = 1536;
/// Maximum number of native-resolution region pairs.
pub const MAX_ROIS: usize = 3;
/// Fixed observation extraction rubric, independent of a routing proposal.
pub const OBSERVATION_RUBRIC: &str = "visual-observation/1";
/// Fixed blind preference rubric; no implementation or performance claims.
pub const PREFERENCE_RUBRIC: &str = "blind-preference/1";

/// Visual task; intent matching remains a separate structured question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionTask {
    /// Describe visible changes with region references and uncertainty.
    Observations,
    /// Choose an anonymous image, tie, or abstain.
    BlindPreference,
}
/// Exact display conversion, separate from measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DisplayTransform {
    /// Native LDR samples interpreted as sRGB and flattened over this gray value.
    Srgb {
        /// Opaque compositing background.
        background: u8,
    },
    /// Existing HDR display conversion, at exposure zero into quantized sRGB.
    Hdr {
        /// Version of the display conversion.
        version: String,
        /// ACES, Hable or Reinhard.
        tonemapper: String,
        /// Exposure in stops; the existing display encoder uses zero.
        exposure_stops: f64,
    },
}
/// Verified display pixels and their source content identity; never sent as metadata.
pub struct DisplayImage {
    input_id: String,
    source: Digest,
    transform: DisplayTransform,
    pixels: RgbImage,
}
impl DisplayImage {
    /// Reads a canonical local path, hashing the source and applying the recorded
    /// display conversion. No path or filename is included in the visual payload.
    pub fn load(input_id: &str, path: &Path, transform: DisplayTransform) -> EvidenceResult<Self> {
        let route = path;
        let path = crate::paths::canonicalize(route)?;
        let before = crate::root_policy::io::read(route)?;
        let pixels = match &transform {
            DisplayTransform::Srgb { background } => {
                require(
                    !crate::hdr::is_hdr_path(&path),
                    "HDR source needs an HDR display transform",
                )?;
                let img = image::load_from_memory(&before).map_err(|e| {
                    crate::evidence::ContractError::Invalid(format!("display decode: {e}"))
                })?;
                crate::compare::flatten_over(&img.to_rgba8(), *background)
            }
            DisplayTransform::Hdr {
                version,
                tonemapper,
                exposure_stops,
            } => {
                require(
                    crate::hdr::is_hdr_path(&path)
                        && version == "hdr-display/1"
                        && *exposure_stops == 0.0,
                    "unsupported HDR display transform",
                )?;
                let tm = crate::hdr::Tonemapper::parse(tonemapper)
                    .map_err(|e| crate::evidence::ContractError::Invalid(e.to_string()))?;
                require(
                    tm.name() == tonemapper,
                    "tone mapper must use its canonical name",
                )?;
                let decoded = image::load_from_memory(&before)
                    .map_err(|e| crate::evidence::ContractError::Invalid(e.to_string()))?;
                let hdr = crate::hdr::from_decoded(decoded);
                crate::hdr::display_image(&hdr, tm)
            }
        };
        require(
            before == crate::root_policy::io::read(route)?,
            "source changed during display encoding",
        )?;
        Ok(Self {
            input_id: input_id.into(),
            source: Digest::of_bytes(&before),
            transform,
            pixels,
        })
    }
}
/// Explicit region selection; native coordinates are validated without resizing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionRoi {
    /// Anonymous region ID, without project semantics or filenames.
    pub region_id: String,
    /// Native x, y, width and height.
    pub rect: [u32; 4],
    /// Add a separately labeled contrast-enhanced pair when useful.
    pub enhance: bool,
}
/// One anonymous display view and its exact transformation metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionView {
    /// Citation ID, for example full-frame-P1 or crop-r1-raw-P2.
    pub id: String,
    /// P1 or P2.
    pub slot: String,
    /// full_frame or r1 through r3.
    pub region_id: String,
    /// full_frame, raw_crop or enhanced_crop.
    pub kind: String,
    /// Native source rectangle, before full-frame downsampling.
    pub rect: [u32; 4],
    /// Display PNG dimensions.
    pub dimensions: [u32; 2],
    /// Recorded source display conversion.
    pub display_transform: DisplayTransform,
    /// Shared gain applied only to enhanced crops; raw display gain is one.
    pub enhancement_gain: f32,
    /// Digest of the actual attachment.
    pub png_sha256: Digest,
    /// Exact PNG bytes; adapter serializes them as inline image data.
    #[serde(skip)]
    pub png: Vec<u8>,
}
/// Only this bounded, anonymous packet is visible to the extractor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionPayload {
    /// Closed visual task.
    pub task: VisionTask,
    /// Fixed rubric identity.
    pub rubric_version: String,
    /// Full frames followed by at most three raw and optional enhanced ROI pairs.
    pub views: Vec<VisionView>,
}
/// Private coordinator binding; never embed this in an anonymous provider body.
#[derive(Debug, Clone)]
pub struct VisionPresentation {
    /// Exact source case.
    pub case_id: Digest,
    /// Local entry, retained only for mapping and canonical scope.
    pub entry_id: String,
    /// Anonymous visual packet.
    pub payload: VisionPayload,
    /// Private source-to-slot mapping.
    pub mapping: PresentationMap,
    /// Ordered source input IDs and content hashes.
    pub sources: [(String, Digest); 2],
    /// Source roots retained for coordinator egress checks after relocation.
    pub source_roots: Vec<String>,
}
impl VisionPresentation {
    /// Binds actual attachments, transforms, sources and order, excluding paths.
    pub fn identity(&self) -> EvidenceResult<Digest> {
        canonical::digest(&json!({"case":self.case_id,"entry":self.entry_id,
            "sources":self.sources,"mapping":self.mapping.entries,"payload":self.payload,
            "source_roots":self.source_roots}))
    }
    /// Rejects tampering or a different source case before extraction or enrichment.
    pub fn validate_for(&self, case: &EvidenceCase) -> EvidenceResult<()> {
        case.validate()?;
        self.mapping.validate()?;
        require(
            self.case_id == case.case_id
                && self.mapping.case_id == case.case_id
                && self.mapping.presentation_identity == self.identity()?
                && case.scope.entries.contains(&self.entry_id),
            "stale visual presentation",
        )?;
        let expected_roots = roots(case);
        require(
            self.source_roots == expected_roots,
            "visual provenance roots differ from case",
        )?;
        require(
            self.sources[0].0 != self.sources[1].0,
            "visual pair needs distinct inputs",
        )?;
        for (id, hash) in &self.sources {
            require(
                case.inputs
                    .iter()
                    .any(|i| &i.id == id && &i.content.sha256 == hash),
                "visual source differs from case input",
            )?;
        }
        let slots = self.mapping.entries.get(&self.entry_id).ok_or_else(|| {
            crate::evidence::ContractError::Invalid("missing visual slot map".into())
        })?;
        require(
            self.mapping.entries.len() == 1
                && slots.len() == 2
                && slots.get("P1") == Some(&self.sources[0].0)
                && slots.get("P2") == Some(&self.sources[1].0),
            "invalid visual slot map",
        )?;
        require(
            self.payload.rubric_version == rubric(self.payload.task)
                && (2..=14).contains(&self.payload.views.len()),
            "invalid visual rubric or view count",
        )?;
        crate::evidence::case::unique(self.payload.views.iter().map(|v| v.id.as_str()))?;
        let full: Vec<_> = self
            .payload
            .views
            .iter()
            .filter(|v| v.kind == "full_frame")
            .collect();
        require(
            full.len() == 2
                && full[0].slot == "P1"
                && full[1].slot == "P2"
                && full.iter().all(|v| {
                    v.region_id == "full_frame"
                        && v.rect[0] == 0
                        && v.rect[1] == 0
                        && v.rect[2] > 0
                        && v.rect[3] > 0
                        && v.dimensions[0].max(v.dimensions[1]) <= FULL_FRAME_EDGE
                        && v.enhancement_gain == 1.0
                })
                && full[0].rect == full[1].rect,
            "invalid full-frame context",
        )?;
        for (id, _) in &self.sources {
            if let Some(native) = case
                .inputs
                .iter()
                .find(|i| &i.id == id)
                .and_then(|i| i.native_samples.value())
            {
                require(
                    [native.width, native.height] == [full[0].rect[2], full[0].rect[3]],
                    "visual dimensions differ from native source",
                )?;
            }
        }
        let regions: std::collections::BTreeSet<_> = self
            .payload
            .views
            .iter()
            .filter(|v| v.kind != "full_frame")
            .map(|v| v.region_id.as_str())
            .collect();
        require(
            regions.len() <= MAX_ROIS && regions.iter().all(|r| ["r1", "r2", "r3"].contains(r)),
            "invalid ROI identities",
        )?;
        for v in &self.payload.views {
            require(
                !v.png.is_empty() && Digest::of_bytes(&v.png) == v.png_sha256,
                "visual attachment differs from recorded content",
            )?;
            require(
                ["P1", "P2"].contains(&v.slot.as_str())
                    && v.dimensions.iter().all(|d| *d > 0)
                    && v.enhancement_gain.is_finite()
                    && v.enhancement_gain >= 1.0,
                "invalid visual slot, dimensions or gain",
            )?;
            let img = image::load_from_memory(&v.png).map_err(|e| {
                crate::evidence::ContractError::Invalid(format!("invalid PNG attachment: {e}"))
            })?;
            require(
                [img.width(), img.height()] == v.dimensions,
                "PNG dimensions differ from metadata",
            )?;
            let raw = v.kind == "raw_crop";
            if v.kind != "full_frame" {
                require(raw || v.kind == "enhanced_crop", "unknown visual view kind")?;
                require(
                    v.rect[2] > 0
                        && v.rect[3] > 0
                        && v.dimensions == [v.rect[2], v.rect[3]]
                        && v.rect[0]
                            .checked_add(v.rect[2])
                            .is_some_and(|x| x <= full[0].rect[2])
                        && v.rect[1]
                            .checked_add(v.rect[3])
                            .is_some_and(|y| y <= full[0].rect[3])
                        && (if raw {
                            v.enhancement_gain == 1.0
                        } else {
                            v.enhancement_gain > 1.0
                        }),
                    "crop must retain native resolution and recorded gain",
                )?;
                require(
                    self.payload.views.iter().any(|other| {
                        other.kind == "raw_crop"
                            && other.slot == v.slot
                            && other.region_id == v.region_id
                            && other.rect == v.rect
                    }),
                    "enhancement requires a separate raw crop",
                )?;
                require(
                    self.payload.views.iter().any(|other| {
                        other.kind == v.kind
                            && other.slot != v.slot
                            && other.region_id == v.region_id
                            && other.rect == v.rect
                            && other.enhancement_gain == v.enhancement_gain
                    }),
                    "ROI requires a matched anonymous pair",
                )?;
            }
        }
        Ok(())
    }
}
fn roots(case: &EvidenceCase) -> Vec<String> {
    case.provenance
        .source_roots
        .iter()
        .chain(case.inputs.iter().flat_map(|i| &i.provenance.source_roots))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn rubric(task: VisionTask) -> &'static str {
    match task {
        VisionTask::Observations => OBSERVATION_RUBRIC,
        VisionTask::BlindPreference => PREFERENCE_RUBRIC,
    }
}
fn full_frame(img: &RgbImage) -> RgbImage {
    let edge = img.width().max(img.height());
    if edge <= FULL_FRAME_EDGE {
        return img.clone();
    }
    let w = (u64::from(img.width()) * u64::from(FULL_FRAME_EDGE) / u64::from(edge)).max(1) as u32;
    let h = (u64::from(img.height()) * u64::from(FULL_FRAME_EDGE) / u64::from(edge)).max(1) as u32;
    imageops::resize(img, w, h, imageops::FilterType::Lanczos3)
}
fn native_crop(img: &RgbImage, rect: [u32; 4], gain: f32) -> RgbImage {
    let mut crop = imageops::crop_imm(img, rect[0], rect[1], rect[2], rect[3]).to_image();
    if gain != 1.0 {
        for pixel in crop.pixels_mut() {
            for channel in &mut pixel.0 {
                *channel = (f32::from(*channel) * gain).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    crop
}
fn view(
    img: RgbImage,
    source: &DisplayImage,
    slot: &str,
    region: &str,
    kind: &str,
    rect: [u32; 4],
    gain: f32,
) -> EvidenceResult<VisionView> {
    let png = png_bytes(&img);
    require(!png.is_empty(), "PNG encoding failed")?;
    let id = if kind == "full_frame" {
        format!("full-frame-{slot}")
    } else {
        format!(
            "crop-{region}-{}-{slot}",
            if kind == "raw_crop" {
                "raw"
            } else {
                "enhanced"
            }
        )
    };
    Ok(VisionView {
        id,
        slot: slot.into(),
        region_id: region.into(),
        kind: kind.into(),
        rect,
        dimensions: [img.width(), img.height()],
        display_transform: source.transform.clone(),
        enhancement_gain: gain,
        png_sha256: Digest::of_bytes(&png),
        png,
    })
}
/// Builds full-frame context even without hotspots. Both orders use identical
/// region selection and pair-shared enhancement; only slot assignment changes.
pub fn prepare_visual(
    case: &EvidenceCase,
    entry_id: &str,
    images: [&DisplayImage; 2],
    rois: &[VisionRoi],
    task: VisionTask,
    swap: bool,
) -> EvidenceResult<VisionPresentation> {
    case.validate()?;
    require(
        rois.len() <= MAX_ROIS,
        "at most three native ROI pairs are supported",
    )?;
    let (w, h) = images[0].pixels.dimensions();
    require(
        w > 0 && h > 0 && images[1].pixels.dimensions() == (w, h),
        "invalid visual pair dimensions",
    )?;
    for (idx, roi) in rois.iter().enumerate() {
        require(
            roi.region_id == format!("r{}", idx + 1),
            "ROI IDs must be anonymous r1 through r3",
        )?;
        let [x, y, rw, rh] = roi.rect;
        require(
            rw > 0
                && rh > 0
                && x.checked_add(rw).is_some_and(|v| v <= w)
                && y.checked_add(rh).is_some_and(|v| v <= h),
            "ROI is outside native image bounds",
        )?;
    }
    let images = if swap { [images[1], images[0]] } else { images };
    let mut views = Vec::new();
    for (image, slot) in images.into_iter().zip(["P1", "P2"]) {
        views.push(view(
            full_frame(&image.pixels),
            image,
            slot,
            "full_frame",
            "full_frame",
            [0, 0, w, h],
            1.0,
        )?);
    }
    for roi in rois {
        for (image, slot) in images.into_iter().zip(["P1", "P2"]) {
            views.push(view(
                native_crop(&image.pixels, roi.rect, 1.0),
                image,
                slot,
                &roi.region_id,
                "raw_crop",
                roi.rect,
                1.0,
            )?);
        }
        if roi.enhance {
            let gain = stretch_gain(&images[0].pixels, &images[1].pixels, roi.rect);
            if gain > 1.0 {
                for (image, slot) in images.into_iter().zip(["P1", "P2"]) {
                    views.push(view(
                        native_crop(&image.pixels, roi.rect, gain),
                        image,
                        slot,
                        &roi.region_id,
                        "enhanced_crop",
                        roi.rect,
                        gain,
                    )?);
                }
            }
        }
    }
    let mut p = VisionPresentation {
        case_id: case.case_id.clone(),
        entry_id: entry_id.into(),
        payload: VisionPayload {
            task,
            rubric_version: rubric(task).into(),
            views,
        },
        mapping: PresentationMap {
            case_id: case.case_id.clone(),
            presentation_identity: Digest::of_bytes(b""),
            entries: BTreeMap::from([(
                entry_id.into(),
                BTreeMap::from([
                    ("P1".into(), images[0].input_id.clone()),
                    ("P2".into(), images[1].input_id.clone()),
                ]),
            )]),
            unblinding_ref: None,
        },
        sources: images.map(|i| (i.input_id.clone(), i.source.clone())),
        source_roots: roots(case),
    };
    p.mapping.presentation_identity = p.identity()?;
    p.validate_for(case)?;
    Ok(p)
}

/// Fixed visibility categories; uncertainty remains distinct from clear evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Clearly visible at the recorded display transform.
    Clear,
    /// Visible but weak.
    Subtle,
    /// Pixels do not establish the proposed visual fact.
    Unclear,
}
/// Untrusted extraction body, without provider identity or source authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualObservation {
    /// full_frame or one of the supplied anonymous regions.
    pub region_id: String,
    /// Visual feature, for example shadow_boundary.
    pub observation: String,
    /// Directional change from the first to the second anonymous slot.
    pub change: String,
    /// Visibility under the recorded display conversion.
    pub visibility: Visibility,
    /// Exact attachment IDs inspected.
    pub evidence_refs: Vec<String>,
    /// Remaining uncertainty; data, never executable instructions.
    pub uncertainty: String,
}
/// Converts a completed, validated extraction to canonical attributed facts.
pub(crate) fn observation_facts(
    p: &VisionPresentation,
    observations: &[VisualObservation],
    response: &ArtifactRef,
) -> EvidenceResult<Vec<Fact>> {
    observations
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let mut value = serde_json::to_value(o)?;
            let slots = &p.mapping.entries[&p.entry_id];
            value["direction"] = json!({"from_input":slots["P1"],"to_input":slots["P2"]});
            value["presentation_identity"] = json!(p.mapping.presentation_identity);
            value["source_hashes"] = json!(p.sources);
            Ok(Fact {
                id: format!("observation-{}-{}", &response.sha256.as_str()[7..19], i + 1),
                name: o.observation.clone(),
                units: "visual_observation".into(),
                scope: crate::evidence::case::Scope {
                    entries: vec![p.entry_id.clone()],
                    exclusions: vec![],
                },
                source: FactSource::ModelObservation,
                artifact: response.clone(),
                source_identity: response.sha256.clone(),
                value: available(FactValue::Text(
                    String::from_utf8(canonical::bytes(&value)?).map_err(|_| {
                        crate::evidence::ContractError::Invalid("non-UTF8 observation".into())
                    })?,
                )),
                depends_on_model_observation: false,
                observation_refs: vec![],
            })
        })
        .collect()
}
