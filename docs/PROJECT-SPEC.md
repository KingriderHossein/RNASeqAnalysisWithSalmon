# Project Specification

## Outcome

Reanalyse the public prostate-cancer RNA-seq study GSE89223 with a reproducible **Salmon -> tximport -> DESeq2** workflow and compare the resulting gene-level differential-expression results with the original publication.

The stable technical workflow and decision gates are defined in [ARCHITECTURE.md](ARCHITECTURE.md).

## Primary execution path

```text
raw reads
-> FastQC / MultiQC
-> Salmon
-> tximport
-> DESeq2
-> gene-level result
```

The original paper's STAR -> HTSeq -> edgeR pipeline is not rerun as part of the primary workflow. It is used as an external comparator after the Salmon-based result is complete.

## Comparison context from the original paper

The paper used:

```text
FastQC
-> Cutadapt
-> STAR against hg19
-> HTSeq with GENCODE release 19
-> edgeR
```

The comparison must distinguish effects caused by:
- sample inclusion/exclusion
- reference/annotation release
- transcript-level versus genome-alignment quantification
- gene summarization
- statistical model/software
- FFPE/Ion Torrent data characteristics

## Authoritative study identifiers

- GEO: GSE89223
- SRA Study: SRP092131
- BioProject: PRJNA350714
- PMID: 28380430
- PMCID: PMC5464844
- DOI: 10.18632/oncotarget.16518

## Verified study facts at project start

The study contains 32 GEO/SRA experiments generated on Ion Torrent Proton. The verified manifest records biological group, patient identifier, SRA run, pairing state, and paper inclusion/exclusion provenance for every sample.

The SRA runs are single-end. Therefore, single-end library type and fragment-length assumptions are explicit technical gates in the Salmon architecture rather than hidden defaults.

The published analysis reports 3,384 differentially expressed genes at FDR < 0.05 after its own filtering/grouping procedure.

## Success criteria

The project is complete only when:

1. Every analysed sample is traceable from GEO sample to SRA run and biological group.
2. The primary reference transcriptome/genome/annotation bundle is version-pinned and checksummed.
3. Raw-read retrieval and QC are reproducible.
4. The single-end Salmon pilot resolves library type and freezes the fragment-length/bias configuration.
5. Salmon quantification completes for the approved analysis cohort using one index and one configuration.
6. Transcript-to-gene summarization uses annotation consistent with the indexed transcriptome.
7. DESeq2 designs and contrasts are explicit and reproducible.
8. The primary prostate-cancer tumor-versus-control result is generated.
9. The Salmon-based gene-level results are compared with the published study where evidence permits.
10. Important disagreements are analysed rather than hidden.
11. Commands, code, environment information, provenance, and compact outputs needed to reproduce the analysis are version controlled.

## Non-goals

- Reproducing every secondary analysis in the paper.
- Rerunning STAR/HTSeq/edgeR as part of the primary pipeline.
- Treating BPH samples as interchangeable with PCa adjacent-normal tissue without an explicit analysis definition.
- Using a modern GRCh38 reference in the primary paper-comparison track.
- Storing raw FASTQ/SRA files, reference FASTA files, Salmon indexes, or large intermediates in Git.

## Data safety

Large data and indexes stay outside the repository. Any local deletion of raw data must occur only after required derived outputs and integrity checks are verified. Existing unrelated user data must never be overwritten or cleaned for this project.

## Project truth

- Durable project intent: this file
- Stable technical architecture: `docs/ARCHITECTURE.md`
- Active tasks/blockers: GitHub Issues
- Implementation and analysis history: commits and pull requests
- Runtime evidence: analysis outputs and logs tied to relevant code/configuration versions
