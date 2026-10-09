# G9 validation extension: real FastQC, PCA and prostate-cancer markers (2026-10-09)

## Actual execution

All analysis read existing G6/G7/G8 artifacts, using R 4.5.3 from existing `rnaseq` environment plus standard Python. Input evidence and execution logs are on Linux:
- `audits/G9_closure_20261009/01_annotation_fastqc.log`
- `audits/G9_closure_20261009/05_marker_pca_review_v4.log`
- `audits/G9_closure_20261009/fastqc_salmon_samples_v1.csv`
- `audits/G9_closure_20261009/marker_annotation_v19.csv`
- `audits/G9_closure_20261009/markers_track_A_v2.csv` and `markers_track_B_v2.csv`
- `audits/G9_closure_20261009/pca_qc_correlations_track_A_v2.csv` and `...track_B_v2.csv`

Reproducible source committed to `scripts/g9_fastqc_marker_annotation_v1.py` and `scripts/g9_marker_pca_review_v1.R`.

## FastQC and technical limitations

For **all 32** samples FastQC 0.13.0:
- per-base sequence quality = **FAIL 32/32**;
- Adapter Content = **PASS 32/32**;
- input FastQC read counts exactly agree with accession manifest and Salmon input counts.
Thus low-quality bases are cohort-wide; not attributable merely to an adapter-content warning. FFPE and Ion Torrent caveats still apply, and no sample has been excluded.

Notable real samples (rate = Salmon transcript mapping, not whole-genome STAR):
- `SRR4453804` tumor (patient C13): mapping **13.44%**, decoy **54.70%**;
- `SRR4453805` adjacent-normal (patient C13): mapping **13.04%**, decoy **56.96%**;
- `SRR4453794` tumor (C8): mapping **9.83%**, decoy **42.33%**;
- `SRR4453814` adjacent-normal (B3): mapping **7.88%**, decoy **48.86%**.

## PCA and technical-variable covariation

Manual inspection of Track A/B PCA plots and sample-distance heatmap indicates **partial tumor/control separation**, not perfect separation. `SRR4453804` and `SRR4453805` (matched C13 pair) are distinctive in both PCA representations, so patient-specific biology, FFPE effects and technical influences all remain plausible; no diagnosis of contamination and no exclusion justified.

Spearman rho for PCA PC1 vs Salmon transcript mapping:
- Track A: **−0.521**
- Track B: **−0.434**

PC1 vs decoy fraction:
- A: **+0.380**
- B: **+0.657**

PC1 vs G+C fraction:
- A: **+0.506**
- B: **+0.405**

These are **exploratory** correlations, with limited sample sizes and arbitrary PC orientation. They are not proof of batch effects/causation, and must not motivate post-hoc model changes to raise DEG count or paper overlap.

## Biological marker sanity checks

Marker mapping was obtained exclusively from the **matching GENCODE v19 GTF**, exact unique gene_name→versioned gene_id (one match per symbol). The recorded differential-expression values for tumor vs comparison are:

| Marker | Track A log2FC | Track A BH padj | Track B log2FC | Track B BH padj |
| --- | ---: | ---: | ---: | ---: |
| PCA3 | +4.63 | 3.1e-10 | +4.69 | 1.2e-11 |
| AMACR | +3.98 | 4.0e-9 | +4.70 | 4.2e-11 |
| ANKRD34B | +5.24 | 1.2e-6 | +4.46 | 0.0011 |
| NEK5 | +3.55 | 0.0029 | +2.94 | **0.057 (not significant)** |
| KCNG3 | +3.54 | 1.7e-5 | +3.33 | 0.0086 |
| PTPRT | +3.38 | 3.4e-5 | +2.74 | 0.0024 |
| KLK3 (additional context) | +0.52 | 0.24 (not significant) | +0.50 | 0.068 (not significant) |

Five prespecified markers pass padj <0.05 in **both** tracks. All six have concordant positive direction. The 1437 intersecting significant genes are direction-concordant but the tracks share specimens and cannot be treated as external validation.

## Gate status and next

The **technical/statistical audit and targeted biological marker checks have been executed**, but G9 should remain **REVIEW_REQUIRED**, not a claim of unqualified biological validity: global FastQC FAIL, high genomic decoy fractions, low Salmon mapping, PC1 correlation with technical parameters and sample C13 PCA peculiarity need explicit interpretation. The observed marker directions support plausibility but do not eliminate these limitations.

No raw reads/index/Salmon quant/tximport/DESeq2 outputs modified. No post-hoc sample exclusion or DEG rerun; publication comparison remains the distinct G10 step.
