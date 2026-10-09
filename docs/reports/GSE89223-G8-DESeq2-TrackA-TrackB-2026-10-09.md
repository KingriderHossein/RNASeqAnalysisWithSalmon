# G8 DESeq2 exploratory primary analysis — 2026-10-09

## Status
**Computational runs completed for Track A and Track B, with structural/numerical validation PASS. Biological/QC interpretation is still REVIEW_REQUIRED.** Retain all samples.

## Reproducible method
- R 4.5.3 in `rnaseq` conda environment, DESeq2 1.50.2.
- Directly load G7 `tximport_gene_all32_v1.rds`; `DESeqDataSetFromTximport()` preserves estimated counts and effective-length-derived gene/sample normalization factors. **Never use TPM as DESeq2 count input.**
- Filter only all-zero genes within each track; run `DESeq()`, `results(alpha=.05)`, BH-adjusted `padj < .05`, no absolute log2FC threshold.
- Track A exactly 22 samples: 10 tumor vs 12 control, `~ group`, design rank 2/2.
- Track B exactly 18 samples from 9 matched patient pairs: `~ patient + condition`, design rank 10/10.
- No sample discarded for low Salmon mapping (26 low + 6 very low across full cohort). No post-hoc parameter optimization based on published DE overlap.
- Code: `scripts/g8_deseq2_trackA_trackB_v1.R` (current tree), with executed host copy in `audits/G7_tximport_20261009/g8_analysis_v1.R`.

## Actual numeric results

| Metric | Track A | Track B |
| --- | ---: | ---: |
| Samples | 22 | 18 |
| Genes with nonzero counts | 29,558 | 28,464 |
| All-zero genes excluded | 28,262 | 29,356 |
| Genes with raw p-values available | 29,558 | 28,464 |
| Genes with BH-adjusted p-values available | 17,516 | 15,219 |
| Adjusted p-value unavailable (independent filtering, etc.) | 12,042 | 13,245 |
| `padj < .05` DE genes | **2,093** | **1,850** |
| Positive log2 fold change (tumor vs reference) | 906 | 858 |
| Negative log2 fold change | 1,187 | 992 |
| PCA PC1 variance explained | 16.4% | 17.2% |
| PCA PC2 variance explained | 8.7% | 10.3% |

These are preliminary statistical findings. PCA labels/outliers and dispersion patterns **must be reviewed** before claiming biological validation. Low Salmon mapping remains a substantive limitation.

## Evidence and quality gates
- Executed on Linux at ~10:20–10:21 local time; launcher reports `G8_EXIT=0`. Host log: `/home/kingrider/GSE89223_download/audits/G8_DESeq2_20261009/run.log`.
- Completed both tracks; `ALL_G8_DONE` marker observed.
- Independent R RDS validation confirmed dimension/name reconciliation, `padj` threshold agreement, positivity/finite DESeq2 `normalizationFactors(dds)` matrices (29,558×22 and 28,464×18) and PNGs for PCA, dispersion, and sample distances. Output `validation.log` contains `G8_VALIDATION_PASS` and internal `VALIDATION_EXIT=0`. Remote wrapper tooling reports exit code 1 despite internal PASS, so its wrapper status should not be conflated with the R assertions.
- **Important correction:** For `DESeqDataSetFromTximport()`, `normalizationFactors(dds)` supplies a gene-by-sample matrix, and `sizeFactors(dds)` can be NULL. Consequently auto-written `size_factors.csv` contains no meaningful values; use the separately validated `normalization_factors_summary.csv` generated for each track. Future script revision should omit or replace misleading size-factor CSV.
- Outputs external to Git: `audits/G8_DESeq2_20261009/track_A/` and `track_B/`, each storing full DE result CSV/RDS, dds RDS, design matrix, PCA coordinates and plots, dispersion plot, distances and heatmap, and normalization factor summary. No original FASTQ/index/G6/G7 artifacts modified.

## Next science checks
1. Review PCA/sample-distance plots for group separation and scientific outliers (without silent deletion); inspect Cook's/dispersion/normalization diagnostics and independent filtering.
2. Cross-check G8 sample depth/mapping quality vs PCA locations and group composition; handle biological interpretation conservatively.
3. Keep Track A and B results separate, then conduct prescribed G9 validation and G10 published-paper comparison with traceable gene-ID harmonization, never using paper DEG overlap to tune processing.
