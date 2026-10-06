# F1 consumer compatibility decisions

- Deprecated top-level spellings are translated before clap parsing. This keeps
  them out of help while preserving the current command implementation and one
  warning per invocation. The table in `CHANGELOG.md` is the release removal
  list. R5 removed `watch` scheduling. Its alias runs one comparison, preserves
  the old `watch-report` default, and names producer scheduling for repeats;
  `--debounce-ms` gives an explicit replacement-naming error. `explain` retains
  its sibling `explain` output directory; `snapshot` supplies PNG format and
  its old output filename to the export operation.
- Removed flags are rejected before argument parsing so a missing positional
  input cannot hide the migration message. `identity` remains exact.
- Performance readers reject unknown fields and newer schema numbers with an
  upgrade message. Report structs reject unknown fields; `inspect` reports
  newer report schema and field errors as version skew. Malformed input keeps
  its ordinary parse error.
- Image reports store declared binary, source, and capture identities per
  side. Metadata precedence is configured sidecar over Moss's flat cost card.
  A performance sidecar's capture hash supplies a fallback content hash.
  Missing binary/source identity remains unknown capture validity with an
  explicit reason and stderr warning. These are producer claims, not binary
  verification by Saccade.
- Equal declared capture IDs or content hashes warn on compare, invalidate
  identity, and are refused as image or performance noise repeats. Equal image
  bytes alone do not imply reused capture provenance.
