# Project Specification — v2.0

## Goal

Reanalyse GSE89223 with a reproducible:

```text
FASTQ → QC → Salmon → tximport → DESeq2
```

workflow and compare the gene-level results with the original 2017 publication.

## Dataset

- GEO: GSE89223
- SRA: SRP092131
- BioProject: PRJNA350714
- PMID: 28380430
- PMCID: PMC5464844
- DOI: 10.18632/oncotarget.16518

## Dataset caveat

The dataset is FFPE, single-end, Ion Torrent Proton, total-RNA/rRNA-depleted RNA-seq. It is not strict Poly(A)+ mRNA-seq.

Therefore the primary analysis is whole-transcriptome and uses a comprehensive GENCODE v19 reference.

## Track A — primary benchmark

- paper final cohort;
- 10 tumor vs 12 control;
- design: `~ group`;
- primary publication-comparison track.

## Track B — paired sensitivity

- 9 matched PCa tumor/adjacent-normal pairs;
- BPH controls excluded;
- unpaired CP2 excluded;
- design: `~ patient + condition`.

## Primary reference

- GRCh37.p13;
- GENCODE Release 19 comprehensive annotation;
- comprehensive transcriptome derived reproducibly from matching genome + GTF;
- matching genome decoys;
- tx2gene from the same GTF.

## Primary quantifier

Salmon 2.x selective alignment using a decoy-aware index.

Full quantification is blocked until the pilot freezes the primary single-end configuration.

## Primary gene-level analysis

tximport → DESeq2.

TPM is not supplied directly to DESeq2.

## Primary DEG definition

BH adjusted p-value < 0.05.

No mandatory absolute log2FC threshold is part of the primary benchmark definition.

## Publication comparator

The original STAR → HTSeq → edgeR result is an external comparator, not ground truth.

The project compares complete pipelines; it does not claim to isolate Salmon-vs-STAR effects without a separate future bridge analysis.

## Required deliverables

1. verified sample/cohort manifest;
2. reproducible environment specification;
3. reference manifest/checksums/tx2gene;
4. raw-data provenance and QC summary;
5. frozen Salmon pilot configuration;
6. full Salmon quantification summary;
7. Track A complete DE result;
8. Track B complete DE result;
9. validation evidence;
10. publication-comparison tables/metrics;
11. final report;
12. reproducibility identity linking results to exact configuration and repository commit.

## Non-goals

Primary completion does not require:
- any general-purpose Rust/Tauri Download Manager, desktop GUI or its release;
- STAR/HTSeq/edgeR rerun;
- TCGA replacement analysis;
- GRCh38 rerun;
- protein-coding-only reference;
- transcript-level DE;
- inferential-replicate DE;
- exact reproduction of 3,384 DE genes.

## Completion

The project is complete only when all gates defined by Architecture v2.0 pass and the final report is traceable to exact cohort, input, reference, software, configuration, statistical design, analysis_id and repository revision.

Canonical documents:
- `ARCHITECTURE.md`
- `DECISIONS.md`
- `DATA-CONTRACTS.md`
- `VALIDATION.md`
- `GOVERNANCE.md`
