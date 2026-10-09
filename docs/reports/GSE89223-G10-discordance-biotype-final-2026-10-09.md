# G10.2 final discordance and biotype analysis (2026-10-09)

## Inputs, method and coverage

Primary paper's authentic supplementary worksheet `results_de_plus`, `oncotarget-08-32990-s002.xlsx`, 3,384 FDR-significant genes, SHA-256 `f97ec7428b1bf6077f173ae3f13384fe86c140bb17c437096dba164b9a589e34`. An independent Track A `DESeq2` table (`audits/G8_DESeq2_20261009/track_A/deseq2_full_results.csv`), 29,558 nonzero genes and 2,093 BH-significant genes. Exact gene biotypes mapped from **matching GENCODE v19** `derived/transcript_biotypes.tsv`; canonical versioned Ensembl IDs retained and version stripped only in temporary comparison keys.

Other source supplements `s003` (TCGA comparison), `s004` (miRNA enrichment), `s005` (transcription factor enrichment) and `s006` (TCGA exclusions) inspected; **none provides the article's complete nonsignificant gene-level test results**. Thus we cannot compute the publication's p-values or effect sizes for the 357 ours-only genes and cannot compute genuine global all-tested-gene agreement. No values are imputed.

## Confirmed counts and reconciliation

| Test | Count |
| --- | ---: |
| Publication significant | 3,384 |
| Our Track A significant | 2,093 |
| Significant in both | 1,736 |
| Publication only | 1,648 |
| Ours only | 357 |
| Union | 3,741 |
| Jaccard | 0.464047 |
| Paper significant IDs found among our 29,558 nonzero gene set | 3,380 |
| Direction agreement over those 3,380 genes | **3,340/3,380 = 98.8166%** |
| Pearson published vs ours log2FC for those 3,380 | **0.963512** |
| Spearman published vs ours log2FC for those 3,380 | **0.953083** |
| Direction agreement among 1,736 significant in both | 100% (selection-conditioned) |

The broadened 3,380-gene calculation **still selects genes by the paper's statistical significance**, not all tested genes. Large correlations remain descriptive.

### Publication-only 1,648 gene classification in our analysis

| Auditable status | Genes |
| --- | ---: |
| Present; our **nominal** p-value < 0.05 but BH padj >= 0.05 | **826** |
| Present; our nominal p-value >= 0.05 | **803** |
| Present with unavailable BH-adjusted p-value (independent filtering possible; specific per-gene reason not proved) | **15** |
| Absent from our nonzero/tested gene set | **4** |
| **Total** | **1,648** |

This is an observable statistical classification, **NOT** validated causal attribution of differences to Salmon, gene expression thresholds, library QC or statistical model. For the 357 ours-only genes the paper's nonsignificant effect sizes/p-values are **unknown**, not zero.

### Ensembl v19 gene biotypes

| Category | Paper significant | Our significant | Significant in both |
| --- | ---: | ---: | ---: |
| protein_coding | **3,013** | **1,778** | **1,535** |
| All non-protein_coding gene types combined | **371** | **315** | **201** |
| **Total** | **3,384** | **2,093** | **1,736** |

The 357 ours-only significant genes consist of 243 protein_coding and 114 non-protein_coding, including 65 snoRNA, 17 lincRNA, 14 pseudogene and 12 antisense among other types. Publication labels and GENCODE v19 labels are different annotation systems; use consistent version-aware IDs and describe type differences without treating class names as guaranteed 1:1.

## Interpretation / limitations

- Strong overlap and consistent direction support **partial reproducibility of gene-level findings**, not identity of methods or superior performance. The article's list is not error-free ground truth, and the proportion of our DE genes overlapping with it is **not precision or specificity**.
- Divergence plausibly stems from differences in genome versus transcriptome quantification, STAR/HTSeq/edgeR vs Salmon/tximport/DESeq2, effective-length modeling, statistical independent filtering, dataset handling and FFPE/Ion Torrent read behavior. These are **candidate explanations, not proven causal allocations**.
- All 32 FastQC reports have per-base quality FAIL; original Salmon mapping rates are low with heavy genome decoy assignments. Prior G9 marked this limitation REVIEW_REQUIRED; these warnings remain even though quantitative benchmarking now succeeded.
- Track B's 9 paired-patient sensitivity results overlap Track A in patients and cannot count as independent validation. No repeated Salmon/DESeq2 runs, sample removals or post-hoc parameter tuning.

## Artifacts and verification

Reproducible committed code `scripts/g10_discordance_biotype_v2.py`.

On authorized host:
`/home/kingrider/GSE89223_download/audits/G10_publication_20261009/g10_discordance_v2/`:
- `discordant_gene_classification.csv` (3,741 per-gene union records, explicit category / publication and our data / gene biotype).
- `discordance_summary.json` (counts, gene-type distribution, effects agreement).
- `../06_discordance.log` and `../07_validation.log`, which show **VALIDATION_PASS** for 3,741 unique IDs, correct 1,736/1,648/357 partition, biotype totals and 3,340/3,380 directions.

## Gate decision

**G10 numerical, identifier, biotype and discordance benchmark = COMPLETE** with explicit publication-data limitations. **G9 scientific QC remains REVIEW_REQUIRED** and should be disclosed in downstream presentations. No unsupported guarantee of biologic sample quality or attribution to a single aligner is implied.
