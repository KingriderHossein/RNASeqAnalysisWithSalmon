# G10: Auditable comparison with Nikitina et al. (2017), 2026-10-09

## Source authority and identity

- Original publication: *Novel RNA biomarkers of prostate cancer revealed by RNA-seq analysis of formalin-fixed samples obtained from Russian patients* (2017), GSE89223, PMC5464844, [Oncotarget article](https://www.oncotarget.com/article/16518/text/).
- **Actual publication DEG table**: `oncotarget-08-32990-s002.xlsx`, worksheet `results_de_plus`, 3,384 data rows (3,385 including header), columns `gene_name, ensemble_id, logFC, PValue, FDR, type, chr, start, end, strand`. Official NCBI PMC OA new public bucket file: `https://pmc-oa-opendata.s3.amazonaws.com/PMC5464844.1/oncotarget-08-32990-s002.xlsx`; SHA-256 `f97ec7428b1bf6077f173ae3f13384fe86c140bb17c437096dba164b9a589e34`.
- Distinguish `s003.xlsx` (worksheet `common_tcga_us_full`, TCGA cross-reference) from the primary published DEG list; not used for primary comparison.
- Our result: frozen GSE89223 **Track A** only, `audits/G8_DESeq2_20261009/track_A/deseq2_full_results.csv` (DESeq2 1.50.2 and tximport GENCODE v19, BH `padj<0.05`). Track B remains a separate paired sensitivity analysis.
- Ensembl version suffix removed **only in a dedicated comparison key**, retaining canonical versioned project gene IDs; publication comparison `ensemble_id` and our `gene_id` are Ensembl gene identifiers. Unique ID and all publication FDR<0.05 checks passed.

## Results actually computed

| Metric | Real result |
| --- | ---: |
| Published significant DE genes | **3,384** (1,490 up, 1,894 down) |
| Our Track A significant DE genes | **2,093** |
| Published IDs that are in our nonzero/tested gene universe | **3,380** |
| Shared significant IDs | **1,736** |
| Published significant IDs not significant in ours | **1,648** (includes four absent from our nonzero gene universe) |
| Our significant IDs not significant in publication | **357** |
| Significant-ID union | **3,741** |
| Jaccard = intersection / union | **0.464047** |
| Recovery = overlap / published significant | **0.513002** |
| Fraction of our significant IDs also significant in publication | **0.829431** |
| Direction agreement among 1,736 shared significant IDs | **1,736 / 1,736 = 100%** |
| Direction disagreements in shared significant IDs | **0** |
| Pearson correlation of log2FC on shared significant IDs | **0.980662** |
| Spearman correlation of log2FC on shared significant IDs | **0.969037** |
| Published protein-coding DE genes | **3,013** |

All six prespecified prostate marker genes `PCA3`, `AMACR`, `ANKRD34B`, `NEK5`, `KCNG3`, `PTPRT` appear among shared statistically significant genes. Other markers, including KLK3, must not be chosen post hoc to tune outputs.

## Audit and caveats

**Independent audit PASS:** 3,741 rows in the ID-union comparison CSV, exactly 3,384 publication and 2,093 ours significant entries, 1,736 shared significant rows with concordant signs, and publication SHA intact.

Interpretation limitations:
1. Concordance statistics on **selected significant overlap** are conditional on selection; they are not genome-wide agreement, and high correlation could be inflated by selection.
2. `our precision as overlap fraction` is descriptive overlap, **not classifier precision or ground-truth accuracy**. Publication list is not ground truth.
3. The pipeline comparison includes preprocessing differences and **STAR → HTSeq → edgeR** vs **Salmon → tximport → DESeq2**, reference/annotation, abundance estimation and statistical modeling differences. It **cannot assign any observed difference specifically to Salmon vs STAR**.
4. 32/32 raw samples had FastQC per-base sequence quality FAIL, substantial genomic-decoy assignment and low Salmon transcriptome mapping; G9 flags remain review-required. Full cohort has six samples under 10%, and two of these are present in Track A; do not silently discard or retune.
5. Track B (9 matched pairs) is important required sensitivity, but should not be mixed into the paper's Track A comparator.
6. Gene-type breakdown **for our DEG list** and exhaustive attribution of the discordant gene sets are pending; avoid declaring G10 completely closed until those are documented.

## Reproduction and durable artifacts

On the authorized host:
- `/home/kingrider/GSE89223_download/audits/G10_publication_20261009/reference/oncotarget-08-32990-s002.xlsx`
- `/home/kingrider/GSE89223_download/audits/G10_publication_20261009/03_benchmark.log`
- `.../04_independent_validation.log`
- `.../benchmark_summary.json`
- `.../gene_level_comparison.csv` (union rows, publication/ours identity and direction columns)
- `.../g10_compare_v1.py` (also source under GitHub `scripts/g10_publication_benchmark_v1.py`)

No raw FASTQ, Salmon index/output, tximport or DESeq2 result was altered for benchmark agreement.
