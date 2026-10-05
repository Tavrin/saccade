//! Exact vector search interoperating with the wave 6 index files.
use super::{Analyzer, MediaError, Result};
use crate::{
    general::{embedding, input},
    wave7::models,
};
use serde_json::{Value, json};
use std::{io::Write, path::Path};
/// Same versioned format as the wave 6 CLI; text queries require joint model identity.
pub const SCHEMA: &str = embedding::INDEX_SCHEMA;
/// Exact flat index: at most 100000 rows and 512 MiB float32 storage.
pub struct Index {
    model: Value,
    model_id: String,
    rows: Vec<Value>,
    vectors: Vec<Vec<f32>>,
    dimensions: usize,
}
impl Index {
    /// Create an empty index for an explicitly pinned image/joint embedding contract.
    pub fn new(model: Value) -> Result<Self> {
        let m = embedding::parse_model(&serde_json::to_vec(&model)?)?;
        let model = serde_json::to_value(&m)?;
        Ok(Self {
            model_id: models::digest(&serde_json::to_vec(&m)?),
            model,
            rows: vec![],
            vectors: vec![],
            dimensions: m.dimensions,
        })
    }
    /// Build from images; a failed input fails the operation rather than silently dropping it.
    pub fn build(analyzer: &Analyzer, paths: &[String]) -> Result<Self> {
        let mut index = Self::new(analyzer.embedding_model()?)?;
        for path in paths {
            let bytes = analyzer.read(path)?;
            index.add_image(analyzer, path, &bytes)?;
        }
        Ok(index)
    }
    /// Append a row, verifying model identity and retained encoded content.
    pub fn add_image(&mut self, analyzer: &Analyzer, label: &str, bytes: &[u8]) -> Result<()> {
        self.check_model(analyzer)?;
        self.add_vector(label, &models::digest(bytes), analyzer.embed_image(bytes)?)
    }
    /// Add a normalized vector produced by this index's model (caller owns inference provenance).
    pub fn add_vector(&mut self, label: &str, hash: &str, mut vector: Vec<f32>) -> Result<()> {
        if self.rows.len() >= 100000
            || (self.rows.len() + 1)
                .saturating_mul(self.dimensions)
                .saturating_mul(4)
                > 512 * 1024 * 1024
            || label.len() > 4096
            || !models::valid_hash(hash)
            || vector.len() != self.dimensions
        {
            return Err(MediaError::new(
                "index_mismatch",
                "index row size, label, digest or dimensions invalid",
            ));
        }
        embedding::normalize(&mut vector)?;
        self.rows.push(json!({"path":label,"encoded_sha256":hash}));
        self.vectors.push(vector);
        Ok(())
    }
    fn check_model(&self, analyzer: &Analyzer) -> Result<()> {
        if analyzer.embedding_id()? != self.model_id {
            Err(MediaError::new(
                "index_mismatch",
                "query model contract differs from index",
            ))
        } else {
            Ok(())
        }
    }
    /// Query by image, using the same installed model as the index.
    pub fn query_image(&self, analyzer: &Analyzer, bytes: &[u8], top: usize) -> Result<Value> {
        self.check_model(analyzer)?;
        self.query_vector(&analyzer.embed_image(bytes)?, top, "image")
    }
    /// Query by text. Image-only models return typed unavailability, never invented similarity.
    pub fn query_text(&self, analyzer: &Analyzer, text: &str, top: usize) -> Result<Value> {
        self.check_model(analyzer)?;
        self.query_vector(&analyzer.embed_text(text)?, top, "text")
    }
    /// Exact cosine ranking with stable tie order; bands remain uncalibrated.
    pub fn query_vector(&self, vector: &[f32], top: usize, kind: &str) -> Result<Value> {
        if !(1..=100).contains(&top) || vector.len() != self.dimensions || self.rows.is_empty() {
            return Err(MediaError::new(
                "index_mismatch",
                "top must be 1..100 and query/index dimensions nonempty",
            ));
        }
        let mut hits = Vec::with_capacity(self.rows.len());
        for (id, row) in self.vectors.iter().enumerate() {
            hits.push((id, embedding::cosine(vector, row)?));
        }
        hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        hits.truncate(top);
        Ok(
            json!({"schema":embedding::QUERY_SCHEMA,"query_kind":kind,"model_contract_sha256":self.model_id,"calibration":"uncalibrated","hits":hits.into_iter().map(|(id,cosine)|json!({"row":self.rows[id],"row_index":id,"cosine":cosine,"band":null})).collect::<Vec<_>>()}),
        )
    }
    /// Persist a new directory using the existing vectors.bin plus versioned metadata format.
    /// Existing index directories are never overwritten.
    pub fn save(&self, dir: &Path) -> Result<()> {
        if self.rows.is_empty() {
            return Err(MediaError::new(
                "index_mismatch",
                "cannot save an empty index",
            ));
        }
        std::fs::create_dir(dir)
            .map_err(|_| MediaError::new("io_error", "new index directory cannot be created"))?;
        let bytes: Vec<u8> = self
            .vectors
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let result = (|| -> Result<()> {
            let mut f = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(dir.join("vectors.bin"))
                .map_err(|e| MediaError::new("io_error", e.to_string()))?;
            f.write_all(&bytes)
                .and_then(|_| f.sync_all())
                .map_err(|e| MediaError::new("io_error", e.to_string()))?;
            let meta = json!({"schema":SCHEMA,"model":self.model,"model_contract_sha256":self.model_id,"dimensions":self.dimensions,"rows":self.rows,"errors":[],"vector_file":"vectors.bin","vector_encoding":"f32-little-endian-row-major-l2-normalized","vectors_sha256":models::digest(&bytes),"calibration":"uncalibrated"});
            let mut f = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(dir.join(format!("{SCHEMA}.json")))
                .map_err(|e| MediaError::new("io_error", e.to_string()))?;
            f.write_all(&serde_json::to_vec_pretty(&meta)?)
                .and_then(|_| f.sync_all())
                .map_err(|e| MediaError::new("io_error", e.to_string()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(dir);
        }
        result
    }
    /// Load and validate legacy/current metadata and one hash-verified retained vector buffer.
    pub fn load(dir: &Path) -> Result<Self> {
        let meta: Value = serde_json::from_slice(&input::bytes(
            &dir.join(format!("{SCHEMA}.json")),
            128 * 1024 * 1024,
        )?)?;
        if meta["schema"] != SCHEMA
            || meta["vector_file"] != "vectors.bin"
            || meta["vector_encoding"] != "f32-little-endian-row-major-l2-normalized"
        {
            return Err(MediaError::new(
                "index_mismatch",
                "index schema/encoding mismatch",
            ));
        }
        let mut index = Self::new(meta["model"].clone())?;
        if meta["model_contract_sha256"] != index.model_id
            || meta["dimensions"].as_u64() != Some(index.dimensions as u64)
        {
            return Err(MediaError::new(
                "index_mismatch",
                "index model/dimensions mismatch",
            ));
        }
        let rows = meta["rows"]
            .as_array()
            .ok_or_else(|| MediaError::new("index_mismatch", "missing index rows"))?;
        if rows.is_empty() || rows.len() > 100000 {
            return Err(MediaError::new("index_mismatch", "index row count invalid"));
        }
        let expected = rows
            .len()
            .saturating_mul(index.dimensions)
            .saturating_mul(4);
        if expected > 512 * 1024 * 1024 {
            return Err(MediaError::new("index_mismatch", "index vector byte limit"));
        }
        let bytes = input::bytes(&dir.join("vectors.bin"), expected as u64)?;
        if bytes.len() != expected || meta["vectors_sha256"] != models::digest(&bytes) {
            return Err(MediaError::new(
                "index_mismatch",
                "retained vector bytes/hash mismatch",
            ));
        }
        for (row, raw) in rows.iter().zip(bytes.chunks_exact(index.dimensions * 4)) {
            let vector: Vec<f32> = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            // Reject corrupted/non-normalized input rather than normalizing and hiding corruption.
            let norm = vector
                .iter()
                .map(|v| f64::from(*v).powi(2))
                .sum::<f64>()
                .sqrt();
            if !norm.is_finite() || (norm - 1.).abs() > 1e-3 {
                return Err(MediaError::new(
                    "index_mismatch",
                    "index contains non-unit/nonfinite vector",
                ));
            }
            index.add_vector(
                row["path"]
                    .as_str()
                    .ok_or_else(|| MediaError::new("index_mismatch", "index path missing"))?,
                row["encoded_sha256"]
                    .as_str()
                    .ok_or_else(|| MediaError::new("index_mismatch", "row digest missing"))?,
                vector,
            )?;
        }
        Ok(index)
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn model() -> Value {
        json!({"schema":embedding::MODEL_SCHEMA,"family":"dinov2-small","artifact":{"role":"embedding","version":"a".repeat(40),"format":"onnx","url":format!("https://example.org/{}/model.onnx","a".repeat(40)),"bytes":1,"sha256":"a".repeat(64),"license":"Apache-2.0"},"input":"pixels","output":"embedding","size":[8,8],"mean":[0.,0.,0.],"std":[1.,1.,1.],"dimensions":2,"calibration":null})
    }
    #[test]
    fn save_load_rank_and_corrupt_rejection() {
        let mut index = Index::new(model()).unwrap();
        index
            .add_vector("a", "a".repeat(64).as_str(), vec![1., 0.])
            .unwrap();
        index
            .add_vector("b", "b".repeat(64).as_str(), vec![0., 1.])
            .unwrap();
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("index");
        index.save(&p).unwrap();
        assert!(index.save(&p).is_err());
        let read = Index::load(&p).unwrap();
        let hits = read.query_vector(&[0., 1.], 1, "text").unwrap();
        assert_eq!(hits["hits"][0]["row"]["path"], "b");
        assert_eq!(hits["calibration"], "uncalibrated");
        std::fs::write(p.join("vectors.bin"), [0; 16]).unwrap();
        assert_eq!(Index::load(&p).err().unwrap().code, "index_mismatch");
    }
}
