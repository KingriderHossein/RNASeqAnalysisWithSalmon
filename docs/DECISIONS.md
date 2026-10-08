# Architecture Decisions — v2.0

Status: **FROZEN WITH ARCHITECTURE v2.0**

This file records the scientific and engineering decisions that define the project. Material changes require an explicit decision update and pull request.

## Core decisions

- Dataset: GSE89223 / SRP092131 / PRJNA350714.
- Dataset type: FFPE, single-end Ion Torrent Proton, total-RNA/rRNA-depleted; not strict Poly(A)+ mRNA-seq.
- Primary pipeline: `FASTQ → QC → Salmon → tximport → DESeq2`.
- Publication pipeline is comparator-only; STAR/HTSeq/edgeR is not rerun in the primary workflow.
- Track A: paper final cohort, 10 tumor vs 12 control, design `~ group`.
- Track B: 9 matched PCa pairs, design `~ patient + condition`.
- Primary reference: GRCh37.p13 + GENCODE Release 19 comprehensive annotation.
- Primary transcriptome: comprehensive transcript FASTA derived reproducibly from the matching genome + comprehensive GTF; do not restrict the benchmark to protein-coding transcripts.
- Primary Salmon strategy: Salmon 2.x selective alignment with a decoy-aware index; `--sketch` is excluded from the primary workflow.
- Full Salmon quantification is blocked until a single-end pilot freezes library type, version-specific fragment-length handling and bias-correction configuration.
- No Salmon parameter is selected by maximizing overlap with the publication.
- Gene aggregation: tximport with `tx2gene` derived from the same GENCODE v19 GTF; Salmon `--geneMap` is not the primary aggregation route.
- TPM is never supplied directly to DESeq2 as raw count input.
- Primary DEG definition: Benjamini–Hochberg adjusted p-value < 0.05; no mandatory absolute log2FC threshold for the primary benchmark.
- Published 3,384-DE-gene result is a benchmark, not ground truth.
- No sample is excluded merely because removal improves PCA, mapping or agreement with the paper.
- Internal canonical GENCODE IDs are preserved; normalized comparison IDs are separate.
- No hidden batch correction is added to Track A without documented evidence and review.
- Git stores compact metadata/configuration/provenance/results; raw FASTQ/SRA, large reference files and Salmon indexes stay outside Git.
- Execution architecture: one shared Rust Core serves both a Rust CLI and a Tauri 2 desktop GUI implemented with HTML/CSS/JavaScript through Tauri's local Rust bridge.
- Tauri 2 is the desktop shell; scientific/workflow logic remains in Rust rather than JavaScript.\n- External bioinformatics tools (including SRA Toolkit, Salmon, FastQC/MultiQC, and R/Bioconductor) remain explicit versioned tool adapters; they are not reimplemented merely to unify the implementation language.\n- Salmon remains an external executable boundary by default even though Salmon itself is implemented in Rust.\n- R/Bioconductor remains the statistical execution layer for tximport/DESeq2 unless a later reviewed scientific decision changes that contract.\n- The automation pipeline is delivered incrementally by module; the current first implementation module is acquisition/download.
- Long-running modules must persist recoverable state. Download interruption, application restart, and transient network loss must not require restarting completed work from zero.
- Recommended disk-space shortfall is a warning, not by itself a hard block; unwritable/invalid destinations and proven impossible writes may block execution.
- The primary project compares complete pipelines, not Salmon alone vs STAR alone.

## Architecture change control

A decision update + PR is required for changes to:

- primary dataset;
- Track A or Track B cohort;
- genome/GENCODE release;
- comprehensive vs protein-coding-only reference scope;
- Salmon mapping mode;
- primary gene aggregation method;
- DE engine;
- statistical design;
- primary DEG threshold;
- project completion criteria.

Architecture v2.0 is the execution baseline. Routine implementation may refine mechanics but must not silently redefine these contracts.
