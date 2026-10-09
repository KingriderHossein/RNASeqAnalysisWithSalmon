# G5 controlled score-threshold and rRNA panel diagnostic — 2026-10-09

**Owner issue:** #5. **Reference:** GENCODE v19 / GRCh37.p13 full-genome-decoy Salmon 2.8.0 `--keepDuplicates` k31, same immutable index in every run. **Scope:** 10,000 existing deterministic representative reads per sample from prior G4 competitive-stratified subsets, NOT 32-sample quantification.

## Experimental design

Two biologically independent accessions: tumor `SRR4453804`, control `SRR4453814`. Three Salmon selective-alignment runs per input, `-l SF --fldMean 150 --fldSD 50` held constant, varying only `--minScoreFraction` among `0.50`, **`0.65` default**, `0.80`. Input FASTQ source and index remain immutable. Separate independent minimap2 v2.31 `-x sr --secondary=no` alignment against six mature human nuclear/mitochondrial rRNA representative sequences on each same sample of 10,000 reads.

## Observed results

| Run | score threshold | mapped / 10000 | mapped % | decoy / 10000 | decoy % | Salmon filtered_vm |
|---|---:|---:|---:|---:|---:|---:|
| SRR4453804 tumor | 0.50 | 1450 | 14.50 | 5967 | 59.67 | 2126 |
| SRR4453804 tumor | 0.65 | 1318 | 13.18 | 5491 | 54.91 | 2736 |
| SRR4453804 tumor | 0.80 | 1221 | 12.21 | 5107 | 51.07 | 3218 |
| SRR4453814 control | 0.50 | 847 | 8.47 | 5539 | 55.39 | 2861 |
| SRR4453814 control | 0.65 | 766 | 7.66 | 4869 | 48.69 | 3616 |
| SRR4453814 control | 0.80 | 692 | 6.92 | 4418 | 44.18 | 4144 |

Independent rRNA-only alignments on **the same 10,000 reads**:

| Run | uniquely named reads with primary rRNA-panel alignment | percent | MAPQ>=20 reads | percent |
|---|---:|---:|---:|---:|
| SRR4453804 | 5784 | 57.84 | 5195 | 51.95 |
| SRR4453814 | 6926 | 69.26 | 5972 | 59.72 |

Most rRNA alignments involve 28S and 18S. The rRNA panel is representative rather than comprehensive, lacks all rDNA variants, and alignment-to-panel is *not synonymous with biochemical contamination*. Decoy classification by Salmon is based on full-genome competition and is *not* proof of rRNA identity; the near proportions are a mechanistically consistent signal but do not imply exact per-read overlap.

## Interpretation and decision

Relaxing score threshold from `0.65` to `0.50` increased 10k-subset transcript mapping by only **132 reads / 1.32 percentage points** (tumor) and **81 reads / 0.81 percentage points** (control), while also changing decoy and filtered counts. A more permissive threshold may admit lower-confidence alignments; **do not relax `minScoreFraction` globally** based on raw mapping rate alone.

Independent rRNA panel supports a substantial rRNA-associated signal, particularly in the control; the persistence of low mapping across thresholds and both samples argues against the strictness of score threshold as the main reason for low mapping. However it does **not** distinguish exactly which Salmon-decoy reads are the rRNA-panel reads. The scientifically clean next discriminating analysis is **per-read concordance** across the same subset: categorize Salmon decoy/transcript/score-filter read IDs (if the installed version exposes a documented read-level trace) against rRNA-panel PAF IDs, without equating alignment multiplicities with fragment counts. If Salmon cannot expose explicit per-fragment decoy classification, retain this as an aggregate diagnostic, do not invent its concordance.

## Acceptance status

- Both full-sample pilot outputs remain technically valid and `SF` matches both sample orientation diagnostics.
- Default 0.65 score threshold remains the baseline; raw FASTQ, validated shared index, decoy policy and G4 evidence preserved.
- **G4 / G5 formal scientific acceptance remains OPEN. 32-sample quantification has not been started.**
- Outputs on user machine: `/home/kingrider/GSE89223_download/qc/gse89223_raw_v1_20261009/G5_score_and_rrna_diagnostic_v1/` (per-run `quant.sf`, Salmon logs, independent rRNA PAF/logs and `threshold_results_v1.json`).
