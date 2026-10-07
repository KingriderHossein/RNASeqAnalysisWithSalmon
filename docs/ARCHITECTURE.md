# Analysis Architecture v1.1

Status: **technical baseline for the Salmon-first workflow**

This document owns the stable technical architecture of the GSE89223 reanalysis. GitHub Issues own live task state. `docs/PROJECT-SPEC.md` owns project-level intent.

## 1. Architectural objective

Reanalyse GSE89223 with a workflow whose **primary quantification path is Salmon**:

```text
verified cohort
    -> raw single-end reads
    -> read QC
    -> version-pinned reference bundle
    -> decoy-aware Salmon index
    -> Salmon transcript quantification
    -> tximport gene summarization
    -> DESeq2
    -> gene-level comparison with the published study
```

The original STAR -> HTSeq -> edgeR workflow is **not** part of the execution path. It is the external comparator used at the final interpretation stage.

## 2. Core invariants

1. Salmon is the only primary transcript quantifier.
2. No sample reaches full quantification before metadata, reference, and single-end parameter gates pass.
3. All samples in one analysis track use the same reference bundle, Salmon index, preprocessing policy, and quantification configuration.
4. `tx2gene` must come from the same annotation release used to define the indexed transcripts.
5. TPM values are not supplied directly to DESeq2.
6. Raw sequencing data and Salmon indexes stay outside Git.
7. The paper-comparison analysis and sensitivity analyses remain separate.
8. Changes to the primary reference, cohort, or Salmon parameter set after full quantification require an explicit documented rerun decision.

## 3. Analysis tracks

### Track A — primary paper-comparison analysis

Purpose: maximize interpretability of differences between this Salmon workflow and the published analysis.

- Cohort: the verified paper-comparison cohort defined in `metadata/README.md` and `metadata/derived/GSE89223_sample_manifest.tsv`.
- Reference context: GENCODE Release 19 / GRCh37.p13, matching the original paper's hg19 + GENCODE 19 context as closely as practical.
- Output: the primary gene-level DEG result used for comparison with the publication.

### Track B — paired PCa sensitivity analysis

Purpose: test robustness in verified matched PCa tumor/adjacent-normal pairs.

- Cohort: the verified matched-pair cohort defined in `metadata/README.md` and `metadata/derived/GSE89223_sample_manifest.tsv`.
- DE design: paired model, conceptually `~ patient + condition`.
- Uses the same Salmon reference/index/quantification outputs where samples overlap Track A.
- Remains secondary and must not replace Track A in the paper-comparison report.

A modern-reference GRCh38 analysis is optional future work and is not required for project completion.

## 4. Stage architecture and gates

### Stage 0 — cohort and provenance lock

Inputs:
- GEO GSE89223
- SRA SRP092131
- BioProject PRJNA350714
- paper and supplementary information

Outputs:
- machine-readable sample manifest
- explicit biological groups and patient pairing
- explicit inclusion/exclusion fields
- named analysis cohorts

Gate G0:
- cohort identities are explicit and reproducible
- no sample grouping is inferred from file order or name prefix alone

Current state: Issue #1 established this layer.

### Stage 1 — reproducible environment

Required components:
- Salmon 2.x, exact version pinned at execution
- FastQC
- MultiQC
- raw-read retrieval tool
- R
- tximport
- DESeq2
- packages/scripts required to construct `tx2gene`

Gate G1:
- Salmon binary identity and exact version are verified
- command semantics are taken from Salmon 2.x documentation when behavior differs from legacy 1.x
- downstream R package versions are recorded

### Stage 2 — raw-read acquisition

The verified manifest is the only source of run accessions.

For each approved run:
- retrieve the raw single-end read file
- record accession, source, file size, and integrity evidence
- keep raw reads outside Git

Gate G2a:
- every downloaded file maps to exactly one manifest row
- integrity checks pass

### Stage 3 — raw-read QC and preprocessing decision

Run:
- FastQC per sample
- MultiQC across the analysis cohort

Policy:
- trimming is evidence-driven, not automatic
- if trimming is required, define one reproducible rule before full quantification
- if trimming is performed, run post-trim QC

Gate G2b:
- QC is reviewed
- the project records either `no trimming required` or the exact preprocessing rule
- sample exclusion requires a documented reason independent of desired DE results

## 5. Stage 4 — reference bundle and Salmon index

### Primary reference bundle

Use matching files from **GENCODE Release 19 / GRCh37.p13**:

1. official transcript FASTA for the indexed transcript set
2. matching comprehensive GTF for transcript-to-gene relationships
3. matching genome FASTA for decoy sequences

Do not regenerate transcript FASTA from GTF/genome unless the official transcript FASTA is unavailable or a documented incompatibility requires it.

Record:
- source URLs
- release/build identifiers
- checksums
- sequence-name compatibility
- transcript identifier/version-suffix policy

### Decoy-aware Salmon index

```text
GENCODE v19 transcript FASTA
        +
matching GRCh37 genome FASTA as decoys
        -> decoy-aware target
        -> salmon index
```

Salmon 2.x selective alignment is the primary mapping mode. The matching genome is used as decoy sequence.

Initial k-mer choice: `k=31`. The study read lengths are predominantly above the range for which Salmon documents k=31 as a normal choice. If pilot mapping is unexpectedly poor and short-read content is implicated, an alternate k-mer index may be evaluated as a technical sensitivity test. Indexes must never be mixed within one analysis track.

Gate G3:
- reference checksums are recorded
- transcript/GTF identifiers reconcile
- decoy list and index command are reproducible
- finished index reports the expected reference identity

## 6. Stage 5 — Salmon quantification

GSE89223 runs are single-end Ion Torrent Proton RNA-seq.

### Library type

Pilot quantification uses `-l A` so Salmon can infer the library type. The detected type must be recorded from Salmon metadata/logs.

If library-preparation documentation and observed behavior disagree, resolve the discrepancy before full quantification.

### Single-end fragment-length distribution

For single-end reads Salmon cannot empirically recover the fragment-length distribution from paired mappings. Therefore `--fldMean` and `--fldSD` are a **pre-full-run technical decision gate**.

Policy:
1. seek library-preparation evidence for expected fragment size
2. define a plausible primary prior
3. run a small pilot sensitivity analysis over plausible alternative priors
4. judge technical stability using mapping/assignment and abundance diagnostics, not downstream DE significance
5. freeze one parameter set before full quantification

Do not tune fragment-length parameters to maximize agreement with the paper.

### Bias correction

The pilot evaluates the planned bias-correction configuration. The primary candidate configuration includes Salmon sequence- and GC-bias correction where compatible with Salmon 2.x and this dataset. Final flags are frozen before the full run and recorded in Salmon metadata plus project configuration.

### Pilot

Before full quantification, run a small representative pilot containing tumor and normal samples.

Pilot acceptance:
- library type is resolved
- fragment-length prior is frozen
- mapping/assignment statistics are credible
- no systematic input/reference incompatibility is visible
- valid `quant.sf` and Salmon metadata are produced

Gate G4:
- one immutable Salmon quantification configuration is approved for full execution

### Full quantification

Each sample gets an independent output directory:

```text
external_results/salmon/<SAMPLE>/
  quant.sf
  cmd_info.json
  aux_info/...
```

Gate G5:
- all expected samples complete with one index and one parameter configuration
- Salmon QC metrics are summarized
- failed or anomalous samples are resolved before R analysis

## 7. Stage 6 — transcript-to-gene import

Primary route:

```text
quant.sf files
    -> tximport(type = "salmon", tx2gene = ...)
    -> gene-level counts + abundance + effective-length information
    -> DESeqDataSetFromTximport(...)
```

Rules:
- construct `tx2gene` from the exact GENCODE v19 GTF used by the reference bundle
- transcript ID/version handling must be explicit
- use tximport/DESeq2 integration instead of feeding TPM directly to DESeq2
- verify imported sample order against the manifest

Gate G6:
- all Salmon files import successfully
- transcript-to-gene mapping losses are quantified and investigated
- sample names exactly match metadata

## 8. Stage 7 — DESeq2

Track A and Track B get separate DESeq2 objects/designs.

Required pre-DE checks:
- sample/library summaries
- PCA
- sample-distance structure
- dispersion diagnostics
- documented outlier review

Primary output includes:
- gene identifier
- optional gene symbol
- baseMean
- log2FoldChange
- standard error
- p-value
- adjusted p-value

Filtering and significance thresholds are declared before interpreting agreement with the original paper.

Protein-coding-only reporting, if required, is a downstream annotation filter and does not silently redefine the indexed transcriptome.

Gate G7:
- DE design is explicit
- contrasts are reproducible
- QC/outlier decisions are documented

## 9. Stage 8 — comparison with the original paper

Only after the Salmon -> tximport -> DESeq2 result is complete:

```text
our gene-level results
        vs
published STAR -> HTSeq -> edgeR results
```

Compare where evidence allows:
- number of DE genes
- gene overlap
- concordance of direction
- fold-change correlation
- top-ranked genes
- pathway/function enrichment
- differences attributable to cohort, annotation, quantifier, or statistical method

The project does **not** rerun STAR/HTSeq/edgeR unless a future task explicitly adds that scope.

## 10. Repository and storage architecture

Version-controlled:

```text
README.md
docs/
  PROJECT-SPEC.md
  ARCHITECTURE.md
metadata/
scripts/
config/
environment/
results/
  qc_summary/
  salmon_summary/
  deseq2/
  comparison/
```

Not version-controlled:

```text
raw FASTQ/SRA
reference genome/transcript FASTA
Salmon indexes
large temporary/intermediate data
package caches
```

Recommended external working layout:

```text
<project-data>/
  raw/
  references/
  indexes/
  salmon/
  qc/
  tmp/
```

GitHub Issues are the source of truth for active work:

```text
Issue -> branch -> implementation/evidence -> PR -> review -> main
```

## 11. Issue-to-stage map

- #1 — Stage 0: metadata and cohort definition
- #2 — Stage 1: reproducible environment
- #3 — Stage 4: reference bundle and Salmon index
- #4 — Stages 2-3: raw reads and QC
- #5 — Stage 5: pilot + full Salmon quantification
- #6 — Stages 6-8: tximport, DESeq2, and paper comparison

The stage numbers describe pipeline order. Issue numbers are management identities and need not match stage numbers.

## 12. Evidence used for this review

- Original study: Nikitina et al. 2017, PMCID PMC5464844
- Salmon 2.x documentation: https://combine-lab.github.io/salmon/
- tximport Bioconductor vignette
- DESeq2 Bioconductor vignette

This architecture changes only when new evidence changes a scientific or reproducibility assumption, not for routine progress.
