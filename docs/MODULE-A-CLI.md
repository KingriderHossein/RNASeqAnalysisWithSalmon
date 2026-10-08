# Module A coordinator and CLI — v0.1.3

This is an engineering implementation for resolved SRR/ERR/DRR accessions. It
does not authorize execution of the accepted raw cohort or any downstream
scientific module. CI uses synthetic tool runners, never SRA downloads.

## Commands

Build with Rust 1.89 or newer: `cargo build --workspace`.

```text
rnaseq-pipeline --help
rnaseq-pipeline create state.sqlite example /path/to/output 2 SRR900001 ERR900002
rnaseq-pipeline inspect state.sqlite example
rnaseq-pipeline start state.sqlite example
rnaseq-pipeline pause /path/to/output/example
rnaseq-pipeline resume state.sqlite example
rnaseq-pipeline retry state.sqlite example
rnaseq-pipeline cancel /path/to/output/example
rnaseq-pipeline tools
```

The example accessions describe command syntax, not an instruction to download
real data. On Windows use a native destination path and quote paths with spaces.
The database parent must already exist. Create reserves a new job directory;
an existing directory, even empty, is rejected. Job IDs contain 1–128 ASCII
letters/digits, hyphens or underscores. Threads are bounded to 1–256. Study,
experiment, URL and batch-file resolution are not exposed by this CLI yet.

Create atomically records all READY runs and the versioned settings/destination
plan. If creation fails after reserving the destination, it retains that
directory for inspection instead of force-cleaning it. It does not require or
execute SRA Toolkit. The first worker freezes tool
paths and version outputs; subsequent workers reject a changed toolchain rather
than silently changing provenance. Install prefetch, vdb-validate and
fasterq-dump on PATH before execution. No shell command string is constructed.

The job root contains `sra`, `fastq`, `temp`, `logs`, `control`, and a
`MODULE-A-JOB` identity marker. Acquisition, conversion and finalization remain
the existing core executors. The coordinator lends its live root leases to
them, verifying every canonical root instead of bypassing ownership. Leases
remain held across stage boundaries and the whole sequential batch. One worker
owns the SQLite database; another worker or Inspect receives an actionable Busy
error. Pause/Cancel use a separate durable request channel and need no SQLite
connection, so they work while a worker owns the database.

## Recovery and control semantics

| Persisted run checkpoint | Start / Resume | Explicit Retry |
| --- | --- | --- |
| READY, PAUSED, WAITING_FOR_NETWORK | Acquisition | Same safe dispatch |
| DOWNLOADED | Validation | Same safe dispatch |
| SRA_VALID | Conversion | Same safe dispatch |
| FASTQ_READY | Compression and checksum | Same safe dispatch |
| COMPRESSING, CHECKSUMMING | Existing validated finalization recovery | Same safe dispatch |
| PAUSED_AT_BOUNDARY | Stage selected by its persisted checkpoint | Same safe dispatch |
| FAILED | Retained; sibling runs can proceed | Retry only its recorded failed stage, at most once per invocation |
| COMPLETE, CANCELLED | Skipped | Skipped |
| DOWNLOADING, VALIDATING, CONVERTING after restart | Blocked with a process-lifetime reconciliation error | Also blocked |
| Unknown state/checkpoint or inconsistent plan | Blocked without reset | Also blocked |

Resume clears the durable Pause request. Start leaves it in place. Cancel has
priority and is irreversible for that job; neither Resume nor Retry clears it.
Requests acknowledge intent, not completed stopping. A running external stage
finishes before control is applied: acquisition includes prefetch and validation
as one boundary, conversion is another, and finalization another. Outputs and
attempt-specific logs are retained. Complete siblings never restart or lose
their artifacts when another run fails, pauses, retries or cancels.

The current system runner can kill a direct child but does not prove that every
descendant is dead. The coordinator therefore does not use asynchronous tool
termination. Parent-crash states remain ambiguous even after the OS releases
ownership; no PID guess, stale marker removal, state reset or second writer is
used. Full process-tree supervision/reconciliation is required before enabling
automatic recovery there. Pausing a hung tool may wait indefinitely at this
version; this limitation is surfaced rather than declaring it stopped.

Network policy is conservative and finite: **zero automatic retries**. An
unclassified nonzero tool exit is FAILED with its recorded stage/error, not
assumed to be transient network loss. Explicit Retry attempts that stage once.
Automatic transient-network classification and bounded backoff remain future
work under the parent download-manager requirements.

Core clients receive typed run snapshots, errors, controls and job completion
events. Progress reports observed bytes at persisted stage boundaries. It does
not invent a percentage: validated total bytes remain unknown until resolution
supplies a validated source-size contract. The CLI currently prints these
events as text; streaming per-byte progress and a desktop event bridge remain
later work. Exit codes: 0 complete/command accepted, 1 failed job, 2 command/tool/
ownership error, 3 paused job, 4 cancelled job.

## Validation boundary

The cross-platform suite covers synthetic single/batch execution, failure
isolation, explicit retry, terminal skips, ownership between stages, durable
pause/cancel with a live SQLite owner, restart at a safe boundary, and refusal
to reset ambiguous external checkpoints. Existing stage recovery and native
no-clobber tests continue to apply. No GUI or production readiness is implied.
See [ownership guarantees and limits](MODULE-A-OWNERSHIP.md).
