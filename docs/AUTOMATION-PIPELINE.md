# Automation Pipeline Architecture

Status: **ACTIVE EXECUTION ARCHITECTURE**

This document defines how the scientific stages in [ARCHITECTURE.md](ARCHITECTURE.md) are progressively turned into one recoverable, cross-platform automation pipeline.

The project is implemented **module by module**. Each module must be independently usable, reviewable, testable, and recoverable before the next module is attached. The complete pipeline is the long-term target; partial modules are valid project deliverables and must remain useful on their own.

## 1. Product goal

The final user experience should support this path:

```text
dataset/accessions
→ acquisition
→ raw-read validation
→ QC / preprocessing decision
→ reference + Salmon index
→ Salmon quantification
→ tximport
→ DESeq2
→ validation
→ publication comparison
→ final report
```

A non-programmer should be able to use the graphical interface, while advanced users and automated workflows can use the CLI.

## 2. Execution architecture

```text
Desktop GUI
HTML + CSS + JavaScript
        │
        │ Tauri 2 command/event bridge
        ▼
Rust Core Engine
        │
        ├── Rust CLI
        ├── local filesystem
        ├── persistent job/state engine
        ├── SRA Toolkit adapters
        ├── FastQC / MultiQC adapters
        ├── Salmon adapter
        ├── R / Bioconductor adapter
        └── future pipeline modules
```

Rules:

- GUI and CLI use the same Rust core.
- Scientific/business logic does not live in the frontend.
- The GUI must be removable without breaking the core pipeline.
- Every module persists enough state to resume after interruption.
- External tools are invoked through explicit adapters with version/provenance capture.
- Large biological data remain outside Git; Git stores code, configuration, metadata, state schemas, compact provenance, tests, and reports.

## 3. Incremental module roadmap

### Module A — Acquisition / Download Manager

Purpose: reliably obtain raw sequencing data without requiring the user to edit code.

Detailed Module A contract: [DOWNLOAD-MANAGER-ARCHITECTURE.md](DOWNLOAD-MANAGER-ARCHITECTURE.md).

Inputs may include:
- one accession;
- a batch accession file;
- a supported study-level accession that resolves to runs;
- a direct URL where explicitly supported.

Primary responsibilities:
- detect operating system;
- validate tool availability;
- resolve accessions;
- estimate download/workspace sizes;
- inspect destination free space;
- show low-space warnings without automatically blocking the user;
- download;
- validate downloaded archives;
- convert to FASTQ when selected;
- compress;
- generate checksums;
- record provenance;
- persist state;
- recover and resume.

### Module B — Raw-read QC and preprocessing decision

Attach FastQC/MultiQC to the acquisition outputs and freeze the preprocessing decision required by Stage 4 / Gate G4.

### Module C — Reference and Salmon index

Construct and verify the frozen reference bundle and Salmon index after the relevant architecture dependencies are resolved.

### Module D — Salmon pilot and full quantification

Run the pilot, freeze configuration, and quantify the approved cohort.

### Module E — Gene-level import and DE

Automate tximport and Track A / Track B DESeq2 while preserving all statistical contracts.

### Module F — Validation, benchmark, report

Automate validation gates, publication comparison, provenance closure, and final reporting.

## 4. Module A reliability contract

The Download Manager is a long-running stateful workflow. Network loss, application restart, tool failure, low disk space, or a failed individual run must not silently destroy progress.

### Required job states

At minimum:

```text
QUEUED
RESOLVING
READY
DOWNLOADING
PAUSED
WAITING_FOR_NETWORK
DOWNLOADED
VALIDATING
CONVERTING
COMPRESSING
CHECKSUMMING
COMPLETE
WARNING
FAILED
CANCELLED
```

The persisted state, not the visible progress bar, is authoritative.

### Resume behavior

- Already completed work is never repeated unless the user explicitly requests re-validation/reprocessing.
- Partial downloads should use the underlying downloader's supported resume behavior where available.
- On transient network failure, preserve the job and downloaded partial state.
- When connectivity returns, the job can continue from the last safe checkpoint.
- Restarting the desktop application must reconstruct active/incomplete jobs from persisted state.
- A failed run in a batch must not erase successful sibling runs.
- Retry must be idempotent: retrying a job must not duplicate completed outputs or overwrite validated data silently.

### GUI recovery controls

The GUI must expose clear actions according to job state:

- **Pause** — stop safely at a recoverable boundary when supported.
- **Resume / Continue** — continue an interrupted or paused job.
- **Retry** — retry the failed step/run without restarting the whole dataset.
- **Cancel** — stop future work while preserving already completed valid outputs.
- **Open logs / Show error details** — expose actionable diagnostics.
- **Choose another folder** — allow recovery from storage pressure when practical.

The user should never need to guess whether a failed download must start from zero.

### Network handling

Transient connectivity is a recoverable condition, not immediate project failure.

Expected behavior:

```text
network available
    ↓
download
    ↓
connection lost
    ↓
persist state + keep partial data
    ↓
WAITING_FOR_NETWORK
    ↓
connection returns / user presses Resume
    ↓
continue safely
```

Automatic retry may use bounded backoff. It must not loop indefinitely or hide repeated failures.

### Storage behavior

Storage estimation distinguishes:

- estimated download size;
- persistent output size;
- temporary/peak workspace requirement.

Insufficient **recommended** workspace produces a visible warning, not a mandatory block.

Hard blocking is reserved for conditions such as:
- destination cannot be created;
- destination is not writable;
- the filesystem/tool reports a condition that makes the requested write impossible.

### Integrity and provenance

For each run/file, retain as applicable:
- accession/source URL;
- destination path;
- expected/reported size;
- observed size;
- checksum;
- SRA validation result;
- conversion status;
- timestamps;
- tool names and versions;
- retry count / last error;
- completion state.

## 5. State persistence

Runtime state stays outside Git but its schema is version-controlled.

A job record should have stable identity and enough information to rebuild the GUI after restart.

Conceptual structure:

```text
job_id
dataset/input identity
resolved runs
output root
current step
per-run state
completed artifacts
checksums
tool versions
warnings
last error
created_at
updated_at
```

State writes should be atomic enough that an application crash does not corrupt the only recovery record.

## 6. Error model

Errors are separated into classes so the user receives the correct recovery action.

### Recoverable
Examples:
- temporary network loss;
- remote throttling;
- interrupted process;
- insufficient recommended workspace;
- one failed run in a batch.

Action: preserve state and offer Resume/Retry/Continue or alternate storage.

### User-action required
Examples:
- invalid accession;
- inaccessible destination;
- missing external tool when not bundled;
- unsupported input type.

Action: explain the exact problem and next action without deleting existing progress.

### Hard data-integrity failure
Examples:
- archive validation failure after bounded retry;
- checksum mismatch;
- malformed required output.

Action: mark only the affected artifact/run failed, preserve evidence, and require explicit recovery/re-download of that artifact.

## 7. Frontend/backend contract

Frontend responsibilities:
- collect input;
- display resolved datasets/runs;
- display estimates and warnings;
- present controls;
- show progress, state, warnings, and errors;
- request user actions.

Rust Core responsibilities:
- validate/resolve inputs;
- inspect filesystem/storage;
- manage persistent job state;
- invoke external bioinformatics tools through explicit process adapters;
- implement retry/resume semantics;
- validate artifacts;
- emit structured progress/state events to Tauri;
- maintain logs/provenance.

Tauri 2 is the desktop application shell and local frontend/backend bridge. The frontend remains ordinary HTML/CSS/JavaScript and does not own scientific or workflow logic.

The frontend must not infer completion from a percentage alone; it renders state emitted by the core.

## 8. CLI parity

GUI convenience must not make the workflow unreproducible.

The same core operations should be available through CLI commands, for example conceptually:

```text
rnaseq-pipeline download <accession>
rnaseq-pipeline status <job>
rnaseq-pipeline resume <job>
rnaseq-pipeline retry <job>
```

Exact command names remain an implementation detail until the CLI contract is frozen.

## 9. Cross-platform target

Target operating systems:
- Linux;
- macOS;
- Windows.

Cross-platform Rust APIs/crates should be preferred for filesystem, path, checksum, process, state, and storage logic. Platform-specific code belongs behind adapters and must not leak into scientific workflow logic.


## 9.1 Technology stack

The accepted implementation stack is:

```text
Desktop shell: Tauri 2
Frontend:      HTML + CSS + JavaScript
Core engine:   Rust
CLI:           Rust
State store:   Rust-owned persistent store (exact backend frozen during Module A design)
External tools:
  - SRA Toolkit
  - FastQC / MultiQC
  - Salmon
  - R / Bioconductor
```

Rationale:

- the project is a long-running local desktop workflow with resume/retry/recovery requirements;
- Rust provides a strong fit for process orchestration, concurrency, filesystem work, checksums, persistent state, and cross-platform binaries;
- Tauri 2 provides the desktop shell and JavaScript↔Rust bridge while preserving the HTML/CSS/JavaScript frontend;
- external scientific tools remain separate versioned executables rather than being reimplemented merely to unify languages;
- R remains responsible for tximport/DESeq2 where the canonical statistical implementation belongs to Bioconductor.

Salmon being implemented in Rust is **not** itself the reason for this choice. Salmon remains an external, explicitly versioned tool invoked by the orchestration layer unless a future reviewed architecture decision proves a library-level integration materially better.

## 10. GitHub delivery model

Each module follows:

```text
Issue
→ architecture/acceptance criteria
→ isolated branch
→ implementation
→ tests/evidence
→ PR
→ review
→ main
```

A later module may depend on an earlier module, but the repository must remain understandable and usable after every merged increment.

## 11. Current focus

**Module A — Acquisition / Download Manager**

Do not begin Module B implementation until Module A's contracts are sufficiently stable to provide trustworthy raw-read artifacts and provenance.
