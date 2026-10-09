# Architecture Decisions — v2.1

Status: **SCIENTIFIC CONTRACT REVISED TO v2.1**

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
- Scientific execution uses reproducible CLI/batch scripts and external versioned
  bioinformatics tools, not a custom shared Rust desktop application.
- The Rust/Tauri Download Manager and its shared Rust core/GUI/CLI are separate
  software, out of scope and never a G4/G5/G6 scientific dependency.
- Salmon remains a pinned external executable and R/Bioconductor owns
  tximport/DESeq2. Raw input provenance is verified directly under G4.
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

Scientific architecture v2.1 is the execution baseline. It keeps all
scientific requirements and explicitly separates desktop acquisition software.


## Owner-directed separation of Download Manager — 2026-10-09

Remove the Rust/Tauri Module A code, Cargo workspace, Rust CI and dedicated
implementation documents from the current repository tree. This is a
scope change requested by the owner so the Download Manager can later move
into an independent repository. No separate repository is created now.

Complete pre-deletion source remains available in Git history at commit
`6ee4dfa99d20c9b593f92525a191cef641eb3f75`.
No source history is rewritten and no scientific data, manifests, FASTQ,
Salmon reference/index or validated quantification output is removed.

G4 evaluates actual raw-read provenance and QC independently; completing
the former Module A product is **not** a condition for its acceptance.
