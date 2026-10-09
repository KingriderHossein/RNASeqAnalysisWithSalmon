# G4/G5 acceptance decision and bounded G6 pilot — 2026-10-09

## G4 — PARTIAL / FORMAL OPEN

**Evidence-backed cohort readiness:** 32/32 immutable single-end FASTQs exist, acquisition-state records include sizes/checksums/provenance, all 32 FastQC ZIPs and a MultiQC report exist, and metadata preflight PASS. Raw initial processing (no adapter trim, quality tail clipping, rRNA filtering or dedup) is the *primary candidate*; controlled rRNA/quality studies documented separately. Input SHA-256 states are prior evidence; latest 32-file metadata preflight did not recompute every digest.

**Formal gate status:** G4 is **NOT ACCEPTED**. Issue #4 explicitly depends on trustworthy provenance delivery through still-open acquisition engineering Issue #14. No scientific or engineering owner may silently waive this dependency. Retain all QC warnings and avoid automatic sample deletion. The candidate raw baseline is still tagged `BASELINE_SELECTED_NOT_GATE_ACCEPTED` in original decision JSON.

## G5 — CONFIG CANDIDATE PINNED / FORMAL OPEN

Authoritative code/config candidate in this branch:
- `config/salmon_g6_raw_SF_v1.json`
- `scripts/g6_resumable_v1.py`

Version Salmon 2.8.0; GRCh37.p13+GENCODE v19 comprehensive transcript + full-genome decoy-aware k31 `--keepDuplicates` index sequence hash `51ab1987...`, info.json SHA-256 `98fb28c9...`; unmodified single-end FASTQ, `-l SF`, `--fldMean 150 --fldSD 50`, `-p 6`, no explicit bias flags. These options derive from two true full-input tumor/control pilots: matching SF inferred orientation, structural EM convergence and quant validity, low transcript mapping warnings 13.44% and 7.88%. Mean fragment length 150 is a **model prior**, not experimentally measured. Controlled mean 250 sensitivity retained near-invariant aggregated estimated gene counts (not tximport), but transcript allocations are more sensitive. No settings selected to increase agreement with publication.

**Formal gate status:** G5 is **CANDIDATE FROZEN IN REVIEW BRANCH** but NOT DECLARED PASSED until review and integration as governed by project architecture and closure of input G4 gate.

## G6 — STAGED PILOT / NO 32-SAMPLE BATCH AUTHORIZATION

A metadata-only dry-run validated the planned 32 accession list and index identity; first test produced a pre-Salmon missing-parent-directory error, corrected. Retained test failure log and successful regression dry-run. Controlled one-sample test `SRR4453783` initiated, with verified FASTQ SHA-256 immediately before launch and a separate Salmon execution log. Final validation still depends on recorded process result. This is *not* full 32-sample batch launch.

The guarded runner implements a single-process non-blocking flock lock, source digest comparison, no overwrite of any result, `.working` partial output preservation, complete required output checks, index/read counts, 196,520 unique transcripts, acceptable TPM sum, EM convergence, sample-level validation JSON, and validated output promotion by rename. Runs are sequential and per-sample logs are outside the working output directory, with an audit log. It skips only complete outputs with matching config/input and revalidated metrics. Interrupted incomplete samples require operator inspection before resuming; **intra-sample transparent resume is NOT implemented**, and no script should claim it.

The `--batch` mode requires `audits/G6_release_20261009/FORMAL_G4_G5_ACCEPTED.json` with accepted:true; do not create this file until the actual governance criteria are satisfied and verified. Merely having the configuration or 32 FASTQs does not authorize setting the gate accepted.

Monitor on Linux:
```bash
tail -f ~/GSE89223_download/audits/G6_release_20261009/02_runner_one_sample_retry.log
tail -f ~/GSE89223_download/salmon/G6_SF_raw_v1/SRR4453783.run.log
```

Source preservation: no raw FASTQ, shared index, or unrelated git work modified. Main checkout remains behind origin/main with unrelated `metadata/source/` untracked; GitHub code added to existing review branch, not overwritten on local main.

## Decision dependency

Next: review candidate source/config, complete single-sample output validation, reconcile Issue #14's acquisition contract without conflating GUI feature completeness with actual G4 provenance, formally accept G4/G5 in their authoritative records, then enable 32-sample runner with explicit monitor.
