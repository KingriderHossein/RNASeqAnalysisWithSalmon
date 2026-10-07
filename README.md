# RNASeqAnalysisWithSalmon

Reproducible reanalysis of **GSE89223 / SRP092131 / PRJNA350714** using a **Salmon-first** workflow for transcript quantification, gene-level summarization with tximport, and differential expression with DESeq2.

## Scientific question

How do gene-level differential-expression results from:

`FASTQ -> QC -> Salmon -> tximport -> DESeq2`

compare with the original study's published:

`FastQC -> Cutadapt -> STAR (hg19) -> HTSeq -> edgeR`

workflow?

The STAR/HTSeq/edgeR workflow is a **final comparator only**. It is not part of this project's primary execution path.

The original study is:

> Nikitina AS et al. *Novel RNA biomarkers of prostate cancer revealed by RNA-seq analysis of formalin-fixed samples obtained from Russian patients.* Oncotarget. 2017. DOI: 10.18632/oncotarget.16518.

## Primary data

- GEO: GSE89223
- SRA: SRP092131
- BioProject: PRJNA350714
- Organism: Homo sapiens
- Platform: Ion Torrent Proton
- Material: FFPE prostate cancer / adjacent-normal and BPH tissues
- Library layout: single-end

## Authoritative project docs

- [Project specification](docs/PROJECT-SPEC.md) — outcome, success criteria, and non-goals
- [Analysis architecture](docs/ARCHITECTURE.md) — Salmon-first technical flow and scientific gates
- [Agent/project rules](AGENTS.md) — execution and data-safety rules

## Pipeline

```text
verified cohort
      |
      v
raw single-end reads
      |
      v
FastQC / MultiQC
      |
      v
GENCODE v19 / GRCh37 reference bundle
      |
      v
decoy-aware Salmon index
      |
      v
single-end Salmon pilot
      |
      v
frozen Salmon configuration
      |
      v
full Salmon quantification
      |
      v
tximport
      |
      v
DESeq2
      |
      v
gene-level comparison with the original paper
```

## Project management

GitHub Issues are the source of truth for active work:

1. #1 Validate GSE89223 metadata and build sample manifest
2. #2 Define reproducible computational environment
3. #3 Build and verify human transcriptome Salmon index
4. #4 Retrieve raw reads and run pre-quantification QC
5. #5 Quantify GSE89223 with Salmon
6. #6 Gene-level DESeq2 analysis and comparison with the original study

## Data policy

Raw sequencing data, reference FASTA files, Salmon indexes, large intermediates, and package caches are **not committed to Git**. The repository contains metadata, code, configuration, provenance, compact QC summaries, and analysis outputs suitable for version control.
