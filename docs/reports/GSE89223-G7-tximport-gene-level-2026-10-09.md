# G7 Salmon tximport gene-level import — 2026-10-09

## Execution and package
- Source: 32 complete, validated G6 Salmon 2.8.0 `quant.sf` files, GRCh37.p13 + GENCODE v19 comprehensive, unchanged `SF` raw single-end configuration.
- R 4.6.1 (Ubuntu), `tximport` 1.40.0 installed **only** under `/home/kingrider/GSE89223_download/audits/G7_tximport_20261009/lib`. Official Bioconductor 3.23 source tarball SHA-256 `72841a6b2ac1f64ccb05af2d9fba4eea781596771798cefcb3a9327f9dc929f9`.
- Reference `/home/kingrider/GSE89223_download/reference/G3_GENCODEv19_GRCh37p13/derived/tx2gene.tsv`; all 196,520 versioned transcript IDs matched, zero missing. Full 32 `quant.sf` files checked for exactly identical sets and order of transcript IDs.
- `tximport::tximport(files,type="salmon",tx2gene=txmap,ignoreTxVersion=FALSE,countsFromAbundance="no",dropInfReps=TRUE)`.
- Initial run failed safely before output generation because inferred replicate reading wanted `jsonlite`; the corrected run sets `dropInfReps=TRUE` (no inferential replicates needed for the designated gene-level DESeq2 route). No source quant changed.

## Observed G7 result
- Full 32-sample import produced matrices of **57,820 genes × 32 samples**: estimated gene counts, gene TPM, sample-specific average effective lengths.
- Track A: **22 samples**, 10 tumor + 12 control, design to be `~ group` at G8.
- Track B: **18 samples**, nine tumor/adjacent-normal patient pairs, design to be `~ patient + condition` at G8.
- Identical sample order verified against complete manifest and selected canonical runs.
- Matrices contain finite, nonnegative values. Gene-count column sums match independently recomputed sums of transcript `NumReads` exactly (reported max delta 0); not an assertion that count sums equal all raw reads.
- Outputs retained at `/home/kingrider/GSE89223_download/audits/G7_tximport_20261009/result_v1/`: `tximport_gene_all32_v1.rds`, `estimated_gene_counts_all32_v1.csv`, `gene_tpm_all32_v1.csv`, `average_effective_lengths_all32_v1.csv`, cohort metadata, per-sample QC, and session info.
- Primary R import script is in this repository at `scripts/g7_import_gene_all32_v1.R`; local exact executed file additionally at `.../g7_import_v1.R`. Source in repository removes a tautological no-op check before future re-execution.

## Scientific boundaries
- This is **gene-level import**, not DEG analysis; there is no DESeq2 run and no sample was excluded for low Salmon mapping.
- 26 `low_mapping_rate` and 6 `very_low_mapping_rate` warnings on G6 remain **review-required** for downstream scientific interpretation, even though G6 files passed structural validation.
- These estimated gene counts are NOT TPM and should not be passed as raw count replacement without the tximport/DESeq2 integration contract. At G8 use the `tximport` RDS through `DESeqDataSetFromTximport()`, preserving the `length` matrix, rather than passing TPM or manually rounded CSV.
- DESeq2 and other R dependencies not installed on the workstation during this step.
- Import log: `.../02_import_retry.log` contains `TXIMPORT_PASS genes=57820 columns=32` and `G7_DONE`. Independent postimport script prints `G7_POSTIMPORT_PASS 57820 genes 32 samples` and max count delta 0; remote shell wrapper reported exit code 1 despite all R assertions succeeding, so the wrapper exit behavior needs a clean rerun before marking fully closure-grade.

No original data or index overwritten; the created matrices are a derived 39MB G7 artifact.
