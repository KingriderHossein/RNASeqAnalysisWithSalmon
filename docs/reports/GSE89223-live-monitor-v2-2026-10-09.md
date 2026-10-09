# GSE89223 live monitor v2 (localhost)

This is a non-destructive observation dashboard on the **authorized Linux analysis machine**. It updates its observations from local file artifacts and logs upon each GET request. The frontend requests a fresh status every 3 seconds. A red/offline connection state means the last rendered numbers may be stale; the dashboard **does not schedule ChatGPT messages or create long-running scientific jobs**.

## Active local deployment

- V1 remains at `http://127.0.0.1:8765`
- V2 is live at `http://127.0.0.1:8766`
- Local project files: `/home/kingrider/GSE89223_download/pipeline_monitor_v2.py` and `pipeline_monitor_v2.html`
- API: `GET /api/status`; report: `GET /multiqc`. All POST routes reject changes with HTTP 405.
- The server is loopback-bound and **cannot be reached from another device without a separately configured secure tunnel**.
- V2 uses the pre-existing `pipeline_monitor.py` as its raw/QC base state. It also reads the current G3 reference/index artifacts and prior G4 rRNA-screen TSVs. Repository copies are implementation snapshots; adapt the hardcoded project root and HTML path if deploying to another machine.
- The original v1 service was not stopped, edited or removed.

## Per-stage detail

Every stage entry contains a gate identifier, status, a short list of evidence-backed completed work / missing work, a relevant evidence/artifact list, and a next action. G3 watches local Salmon index build logs and successful return marker, and G4 retains the distinction between QC execution and scientific acceptance.

**Evidence and limitations:** The read-only detector is not the workflow engine or GitHub connector: it cannot infer later G5–G10 activity unless those stage-specific artifact detectors are implemented in a subsequent monitored workflow revision. It does not infer gate approval from source-file existence. A zero-return index build is shown as *built, needs review*, not as a scientifically approved index. Any counts and stage descriptions tied to a documented prior audit are static narrative, while local files, process and disk status are live observations.

## Smoke tests on Linux (2026-10-09)

- Python syntax and `--once` JSON parse passed.
- `GET /`, `GET /api/status`, `GET /multiqc`: HTTP 200.
- `POST /api/status`: HTTP 405.
- Verified 10 stage entries; G3 status remained `attention` with `index_build_returned_zero=true` and G4 `g4_accepted=false`.
- V1 `GET /` still HTTP 200.

Project G3 reports live in PR #52; G4 in PR #51. Raw FASTQ/SRA and large indexes remain outside Git.
