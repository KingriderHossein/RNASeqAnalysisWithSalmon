# Module A finalization control — v0.1.6

The shared coordinator now connects durable Pause/Cancel to its in-process
gzip, gzip/source verification and SHA-256 streams. Both CLI and Tauri use this
path. This is bounded engineering work under parent #14, tracked by #48.

## Control contract

The stream token polls the existing job control channel at buffer boundaries.
Each check validates that the control directory remains a real directory and
request markers are regular files. Cancel retains priority. Once a request or
read failure is observed, that token stays stopped even if its source changes.
The first read failure remains available to the coordinator; it stops the batch,
records a job error, and does not let unfinished siblings start.

Publication and COMPLETE also have stop checks. A request can arrive immediately
after a check: acknowledgement is intent, never proof that stopping is complete.
Wait for the worker to finish before closing the desktop. A blocked filesystem
operation can delay reaching the next check; this is cooperative interruption,
not an absolute wall-clock cancellation deadline.

| Work at stop | Persisted run and retained data | Resume |
| --- | --- | --- |
| New compression, before publication | PAUSED_AT_BOUNDARY / gzip; source FASTQ and SRA kept; only owned unpublished staging cleaned | New attempt compresses and verifies |
| Verification/adoption of already published gzip after a crash | COMPRESSING; published bytes and checkpoint kept | Reverify and adopt the exact set; never republish over it |
| Checksum verification or hashing | PAUSED_AT_BOUNDARY / sha256; published gzip kept | Verify original/gzip equality and recompute checksum |
| After checksum publication, before COMPLETE | PAUSED_AT_BOUNDARY / sha256; validated manifest kept | Verify exact existing manifest and artifact records |

Cancel uses the same safe stream stop, then the coordinator marks unfinished
runs CANCELLED. Completed siblings and valid artifacts are retained. Cancel is
irreversible for that job; Resume does not clear its marker.

A bad or missing control channel is an actionable job failure, not a normal
Pause. Repair the channel while no worker owns the job, then use Resume. Do not
edit run states or remove output ownership markers to bypass the error.

## External tools and remaining scope

prefetch, vdb-validate and fasterq-dump keep the stage-boundary policy. Their
tokens have no durable probe, so this change never asynchronously kills an
unproven descendant tree. External crash checkpoints remain blocked. Full
cross-platform process supervision/reconciliation, classified finite network
backoff, study/URL resolution, size estimates and release validation remain
parent requirements. No raw cohort or downstream scientific execution occurs.

## Validation

Synthetic Rust tests exercise stream stops after bytes have been processed,
latched/shared tokens, retained read errors, Pause/Cancel through the coordinator,
missing/invalid control channels, protected completed siblings, and published
gzip recovery without overwrite. A publication test injects a read error after
the checksum candidate is written and verifies that it cannot become the final
manifest or COMPLETE; Resume preserves gzip bytes.

Frontend tests distinguish buffer-boundary finalization from external-stage
control. Existing ownership, corruption, restart, CLI/bridge and native Linux
smoke checks remain required. CI builds/tests the workspace and builds native
desktop targets on Linux, Windows and macOS. Exact candidate/main CI evidence is
recorded in #48 and its PR; this inventory alone is not a passing result.
