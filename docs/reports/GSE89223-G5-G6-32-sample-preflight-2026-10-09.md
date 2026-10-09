# G5 → G6 preflight: 32-sample metadata integrity (2026-10-09)

## Executed, not inferred

Host: Linux `kingrider`. Python non-destructive preflight exited **0 / PASS**, validating **32/32 accession-level records**. Evidence: `/home/kingrider/GSE89223_download/audits/G5_G6_handoff_20261009/01_g6_preflight.log` and `g6_preflight_metadata_v1.json`.

Inputs verified:
- All 32 distinct SRR accessions match the repository cohort manifest, `metadata/derived/GSE89223_sample_manifest.tsv`, and `metadata/runs.txt`.
- One canonical raw FASTQ per accession in `fastq/all` or `fastq/pilot`; paths and sizes match each file's persisted `metadata/fastq_conversion_state/SRR*.json`. Read/spot counts agree with manifest.
- State records contain previously verified SHA-256 and identify single-end inputs. **This run did not rehash FASTQ bytes**; avoid calling it a new 32-file checksum verification.
- Exactly 32 FastQC ZIPs; MultiQC HTML exists.
- Reference PASS in `derived/G3_keepDuplicates_validation_v1.json`, index components present; shared Salmon 2.8.0 k31 GENCODE v19 + GRCh37.p13 genome-decoy `--keepDuplicates` index.
- Host system ~192 GiB free disk, ~22 GiB available RAM during preflight. No active competing Salmon quant process observed at that time.

## Candidate primary quantification policy, not formal freeze

- **Single-end, stranded SF**, Salmon 2.8.0, shared validated index, no automatic FASTQ trimming / deduplication / rRNA filtering, fixed `--fldMean 150 --fldSD 50`, no bias switches.
- The mean 150 is an *unmeasured modeling prior*, not a physical fragment-length measurement. A prior shift to 250 produced stable *summed gene-level raw estimated counts* across two full-input pilots (Pearson 0.9957 / 0.9937) but materially more variable isoform estimates; detailed report `GSE89223-G5-expression-prior-stability-2026-10-09.md`. Both runs have low transcript assignment (13.44% tumor; 7.88% adjacent normal). Do not claim DE results are validated.
- Maintain `-l SF` protocol / diagnosis evidence. Enforce full provenance (input accession, SHA-256, command, versions, output identity) and complete outputs; fatal runs and warnings not silently accepted.
- MultiQC module FAIL does not by itself justify blanket trimming; raw baseline remains a candidate until G4 scientific gate.
- **Do not launch G6 just because metadata preflight passed**: G4 status remains `BASELINE_SELECTED_NOT_GATE_ACCEPTED`, Issue #4 remains open, and Issue #14 (Download Manager feature completeness) remains open but software GUI delivery progress should be distinguished from provenance truth for this cohort. G5 configuration still needs accepted policy; scientific QC criteria before expansion.

## Sample-level G6 QA when authorized

Require for each of 32: complete output directory, `quant.sf`, `aux_info/meta_info.json`, `cmd_info.json`, library-format report, logs, index compatibility, successful process status, all expected 196,520 transcript IDs, EM convergence, positive usable mapped counts, TPM sum near one million, QC status plus warning review; never automatically drop low-mapping samples without independent scientific decision. Preserve real original input hashes, all output paths; never invent progress percentages. Use a journal with one row per sample and explicit complete/failed/retry-safe status. Resume must not overwrite valid completed results.

## Project reporting scope

This stage currently reports Salmon transcript mapping and technical validity only. STAR genomic mapping and HTSeq gene assignment are separate deferred analyses—not computed or required in this handoff. User may request them when needed.

## Monitor

`tail -f ~/GSE89223_download/audits/G5_G6_handoff_20261009/01_g6_preflight.log`

No 32-sample quantification has started.
