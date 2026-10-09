# G5-D2: per-read rRNA vs Salmon transcript mapping concordance — 2026-10-09

**Scope:** Two existing systematic/competition subsamples of 10,000 reads each, tumor SRR4453804 and adjacent-normal SRR4453814. Both unmodified fastq inputs, GENCODE-v19/GRCh37.p13 full decoy-aware Salmon 2.8.0 k31 `--keepDuplicates`, SF, fld prior 150/50 and default minScoreFraction 0.65. Independent minimap2 2.31 short-read alignment to representative rRNA panel (18S,28S,mitochondrial rRNA etc). **No 32-run quantification.**

## Read-level output feasibility

Salmon 2.8.0 accepts `--writeMappings <SAM>` and `--writeUnmappedNames`. Executed identical pilot subset with both; `mappings.sam` and `aux_info/unmapped_names.txt` generated. Read sets were verified disjoint and exhaustive on both 10k FASTQ subsets. Read SAM alignments are candidate *transcript* alignments, and `unmapped_names` groups reads lacking an output candidate transcript alignment: neither file labels each read as Salmon genome decoy vs selective-alignment score-filtered. Consequently an exact per-read `Salmon decoy ∩ independent rRNA` comparison **is not identified by the available output**. Do not claim otherwise.

## Quantitative concordance

| Set (unique read IDs out of 10,000) | SRR4453804 tumor | SRR4453814 control |
|---|---:|---:|
| rRNA-panel alignment | 5,784 (57.84%) | 6,926 (69.26%) |
| Salmon SAM transcript-candidate mapped | 1,346 (13.46%) | 784 (7.84%) |
| Salmon successfully quantified fragments | 1,318 (13.18%) | 766 (7.66%) |
| rRNA-panel and SAM transcript-candidate | 146 (1.46%) | 78 (0.78%) |
| rRNA-panel and *no* SAM transcript candidate | **5,638 (56.38%)** | **6,848 (68.48%)** |
| neither rRNA panel nor SAM transcript candidate | 3,016 (30.16%) | 2,368 (23.68%) |

Check: `SAM transcript IDs` ∩ `Salmon unmapped IDs` = 0; their union = exactly 10,000 FASTQ IDs for both. Alignment records with multiple transcript hits are not independent fragments.

## Read-level quality and length

For rRNA+no-transcript-SAM category: tumor median read length 114 nt, mean PHRED 23.69; control median 106 nt, mean PHRED 23.54. For neither rRNA nor transcript-SAM category: tumor median 111 nt and mean PHRED 23.00; control median **80 nt** and mean PHRED **22.25**. This supports a length-related hypothesis for some residual non-mapping reads, especially in the control; no causality proven yet.

## Interpretation and decisions

1. A substantial rRNA-associated population, mostly 18S and 28S according to minimap2, appears among reads without a Salmon SAM transcript alignment.
2. This is **not proof** that the same reads are counted among Salmon's `num_decoy_fragments`. Decoy and low-score filtration counts must not be conflated or summed with candidate SAM counts as though they were disjoint by read identity.
3. Default minScoreFraction 0.65 remains: earlier 0.50/0.65/0.80 controlled sensitivity showed limited improvement from threshold relaxation.
4. Next discriminating check, if scientifically necessary to close G5, is a read-level competing map to genomic decoy vs transcriptome using a tool/trace that emits per-read competition and a read-length / mean-PHRED stratification. A transparent alignment-based proxy is not labeled Salmon's internal classification.
5. Keep 32-sample quantification blocked pending formal G4/G5 decisions. Do not remove rRNA, trim or modify original FASTQ; do not alter verified reference/index.

## Provenance

Local Linux sources:
- `/home/kingrider/GSE89223_download/qc/gse89223_raw_v1_20261009/rrna_read_mapping_v1/{SRR4453804,SRR4453814}_competition_10k.fastq`
- `/home/kingrider/GSE89223_download/qc/gse89223_raw_v1_20261009/G5_score_and_rrna_diagnostic_v1/{SRR4453804,SRR4453814}_rrna_primary.paf`
- `/home/kingrider/GSE89223_download/qc/gse89223_raw_v1_20261009/G5_D2_readlevel_v1/` — `mappings.sam`, per-sample `aux_info/unmapped_names.txt`, `readlevel_concordance_v1.json`.

G3 index `info.json` and both independent samples' real pilots had previously passed structural check. Read-level classifications use actual FASTQ/PAF/SAM names; source fastq and shared index remain unchanged.
