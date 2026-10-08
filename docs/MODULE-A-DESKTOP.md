# Module A desktop — v0.1.6

The Tauri 2 desktop is a thin HTML/CSS/JavaScript view over `pipeline-core`.
It uses the same SQLite job file and stage coordinator as `rnaseq-pipeline`.
This is an engineering preview, not acceptance of raw study data or a release.

## Run

Install Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).
On Linux the GTK/WebKitGTK development libraries are required. The native shell
is an explicit feature so workspace core/bridge tests do not require a GUI SDK:

```sh
cargo run -p pipeline-desktop --features native --bin rnaseq-desktop
```

Install SRA Toolkit separately and expose `prefetch`, `vdb-validate` and
`fasterq-dump` on PATH before launching. The app shows tool paths/versions and
keeps them frozen per job; a changed toolchain requires restoring the saved
versions. A queued job can be inspected or cancelled without installed tools.

## Supported flow

1. Choose an existing writable workspace, enter a job name, distinct resolved
   SRR/ERR/DRR runs and 1–256 conversion threads. Multiple runs form one batch.
   You can also expand Import runs from a file, Browse or enter a TXT/CSV/TSV
   path, and Preview. Review the loaded list before Create. Invalid files retain
   the previous list; preview does not start tools or create a job.
2. Optionally enter peak workspace guidance. Free space is inspected; download
   and persistent-output estimates remain unknown. A recommendation shortfall
   warns but permits creation/start. Invalid/unwritable destinations report errors.
3. Create the job, then Start. The database is `<workspace>/module-a.sqlite` and
   job outputs are `<workspace>/<job-name>/`. Existing destinations are not adopted.
4. Pause/Cancel requests are acknowledged immediately. Compression, gzip
   verification and checksum streams stop at safe buffer boundaries; acquisition
   and conversion finish their external stage. Closing an active window
   is blocked with a safe-stop instruction; it does not kill a tool.
5. Resume a paused job, Retry recorded failures, or open the saved SQLite job file.
   Completed siblings are skipped and their outputs kept. The live window serves
   cached snapshots while its worker exclusively owns SQLite.
6. Use View to inspect saved run state/paths and tail up to 64 KiB from its latest
   stdout/stderr attempt. Log paths are constrained to the selected job/run.

The current activity identifies the stage being called; the Saved line is the
last persisted checkpoint observed by the bridge. Byte counts are checkpoint
observations, not a continuously sampled transfer rate. Unknown total size has
no percentage. `Worker active` describes a live session, not proof of transfer.
Settings, tool versions, artifact paths and available SHA-256 values are inspectable.

## Failure and recovery boundary

Only one worker can be started in a window. Other CLI/windows contend on the
same core database/output ownership locks. Busy errors are actionable; the app
never opens a second connection to inspect around a live owner's database lock.

If a worker errors, Refresh reloads persisted checkpoints before another action.
DOWNLOADING/VALIDATING/CONVERTING after interruption remain blocked because an
earlier process/descendant may still write. A process wait/kill failure preserves
the in-flight checkpoint and conversion staging/temp files. It does not reset
the run to FAILED or enable Retry. Do not delete ownership files or edit states.
Full process-tree supervision and automatic network classification/backoff are
still parent #14 requirements. Zero automatic retries is the current policy.

In-process finalization polls the same durable intent as the CLI. A control read
failure stops safely, records a job error and leaves recoverable output. Published
gzip recovery retains COMPRESSING when paused so the next Resume verifies/adopts
the existing set. See [finalization control](MODULE-A-FINALIZATION-CONTROL.md).
Bridge activity includes a machine-readable `stage` alongside its display label;
control acknowledgement describes that stage without declaring the worker stopped.

The current form accepts resolved run accessions. Study/project resolution,
direct URLs, automatic size estimates and additional analysis modules are not
implemented here. Scientific gates and cohort contracts remain unchanged.
File format, limits, selection rules and client behavior are described in
[batch-file input](MODULE-A-BATCH-INPUT.md). Import does not apply cohort filters.

## Security and presentation

The main window has a bounded command permission list plus event listen/unlisten.
There are no general frontend shell, filesystem, HTTP or opener plugins. CSP
loads packaged local scripts/styles, with Tauri IPC endpoints only for connections.
Untrusted job IDs, paths, errors and logs are rendered as text, not HTML. Core
control requests check destination identity and reject preexisting control-directory
symlinks before writing flags or creating stage output roots. Path
membership/regular-file checks are defense in depth; this does not promise a
hostile-filesystem race boundary beyond the core ownership model.

The English operational UI uses labelled inputs, visible keyboard focus, table
headers, a polite control-status announcement, alert errors and native HTML
modal dialogs. Native minimum size is 900×650; long paths wrap and the run table
can scroll horizontally without widening the whole window. Browser-only previews
disable job actions and cannot start downloads.

## Verification

`cargo test --workspace` covers synthetic bridge/coordinator behavior, including
one-worker reservation, cached inspection under SQLite ownership, stage-boundary
Pause/Resume, SDK-free idle Cancel, terminal skips, artifact retention, bounded
logs, warn-only space guidance and unknown-child preservation. Frontend integer/
revision/unknown-size tests use `node --test crates/pipeline-desktop/ui/tests/presentation.test.mjs`.

The workspace Cargo.lock records the resolved dependencies; CI uses `--locked`.
CI separately builds and runs native Clippy for Linux, Windows and macOS. Linux
also runs a real Tauri/WebKitGTK window under Xvfb through `tauri-driver` 2.1.0:

```sh
cargo build -p pipeline-desktop -p pipeline-cli --features native
cargo install tauri-driver --version 2.1.0 --locked
xvfb-run node crates/pipeline-desktop/tests/native-smoke.mjs
```

That smoke uses only small synthetic executable-tool fixtures on a temporary
PATH. It exercises real frontend→IPC→coordinator batch creation/start/Pause/
Resume/finalization/Refresh and verifies the checksum manifest is preserved.
It checks native minimum-window overflow, GUI Busy under a real CLI owner,
Refresh after release, and missing-tool installation guidance. Screenshots are
available in the native CI job log for visual review.
It never contacts an archive or downloads cohort data. Native file-picker
interaction, Windows/macOS GUI interaction, packaging/signing, abrupt process-tree
reconciliation and production network recovery still need release-gate evidence.
Build success alone must not be reported as those behaviors passing.

Candidate CI/evidence is recorded in PR #45 and Issue #44; do not infer success
from this test inventory while a required check is pending or failed.
