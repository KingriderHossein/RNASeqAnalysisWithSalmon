# Getting started

Guide version: **1.0.0**.

## 1. Read the results

Start with the [scientific summary](reports/GSE89223-final-advisor-scientific-summary-2026-10-09.md). For interactive figures, download [the report HTML](dashboard/index.html) with GitHub's **Download raw file** control and open it in a browser.

The dashboard includes its data and JavaScript. It requests Vazirmatn from Google Fonts; without network access, the browser uses a fallback font. The report is a dated snapshot, not a live connection to analysis data.

## 2. Inspect the code

Requirements for repository-only checks: **Git and Python 3.10 or newer**. Bash is also needed for the shell syntax check used in CI. These are not the full scientific environment requirements.

```bash
git clone https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon.git
cd RNASeqAnalysisWithSalmon
python3 scripts/check_repository_v1.py
```

This command reads tracked project files. It checks Python syntax, JSON, SVG, relative Markdown file links and cohort-manifest invariants. It does not import the analysis scripts, modify their results or contact a data service.

To check shell syntax from the repository root:

```bash
for script in scripts/*.sh; do
  bash -n "$script" || exit 1
done
```

These checks do not execute Salmon or R and do not establish scientific gate acceptance.

## 3. Acquire raw SRA data (Stage 4)

The **Stage 1** manifest/cohort gate is distinct from **Stage 4** raw-data acquisition. Use the [Stage 4 acquisition guide](RAW-ACQUISITION.md) to export the 32 approved SRR IDs from the manifest and download them using either Ubuntu Bash or Windows PowerShell with WSL2 Ubuntu. The two entry points share one NCBI SRA Toolkit engine. Both offer resume/retry, local SHA-256 records and logs; neither extracts FASTQ or silently replaces existing study inputs.

~~~bash
python3 scripts/acquisition/manifest_runs.py --output raw/acquisition/GSE89223_runs.txt
bash scripts/acquisition/sra_downloader_v1.5.0.sh --run SRR4453804 --output "$HOME/gse89223/sra" --dry-run
~~~

No data download or scientific acceptance gate runs in CI. Extract FASTQ separately, verify sample identity, checksums and read counts, and satisfy G4 before dependent work.

## 4. Prepare a scientific rerun

**The current stage scripts document a workstation workflow. They are not yet a portable installer or one-command pipeline.** In particular, several scripts refer to `/home/kingrider/GSE89223_download`, and the G6 runner reads an external audited configuration. The committed configuration record is not automatically its runtime input.

Before a rerun:

1. Read [the specification](PROJECT-SPEC.md), [architecture](ARCHITECTURE.md), [decisions](DECISIONS.md) and [validation gates](VALIDATION.md).
2. Resolve the [environment work](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/2) and verify stage-specific tool versions. G7 and G8 reports record different R environments; do not combine them without validation.
3. Confirm the manifest and both cohorts. Keep raw reads and large references outside the repository.
4. Reconcile exact FASTQ, genome, annotation, index and tx2gene identities with the recorded checksums and provenance.
5. Adapt paths in a reviewed, versioned change. Choose a new analysis ID and output location; retain prior results.
6. Execute a dependent stage only after its input gates are satisfied. Retain commands, software versions, logs, checksums and outputs.

See [Reproducibility](REPRODUCIBILITY.md) for known gaps and the evidence needed to close them.

## Script map

| Stage | Entry point | Required context |
| :--- | :--- | :--- |
| Metadata | [fetch_metadata.sh](../scripts/fetch_metadata.sh), [build_manifest.py](../scripts/build_manifest.py) | GEO/SRA metadata; rebuilding writes the derived manifest |
| Reference audit | [validate_reference_g3.py](../scripts/validate_reference_g3.py), [g3_validate_keepduplicates_v1.py](../scripts/g3_validate_keepduplicates_v1.py) | Matching GENCODE v19 reference and index evidence |
| Pilot | [g5_pilot_SRR4453804_v1.sh](../scripts/g5_pilot_SRR4453804_v1.sh) | Audited inputs, reference and G4/G5 decisions |
| Quantification | [g6_resumable_v1.py](../scripts/g6_resumable_v1.py) | External config, accepted gates, exact index and FASTQ checksums |
| Gene-level import | [g7_import_gene_all32_v1.R](../scripts/g7_import_gene_all32_v1.R) | All 32 Salmon outputs and matching tx2gene |
| Differential expression | [g8_deseq2_trackA_trackB_v1.R](../scripts/g8_deseq2_trackA_trackB_v1.R) | G7 object, DESeq2 environment and frozen cohort designs |
| Review | [g9_validation_review_v1.R](../scripts/g9_validation_review_v1.R), [g9_marker_pca_review_v1.R](../scripts/g9_marker_pca_review_v1.R) | Per-track outputs and QC evidence |
| Publication comparison | [g10_publication_benchmark_v1.py](../scripts/g10_publication_benchmark_v1.py), [g10_discordance_biotype_v2.py](../scripts/g10_discordance_biotype_v2.py) | Official supplement, full results and openpyxl |

The [workstation monitor script](../scripts/pipeline_monitor_v2.py) also has local path dependencies. The portable report under `docs/dashboard/` can be viewed without running that server.
