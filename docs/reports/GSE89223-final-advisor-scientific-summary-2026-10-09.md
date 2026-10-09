# Final scientific advisor summary — GSE89223 (2026-10-09)

## Status
**Computational reanalysis COMPLETE through G10; biological/scientific G9 acceptance remains REVIEW_REQUIRED.** No parameter tuning to maximize published-paper agreement, no post-hoc sample removal. This summary is derived from a fresh on-host read-only cross-check of G6–G10 results (`audits/G11_final_report_20261009/01_evidence_check.log`, `final_scientific_facts_v1.json`).

## Data and workflow
- GSE89223, 32 FFPE prostate single-end Ion Torrent RNA-seq samples.
- Source/reference GRCh37.p13 + GENCODE v19, full-genome decoy-aware Salmon 2.8.0, keepDuplicates, k31; 32/32 valid quantification outputs.
- 340,724,809 processed reads; 46,825,370 transcript-mapped (weighted 13.743%).
- G7: 196,520 versioned transcripts, 57,820 gene-level rows across 32 samples using tximport.
- DESeq2 Track A: 22 (10 tumor / 12 control), ~group, 2,093 BH-significant DE genes (906 up, 1,187 down).
- DESeq2 Track B: 18 matched samples / nine patients, ~patient + condition, 1,850 BH-significant DE genes (858 up, 992 down).

## Original publication benchmark
- Source Nikitina et al., 2017, PMC5464844; actual official `oncotarget-08-32990-s002.xlsx` from NCBI PMC OA, SHA-256 `f97ec7428b1bf6077f173ae3f13384fe86c140bb17c437096dba164b9a589e34`.
- Published FDR-significant: 3,384 (1,490 up, 1,894 down); our Track A: 2,093.
- Significant intersection **1,736**; union **3,741**; published recovery **51.30%**; Jaccard **0.464**; 82.94% of our significant genes occur in the published-significant list.
- Among all 3,380 published-significant genes with our fold changes available, 3,340 (98.82%) agree in direction and Pearson log2FC r ≈ 0.964. This agreement is still **conditional on significance in the paper**, not whole-gene concordance or clinical validity.
- Published-only 1,648: 826 with our nominal p <0.05 but BH not significant, 803 nominal nonsignificant, 15 padj missing, 4 not in our nonzero gene set. The official table does not provide nonsignificant paper results for our 357 ours-only genes.
- Protein-coding among published/ours/shared significant: 3,013 / 1,778 / 1,535. Complementary non-protein-coding: 371 / 315 / 201.
- All six designated markers PCA3, AMACR, ANKRD34B, NEK5, KCNG3, PTPRT show positive tumor fold change in both tracks, five of six BH-significant in both; Track B NEK5 padj ~0.057.

## Scientific QC constraints
- FastQC per-base quality **FAIL 32/32** and Adapter Content **PASS 32/32**.
- Low Salmon transcriptome mapping and genomic decoy fraction; six samples below 10% mapping. Salmon transcript mapping is **not directly comparable** with STAR whole-genome mapping.
- Partial group separation in PCA; C13 pair SRR4453804/3805 distinct; PC1 mapping Spearman ~−0.521 (A), −0.434 (B). No demonstrated causal batch factor or sample exclusion.
- Track A/B overlap in specimens and are not independent biological validation.
- Pipelines differ in multiple stages: published Cutadapt → STAR → HTSeq → edgeR compared with our Salmon → tximport → DESeq2. Differences **cannot** be attributed exclusively to Salmon.

## Conclusion and decision
The completed workflow reproduces an informative subset of the published gene-level signal with high direction agreement in the comparable published-significant subset. The computational result is verifiable and reusable; unqualified biological validation remains open under Issue #10 (G9 scientific QC). Issue #11 (G10 publication benchmark) was closed after numeric/biotype/discrepancy reconciliation.

## Authoritative evidence
- `docs/reports/GSE89223-G8-DESeq2-TrackA-TrackB-2026-10-09.md`
- `docs/reports/GSE89223-G9-fastqc-pca-marker-validation-2026-10-09.md`
- `docs/reports/GSE89223-G10-publication-benchmark-2026-10-09.md`
- `docs/reports/GSE89223-G10-discordance-biotype-final-2026-10-09.md`
- `/home/kingrider/GSE89223_download/audits/G11_final_report_20261009/final_scientific_facts_v1.json`

The Persian advisor-ready Word/PDF report is delivered as separate ChatGPT files; this GitHub Markdown version keeps the durable executive findings accessible.
