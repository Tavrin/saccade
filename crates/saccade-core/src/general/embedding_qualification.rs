//! Export parity and corpus-specific binary band calibration, run only in the heavy queue.
use super::embedding as e;
#[cfg(feature = "embeddings")]
use super::input;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
/// Frozen export qualification corpus schema.
pub const SCHEMA: &str = "saccade-embedding-corpus.v1";
/// Exact preprocessing tensor package schema.
pub const EXPORT_INPUT_SCHEMA: &str = "saccade-embedding-export-inputs.v1";
/// Offline exporter provenance receipt schema.
pub const EXPORT_RECEIPT_SCHEMA: &str = "saccade-embedding-export-receipt.v1";
/// Qualification receipt schema.
pub const RECEIPT_SCHEMA: &str = "saccade-embedding-qualification.v1";
/// An independently produced checkpoint embedding for exactly the Rust preprocessing tensor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// Relative image path inside corpus directory.
    pub image: String,
    /// Original encoded image digest.
    pub sha256: String,
    /// Exact NCHW f32 little-endian tensor digest.
    pub tensor_sha256: String,
    /// Independent source checkpoint's pooled, normalized embedding.
    pub embedding: Vec<f32>,
}
/// Labeled pair with a frozen fit or holdout assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// Sample index.
    pub a: usize,
    /// Sample index, different from a.
    pub b: usize,
    /// fit or holdout; never chosen after observing scores.
    pub split: String,
    /// Constructed or reviewed same-content label.
    pub same_content: bool,
}
/// A caller-frozen corpus binds export and checkpoint identities and scoped truth labels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// saccade-embedding-corpus.v1.
    pub schema: String,
    /// Export graph hash, equal to Model.artifact.sha256.
    pub export_sha256: String,
    /// Independently sourced checkpoint pin.
    pub checkpoint_sha256: String,
    /// Export source code revision and recipe; supplied provenance, not independently attested.
    pub source_revision: String,
    /// Content scope, truth construction and limits.
    pub scope: String,
    /// Max absolute component error and cosine loss (finite 0..0.001).
    pub parity_tolerance: f64,
    /// Up to 128 samples.
    pub samples: Vec<Sample>,
    /// Up to 4096 unique pairs, positive/negative in each split; split samples must be disjoint.
    pub pairs: Vec<Pair>,
}
fn invalid(msg: &str) -> Error {
    Error::Config(msg.into())
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
/// Validate frozen corpus topology, pins and split leakage before any inference.
pub fn validate(c: &Corpus, model: &e::Model) -> Result<()> {
    e::validate(model)?;
    if c.schema != SCHEMA
        || c.export_sha256 != model.artifact.sha256
        || !hash(&c.checkpoint_sha256)
        || c.source_revision.is_empty()
        || c.source_revision.len() > 4096
        || c.scope.is_empty()
        || c.scope.len() > 4096
        || !c.parity_tolerance.is_finite()
        || !(0. ..=0.001).contains(&c.parity_tolerance)
        || c.samples.is_empty()
        || c.samples.len() > 128
        || c.pairs.is_empty()
        || c.pairs.len() > 4096
    {
        return Err(invalid("invalid embedding qualification corpus"));
    }
    let mut identities = std::collections::BTreeSet::new();
    for s in &c.samples {
        let p = Path::new(&s.image);
        if p.is_absolute()
            || p.components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
            || s.image.len() > 4096
            || !hash(&s.sha256)
            || !hash(&s.tensor_sha256)
            || !identities.insert(&s.sha256)
            || s.embedding.len() != model.dimensions
        {
            return Err(invalid("invalid/duplicate export parity sample"));
        }
        e::cosine(&s.embedding, &s.embedding)?;
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut fit = std::collections::BTreeSet::new();
    let mut holdout = std::collections::BTreeSet::new();
    let mut classes = std::collections::BTreeSet::new();
    for p in &c.pairs {
        if p.a >= c.samples.len()
            || p.b >= c.samples.len()
            || p.a == p.b
            || !matches!(p.split.as_str(), "fit" | "holdout")
            || !seen.insert((p.a.min(p.b), p.a.max(p.b)))
        {
            return Err(invalid("invalid/duplicate calibration pair"));
        }
        classes.insert((p.split.as_str(), p.same_content));
        let set = if p.split == "fit" {
            &mut fit
        } else {
            &mut holdout
        };
        set.insert(p.a);
        set.insert(p.b);
    }
    if classes.len() != 4 || !fit.is_disjoint(&holdout) {
        return Err(invalid(
            "qualification needs positive/negative in both sample-disjoint splits",
        ));
    }
    Ok(())
}
/// Fits a separating threshold from fit-only labels, then tests frozen holdout without retuning.
pub fn calibrate(
    c: &Corpus,
    vectors: &[Vec<f32>],
    corpus_sha256: &str,
) -> Result<(e::Calibration, Value)> {
    if vectors.len() != c.samples.len() || !hash(corpus_sha256) {
        return Err(invalid("qualification vector/corpus identity mismatch"));
    }
    if c.pairs.is_empty()
        || c.pairs.len() > 4096
        || c.pairs.iter().any(|p| {
            p.a >= vectors.len()
                || p.b >= vectors.len()
                || p.a == p.b
                || !matches!(p.split.as_str(), "fit" | "holdout")
        })
    {
        return Err(invalid("invalid calibration pair topology"));
    }
    let classes: std::collections::BTreeSet<_> = c
        .pairs
        .iter()
        .map(|p| (p.split.as_str(), p.same_content))
        .collect();
    if classes.len() != 4 {
        return Err(invalid(
            "calibration requires positive and negative fit/holdout classes",
        ));
    }
    let fit: std::collections::BTreeSet<_> = c
        .pairs
        .iter()
        .filter(|p| p.split == "fit")
        .flat_map(|p| [p.a, p.b])
        .collect();
    let holdout: std::collections::BTreeSet<_> = c
        .pairs
        .iter()
        .filter(|p| p.split == "holdout")
        .flat_map(|p| [p.a, p.b])
        .collect();
    let unique: std::collections::BTreeSet<_> = c
        .pairs
        .iter()
        .map(|p| (p.a.min(p.b), p.a.max(p.b)))
        .collect();
    if !fit.is_disjoint(&holdout) || unique.len() != c.pairs.len() {
        return Err(invalid("calibration split leakage or duplicate pairs"));
    }
    let scores = c
        .pairs
        .iter()
        .map(|p| e::cosine(&vectors[p.a], &vectors[p.b]))
        .collect::<Result<Vec<_>>>()?;
    let mut low: f64 = 1.;
    let mut high: f64 = -1.;
    for (p, &score) in c
        .pairs
        .iter()
        .zip(&scores)
        .filter(|(p, _)| p.split == "fit")
    {
        if p.same_content {
            low = low.min(score);
        } else {
            high = high.max(score);
        }
    }
    if high >= low {
        return Err(invalid(
            "fit labels do not admit a separating cosine band; no calibration emitted",
        ));
    }
    let threshold = (low + high) / 2.;
    let mut false_positive = 0;
    let mut false_negative = 0;
    let mut holdout_count = 0;
    for (p, &score) in c
        .pairs
        .iter()
        .zip(&scores)
        .filter(|(p, _)| p.split == "holdout")
    {
        holdout_count += 1;
        if score >= threshold && !p.same_content {
            false_positive += 1;
        }
        if score < threshold && p.same_content {
            false_negative += 1;
        }
    }
    let calibration = e::Calibration {
        corpus_sha256: corpus_sha256.into(),
        scope: c.scope.clone(),
        bands: vec![
            e::Band {
                minimum: -1.,
                label: "different_content_in_supplied_scope".into(),
            },
            e::Band {
                minimum: threshold,
                label: "same_content_in_supplied_scope".into(),
            },
        ],
    };
    Ok((
        calibration,
        json!({"threshold":threshold,"fit_positive_minimum":low,"fit_negative_maximum":high,"holdout_pairs":holdout_count,"false_positive":false_positive,"false_negative":false_negative,"passed":false_positive==0&&false_negative==0,"scores":scores,"policy":"strict separation on fit, zero observed holdout errors; no universal semantic guarantee"}),
    ))
}
/// Executes actual ONNX parity and holdout calibration; no runtime/model downloads are implicit.
#[cfg(feature = "embeddings")]
pub fn qualify(
    c: &Corpus,
    corpus_sha256: &str,
    root: &Path,
    engine: &mut e::Engine,
) -> Result<(e::Model, Value)> {
    validate(c, engine.model())?;
    let root = crate::paths::canonicalize(root)
        .map_err(crate::run::io_err("resolving corpus root".into()))?;
    let mut vectors = Vec::new();
    let mut parity = Vec::new();
    let mut parity_pass = true;
    for s in &c.samples {
        let path = crate::paths::canonicalize(root.join(&s.image))
            .map_err(crate::run::io_err("resolving corpus sample".into()))?;
        if !path.starts_with(&root) {
            return Err(invalid("export sample escapes corpus root"));
        }
        let bytes = input::bytes(&path, input::MAX_BYTES)?;
        if crate::localized::digest(&bytes) != s.sha256 {
            return Err(invalid("export parity image hash mismatch"));
        }
        let image = input::decode(&bytes)?;
        let tensor = e::preprocess(engine.model(), &image)?;
        let tensor: Vec<u8> = tensor.iter().flat_map(|v| v.to_le_bytes()).collect();
        if crate::localized::digest(&tensor) != s.tensor_sha256 {
            return Err(invalid(
                "independent checkpoint parity used a different preprocessing tensor",
            ));
        }
        let actual = engine.embed(&image)?;
        let mut expected = s.embedding.clone();
        e::normalize(&mut expected)?;
        let max_error = actual
            .iter()
            .zip(&expected)
            .map(|(&a, &b)| (f64::from(a) - f64::from(b)).abs())
            .fold(0., f64::max);
        let cosine_loss = 1. - e::cosine(&actual, &expected)?;
        let pass = max_error <= c.parity_tolerance && cosine_loss <= c.parity_tolerance;
        parity_pass &= pass;
        parity.push(json!({"image":s.image,"maximum_component_error":max_error,"cosine_loss":cosine_loss,"passed":pass}));
        vectors.push(actual);
    }
    let (calibration, result) = calibrate(c, &vectors, corpus_sha256)?;
    let passed = parity_pass && result["passed"] == true;
    let mut model = engine.model().clone();
    model.calibration = passed.then_some(calibration);
    let receipt = json!({"schema":RECEIPT_SCHEMA,"operation":"embedding_qualification","verdict":if passed{"pass"}else{"regression"},"counts":{"samples":c.samples.len(),"pairs":c.pairs.len()},"corpus_sha256":corpus_sha256,"export_sha256":c.export_sha256,"checkpoint_sha256":c.checkpoint_sha256,"source_revision":c.source_revision,"scope":c.scope,"parity":parity,"calibration":result,"qualified_model_contract_sha256":crate::localized::digest(&serde_json::to_vec(&model)?),"limits":["source checkpoint vectors and labels are supplied evidence; source execution provenance is not independently attested","qualification applies only to frozen export/preprocessing and corpus scope","passing finite holdout does not establish universal semantic accuracy"]});
    Ok((model, receipt))
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn holdout_failure_does_not_retune_fit_threshold() {
        let c = Corpus {
            schema: SCHEMA.into(),
            export_sha256: "a".repeat(64),
            checkpoint_sha256: "b".repeat(64),
            source_revision: "frozen".into(),
            scope: "constructed vectors".into(),
            parity_tolerance: 0.0001,
            samples: (0..6)
                .map(|i| Sample {
                    image: format!("{i}.png"),
                    sha256: format!("{i:064x}"),
                    tensor_sha256: "c".repeat(64),
                    embedding: vec![1., 0.],
                })
                .collect(),
            pairs: vec![
                Pair {
                    a: 0,
                    b: 1,
                    split: "fit".into(),
                    same_content: true,
                },
                Pair {
                    a: 0,
                    b: 2,
                    split: "fit".into(),
                    same_content: false,
                },
                Pair {
                    a: 3,
                    b: 4,
                    split: "holdout".into(),
                    same_content: true,
                },
                Pair {
                    a: 3,
                    b: 5,
                    split: "holdout".into(),
                    same_content: false,
                },
            ],
        };
        let v = vec![
            vec![1., 0.],
            vec![1., 0.],
            vec![0., 1.],
            vec![1., 0.],
            vec![1., 0.],
            vec![1., 0.],
        ];
        let (bands, r) = calibrate(&c, &v, &"d".repeat(64)).unwrap();
        assert_eq!(bands.bands[1].minimum, 0.5);
        assert_eq!(r["false_positive"], 1);
        assert_eq!(r["passed"], false);
    }
    #[cfg(feature = "embeddings")]
    #[test]
    #[ignore = "heavy: embedding-qualification"]
    fn frozen_export_parity_and_disjoint_holdout_qualify() {
        let path = std::env::var_os("SACCADE_W6_EMBEDDING_CORPUS").expect("frozen corpus");
        let path = Path::new(&path);
        let bytes = input::bytes(path, 16 * 1024 * 1024).unwrap();
        assert_eq!(
            crate::localized::digest(&bytes),
            std::env::var("SACCADE_W6_EMBEDDING_CORPUS_SHA256").expect("frozen corpus pin")
        );
        let c: Corpus = serde_json::from_slice(&bytes).unwrap();
        let model = std::env::var_os("SACCADE_W6_EMBEDDING_MODEL").unwrap();
        let model: e::Model =
            serde_json::from_slice(&input::bytes(Path::new(&model), 65536).unwrap()).unwrap();
        let cache = std::env::var_os("SACCADE_W6_MODEL_CACHE").unwrap();
        let library = std::env::var_os("SACCADE_W6_ORT_LIBRARY").unwrap();
        let mut engine =
            e::Engine::load(model, Path::new(&cache), Path::new(&library), false).unwrap();
        let (_, receipt) = qualify(
            &c,
            &crate::localized::digest(&bytes),
            path.parent().unwrap(),
            &mut engine,
        )
        .unwrap();
        assert_eq!(receipt["verdict"], "pass", "{receipt}");
        if let Some(out) = std::env::var_os("SACCADE_W6_EMBEDDING_RECEIPT") {
            std::fs::write(out, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
        }
    }
}
