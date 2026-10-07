//! Durable exact embedding shards, with legacy flat-index reads and transactional updates.
use super::{embedding, input};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
/// Segmented index manifest contract.
pub const SCHEMA: &str = "saccade-embedding-index.v2";
/// Incremental build/update execution receipt contract.
pub const UPDATE_SCHEMA: &str = "saccade-embedding-index-update.v1";
/// Maximum live rows; exceeding this bound fails rather than truncating.
pub const MAX_ROWS: usize = 1_000_000;
/// Maximum live vector storage (16 GiB).
pub const MAX_VECTOR_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const SEGMENT_ROWS: usize = 4096;
const MAX_SEGMENTS: usize = 4096;
const META_BYTES: u64 = 32 * 1024 * 1024;
const PATH_BYTES: usize = 256 * 1024 * 1024;
const ENCODING: &str = "f32-little-endian-row-major-l2-normalized";
fn invalid(message: impl Into<String>) -> Error {
    Error::Config(message.into())
}
fn io(e: std::io::Error) -> Error {
    invalid(format!("embedding index IO: {e}"))
}
fn hash_ok(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn shard(path: &str) -> u8 {
    use sha2::{Digest, Sha256};
    Sha256::digest(path.as_bytes())[0]
}
fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.contains('\\')
        && !Path::new(path).is_absolute()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}
fn file_name(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().count() == 1 && valid_path(path)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    encoded_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Segment {
    shard: u8,
    rows: usize,
    metadata: String,
    metadata_sha256: String,
    vectors: String,
    vectors_sha256: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    model: embedding::Model,
    model_contract_sha256: String,
    dimensions: usize,
    vector_encoding: String,
    rows: usize,
    segments: Vec<Segment>,
}
/// Snapshot reader. Queries stream one segment's metadata and one vector at a time.
/// Old generations must remain on disk until all snapshot readers have finished.
pub struct Index {
    dir: PathBuf,
    manifest: Manifest,
    legacy: Option<Value>,
    metadata_sha256: String,
}
impl Index {
    /// Read a bounded v2 manifest, or the historical v1 metadata when no v2 exists.
    pub fn load(dir: &Path) -> Result<Self> {
        let v2 = dir.join(format!("{SCHEMA}.json"));
        let is_v2 = v2.try_exists().map_err(io)?;
        let bytes = input::bytes(
            &if is_v2 {
                v2
            } else {
                dir.join(format!("{}.json", embedding::INDEX_SCHEMA))
            },
            if is_v2 {
                4 * 1024 * 1024
            } else {
                128 * 1024 * 1024
            },
        )?;
        let (manifest, legacy) = if is_v2 {
            (
                serde_json::from_slice::<Manifest>(&bytes).map_err(|e| invalid(e.to_string()))?,
                None,
            )
        } else {
            let meta: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
            let model = embedding::parse_model(
                &serde_json::to_vec(&meta["model"]).map_err(|e| invalid(e.to_string()))?,
            )?;
            let rows = meta["rows"]
                .as_array()
                .ok_or_else(|| invalid("missing legacy rows"))?;
            if meta["schema"] != embedding::INDEX_SCHEMA
                || meta["vector_file"] != "vectors.bin"
                || meta["vector_encoding"] != ENCODING
                || rows.len() > 100000
                || rows.len() as u64 * model.dimensions as u64 * 4 > 512 * 1024 * 1024
            {
                return Err(invalid("legacy index schema/size mismatch"));
            }
            let manifest = Manifest {
                schema: SCHEMA.into(),
                dimensions: meta["dimensions"].as_u64().unwrap_or(0) as usize,
                model,
                model_contract_sha256: meta["model_contract_sha256"].as_str().unwrap_or("").into(),
                vector_encoding: ENCODING.into(),
                rows: rows.len(),
                segments: vec![Segment {
                    shard: 0,
                    rows: rows.len(),
                    metadata: format!("{}.json", embedding::INDEX_SCHEMA),
                    metadata_sha256: crate::localized::digest(&bytes),
                    vectors: "vectors.bin".into(),
                    vectors_sha256: meta["vectors_sha256"].as_str().unwrap_or("").into(),
                }],
            };
            (manifest, Some(meta))
        };
        embedding::validate(&manifest.model)?;
        let identity = crate::localized::digest(
            &serde_json::to_vec(&manifest.model).map_err(|e| invalid(e.to_string()))?,
        );
        if manifest.schema != SCHEMA
            || manifest.vector_encoding != ENCODING
            || manifest.dimensions != manifest.model.dimensions
            || manifest.model_contract_sha256 != identity
            || manifest.rows > MAX_ROWS
            || manifest.rows as u64 * manifest.dimensions as u64 * 4 > MAX_VECTOR_BYTES
            || manifest.segments.len() > MAX_SEGMENTS
            || manifest
                .segments
                .iter()
                .try_fold(0u64, |count, s| count.checked_add(s.rows as u64))
                != Some(manifest.rows as u64)
        {
            return Err(invalid("index contract/count/size mismatch"));
        }
        let mut names = BTreeSet::new();
        let mut last = 0;
        for s in &manifest.segments {
            if s.shard < last
                || s.rows == 0
                || (legacy.is_none() && s.rows > SEGMENT_ROWS)
                || !file_name(&s.metadata)
                || !file_name(&s.vectors)
                || !hash_ok(&s.metadata_sha256)
                || !hash_ok(&s.vectors_sha256)
                || !names.insert(s.metadata.clone())
                || !names.insert(s.vectors.clone())
            {
                return Err(invalid("invalid segment manifest"));
            }
            last = s.shard;
        }
        Ok(Self {
            dir: dir.into(),
            manifest,
            legacy,
            metadata_sha256: crate::localized::digest(&bytes),
        })
    }
    /// Bound model, graph, tokenizer, preprocessing and calibration identity.
    pub fn model_id(&self) -> &str {
        &self.manifest.model_contract_sha256
    }
    /// Live row count.
    pub fn len(&self) -> usize {
        self.manifest.rows
    }
    /// Whether the snapshot contains no live vectors.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Hash of the exact retained manifest/legacy metadata bytes.
    pub fn metadata_sha256(&self) -> &str {
        &self.metadata_sha256
    }
    /// Number of recorded legacy build errors; v2 updates fail atomically on input errors.
    pub fn error_count(&self) -> usize {
        self.legacy
            .as_ref()
            .and_then(|v| v["errors"].as_array())
            .map_or(0, Vec::len)
    }
    fn rows(&self, segment: &Segment) -> Result<Vec<Row>> {
        let rows: Vec<Row> = if let Some(meta) = &self.legacy {
            serde_json::from_value(meta["rows"].clone()).map_err(|e| invalid(e.to_string()))?
        } else {
            let bytes = input::bytes(&self.dir.join(&segment.metadata), META_BYTES)?;
            if crate::localized::digest(&bytes) != segment.metadata_sha256 {
                return Err(invalid("segment metadata hash mismatch"));
            }
            serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?
        };
        if rows.len() != segment.rows
            || rows.iter().any(|r| {
                (if self.legacy.is_some() {
                    r.path.is_empty() || r.path.len() > 4096
                } else {
                    !valid_path(&r.path)
                }) || !hash_ok(&r.encoded_sha256)
                    || (self.legacy.is_none() && shard(&r.path) != segment.shard)
            })
            || (self.legacy.is_none() && rows.windows(2).any(|r| r[0].path >= r[1].path))
        {
            return Err(invalid("segment rows invalid"));
        }
        Ok(rows)
    }
    fn reader(&self, s: &Segment) -> Result<BufReader<File>> {
        // Hash and consume the same opened file; avoid a pathname reopen between validation and use.
        let bytes = s.rows as u64 * self.manifest.dimensions as u64 * 4;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let mut file = options.open(self.dir.join(&s.vectors)).map_err(io)?;
        if !file.metadata().map_err(io)?.is_file() || file.metadata().map_err(io)?.len() != bytes {
            return Err(invalid("segment vector size mismatch"));
        }
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        let mut buffer = [0; 65536];
        let mut remaining = bytes;
        while remaining > 0 {
            let n = remaining.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..n]).map_err(io)?;
            hasher.update(&buffer[..n]);
            remaining -= n as u64;
        }
        if format!("{:x}", hasher.finalize()) != s.vectors_sha256 {
            return Err(invalid("segment vector hash mismatch"));
        }
        file.seek(SeekFrom::Start(0)).map_err(io)?;
        Ok(BufReader::new(file))
    }
    /// Exact cosine top-k. V2 ties use lexical source paths; v1 preserves historical row order.
    pub fn query(&self, contract: &str, vector: &[f32], top: usize) -> Result<Value> {
        if contract != self.model_id()
            || vector.len() != self.manifest.dimensions
            || !(1..=100).contains(&top)
            || self.is_empty()
        {
            return Err(invalid(
                "query/index contract, dimensions, count or top mismatch",
            ));
        }
        embedding::cosine(vector, vector)?;
        let mut hits: Vec<(usize, Row, f64)> = Vec::with_capacity(top + 1);
        let mut id = 0;
        let mut last_path: Option<(u8, String)> = None;
        for s in &self.manifest.segments {
            let rows = self.rows(s)?;
            if self.legacy.is_none()
                && last_path
                    .as_ref()
                    .is_some_and(|(sh, path)| *sh == s.shard && path >= &rows[0].path)
            {
                return Err(invalid("overlapping segment rows"));
            }
            last_path = rows.last().map(|r| (s.shard, r.path.clone()));
            let mut reader = self.reader(s)?;
            for row in rows {
                let v = read_vector(&mut reader, self.manifest.dimensions)?;
                let score = embedding::cosine(vector, &v)?;
                hits.push((id, row, score));
                hits.sort_by(|a, b| {
                    b.2.total_cmp(&a.2).then_with(|| {
                        if self.legacy.is_some() {
                            a.0.cmp(&b.0)
                        } else {
                            a.1.path.cmp(&b.1.path)
                        }
                    })
                });
                hits.truncate(top);
                id += 1;
            }
        }
        Ok(
            json!({"hits":hits.into_iter().map(|(id,row,cosine)|json!({"row_index":id,"row":row,"cosine":cosine})).collect::<Vec<_>>()}),
        )
    }
}
fn read_vector(reader: &mut impl Read, dimensions: usize) -> Result<Vec<f32>> {
    let mut bytes = vec![0; dimensions * 4];
    reader.read_exact(&mut bytes).map_err(io)?;
    let v: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    let norm = v.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt();
    if !norm.is_finite() || (norm - 1.).abs() > 1e-3 {
        return Err(invalid("non-unit/nonfinite index vector"));
    }
    Ok(v)
}
#[derive(Clone)]
struct Entry {
    row: Row,
    source: Source,
    shard: u8,
}
#[derive(Clone)]
enum Source {
    Old(usize, usize),
    New(u64),
}
/// Exclusive transactional writer. Drop/failed commit keeps the old manifest authoritative.
/// A crash may retain unpublished files; cleanup is an offline operator task.
pub struct Update {
    dir: PathBuf,
    model: embedding::Model,
    identity: String,
    old: Option<Index>,
    entries: BTreeMap<String, Entry>,
    dirty: BTreeSet<u8>,
    spool: tempfile::NamedTempFile,
    _lock: File,
    added: usize,
    replaced: usize,
    removed: usize,
    path_bytes: usize,
}
impl Update {
    /// Create/open an index for this exact model contract, locking out concurrent writers.
    /// Legacy v1 is migrated only when the new manifest is successfully published.
    pub fn begin(dir: &Path, model: embedding::Model) -> Result<Self> {
        embedding::validate(&model)?;
        std::fs::create_dir_all(dir).map_err(io)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(".embedding-update.lock"))
            .map_err(io)?;
        lock.try_lock()
            .map_err(|e| invalid(format!("index writer busy: {e}")))?;
        let identity = crate::localized::digest(
            &serde_json::to_vec(&model).map_err(|e| invalid(e.to_string()))?,
        );
        let old = if dir
            .join(format!("{SCHEMA}.json"))
            .try_exists()
            .map_err(io)?
            || dir
                .join(format!("{}.json", embedding::INDEX_SCHEMA))
                .try_exists()
                .map_err(io)?
        {
            Some(Index::load(dir)?)
        } else {
            None
        };
        if old.as_ref().is_some_and(|i| i.model_id() != identity) {
            return Err(invalid("update model contract differs from index"));
        }
        let mut entries = BTreeMap::new();
        let mut dirty = BTreeSet::new();
        let mut retained_path_bytes = 0usize;
        if let Some(index) = &old {
            for (s, segment) in index.manifest.segments.iter().enumerate() {
                // Validate every retained vector segment before accepting an update.
                index.reader(segment)?;
                for (r, row) in index.rows(segment)?.into_iter().enumerate() {
                    if index.legacy.is_some() {
                        if !valid_path(&row.path) {
                            return Err(invalid(
                                "legacy labels need relative normalized paths for v2 migration",
                            ));
                        }
                        dirty.insert(shard(&row.path));
                    }
                    retained_path_bytes += row.path.len();
                    if retained_path_bytes > PATH_BYTES {
                        return Err(invalid("index path budget exceeded"));
                    }
                    if entries
                        .insert(
                            row.path.clone(),
                            Entry {
                                shard: shard(&row.path),
                                row,
                                source: Source::Old(s, r),
                            },
                        )
                        .is_some()
                    {
                        return Err(invalid("duplicate index path"));
                    }
                }
            }
        }
        let path_bytes = entries.keys().map(String::len).sum::<usize>();
        if path_bytes > PATH_BYTES {
            return Err(invalid("index path budget exceeded"));
        }
        let spool = tempfile::NamedTempFile::new_in(dir).map_err(io)?;
        Ok(Self {
            dir: dir.into(),
            model,
            identity,
            old,
            entries,
            dirty,
            spool,
            _lock: lock,
            added: 0,
            replaced: 0,
            removed: 0,
            path_bytes,
        })
    }
    /// Skip inference only when retained encoded bytes have exactly this digest.
    pub fn unchanged(&self, path: &str, hash: &str) -> bool {
        self.entries
            .get(path)
            .is_some_and(|e| e.row.encoded_sha256 == hash)
    }
    /// Add or replace a path using an explicitly identified embedding contract.
    /// Every vector is finite/nonzero and normalized before writing the staging spool.
    pub fn upsert(
        &mut self,
        contract: &str,
        path: &str,
        hash: &str,
        mut vector: Vec<f32>,
    ) -> Result<()> {
        if contract != self.identity
            || !valid_path(path)
            || !hash_ok(hash)
            || vector.len() != self.model.dimensions
        {
            return Err(invalid(
                "update vector contract/path/hash/dimensions mismatch",
            ));
        }
        if !self.entries.contains_key(path) && self.entries.len() >= MAX_ROWS {
            return Err(invalid("index row limit exceeded"));
        }
        let added_path_bytes = if self.entries.contains_key(path) {
            0
        } else {
            path.len()
        };
        if self.path_bytes + added_path_bytes > PATH_BYTES
            || (self.entries.len() + usize::from(added_path_bytes > 0)) as u64
                * self.model.dimensions as u64
                * 4
                > MAX_VECTOR_BYTES
            || self.spool.as_file().metadata().map_err(io)?.len() + vector.len() as u64 * 4
                > MAX_VECTOR_BYTES
        {
            return Err(invalid("update path/vector/staging budget exceeded"));
        }
        embedding::normalize(&mut vector)?;
        self.path_bytes += added_path_bytes;
        let offset = self
            .spool
            .as_file_mut()
            .seek(SeekFrom::End(0))
            .map_err(io)?;
        self.spool
            .write_all(
                &vector
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
            )
            .map_err(io)?;
        let row = Row {
            path: path.into(),
            encoded_sha256: hash.into(),
        };
        if self
            .entries
            .insert(
                path.into(),
                Entry {
                    row,
                    source: Source::New(offset),
                    shard: shard(path),
                },
            )
            .is_some()
        {
            self.replaced += 1;
        } else {
            self.added += 1;
        }
        self.dirty.insert(shard(path));
        Ok(())
    }
    /// Hash a complete bounded source directory; infer only new/changed encoded bytes.
    /// Pruning is explicit. An error must abort the transaction (drop it without commit).
    pub fn sync_directory<F>(&mut self, dir: &Path, prune: bool, mut embed: F) -> Result<()>
    where
        F: FnMut(&[u8]) -> Result<Vec<f32>>,
    {
        let files = input::files(dir, MAX_ROWS)?;
        let mut present = BTreeSet::new();
        for path in files {
            let name = path
                .strip_prefix(dir)
                .map_err(|e| invalid(e.to_string()))?
                .to_str()
                .ok_or_else(|| invalid("index paths must be UTF-8"))?
                .replace('\\', "/");
            let bytes = input::bytes(&path, input::MAX_BYTES)?;
            let hash = crate::localized::digest(&bytes);
            if !self.unchanged(&name, &hash) {
                let vector = embed(&bytes)?;
                let id = self.identity.clone();
                self.upsert(&id, &name, &hash, vector)?;
            }
            present.insert(name);
        }
        if prune {
            self.prune(&present);
        }
        Ok(())
    }
    /// Remove a source path; a missing path is a no-op.
    pub fn remove(&mut self, path: &str) {
        if self.entries.remove(path).is_some() {
            self.path_bytes -= path.len();
            self.removed += 1;
            self.dirty.insert(shard(path));
        }
    }
    /// Prune all paths absent from an explicit complete source listing.
    pub fn prune(&mut self, present: &BTreeSet<String>) {
        let removed: Vec<_> = self
            .entries
            .keys()
            .filter(|p| !present.contains(*p))
            .cloned()
            .collect();
        for p in removed {
            self.remove(&p);
        }
    }
    /// Publish synced immutable segments, then atomically replace and sync the manifest.
    pub fn commit(self) -> Result<Value> {
        self.commit_inner(false)
    }
    fn commit_inner(mut self, interrupt: bool) -> Result<Value> {
        let paths = self.entries.keys().map(String::len).sum::<usize>();
        if paths > PATH_BYTES
            || self.entries.len() as u64 * self.model.dimensions as u64 * 4 > MAX_VECTOR_BYTES
        {
            return Err(invalid("index path/vector budget exceeded"));
        }
        let live_rows = self.entries.len();
        let mut groups: BTreeMap<u8, Vec<Entry>> = BTreeMap::new();
        for entry in self.entries.into_values() {
            if self.dirty.contains(&entry.shard) {
                groups.entry(entry.shard).or_default().push(entry);
            }
        }
        let mut segments = Vec::new();
        for sh in 0..=255u8 {
            if !self.dirty.contains(&sh) {
                if let Some(old) = &self.old
                    && old.legacy.is_none()
                {
                    segments.extend(
                        old.manifest
                            .segments
                            .iter()
                            .filter(|s| s.shard == sh)
                            .cloned(),
                    );
                }
                continue;
            }
            let entries = groups.remove(&sh).unwrap_or_default();
            // Readers are retained only while rewriting this shard, never all vectors.
            let mut readers: BTreeMap<usize, BufReader<File>> = BTreeMap::new();
            for chunk in entries.chunks(SEGMENT_ROWS) {
                let mut vectors = tempfile::NamedTempFile::new_in(&self.dir).map_err(io)?;
                let mut rows = Vec::with_capacity(chunk.len());
                for e in chunk {
                    let v = match e.source {
                        Source::New(offset) => {
                            self.spool
                                .as_file_mut()
                                .seek(SeekFrom::Start(offset))
                                .map_err(io)?;
                            read_vector(self.spool.as_file_mut(), self.model.dimensions)?
                        }
                        Source::Old(s, r) => {
                            let old = self
                                .old
                                .as_ref()
                                .ok_or_else(|| invalid("missing retained index"))?;
                            if let std::collections::btree_map::Entry::Vacant(entry) =
                                readers.entry(s)
                            {
                                entry.insert(old.reader(&old.manifest.segments[s])?);
                            }
                            let reader = readers
                                .get_mut(&s)
                                .ok_or_else(|| invalid("missing segment reader"))?;
                            reader
                                .seek(SeekFrom::Start(r as u64 * self.model.dimensions as u64 * 4))
                                .map_err(io)?;
                            read_vector(reader, self.model.dimensions)?
                        }
                    };
                    vectors
                        .write_all(&v.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>())
                        .map_err(io)?;
                    rows.push(e.row.clone());
                }
                vectors.as_file().sync_all().map_err(io)?;
                let vectors_sha256 = input::sha256(
                    vectors.path(),
                    SEGMENT_ROWS as u64 * self.model.dimensions as u64 * 4,
                )?;
                let vector_path = vectors.into_temp_path().keep().map_err(|e| io(e.error))?;
                let mut metadata = tempfile::NamedTempFile::new_in(&self.dir).map_err(io)?;
                let bytes = serde_json::to_vec(&rows).map_err(|e| invalid(e.to_string()))?;
                if bytes.len() as u64 > META_BYTES {
                    return Err(invalid("segment metadata budget exceeded"));
                }
                metadata.write_all(&bytes).map_err(io)?;
                metadata.as_file().sync_all().map_err(io)?;
                let meta_path = metadata.into_temp_path().keep().map_err(|e| io(e.error))?;
                let name = |p: &Path| {
                    p.file_name()
                        .and_then(|s| s.to_str())
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("invalid segment name"))
                };
                segments.push(Segment {
                    shard: sh,
                    rows: rows.len(),
                    metadata: name(&meta_path)?,
                    metadata_sha256: crate::localized::digest(&bytes),
                    vectors: name(&vector_path)?,
                    vectors_sha256,
                });
            }
        }
        if segments.len() > MAX_SEGMENTS {
            return Err(invalid("segment count limit exceeded"));
        }
        let manifest = Manifest {
            schema: SCHEMA.into(),
            dimensions: self.model.dimensions,
            model: self.model,
            model_contract_sha256: self.identity,
            vector_encoding: ENCODING.into(),
            rows: live_rows,
            segments,
        };
        let bytes = serde_json::to_vec(&manifest).map_err(|e| invalid(e.to_string()))?;
        let mut pending = tempfile::NamedTempFile::new_in(&self.dir).map_err(io)?;
        pending.write_all(&bytes).map_err(io)?;
        pending.as_file().sync_all().map_err(io)?;
        sync_dir(&self.dir)?;
        if interrupt {
            return Err(invalid("injected interruption before manifest publication"));
        }
        pending
            .persist(self.dir.join(format!("{SCHEMA}.json")))
            .map_err(|e| io(e.error))?;
        sync_dir(&self.dir)?;
        Ok(
            json!({"schema":UPDATE_SCHEMA,"operation":"index_update","verdict":"pass","counts":{"images":manifest.rows,"added":self.added,"replaced":self.replaced,"removed":self.removed,"segments":manifest.segments.len()},"model_contract_sha256":manifest.model_contract_sha256,"index_metadata_sha256":crate::localized::digest(&bytes),"limitations":["exact retrieval; no semantic accuracy qualification","unreferenced generations require offline cleanup after readers finish"]}),
        )
    }
}
fn sync_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(dir).and_then(|f| f.sync_all()).map_err(io)?;
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn model() -> embedding::Model {
        embedding::parse_model(&serde_json::to_vec(&json!({"schema":embedding::MODEL_SCHEMA,"family":"dinov2-small","artifact":{"role":"embedding","version":"a".repeat(40),"format":"onnx","url":format!("https://example.org/{}/model.onnx","a".repeat(40)),"bytes":1,"sha256":"a".repeat(64),"license":"Apache-2.0"},"input":"pixels","output":"embedding","size":[8,8],"mean":[0.,0.,0.],"std":[1.,1.,1.],"dimensions":4,"calibration":null})).unwrap()).unwrap()
    }
    fn identity(m: &embedding::Model) -> String {
        crate::localized::digest(&serde_json::to_vec(m).unwrap())
    }
    fn generated(i: u32) -> (String, Vec<f32>) {
        use image::ImageEncoder;
        // Procedural PNGs only; the stand-in derives its vector from decoded source pixels.
        let rgb = i.to_le_bytes();
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&rgb, 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let image = input::decode(&bytes).unwrap();
        let p = image.get_pixel(0, 0).0;
        (
            crate::localized::digest(&bytes),
            p.iter().map(|v| f32::from(*v) + 1.).collect(),
        )
    }
    fn insert(update: &mut Update, id: &str, i: u32) {
        let (hash, v) = generated(i);
        update.upsert(id, &format!("{i:06}.png"), &hash, v).unwrap();
    }
    #[test]
    fn add_ten_thousand_generated_images_matches_full_rebuild() {
        let d = tempfile::tempdir().unwrap();
        let m = model();
        let id = identity(&m);
        let incremental = d.path().join("incremental");
        let rebuilt = d.path().join("rebuilt");
        let images = d.path().join("images");
        std::fs::create_dir(&images).unwrap();
        let write_image = |i: u32| {
            use image::ImageEncoder;
            let mut bytes = Vec::new();
            image::codecs::png::PngEncoder::new(&mut bytes)
                .write_image(&i.to_le_bytes(), 1, 1, image::ExtendedColorType::Rgba8)
                .unwrap();
            std::fs::write(images.join(format!("{i:06}.png")), bytes).unwrap();
        };
        let stand_in = |bytes: &[u8]| -> Result<Vec<f32>> {
            Ok(input::decode(bytes)?
                .get_pixel(0, 0)
                .0
                .iter()
                .map(|v| f32::from(*v) + 1.)
                .collect())
        };
        for i in 0..32 {
            write_image(i);
        }
        let mut u = Update::begin(&incremental, m.clone()).unwrap();
        u.sync_directory(&images, false, stand_in).unwrap();
        u.commit().unwrap();
        for i in 32..10032 {
            write_image(i);
        }
        let mut u = Update::begin(&incremental, m.clone()).unwrap();
        let mut calls = 0;
        u.sync_directory(&images, false, |bytes| {
            calls += 1;
            stand_in(bytes)
        })
        .unwrap();
        assert_eq!(calls, 10000);
        assert_eq!(u.commit().unwrap()["counts"]["added"], 10000);
        let mut u = Update::begin(&rebuilt, m).unwrap();
        u.sync_directory(&images, false, stand_in).unwrap();
        u.commit().unwrap();
        let mut flat =
            crate::media::search::Index::new(serde_json::to_value(model()).unwrap()).unwrap();
        for i in 0..10032 {
            let (hash, v) = generated(i);
            flat.add_vector(&format!("{i:06}.png"), &hash, v).unwrap();
        }
        let a = Index::load(&incremental).unwrap();
        let b = Index::load(&rebuilt).unwrap();
        for i in [0, 31, 32, 517, 10031] {
            let (_, v) = generated(i);
            let aa = a.query(&id, &v, 100).unwrap();
            let bb = b.query(&id, &v, 100).unwrap();
            assert_eq!(aa["hits"], bb["hits"]);
            let reference = flat.query_vector(&v, 100, "image").unwrap();
            let sources = |value: &Value| {
                value["hits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|h| (h["row"]["path"].clone(), h["cosine"].clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(sources(&aa), sources(&reference));
        }
        assert_eq!(
            crate::media::search::Index::load(&incremental)
                .unwrap()
                .query_vector(&[1., 2., 3., 4.], 3, "image")
                .unwrap()["hits"]
                .as_array()
                .unwrap(),
            &a.query(&id, &[1., 2., 3., 4.], 3).unwrap()["hits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|h| {
                    let mut h = h.clone();
                    h["band"] = Value::Null;
                    h
                })
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn mixed_contract_refused_before_write() {
        let d = tempfile::tempdir().unwrap();
        let m = model();
        let id = identity(&m);
        let mut u = Update::begin(d.path(), m.clone()).unwrap();
        insert(&mut u, &id, 0);
        u.commit().unwrap();
        let mut different = m.clone();
        different.mean[0] = 0.5;
        assert!(Update::begin(d.path(), different).is_err());
        let mut u = Update::begin(d.path(), m).unwrap();
        let (h, v) = generated(1);
        assert!(u.upsert(&"b".repeat(64), "other.png", &h, v).is_err());
        assert!(
            Index::load(d.path())
                .unwrap()
                .query(&"b".repeat(64), &[1.; 4], 1)
                .is_err()
        );
    }
    #[test]
    fn changed_bytes_replace_prune_and_unchanged_shards_reused() {
        let d = tempfile::tempdir().unwrap();
        let m = model();
        let id = identity(&m);
        let mut u = Update::begin(d.path(), m.clone()).unwrap();
        for i in 0..3 {
            insert(&mut u, &id, i);
        }
        u.commit().unwrap();
        let before = Index::load(d.path()).unwrap();
        let retained: Vec<_> = before
            .manifest
            .segments
            .iter()
            .filter(|s| s.shard != shard("000000.png") && s.shard != shard("000002.png"))
            .map(|s| s.vectors.clone())
            .collect();
        let mut u = Update::begin(d.path(), m).unwrap();
        let (old, _) = generated(0);
        assert!(u.unchanged("000000.png", &old));
        let (h, v) = generated(100);
        u.upsert(&id, "000000.png", &h, v).unwrap();
        u.prune(&BTreeSet::from(["000000.png".into(), "000001.png".into()]));
        let r = u.commit().unwrap();
        assert_eq!(r["counts"]["replaced"], 1);
        assert_eq!(r["counts"]["removed"], 1);
        let after = Index::load(d.path()).unwrap();
        assert_eq!(after.len(), 2);
        assert!(
            retained
                .iter()
                .all(|name| after.manifest.segments.iter().any(|s| &s.vectors == name))
        );
        let hits = after.query(&id, &[1.; 4], 2).unwrap();
        let changed = hits["hits"]
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["row"]["path"] == "000000.png")
            .unwrap();
        assert_eq!(changed["row"]["encoded_sha256"], h);
        // A retained reader remains usable after publication of a new generation.
        assert_eq!(
            before.query(&id, &[1.; 4], 3).unwrap()["hits"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn interruption_keeps_previous_manifest_and_writer_lock() {
        let d = tempfile::tempdir().unwrap();
        let m = model();
        let id = identity(&m);
        let mut u = Update::begin(d.path(), m.clone()).unwrap();
        insert(&mut u, &id, 0);
        u.commit().unwrap();
        let before = std::fs::read(d.path().join(format!("{SCHEMA}.json"))).unwrap();
        let mut u = Update::begin(d.path(), m.clone()).unwrap();
        assert!(Update::begin(d.path(), m.clone()).is_err());
        insert(&mut u, &id, 1);
        assert!(u.commit_inner(true).is_err());
        assert_eq!(
            std::fs::read(d.path().join(format!("{SCHEMA}.json"))).unwrap(),
            before
        );
        assert_eq!(Index::load(d.path()).unwrap().len(), 1);
        let mut u = Update::begin(d.path(), m).unwrap();
        insert(&mut u, &id, 1);
        u.commit().unwrap();
        assert_eq!(Index::load(d.path()).unwrap().len(), 2);
    }
    #[test]
    fn legacy_reads_and_atomic_migration() {
        let d = tempfile::tempdir().unwrap();
        let m = model();
        let id = identity(&m);
        let p = d.path().join("index");
        let mut flat = crate::media::search::Index::new(serde_json::to_value(&m).unwrap()).unwrap();
        flat.add_vector("a.png", &"a".repeat(64), vec![1., 0., 0., 0.])
            .unwrap();
        flat.add_vector("b.png", &"b".repeat(64), vec![1., 0., 0., 0.])
            .unwrap();
        flat.save(&p).unwrap();
        let legacy = Index::load(&p).unwrap();
        assert_eq!(
            legacy.query(&id, &[1., 0., 0., 0.], 2).unwrap()["hits"][0]["row_index"],
            0
        );
        let u = Update::begin(&p, m.clone()).unwrap();
        assert!(u.commit_inner(true).is_err());
        assert_eq!(Index::load(&p).unwrap().len(), 2);
        Update::begin(&p, m).unwrap().commit().unwrap();
        assert!(Index::load(&p).unwrap().legacy.is_none());
        assert!(p.join(format!("{}.json", embedding::INDEX_SCHEMA)).exists());
    }
    #[test]
    fn directory_inference_failure_does_not_publish_partial_update() {
        let d = tempfile::tempdir().unwrap();
        let images = d.path().join("images");
        std::fs::create_dir(&images).unwrap();
        std::fs::write(images.join("new.png"), b"invalid raster").unwrap();
        let index = d.path().join("index");
        let m = model();
        let id = identity(&m);
        let mut u = Update::begin(&index, m.clone()).unwrap();
        insert(&mut u, &id, 0);
        u.commit().unwrap();
        {
            let mut u = Update::begin(&index, m).unwrap();
            assert!(
                u.sync_directory(&images, true, |bytes| {
                    input::decode(bytes)?;
                    Ok(vec![1.; 4])
                })
                .is_err()
            );
        }
        assert_eq!(Index::load(&index).unwrap().len(), 1);
    }
    #[test]
    fn corruption_and_path_escape_refused() {
        let d = tempfile::tempdir().unwrap();
        let m = model();
        let id = identity(&m);
        let mut u = Update::begin(d.path(), m).unwrap();
        assert!(
            u.upsert(&id, "../outside", &"a".repeat(64), vec![1.; 4])
                .is_err()
        );
        insert(&mut u, &id, 0);
        u.commit().unwrap();
        let i = Index::load(d.path()).unwrap();
        std::fs::write(d.path().join(&i.manifest.segments[0].vectors), [0; 16]).unwrap();
        assert!(i.query(&id, &[1.; 4], 1).is_err());
    }
}
