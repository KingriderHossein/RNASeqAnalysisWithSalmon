# Reproducibility and evidence boundaries

Document version: **1.0.0**. Inventory checked against the repository on **2026-10-09**.

The repository records a completed computational analysis through publication comparison. A fresh checkout does **not yet contain everything needed for an independently verified end-to-end rerun**. G9 biological/scientific acceptance also remains open.

## What is available

| Component | Repository evidence | Boundary |
| :--- | :--- | :--- |
| Cohort identity | [32-run manifest](../metadata/derived/GSE89223_sample_manifest.tsv), [provenance](../metadata/README.md) | Track A and Track B have separate membership and designs |
| Scientific contract | [Specification](PROJECT-SPEC.md), [decisions](DECISIONS.md), [gates](VALIDATION.md) | These define acceptance; reports alone do not close gates |
| Stage code | [Python, Bash and R scripts](../scripts/) | Several retain workstation-specific paths |
| Salmon config record | [salmon_g6_raw_SF_v1.json](../config/salmon_g6_raw_SF_v1.json) | Records candidate scope and absolute paths; G6 reads an external config |
| Reference and execution evidence | [Dated reports](reports/README.md) | Full audit logs and large files remain outside Git |
| Results presentation | [Summary](reports/GSE89223-final-advisor-scientific-summary-2026-10-09.md), [dashboard](dashboard/index.html) | Derived snapshots; not the complete machine-readable result archive |

## Recorded environments

These versions are reported execution evidence, **not an installation lockfile**:

| Stage | Recorded software | Source |
| :--- | :--- | :--- |
| Quantification | Salmon 2.8.0 | [Scientific summary](reports/GSE89223-final-advisor-scientific-summary-2026-10-09.md) |
| G7 import | R 4.6.1; tximport 1.40.0 | [G7 report](reports/GSE89223-G7-tximport-gene-level-2026-10-09.md) |
| G8 analysis | R 4.5.3; DESeq2 1.50.2 | [G8 report](reports/GSE89223-G8-DESeq2-TrackA-TrackB-2026-10-09.md) |

Do not assume a single environment reproduces both R stages. The G7 and G8 reports also distinguish successful internal checks from ambiguous wrapper exit codes. Closure needs unambiguous retained execution evidence.

## Remaining closure work

| Work | Why it matters | Tracking |
| :--- | :--- | :--- |
| Commit a verified environment recreation specification | Package names alone do not fix versions, channels or binary identities | [#2](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/2) |
| Make paths and runtime inputs portable | Existing absolute paths and external configs prevent a clean-checkout rerun | [#12](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/12) |
| Reconcile the committed candidate config with accepted runtime provenance | Readers must be able to identify the exact executed configuration | [#12](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/12) |
| Complete biological/QC interpretation | Low mapping, quality warnings and sample structure limit interpretation | [#10](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/10) |
| Publish compact result/provenance artifacts and verify a clean rerun | All reported values must trace to inputs, software, analysis ID and commit | [#12](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/12) |

Keep raw FASTQ/SRA, full reference sequences, indexes and large intermediate objects outside Git. Record their approved locations, checksums and access requirements in the reproducibility package. See [data contracts](DATA-CONTRACTS.md).

## What CI establishes

[Repository checks](../.github/workflows/repository-checks.yml) run offline checks after checkout:

- Python syntax without importing or running scientific scripts;
- Bash syntax without executing commands;
- JSON parsing and SVG XML parsing;
- relative Markdown file-link existence, including reference-style links;
- unique sample/run IDs, 32 total runs, Track A membership counts, and Track B paired structure.

They do not run R, validate external URLs or Markdown heading anchors, recompute results, establish environment equivalence, assess read quality, or accept a scientific gate. Use the stage-specific validation requirements for those claims.
