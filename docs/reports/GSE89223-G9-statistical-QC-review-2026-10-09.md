# G9 statistical/technical validation — initial audit, 2026-10-09

**Status:** Read-only analysis completed on authorized Linux machine using R 4.5.3 and DESeq2 1.50.2, exit 0. **G9 is not yet an unqualified biological PASS**: evaluate sample quality / batch confounding and biological marker directions before publication benchmarking.

## Observed evidence

- Track A: 22 samples (10 tumor / 12 control), 29,558 nonzero genes; 2,093 significant at BH padj <0.05.
- Track B: 18 samples (9 paired tumor/adjacent-normal), 28,464 nonzero genes; 1,850 significant.
- Intersection of significant genes: **1,437**, ALL with the same sign of log2FC. Note Track A/B overlap in sample membership, so these results **are not statistically independent replication**.
- PCA PC1 vs Salmon percent mapped, Spearman rho: **−0.521** Track A and **−0.434** Track B; associations are descriptive, not proven causality; PCs can have arbitrary sign.
- In each track two retained samples have Salmon transcript mapping below 10%: `SRR4453794` tumor (~9.83%) and `SRR4453814` adjacent-normal control (~7.88%). Neither has been excluded.
- Some strong PCA-distance candidates (ranked by median-centered 2-PC Euclidean distance): Track A `SRR4453805`, `SRR4453804`, `SRR4453800`, `SRR4453808`; Track B `SRR4453805`, `SRR4453804`, `SRR4453800`, `SRR4453814`. **These are candidate flags only**, not an objective multivariate outlier test or reason for sample deletion.
- DESeq2 per-gene dispersion-outlier flags: 355 (A) / 223 (B); *not* sample outliers.
- Raw p-values missing: 0 for both tracks; adjusted p-values absent (e.g., independent filtering) for 12,042 (A) / 13,245 (B), an expected mechanism needing transparent review rather than assuming these genes were all tested at BH significance. Contrast results' statistical scope must be stated.
- Cook's `NA` pvalue count is 0 in this run; this alone does not prove that no influential observations exist. The script captures a per-sample count of Cook's values above a generic exploratory F-distribution 99th percentile; **do not interpret this metric as DESeq2's formal sample exclusion test**.
- G6: 32/32 validated Salmon quantifications, original cohort 26 low mapping / 6 very low mapping warnings. Reference and tximport concordance passed at G6 and G7.

## QC interpretation and limits

Salmon warnings arise from quantification to annotated transcriptome with genomic decoys (not comparable directly with whole-genome STAR rate). The FFPE/rRNA-depleted Ion Torrent material has uneven read quality, length and possible abundant ribosomal transcripts. Rate differences may co-vary with PCA because of quality or biological composition. Avoid asserting contamination or technical causation without independent evidence. Preserve labels and all samples.

**No DESeq2 rerun, no modification of raw/index/G6/G7/G8, and no parameter tuning to match published DEG counts.** Track B is a required sensitivity analysis, with substantial same-direction overlap, not independent validation.

## Files
On host:
- `/home/kingrider/GSE89223_download/audits/G9_validation_20261009/run.log`
- `summary.tsv`
- `track_A_sample_qc.csv`, `track_B_sample_qc.csv`
- `track_A_B_significant_overlap.csv`
Reproducible audit code: `scripts/g9_validation_review_v1.R`.

## Remaining G9 actions
1. Inspect named PCA, dispersion and sample-distance plots; examine clustering relative to tumor/control labels, mapping and paired identity without excluding samples.
2. Examine independent filtering, Cook's influence and normalization-factor diagnostics in context.
3. Biological marker sanity check PCA3, AMACR, ANKRD34B, NEK5, KCNG3, PTPRT using exact GENCODE v19 gene IDs, preserving annotation provenance.
4. Then reconcile Issue #10 G9 gate. Continue to G10 publication benchmarking only with explicit unresolved QC caveats and verified published comparator file.
