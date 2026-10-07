# Incremental embedding index decisions

- Keep flat v1 construction as the default and add `--segmented` plus explicit
  `index update`. This preserves the historical output contract. Reject silently
  replacing the default format. Reversal requires retiring opt-in writers while
  retaining the v2 reader for existing archives.
- Keep exact cosine as the reference. Shard by the first SHA-256 byte of the source
  path, sort each shard, and rewrite only affected shards. Reject approximate
  search without a measured latency requirement and recall qualification. Reversal
  requires another versioned writer layout; the exact reader remains an oracle.
- Bind every stored vector to the serialized model contract and encoded source
  hash. Reuse unchanged vectors, replace changed bytes, and prune only when
  explicitly requested against a complete listing. Reject dimension-only model
  checks. Reversal would require reembedding or explicit migration.
- Publish immutable synced files before the atomic manifest swap; serialize
  writers with an OS lock. Preserve old generations for snapshot readers. Reject
  online garbage collection without a reader-retention protocol. Reversal requires
  a separate compaction/retention design.
- Bound the archive at one million rows and 16 GiB live vectors rather than
  claiming unlimited retrieval. Query holds one segment's metadata, one vector
  and top-k hits; update retains bounded row metadata and a disk spool. The cost
  card measures 100,000 and 400,000 synthetic rows, including an archive exceeding
  the historical vector cap; it does not qualify every permitted shape or size.
- No new models, vector server or MCP mutation mirrors. Existing MCP queries use
  the shared reader. Mutation mirroring remains a follow-up with explicit write
  authority, rather than expanding this lane.

Acceptance and reproduction are described in [embeddings](../embeddings.md).
