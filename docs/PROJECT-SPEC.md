# Project Specification

## Outcome

Reanalyse the public prostate-cancer RNA-seq study GSE89223 with a reproducible Salmon-based workflow and compare the gene-level differential-expression results with the original publication.

## Primary comparison

The original paper used:

```text
FastQC
-> Cutadapt
-> STAR against hg19
-> HTSeq with GENCODE release 19
-> edgeR
```

This project will use:

```text
raw reads
-> QC
-> Salmon
-> tximport or tximeta
-> gene-level abundance/count representation
-> DESeq2
```

The comparison must distinguish effects caused by:
- sample inclusion/exclusion;
- reference/annotation release;
- transcript-level versus genome-alignment quantification;
- gene summarization;
- statistical model/software;
- FFPE/Ion Torrent data characteristics.

## Authoritative study identifiers

- GEO: GSE89223
- SRA Study: SRP092131
- BioProject: PRJNA350714
- PMID: 28380430
- PMCID: PMC5464844
- DOI: 10.18632/oncotarget.16518

## Verified study facts at project start

NCBI GEO describes whole-transcriptome profiling of tumor and matched adjacent-normal tissue from prostate-cancer patients plus BPH samples. The study has 32 GEO/SRA experiments on Ion Torrent Proton. BioProject reports approximately 37 Gbases and 28,790 MB of archived SRA data.

The published analysis reports 3,384 differentially expressed genes at FDR < 0.05 after its own filtering/grouping procedure.

These facts are study context, not permission to assume the exact analysis cohort. The exact sample/run inclusion and pairing must be reconstructed in Issue #1 before large-scale download or differential-expression analysis.

## Success criteria

The project is complete only when:

1. Every analysed sample is traceable from GEO sample to SRA run and biological group.
2. Reference transcriptome and annotation versions are pinned and recorded.
3. Raw-read retrieval and QC are reproducible.
4. Salmon quantification completes for the approved analysis cohort.
5. Transcript-to-gene summarization is reproducible.
6. DESeq2 design and contrasts are explicit.
7. The primary prostate-cancer tumor versus adjacent-normal result is generated.
8. Results are compared with the original paper at gene level where evidence permits.
9. Important disagreements are analysed rather than hidden.
10. Commands, code, environment information, and compact outputs needed to reproduce the analysis are version controlled.

## Non-goals

- Reproducing every secondary analysis in the paper.
- Treating the BPH samples as part of the primary PCa-vs-normal contrast unless the analysis plan explicitly requires them.
- Reproducing the original hg19/STAR pipeline as the main workflow.
- Storing raw FASTQ/SRA files or Salmon indexes in Git.

## Data safety

Large data and indexes stay outside the repository. Any local deletion of raw data must occur only after downstream outputs and integrity checks are verified. Existing unrelated user data must never be overwritten or cleaned for this project.

## Project truth

- Durable scope: this file.
- Active tasks/blockers: GitHub Issues.
- Implementation and analysis history: commits and pull requests.
- Runtime evidence: analysis outputs and logs tied to the relevant code/version.
