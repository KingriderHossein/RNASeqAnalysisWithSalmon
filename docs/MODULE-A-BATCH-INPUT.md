# Module A batch-file input — v0.1.5

Both clients use `pipeline_core::run_batch` to read and validate local files.
Preview has no job, database, output-directory or tool execution side effect.
CLI `create-batch` validates the entire input before opening SQLite, then uses
the same transactional `create_job` operation as manual input. The desktop
replaces its editable run list only after a successful preview. Create and Start
remain separate actions. An invalid import retains the previous list.

## Supported files

- UTF-8 TXT, CSV or TSV, with case-insensitive filename extensions.
- Optional leading UTF-8 BOM and CRLF line endings.
- At most 2 MiB of input and 10,000 unique runs. Split larger files explicitly.
- TXT: whitespace, commas or semicolons separate runs; the first line can
  contain a run header. Blank lines are ignored.
- CSV/TSV with a header: exactly one column named `run`, `run_accession`,
  `sra_run`, `srr`, `accession` or `accessions`, case-insensitively. Only this
  column supplies runs. Other columns are ignored and reported by name/count.
- CSV/TSV without a header: every cell must contain a run. Column counts must
  be consistent. Blank lines are ignored. Properly quoted fields, escaped
  quotes and multiline metadata are supported; malformed quotes are rejected.

SRR, ERR and DRR runs are normalized to uppercase. Duplicate entries are removed
in first-seen order and the count is reported. Empty/invalid run cells,
study/experiment accessions, multiple candidate run columns, unsupported formats,
invalid UTF-8 and limits fail the entire import. No partial valid subset is
returned. Errors identify the row/condition without repeating metadata values.
The read itself is capped, including if a file grows during reading.

```csv
run,group
SRR900001,tumor
ERR900002,control
```

These synthetic identifiers show syntax only. Preview does not resolve a study,
check accession existence, infer layout, obtain size estimates, or download data.

## Scientific selection boundary

Import includes **all runs listed in the selected column**. It does not filter
by `paper_final_set`, Track A, Track B, diagnosis or any other metadata. Importing
the full project manifest therefore previews 32 runs, not the approved analysis
cohort. Use an explicitly prepared run list for a selected cohort. Import is an
engineering input feature; accepted-project raw execution still requires its
own gates. Source-manifest provenance for accepted scientific artifacts remains
the Stage 4 contract; this feature persists the normalized job runs and does not
claim a full scientific handoff.

## Client use and evidence boundary

CLI: `preview-batch FILE`, then `create-batch DB JOB OUTPUT_PARENT THREADS FILE`.
Each CLI command reads its current file; a later edit is reflected in the later
command. The created job's run identities are persisted. Paths can be quoted
and need not be UTF-8 on the CLI. No SRA tools are needed for preview/create.

Desktop: expand **Import runs from a file**, Browse or enter a path, and choose
**Preview file**. Review normalized runs, duplicate count and ignored-column
count, then Create job. Browse cancellation leaves inputs unchanged. Edits to
the path/list during an outstanding preview prevent replacement by a late
response. A manual list edit clears the old file-summary display.

Focused tests cover input selection, malformed input, resource bounds, CLI
pre-write rejection, and bridge preview/create. The Linux native smoke exercises
the path field and Preview through real Tauri IPC, then Create/Start with
synthetic tools. Picker interaction and Windows/macOS native interaction require
separate evidence; compilation does not prove those interactions. No release
or real data acquisition is part of these checks.
