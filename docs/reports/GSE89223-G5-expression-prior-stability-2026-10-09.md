# G5 gene-expression usefulness: fragment-length prior sensitivity (2026-10-09)

**Scope:** Pilot-only. Two genuine full FASTQs: tumor SRR4453804 and adjacent-normal SRR4453814. No 32-run batch started.

## Real tool and index

- Salmon 2.8.0 via isolated, previously validated `rnaseq` conda env.
- Exact shared GRCh37.p13 + GENCODE v19 full-genome decoy-aware `--keepDuplicates` k31 index. Independent G3 validation PASS.
- Original pilot: `-l SF -p 6 --fldMean 150 --fldSD 50`.
- Controlled sensitivity: `-l SF -p 6 --fldMean 250 --fldSD 50`.
- **This experiment changes only the fragment-length PRIOR MEAN, 150 to 250**. It does not justify selecting either prior based on DEG results.
- New independent outputs under `/home/kingrider/GSE89223_download/audits/G5_expression_validity_20261009/{SRR4453804,SRR4453814}_SF_fld250_50_v1/`.
- Actual execution log: `01_fragment_prior_sensitivity.log`; exit code 0, successful structurally verified `quant.sf`, indexed target count 196,520, `frag_length_source=prior`, EM converged, no `quant_errors`.

## Mapping vs assignment

| Metric | Tumor | Adjacent normal |
| --- | ---: | ---: |
| Salmon input fragments | 5,575,676 | 7,940,368 |
| Salmon accepted transcript fragments | 749,597 (13.4441%) | 625,965 (7.8833%) |
| Genome-decoy assigned | 3,049,750 | 3,879,343 |
| Change in mapped fragments after prior shift | 0 | 0 |

**STAR genomic mapping:** N/A—not run. **HTSeq gene assignment:** N/A—not run. These are deliberately *different denominators and concepts*, to be measured separately in later comparator branch.

## Count-aggregation diagnostic (NOT tximport)

Read both pairs of `quant.sf` and project exact versioned transcript IDs through validated `derived/tx2gene.tsv`; no missing mappings among 196,520 references. Sum Salmon estimated `NumReads` per gene as a **sensitivity-only diagnostic**. This is neither normalized gene expression nor a replacement for `tximport` or `countsFromAbundance`.

| Diagnostic | SRR4453804 | SRR4453814 |
| --- | ---: | ---: |
| Gene IDs in tx2gene projection | 57,820 | 57,820 |
| Gene IDs with ≥10 estimated count in either run | 9,000 | 7,041 |
| Pearson r on log1p estimated gene counts for that subset | 0.995683 | 0.993657 |
| Top-100 genes by raw estimated count overlap | 100/100 | 100/100 |
| Genes changing at least twofold (with +1 pseudocount) | 11 | 7 |
| 95th percentile absolute log2 fold-change of gene raw-count estimates | 0.00349 | 0.00509 |
| Transcripts ≥10 estimated count | 10,975 | 7,091 |
| 95th percentile absolute log2 FC of transcript raw-count estimates | **1.0943** | **1.3835** |

Interpretation: gene-level aggregated estimated counts are largely stable to the single-end fragment-length prior in these two samples; transcript-level allocation is more sensitive. No sample-wide/DEG validity assertion without cross-sample QC, proper gene-level `tximport`, model/replicate diagnostics, and uncertainty evaluation. A high gene-count Pearson or 100/100 top-gene agreement does not establish biological reproducibility, statistical power, or adequacy of the few hundred thousand accepted reads.

## Still unresolved before unblocking G6

- G4 initial raw-input policy status `BASELINE_SELECTED_NOT_GATE_ACCEPTED`; acquisition/provenance issue #14 still must be reconciled for formal G4 close.
- G5 explicit configuration acceptance: set single-end length prior rationale and verify `-l SF` with protocol evidence; freeze bias options and output contract. Frag mean 150 remains provisional, with 250 a stress test, not an independently measured true library fragment length.
- Future G6: same index/software/preprocessing config, all 32 with monitoring and retained diagnostics; sample-level suitability QC and independent gene aggregation via `tximport` once all counts exist.
- **Do not run STAR or HTSeq yet**; comparison phase later, and keep their metrics separate from Salmon rate.

## Evidence / reproducibility

Host monitoring: `tail -f ~/GSE89223_download/audits/G5_expression_validity_20261009/01_fragment_prior_sensitivity.log`; analysis report `02_abundance_comparison.log`.
Scripts: `run_fragment_prior_v1.sh`, `compare_abundance_v1.py`; machine JSON `expression_sensitivity_summary_v1.json`.
Old original outputs unchanged; no index modification, no invented figures, no full 32-run yet.
