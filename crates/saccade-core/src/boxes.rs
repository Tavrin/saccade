//! Bounding-box annotation interchange: COCO and YOLO export/import with explicit coordinates.
//!
//! The canonical document ([`BoxDoc`](crate::boxes::BoxDoc), `saccade-boxes.v1`) states its
//! coordinate convention instead of implying one: pixel units, top-left origin,
//! `x y w h` boxes in the coordinates of the **named image**. Anything else is
//! refused rather than guessed. One document describes one image.
//!
//! * COCO: `bbox` is `[x, y, w, h]` in pixels; categories are numbered from 1 in
//!   class order. Import accepts exactly one image and requires `bbox` on every
//!   annotation.
//! * YOLO: one text line per box, `class cx cy w h [score]`, centre and size
//!   normalised by the image size; class indices are positions in the class
//!   list (`classes.txt` on export). An empty file is a valid empty annotation.
//!   YOLO carries no image size, so import needs the size stated.
//! * A box outside the image is an error unless clipping is requested; clipping
//!   and dropping are counted in [`Adjustments`](crate::boxes::Adjustments), never silent.
//! * [`crop`](crate::boxes::crop) and [`resize`](crate::boxes::resize) re-express boxes for a cropped or resized image;
//!   the derivation is recorded and the source hash is not carried over.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Annotation document discriminator.
pub const SCHEMA: &str = "saccade-boxes.v1";
/// Operation receipt discriminator.
pub const RESULT_SCHEMA: &str = "saccade-boxes-result.v1";
/// Largest accepted number of boxes.
pub const MAX_BOXES: usize = 100_000;

fn err(msg: impl Into<String>) -> Error {
    Error::Config(msg.into())
}

/// Declared coordinate convention; only one value of each field is accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coordinates {
    /// Must be `pixel`.
    pub unit: String,
    /// Must be `top_left`.
    pub origin: String,
    /// Must be `xywh` (left, top, width, height).
    pub form: String,
}

impl Default for Coordinates {
    fn default() -> Self {
        Self {
            unit: "pixel".into(),
            origin: "top_left".into(),
            form: "xywh".into(),
        }
    }
}

/// The image the boxes refer to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRef {
    /// File name as recorded by the producer.
    pub file: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// SHA-256 (hex) of the image bytes, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// SHA-256 of the image these coordinates were derived from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from_sha256: Option<String>,
    /// Human-readable derivation, for example `crop x=10 y=4 w=100 h=80`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<String>,
}

/// One box in image pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxItem {
    /// Class name; must appear in [`BoxDoc::classes`].
    pub class: String,
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width, positive.
    pub w: f64,
    /// Height, positive.
    pub h: f64,
    /// Optional detector confidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

/// Canonical annotation document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxDoc {
    /// Version discriminator.
    pub schema: String,
    /// Explicit coordinate convention.
    pub coordinates: Coordinates,
    /// Image the coordinates refer to.
    pub image: ImageRef,
    /// Ordered class names; the order defines COCO and YOLO class numbers.
    pub classes: Vec<String>,
    /// Boxes, possibly none.
    pub boxes: Vec<BoxItem>,
}

/// Counts of changes made to satisfy a request; all zero means none were needed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Adjustments {
    /// Boxes clipped to the image or crop window.
    pub clipped: u64,
    /// Boxes removed because nothing remained inside.
    pub dropped: u64,
}

impl BoxDoc {
    /// Build an empty document for an image.
    pub fn new(image: ImageRef, classes: Vec<String>) -> Self {
        Self {
            schema: SCHEMA.into(),
            coordinates: Coordinates::default(),
            image,
            classes,
            boxes: Vec::new(),
        }
    }

    /// Check the schema, coordinate convention, classes and finite positive extents.
    ///
    /// Boxes extending beyond the image are an error here; see [`BoxDoc::fit`].
    pub fn validate(&self) -> Result<()> {
        self.validate_inner(false)
    }

    fn validate_inner(&self, allow_outside: bool) -> Result<()> {
        if self.schema != SCHEMA {
            return Err(err(format!("expected schema {SCHEMA}")));
        }
        if self.coordinates != Coordinates::default() {
            return Err(err(
                "coordinates must be unit=pixel, origin=top_left, form=xywh",
            ));
        }
        if self.image.width == 0 || self.image.height == 0 || self.image.file.is_empty() {
            return Err(err("image needs a file name and positive width and height"));
        }
        if self.boxes.len() > MAX_BOXES || self.classes.len() > MAX_BOXES {
            return Err(err("too many boxes or classes"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for c in &self.classes {
            if c.is_empty() || c.contains(['\n', '\r']) || !seen.insert(c) {
                return Err(err(format!(
                    "class names must be unique, non-empty, one line: {c:?}"
                )));
            }
        }
        let (iw, ih) = (f64::from(self.image.width), f64::from(self.image.height));
        for (i, b) in self.boxes.iter().enumerate() {
            let finite = [b.x, b.y, b.w, b.h].iter().all(|v| v.is_finite());
            if !finite || b.w <= 0.0 || b.h <= 0.0 {
                return Err(err(format!("box {i}: needs finite x, y and positive w, h")));
            }
            if b.score.is_some_and(|s| !s.is_finite()) {
                return Err(err(format!("box {i}: score must be finite")));
            }
            if !self.classes.contains(&b.class) {
                return Err(err(format!("box {i}: class {:?} is not declared", b.class)));
            }
            if !allow_outside && (b.x < 0.0 || b.y < 0.0 || b.x + b.w > iw || b.y + b.h > ih) {
                return Err(err(format!(
                    "box {i} lies outside the {}x{} image; fix it or request clipping",
                    self.image.width, self.image.height
                )));
            }
        }
        Ok(())
    }

    /// Return a valid copy, clipping boxes to the image when `clip` is set.
    ///
    /// Without `clip`, a box outside the image is an error. With it, boxes are
    /// intersected with the image and boxes with no area left are dropped; both
    /// are counted.
    pub fn fit(&self, clip: bool) -> Result<(BoxDoc, Adjustments)> {
        if !clip {
            self.validate()?;
            return Ok((self.clone(), Adjustments::default()));
        }
        self.validate_inner(true)?;
        let window = (
            0.0,
            0.0,
            f64::from(self.image.width),
            f64::from(self.image.height),
        );
        let (boxes, adj) = clip_boxes(&self.boxes, window);
        let mut out = self.clone();
        out.boxes = boxes;
        Ok((out, adj))
    }
}

fn clip_boxes(
    boxes: &[BoxItem],
    (wx, wy, ww, wh): (f64, f64, f64, f64),
) -> (Vec<BoxItem>, Adjustments) {
    let mut adj = Adjustments::default();
    let mut out = Vec::new();
    for b in boxes {
        let (x0, y0) = (b.x.max(wx), b.y.max(wy));
        let (x1, y1) = ((b.x + b.w).min(wx + ww), (b.y + b.h).min(wy + wh));
        if x1 <= x0 || y1 <= y0 {
            adj.dropped += 1;
            continue;
        }
        if (x0, y0, x1, y1) != (b.x, b.y, b.x + b.w, b.y + b.h) {
            adj.clipped += 1;
        }
        out.push(BoxItem {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
            ..b.clone()
        });
    }
    (out, adj)
}

/// Re-express boxes for the image cropped to `x, y, w, h` (a window inside the image).
///
/// Boxes crossing the window edge are clipped and counted; boxes outside are
/// dropped and counted. Output coordinates are relative to the crop's top-left.
pub fn crop(
    doc: &BoxDoc,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    file: Option<&str>,
) -> Result<(BoxDoc, Adjustments)> {
    doc.validate()?;
    if w == 0
        || h == 0
        || u64::from(x) + u64::from(w) > u64::from(doc.image.width)
        || u64::from(y) + u64::from(h) > u64::from(doc.image.height)
    {
        return Err(err("crop window must be non-empty and inside the image"));
    }
    let (ox, oy) = (f64::from(x), f64::from(y));
    let (mut boxes, adj) = clip_boxes(&doc.boxes, (ox, oy, f64::from(w), f64::from(h)));
    for b in &mut boxes {
        b.x -= ox;
        b.y -= oy;
    }
    let mut out = doc.clone();
    out.boxes = boxes;
    out.image = derived(doc, w, h, file, format!("crop x={x} y={y} w={w} h={h}"));
    Ok((out, adj))
}

/// Re-express boxes for the image resized to `w` x `h`, scaling each axis independently.
pub fn resize(doc: &BoxDoc, w: u32, h: u32, file: Option<&str>) -> Result<BoxDoc> {
    doc.validate()?;
    if w == 0 || h == 0 {
        return Err(err("resize dimensions must be positive"));
    }
    let (sx, sy) = (
        f64::from(w) / f64::from(doc.image.width),
        f64::from(h) / f64::from(doc.image.height),
    );
    let mut out = doc.clone();
    for b in &mut out.boxes {
        b.x *= sx;
        b.y *= sy;
        b.w *= sx;
        b.h *= sy;
    }
    out.image = derived(
        doc,
        w,
        h,
        file,
        format!("resize w={w} h={h} (independent axis scale)"),
    );
    Ok(out)
}

fn derived(doc: &BoxDoc, w: u32, h: u32, file: Option<&str>, how: String) -> ImageRef {
    ImageRef {
        file: file.unwrap_or(&doc.image.file).into(),
        width: w,
        height: h,
        sha256: None,
        derived_from_sha256: doc
            .image
            .sha256
            .clone()
            .or_else(|| doc.image.derived_from_sha256.clone()),
        derivation: Some(match &doc.image.derivation {
            Some(prev) => format!("{prev}; {how}"),
            None => how,
        }),
    }
}

/// Export as a COCO detection document (one image).
pub fn to_coco(doc: &BoxDoc) -> Result<Value> {
    doc.validate()?;
    let mut image = json!({"id":1,"file_name":doc.image.file,"width":doc.image.width,"height":doc.image.height});
    if let Some(h) = &doc.image.sha256 {
        image["saccade_source_sha256"] = h.clone().into();
    }
    let annotations: Vec<Value> = doc
        .boxes
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let id = doc.classes.iter().position(|c| *c == b.class).unwrap_or(0) + 1;
            let mut a = json!({"id":i + 1,"image_id":1,"category_id":id,"bbox":[b.x,b.y,b.w,b.h],"area":b.w * b.h,"iscrowd":0});
            if let Some(s) = b.score {
                a["score"] = s.into();
            }
            a
        })
        .collect();
    let categories: Vec<Value> = doc
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| json!({"id":i + 1,"name":c}))
        .collect();
    Ok(
        json!({"info":{"description":"saccade-boxes.v1 export"},"images":[image],"categories":categories,"annotations":annotations}),
    )
}

/// Import a COCO detection document holding exactly one image.
pub fn from_coco(value: &Value) -> Result<BoxDoc> {
    let images = value["images"]
        .as_array()
        .ok_or_else(|| err("COCO document needs images"))?;
    let [image] = images.as_slice() else {
        return Err(err(format!(
            "COCO import needs exactly one image, found {}; split the file first",
            images.len()
        )));
    };
    let dim = |k: &str| {
        image[k]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v > 0)
            .ok_or_else(|| err(format!("COCO image needs a positive integer {k}")))
    };
    let file = image["file_name"]
        .as_str()
        .ok_or_else(|| err("COCO image needs file_name"))?;
    let cats = value["categories"]
        .as_array()
        .ok_or_else(|| err("COCO document needs categories"))?;
    let mut ids = Vec::new();
    let mut classes = Vec::new();
    for c in cats {
        ids.push(
            c["id"]
                .as_i64()
                .ok_or_else(|| err("category needs an integer id"))?,
        );
        classes.push(
            c["name"]
                .as_str()
                .ok_or_else(|| err("category needs a name"))?
                .to_string(),
        );
    }
    let mut doc = BoxDoc::new(
        ImageRef {
            file: file.into(),
            width: dim("width")?,
            height: dim("height")?,
            sha256: image["saccade_source_sha256"].as_str().map(str::to_string),
            derived_from_sha256: None,
            derivation: None,
        },
        classes,
    );
    let image_id = &image["id"];
    for (i, a) in value["annotations"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .enumerate()
    {
        if a["image_id"] != *image_id {
            return Err(err(format!("annotation {i} refers to a different image")));
        }
        let cat = a["category_id"]
            .as_i64()
            .ok_or_else(|| err(format!("annotation {i}: needs category_id")))?;
        let class = ids
            .iter()
            .position(|c| *c == cat)
            .ok_or_else(|| err(format!("annotation {i}: unknown category_id {cat}")))?;
        let bbox: Vec<f64> = a["bbox"]
            .as_array()
            .filter(|v| v.len() == 4)
            .and_then(|v| v.iter().map(Value::as_f64).collect())
            .ok_or_else(|| {
                err(format!(
                    "annotation {i}: needs a four-number bbox (segmentation-only is not boxes)"
                ))
            })?;
        doc.boxes.push(BoxItem {
            class: doc.classes[class].clone(),
            x: bbox[0],
            y: bbox[1],
            w: bbox[2],
            h: bbox[3],
            score: a["score"].as_f64(),
        });
    }
    doc.validate()?;
    Ok(doc)
}

/// Export as YOLO text: the label file contents and the `classes.txt` contents.
pub fn to_yolo(doc: &BoxDoc) -> Result<(String, String)> {
    doc.validate()?;
    let (iw, ih) = (f64::from(doc.image.width), f64::from(doc.image.height));
    let mut labels = String::new();
    for b in &doc.boxes {
        let class = doc.classes.iter().position(|c| *c == b.class).unwrap_or(0);
        labels.push_str(&format!(
            "{class} {} {} {} {}",
            (b.x + b.w / 2.0) / iw,
            (b.y + b.h / 2.0) / ih,
            b.w / iw,
            b.h / ih
        ));
        if let Some(s) = b.score {
            labels.push_str(&format!(" {s}"));
        }
        labels.push('\n');
    }
    let mut classes = doc.classes.join("\n");
    if !classes.is_empty() {
        classes.push('\n');
    }
    Ok((labels, classes))
}

/// Import YOLO label text for an image of the stated size.
///
/// Values outside `[0, 1]` (beyond float noise) are refused.
pub fn from_yolo(text: &str, classes: Vec<String>, image: ImageRef) -> Result<BoxDoc> {
    let mut doc = BoxDoc::new(image, classes);
    let (iw, ih) = (f64::from(doc.image.width), f64::from(doc.image.height));
    for (n, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() != 5 && t.len() != 6 {
            return Err(err(format!("line {}: expected 5 or 6 fields", n + 1)));
        }
        let class = t[0]
            .parse::<usize>()
            .ok()
            .and_then(|i| doc.classes.get(i).cloned())
            .ok_or_else(|| {
                err(format!(
                    "line {}: class index outside the class list",
                    n + 1
                ))
            })?;
        let v: Vec<f64> = t[1..]
            .iter()
            .map(|s| s.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect::<Option<_>>()
            .ok_or_else(|| err(format!("line {}: numbers must be finite", n + 1)))?;
        let eps = 1e-9;
        let (cx, cy, w, h) = (v[0], v[1], v[2], v[3]);
        if w <= 0.0
            || h <= 0.0
            || cx - w / 2.0 < -eps
            || cy - h / 2.0 < -eps
            || cx + w / 2.0 > 1.0 + eps
            || cy + h / 2.0 > 1.0 + eps
        {
            return Err(err(format!(
                "line {}: box is not inside the normalised image",
                n + 1
            )));
        }
        let x = ((cx - w / 2.0) * iw).max(0.0);
        let y = ((cy - h / 2.0) * ih).max(0.0);
        doc.boxes.push(BoxItem {
            class,
            x,
            y,
            w: (w * iw).min(iw - x),
            h: (h * ih).min(ih - y),
            score: v.get(4).copied(),
        });
    }
    doc.validate()?;
    Ok(doc)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn doc() -> BoxDoc {
        let mut d = BoxDoc::new(
            ImageRef {
                file: "scene.png".into(),
                width: 200,
                height: 100,
                sha256: Some("ab".repeat(32)),
                derived_from_sha256: None,
                derivation: None,
            },
            vec!["a".into(), "b".into(), "unused".into()],
        );
        for (c, x, y, w, h, s) in [
            ("a", 10.0, 20.0, 50.0, 30.0, None),
            ("b", 150.0, 60.0, 50.0, 40.0, Some(0.5)),
        ] {
            d.boxes.push(BoxItem {
                class: c.into(),
                x,
                y,
                w,
                h,
                score: s,
            });
        }
        d
    }

    fn close(a: &BoxDoc, b: &BoxDoc) {
        assert_eq!((&a.classes, a.boxes.len()), (&b.classes, b.boxes.len()));
        for (p, q) in a.boxes.iter().zip(&b.boxes) {
            assert_eq!((&p.class, p.score), (&q.class, q.score));
            for (u, v) in [(p.x, q.x), (p.y, q.y), (p.w, q.w), (p.h, q.h)] {
                assert!((u - v).abs() < 1e-9, "{u} vs {v}");
            }
        }
    }

    fn yolo_round(d: &BoxDoc) -> BoxDoc {
        let (labels, classes) = to_yolo(d).unwrap();
        let names = classes.lines().map(str::to_string).collect();
        from_yolo(&labels, names, d.image.clone()).unwrap()
    }

    #[test]
    fn coco_and_yolo_round_trip_including_empty_annotations() {
        let d = doc();
        close(&from_coco(&to_coco(&d).unwrap()).unwrap(), &d);
        close(&yolo_round(&d), &d);
        let mut empty = d;
        empty.boxes.clear();
        assert_eq!(to_yolo(&empty).unwrap().0, "");
        close(&from_coco(&to_coco(&empty).unwrap()).unwrap(), &empty);
        close(&yolo_round(&empty), &empty);
    }

    #[test]
    fn crop_and_resize_re_express_boxes_and_round_trip_through_both_formats() {
        let (cropped, adj) = crop(&doc(), 40, 10, 100, 60, Some("crop.png")).unwrap();
        // First box is clipped on its left edge by the window; the second lies outside.
        assert_eq!(
            adj,
            Adjustments {
                clipped: 1,
                dropped: 1
            }
        );
        assert_eq!(
            (cropped.boxes[0].x, cropped.boxes[0].y, cropped.boxes[0].w),
            (0.0, 10.0, 20.0)
        );
        assert!(cropped.image.sha256.is_none() && cropped.image.derived_from_sha256.is_some());
        close(&from_coco(&to_coco(&cropped).unwrap()).unwrap(), &cropped);
        close(&yolo_round(&cropped), &cropped);
        let small = resize(&doc(), 100, 200, None).unwrap();
        assert_eq!(
            (small.boxes[0].x, small.boxes[0].y, small.boxes[0].h),
            (5.0, 40.0, 60.0)
        );
        close(&yolo_round(&small), &small);
        // Normalised YOLO text is identical for the original and resized image.
        assert_eq!(to_yolo(&small).unwrap().0, to_yolo(&doc()).unwrap().0);
        assert!(crop(&doc(), 150, 0, 100, 10, None).is_err());
    }

    #[test]
    fn outside_boxes_are_refused_unless_clipping_is_requested_and_counted() {
        let mut d = doc();
        d.boxes[1].w = 80.0;
        assert!(to_coco(&d).is_err() && to_yolo(&d).is_err() && d.fit(false).is_err());
        let (fit, adj) = d.fit(true).unwrap();
        assert_eq!(
            adj,
            Adjustments {
                clipped: 1,
                dropped: 0
            }
        );
        assert_eq!(fit.boxes[1].w, 50.0);
        d.boxes[1].x = 300.0;
        assert_eq!(
            d.fit(true).unwrap().1,
            Adjustments {
                clipped: 0,
                dropped: 1
            }
        );
        let mut bad = doc();
        bad.coordinates.origin = "bottom_left".into();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn imports_refuse_ambiguity() {
        let names = vec!["a".to_string()];
        let image = doc().image;
        assert!(from_yolo("1 0.5 0.5 0.1 0.1", names.clone(), image.clone()).is_err());
        assert!(from_yolo("0 0.99 0.5 0.1 0.1", names, image).is_err());
        let mut coco = to_coco(&doc()).unwrap();
        coco["images"].as_array_mut().unwrap().push(json!({"id":2}));
        assert!(from_coco(&coco).is_err());
    }
}
