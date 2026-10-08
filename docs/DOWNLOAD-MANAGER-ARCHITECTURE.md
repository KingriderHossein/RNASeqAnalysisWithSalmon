# Download Manager Architecture — Rust/Tauri

Status: **MODULE A EXECUTION CONTRACT**

Owner: Issue #14
Parent architecture: [AUTOMATION-PIPELINE.md](AUTOMATION-PIPELINE.md)

This document freezes the internal architecture for Module A before implementation.

## 1. Outcome

Module A provides a cross-platform acquisition engine that a non-programmer can operate from a desktop GUI and an advanced user can operate from a CLI. The same Rust core owns both paths.

~~~text
Desktop GUI (HTML/CSS/JS)         Rust CLI
             │                       │
             └──────────┬────────────┘
                        ▼
                   Rust Core
                        │
       ┌────────────────┼─────────────────┐
       ▼                ▼                 ▼
  Job Engine       SQLite State      Tool Adapters
       │                                  │
       ▼                         prefetch / vdb-validate / fasterq-dump
 Progress/events
~~~

## 2. Repository/component layout

Target structure:

~~~text
apps/
  desktop/
    ui/
      index.html
      app.css
      app.js
    src-tauri/
      Cargo.toml
      src/

crates/
  pipeline-core/
    src/
      domain/
      workflow/
      state/
      storage/
      integrity/
      events/
      errors/
      adapters/
        sra_toolkit/
        direct_http/
  pipeline-cli/
    src/

schemas/
  job-state/

tests/
  fixtures/
  integration/
~~~

- pipeline-core: all workflow/business logic.
- pipeline-cli: thin CLI over the same core.
- apps/desktop: Tauri shell + frontend only.
- schemas: versioned persistent-state contracts.
- external bioinformatics tools remain external processes.

## 3. Core components

### 3.1 InputResolver

Accepts a single supported accession, batch TXT/CSV/TSV accession list, supported study-level accession, or supported direct URL. Produces a normalized ResolvedInput with stable source identity and run-level work items.

### 3.2 JobEngine

The authoritative workflow coordinator. It creates/opens jobs, schedules run work, enforces state transitions, checkpoints durable steps, isolates run failures, coordinates pause/resume/retry/cancel, and emits structured events. UI state is never authoritative.

### 3.3 StateStore

Persistent backend: **SQLite**.

Reasons: transactional updates, crash-safe durable state, efficient per-job/per-run queries, embedded deployment, and desktop restart recovery. The exact Rust SQLite crate remains an implementation choice. State schema is versioned and migrations are explicit.

### 3.4 StorageInspector

Checks destination creation/writability and free space, estimates download size, persistent output, and peak temporary workspace, and emits warnings. A recommended-space shortfall is a warning, not a hard block.

### 3.5 ToolRegistry / ToolRunner

Owns external executable discovery, version capture, invocation, child-process lifecycle, and log capture. Adapters include prefetch, vdb-validate, fasterq-dump, and later FastQC/MultiQC/Salmon/Rscript.

Each invocation records executable path, version, arguments after safe redaction, start/end timestamps, exit status, and stdout/stderr log locations.

### 3.6 SraToolkitAdapter

Run-level flow:

~~~text
prefetch
   ↓
vdb-validate
   ↓
fasterq-dump
   ↓
compression
   ↓
checksum
~~~

Repeated prefetch invocation is the supported resume path for interrupted SRA downloads. Downloaded accession directories are preserved. After successful prefetch, conversion can proceed locally without live network access. fasterq-dump is treated as a one-accession local conversion step.

### 3.7 DirectHttpAdapter

For explicitly supported HTTP(S) resources: detect range support before claiming resumability; write to a .part artifact; resume only when remote identity/size/range semantics still match; otherwise restart that file safely. Never label a non-resumable source as resumable.

### 3.8 IntegrityManager

Owns archive validation, output existence/size checks, SHA-256 generation/verification, atomic artifact finalization, and protection against silent overwrite of validated output.

### 3.9 EventBus

Core emits typed events such as JobCreated, InputResolved, RunQueued, RunStateChanged, ProgressUpdated, StorageWarning, NetworkInterrupted, RetryScheduled, UserActionRequired, ArtifactValidated, RunCompleted, and JobCompleted.

Tauri transports events to JavaScript. The frontend does not create authoritative workflow state.

## 4. Persisted identity

Job fields: job_id, schema_version, input_type, input_identity, output_root, created_at, updated_at, overall_state, settings_snapshot, tool_versions_snapshot, last_error.

Run fields: run_id, job_id, accession_or_source, state, attempt_count, downloaded_bytes, source_size, sra_path, fastq_paths, checksum_paths, last_checkpoint, last_error, updated_at.

Artifact fields: artifact_id, run_id, kind, path, size_bytes, sha256, validation_state, created_at, finalized_at.

## 5. State machine

~~~text
QUEUED
  ↓
RESOLVING
  ↓
READY
  ↓
DOWNLOADING
  ├──→ PAUSED
  ├──→ WAITING_FOR_NETWORK
  ├──→ FAILED
  ↓
DOWNLOADED
  ↓
VALIDATING
  ├──→ FAILED
  ↓
SRA_VALID
  ↓
CONVERTING
  ├──→ PAUSED_AT_BOUNDARY
  ├──→ FAILED
  ↓
FASTQ_READY
  ↓
COMPRESSING
  ├──→ FAILED
  ↓
CHECKSUMMING
  ├──→ FAILED
  ↓
COMPLETE
~~~

WARNING is metadata/severity, not a replacement for actual workflow state. CANCELLED is terminal for future work while finalized valid artifacts remain preserved.

## 6. Resume semantics by stage

### 6.1 Download / prefetch

**True resume.** Preserve partial accession data and persisted checkpoints. Reinvoke the same prefetch operation after interruption. Do not delete valid partial download state merely to simplify retry.

### 6.2 SRA validation

**Idempotent rerun.** If validation is interrupted or ambiguous, validate the downloaded accession again.

### 6.3 FASTQ conversion / fasterq-dump

**Restartable, not claimed as byte-level resumable.** Preserve validated SRA input. Mark partial FASTQ/temp output non-final. Safely remove or quarantine only owned partial conversion artifacts and rerun conversion. Never redownload SRA solely because conversion failed.

### 6.4 Compression

Write to an owned temporary output such as *.gz.part and atomically finalize where supported. If interrupted, restart compression from valid FASTQ unless the selected compressor has a separately validated resume contract.

### 6.5 Checksums

Checksum generation is idempotent. A run becomes COMPLETE only after required artifacts are finalized and integrity state is persisted.

## 7. Network recovery

~~~text
DOWNLOADING
    ↓ network-classified failure
persist checkpoint
    ↓
WAITING_FOR_NETWORK
    ├── automatic bounded retry/backoff
    └── Resume / Continue button
             ↓
          DOWNLOADING
~~~

Automatic retries are finite and non-tight. Manual Resume remains available for retryable interrupted work. Network recovery never discards completed sibling runs.

## 8. Pause / Resume / Retry / Cancel

- Pause is stage-aware. During resumable download it stops the owned child process safely and preserves state. During a non-resumable local step it may mean stop/restart from the next safe stage boundary rather than suspending arbitrary process memory.
- Resume / Continue applies to paused jobs, network waits, app-recovered incomplete work, and acknowledged non-blocking warnings.
- Retry targets only the failed run/stage unless the user chooses broader scope.
- Cancel stops future work and job-owned child processes but does not delete finalized valid artifacts.

## 9. Storage contract

The UI separately displays estimated download size, estimated persistent output, recommended temporary/peak workspace, and current free space.

Low recommended space produces a warning and leaves Continue enabled. Hard blocking is reserved for conditions where execution is actually impossible, such as an uncreatable or unwritable destination.

## 10. GUI ↔ Rust contract

Frontend calls Tauri commands conceptually equivalent to:

~~~text
resolve_input(request)
create_job(request)
inspect_storage(path, plan)
start_job(job_id)
pause_job(job_id)
resume_job(job_id)
retry_run(job_id, run_id)
cancel_job(job_id)
get_job(job_id)
list_jobs()
get_logs(job_id, run_id?)
~~~

Rust emits state/progress/warning/error/recovery events. Exact names may be refined during implementation, but semantic responsibility may not move into JavaScript.

## 11. Concurrency

Initial policy is conservative run-level concurrency with an explicit configurable limit. Conversion concurrency must account for disk and temporary-space pressure. No unbounded task spawning. Reliability is optimized before throughput.

## 12. Error classes

- Retryable: transient network failure, remote throttling, interrupted child process where recovery semantics permit it.
- User action required: invalid accession, missing required external tool, unusable destination, unsupported input/source.
- Integrity failure: failed SRA validation after bounded recovery, checksum mismatch, malformed/final artifact inconsistency.
- Internal failure: corrupted/incompatible local state or invariant/state-transition violation.

Internal failures must be diagnosable and never silently coerced to COMPLETE.

## 13. Logging and diagnostics

Structured context includes job_id, run_id when applicable, stage, tool, attempt, and error class. Do not log secrets or credential material. GUI shows concise user-facing error, expandable technical details, log location, and suggested recovery action.

## 14. Restart recovery

~~~text
open/migrate SQLite
        ↓
find non-terminal jobs
        ↓
reconcile DB state with owned filesystem artifacts
        ↓
mark recoverable state
        ↓
emit recovered event
        ↓
show Resume / Retry / Continue
~~~

Filesystem evidence and persisted state are reconciled; neither is trusted blindly when they disagree.

## 15. Architecture freeze boundary

Implementation may start when the Rust/Tauri stack, SQLite state backend, state transitions, recovery semantics, prefetch-resume vs fasterq-restart distinction, shared GUI/CLI core, non-blocking storage warning, and artifact integrity/finalization behavior are accepted and no unresolved architecture question blocks a safe pilot.