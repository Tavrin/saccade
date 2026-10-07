//! Unicode text observations, geometric line/word correspondence and CER/WER.
//! Extracted strings are inert data; matching never interprets their contents.
use crate::{Error, Result, ui_review::Source};
use serde::{Deserialize, Serialize};
/// Text comparison evidence schema.
pub const SCHEMA: &str = "saccade-text.v1";
/// Observed text unit with a pixel box.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    /// Exact Unicode content, no case folding or accent removal.
    pub text: String,
    /// Pixel coordinates x,y,width,height.
    pub bounds: [f64; 4],
    /// Lowest constituent OCR confidence (not a probability).
    pub confidence: Option<f64>,
}
/// Position/content correspondence, not authoritative producer identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    /// changed, missing, added or moved.
    pub kind: String,
    /// Exact before observation.
    pub before: Option<Unit>,
    /// Exact after observation.
    pub after: Option<Unit>,
    /// Explicit basis for heuristic correspondence.
    pub basis: String,
}
/// Text error-rate measurement retains raw edit counts and denominators.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rates {
    /// Unicode scalar edit count (Latin accents are retained).
    pub character_edits: usize,
    /// Reference Unicode scalar count.
    pub reference_characters: usize,
    /// Character error rate, None for an empty reference (never silently zero).
    pub cer: Option<f64>,
    /// Whitespace-token edit count.
    pub word_edits: usize,
    /// Reference token count.
    pub reference_words: usize,
    /// Word error rate, None for an empty reference.
    pub wer: Option<f64>,
}
/// String-presence and readability observation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Expected {
    /// Requested literal string.
    pub text: String,
    /// Exact observed substring in geometric reading order.
    pub present: bool,
    /// Whether every participating OCR word meets the declared confidence cutoff.
    /// None for source text lacking OCR confidence; never a human readability guarantee.
    pub readable: Option<bool>,
}
/// Independently grouped words and lines plus heuristic change evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comparison {
    /// Word correspondence changes.
    pub words: Vec<Change>,
    /// Line correspondence changes.
    pub lines: Vec<Change>,
    /// CER/WER from geometric reading order (moves can change order).
    pub rates: Rates,
    /// Expected-string observations on the candidate.
    pub expected: Vec<Expected>,
}
fn edit<T: Eq>(a: &[T], b: &[T]) -> Result<usize> {
    if a.len().saturating_mul(b.len()) > 16 * 1024 * 1024 {
        return Err(Error::Config("text edit matrix work limit exceeded".into()));
    }
    let mut row: Vec<_> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j] + 1)
                .min(old + 1)
                .min(diagonal + usize::from(x != y));
            diagonal = old;
        }
    }
    Ok(row[b.len()])
}
type GroupedUnits = (Vec<Unit>, Vec<Unit>, Vec<Vec<Unit>>);
fn units(source: &Source) -> Result<GroupedUnits> {
    if source.nodes.len() > 2048 {
        return Err(Error::Config(
            "text supports <=2048 observations per image".into(),
        ));
    }
    let mut words = Vec::new();
    let mut count = 0;
    for n in &source.nodes {
        if n.text.trim().is_empty() {
            continue;
        }
        count += n.text.chars().count();
        if count > 4096 {
            return Err(Error::Config(
                "text supports <=4096 Unicode scalars per image".into(),
            ));
        }
        let bounds = n
            .bounds
            .ok_or_else(|| Error::Config("text comparison requires observed boxes".into()))?;
        if bounds[0] < 0.
            || bounds[1] < 0.
            || bounds[2] <= 0.
            || bounds[3] <= 0.
            || bounds[0] + bounds[2] > f64::from(source.dimensions[0])
            || bounds[1] + bounds[3] > f64::from(source.dimensions[1])
        {
            return Err(Error::Config("text box outside image or empty".into()));
        }
        words.push(Unit {
            text: n.text.clone(),
            bounds,
            confidence: n.ocr_confidence,
        });
    }
    words.sort_by(|a, b| {
        a.bounds[1]
            .total_cmp(&b.bounds[1])
            .then(a.bounds[0].total_cmp(&b.bounds[0]))
    });
    let mut grouped: Vec<Vec<Unit>> = Vec::new();
    for word in &words {
        let centre = word.bounds[1] + word.bounds[3] / 2.;
        let slot = grouped.iter().position(|line| {
            let b = &line[0].bounds;
            (centre - (b[1] + b[3] / 2.)).abs() <= word.bounds[3].min(b[3]) * 0.5
        });
        if let Some(slot) = slot {
            grouped[slot].push(word.clone());
        } else {
            grouped.push(vec![word.clone()]);
        }
    }
    let mut lines = Vec::new();
    let mut ordered = Vec::new();
    let mut groups = Vec::new();
    for mut line in grouped {
        line.sort_by(|a, b| a.bounds[0].total_cmp(&b.bounds[0]));
        let mut bounds = line[0].bounds;
        let x1 = line
            .iter()
            .map(|u| u.bounds[0] + u.bounds[2])
            .fold(0., f64::max);
        let y1 = line
            .iter()
            .map(|u| u.bounds[1] + u.bounds[3])
            .fold(0., f64::max);
        bounds[0] = line
            .iter()
            .map(|u| u.bounds[0])
            .fold(f64::INFINITY, f64::min);
        bounds[1] = line
            .iter()
            .map(|u| u.bounds[1])
            .fold(f64::INFINITY, f64::min);
        bounds[2] = x1 - bounds[0];
        bounds[3] = y1 - bounds[1];
        let confidence = if line.iter().all(|u| u.confidence.is_some()) {
            line.iter()
                .filter_map(|u| u.confidence)
                .min_by(f64::total_cmp)
        } else {
            None
        };
        lines.push(Unit {
            text: line
                .iter()
                .map(|u| u.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            bounds,
            confidence,
        });
        groups.push(line.clone());
        ordered.extend(line);
    }
    Ok((ordered, lines, groups))
}
fn position(unit: &Unit, size: [u32; 2]) -> [f64; 2] {
    [
        (unit.bounds[0] + unit.bounds[2] * 0.5) / f64::from(size[0]),
        (unit.bounds[1] + unit.bounds[3] * 0.5) / f64::from(size[1]),
    ]
}
fn delta(a: &Unit, b: &Unit, asize: [u32; 2], bsize: [u32; 2]) -> f64 {
    let a = position(a, asize);
    let b = position(b, bsize);
    ((a[0] - b[0]) * f64::from(asize[0])).hypot((a[1] - b[1]) * f64::from(asize[1]))
}
fn changes(a: &[Unit], b: &[Unit], asize: [u32; 2], bsize: [u32; 2], moved_px: f64) -> Vec<Change> {
    let mut used = vec![false; b.len()];
    let mut matched = vec![false; a.len()];
    let mut out = Vec::new();
    for (i, aa) in a.iter().enumerate() {
        let found = b
            .iter()
            .enumerate()
            .filter(|(j, bb)| !used[*j] && aa.text == bb.text)
            .min_by(|(j, x), (k, y)| {
                delta(aa, x, asize, bsize)
                    .total_cmp(&delta(aa, y, asize, bsize))
                    .then(j.cmp(k))
            });
        if let Some((j, bb)) = found {
            used[j] = true;
            matched[i] = true;
            if delta(aa, bb, asize, bsize) > moved_px {
                out.push(Change {
                    kind: "moved".into(),
                    before: Some(aa.clone()),
                    after: Some(bb.clone()),
                    basis: "exact text; nearest normalized box centre".into(),
                });
            }
        }
    }
    // Nearest geometry among remaining units; exact content took priority above.
    for (i, aa) in a.iter().enumerate() {
        if matched[i] {
            continue;
        }
        let found = b
            .iter()
            .enumerate()
            .filter(|(j, _)| !used[*j])
            .min_by(|(j, x), (k, y)| {
                delta(aa, x, asize, bsize)
                    .total_cmp(&delta(aa, y, asize, bsize))
                    .then(j.cmp(k))
            });
        if let Some((j, bb)) = found
            && delta(aa, bb, asize, bsize) <= aa.bounds[3].max(8.)
        {
            used[j] = true;
            matched[i] = true;
            out.push(Change {
                kind: "changed".into(),
                before: Some(aa.clone()),
                after: Some(bb.clone()),
                basis: "nearby unmatched normalized box centre; OCR correspondence is heuristic"
                    .into(),
            });
        }
        if !matched[i] {
            out.push(Change {
                kind: "missing".into(),
                before: Some(aa.clone()),
                after: None,
                basis: "not observed in candidate; incomplete OCR cannot prove removal".into(),
            });
        }
    }
    for (j, bb) in b.iter().enumerate() {
        if !used[j] {
            out.push(Change {
                kind: "added".into(),
                before: None,
                after: Some(bb.clone()),
                basis: "unmatched candidate observation".into(),
            });
        }
    }
    out
}
/// Exact Unicode CER/WER for plain text or page-structured document observations.
pub fn rates(aa: &str, bb: &str) -> Result<Rates> {
    if aa.chars().count() > 16384 || bb.chars().count() > 16384 {
        return Err(Error::Config("text rate scalar bound".into()));
    }
    let ac: Vec<_> = aa.chars().collect();
    let bc: Vec<_> = bb.chars().collect();
    let at: Vec<_> = aa.split_whitespace().collect();
    let bt: Vec<_> = bb.split_whitespace().collect();
    let ce = edit(&ac, &bc)?;
    let we = edit(&at, &bt)?;
    Ok(Rates {
        character_edits: ce,
        reference_characters: ac.len(),
        cer: (!ac.is_empty()).then(|| ce as f64 / ac.len() as f64),
        word_edits: we,
        reference_words: at.len(),
        wer: (!at.is_empty()).then(|| we as f64 / at.len() as f64),
    })
}
/// Compares image-bound source/OCR observations with literal Unicode matching.
/// No extracted content is used as an instruction. Geometric reading order is heuristic.
pub fn compare(
    a: &Source,
    b: &Source,
    expected: &[String],
    readable_confidence: f64,
    moved_px: f64,
) -> Result<Comparison> {
    a.validate(&a.capture_sha256, a.dimensions)?;
    b.validate(&b.capture_sha256, b.dimensions)?;
    if !readable_confidence.is_finite()
        || !(0.0..=100.0).contains(&readable_confidence)
        || !moved_px.is_finite()
        || moved_px < 0.
        || expected.len() > 64
        || expected.iter().any(|s| s.is_empty() || s.len() > 4096)
    {
        return Err(Error::Config(
            "invalid text expectation or movement policy".into(),
        ));
    }
    let (aw, al, _) = units(a)?;
    let (bw, bl, groups) = units(b)?;
    let aa = al
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let bb = bl
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let rates = rates(&aa, &bb)?;
    let expectations = expected
        .iter()
        .map(|text| {
            let mut readings = Vec::new();
            for (line, words) in bl.iter().zip(&groups) {
                for (start, _) in line.text.match_indices(text.as_str()) {
                    let end = start + text.len();
                    let mut offset = 0;
                    let mut confidences = Vec::new();
                    for word in words {
                        let next = offset + word.text.len();
                        if next > start && offset < end {
                            confidences.push(word.confidence);
                        }
                        offset = next + 1;
                    }
                    readings.push(if confidences.iter().all(Option::is_some) {
                        Some(
                            confidences
                                .iter()
                                .flatten()
                                .all(|&c| c >= readable_confidence),
                        )
                    } else {
                        None
                    });
                }
            }
            let present = !readings.is_empty();
            let readable = if readings.contains(&Some(true)) {
                Some(true)
            } else if !present || readings.iter().all(Option::is_some) {
                Some(false)
            } else {
                None
            };
            Expected {
                text: text.clone(),
                present,
                readable,
            }
        })
        .collect();
    Ok(Comparison {
        words: changes(&aw, &bw, a.dimensions, b.dimensions, moved_px),
        lines: changes(&al, &bl, a.dimensions, b.dimensions, moved_px),
        rates,
        expected: expectations,
    })
}
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    fn page(items: &[(&str, f64, f64)]) -> Source {
        Source {
            schema: "saccade-ui-source.v1".into(),
            capture_sha256: "0".repeat(64),
            dimensions: [200, 150],
            kind: "tesseract_tsv".into(),
            producer: serde_json::json!({"fixture":"generated; project licence"}),
            complete: false,
            nodes: items
                .iter()
                .enumerate()
                .map(|(i, (text, x, y))| crate::ui_review::Node {
                    id: format!("word-{i}"),
                    text: (*text).into(),
                    role: String::new(),
                    bounds: Some([*x, *y, 30., 10.]),
                    reading_order: None,
                    keyboard_order: None,
                    disclosure: false,
                    ocr_confidence: Some(95.),
                })
                .collect(),
        }
    }
    #[test]
    fn accents_use_scalar_cer_and_literal_expectations() {
        let a = page(&[("café", 10., 10.)]);
        let b = page(&[("cafe", 10., 10.)]);
        let result = compare(&a, &b, &["café".into(), "cafe".into()], 80., 3.).expect("diff");
        assert_eq!(result.rates.character_edits, 1);
        assert_eq!(result.rates.cer, Some(0.25));
        assert_eq!(result.rates.wer, Some(1.));
        assert_eq!(result.words[0].kind, "changed");
        assert!(!result.expected[0].present);
        assert_eq!(result.expected[1].readable, Some(true));
    }
    #[test]
    fn moved_missing_and_added_have_boxes() {
        let a = page(&[("alpha", 10., 10.), ("removed", 10., 70.)]);
        let b = page(&[("alpha", 80., 30.), ("added", 100., 100.)]);
        let r = compare(&a, &b, &[], 80., 3.).expect("diff");
        let kinds: Vec<_> = r.words.iter().map(|f| f.kind.as_str()).collect();
        assert_eq!(kinds, vec!["moved", "missing", "added"]);
        assert_eq!(r.words[0].after.as_ref().expect("after").bounds[0], 80.);
    }
    #[test]
    fn empty_reference_rates_remain_unavailable_and_low_confidence_fails_readability() {
        let a = page(&[]);
        let mut b = page(&[("résumé", 10., 10.)]);
        b.nodes[0].ocr_confidence = Some(20.);
        let r = compare(&a, &b, &["résumé".into()], 80., 3.).expect("diff");
        assert_eq!(r.rates.cer, None);
        assert_eq!(r.rates.wer, None);
        assert_eq!(r.expected[0].readable, Some(false));
    }
}
