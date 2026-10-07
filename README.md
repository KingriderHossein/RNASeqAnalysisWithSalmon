# RNASeqAnalysisWithSalmon

Reproducible reanalysis of **GSE89223 / SRP092131 / PRJNA350714** using Salmon for transcript quantification, gene-level summarization with tximport/tximeta, and differential expression with DESeq2.

## Scientific question

How do gene-level differential-expression results from a Salmon-based workflow compare with the original study pipeline:

`FastQC -> Cutadapt -> STAR (hg19) -> HTSeq -> edgeR`?

The original study is:

> Nikitina AS et al. *Novel RNA biomarkers of prostate cancer revealed by RNA-seq analysis of formalin-fixed samples obtained from Russian patients.* Oncotarget. 2017. DOI: 10.18632/oncotarget.16518.

## Primary data

- GEO: GSE89223
- SRA: SRP092131
- BioProject: PRJNA350714
- Organism: Homo sapiens
- Platform: Ion Torrent Proton
- Material: FFPE prostate cancer / adjacent-normal and BPH tissues
- Public SRA record: 32 experiments, approximately 37 Gbases / 28.8 GB archived data

## Planned workflow

```text
GEO/SRA metadata
      |
      v
verified sample manifest
      |
      v
raw reads -> FastQC/MultiQC
      |
      v
reference transcriptome -> Salmon index
      |
      v
Salmon quantification
      |
      v
tximport / tximeta
      |
      v
DESeq2 gene-level analysis
      |
      v
comparison with original STAR/HTSeq/edgeR results
```

## Project management

GitHub Issues are the source of truth for active work:

1. #1 Validate GSE89223 metadata and build sample manifest
2. #2 Define reproducible computational environment
3. #3 Build and verify human transcriptome Salmon index
4. #4 Retrieve raw reads and run pre-quantification QC
5. #5 Quantify GSE89223 with Salmon
6. #6 Gene-level DESeq2 analysis and comparison with the original study

See [docs/PROJECT-SPEC.md](docs/PROJECT-SPEC.md) for the durable project scope.

## Data policy

Raw sequencing data, reference indexes, large intermediate files, and generated caches are **not committed to Git**. Only metadata, code, small reproducibility artifacts, summaries, and final analysis outputs appropriate for version control belong in the repository.
