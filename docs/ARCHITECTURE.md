# Project Architecture v2.0 — FINAL

Status: **FROZEN BASELINE**

This is the canonical technical map for the GSE89223 project. Detailed scientific decisions, data schemas, validation rules, and governance live in the linked architecture documents.

## Canonical architecture set

- [PROJECT-SPEC.md](PROJECT-SPEC.md) — goal, scope, deliverables, completion
- [ARCHITECTURE.md](ARCHITECTURE.md) — end-to-end scientific stage model\n- [AUTOMATION-PIPELINE.md](AUTOMATION-PIPELINE.md) — modular execution/orchestration architecture
- [DECISIONS.md](DECISIONS.md) — frozen scientific/engineering decisions
- [DATA-CONTRACTS.md](DATA-CONTRACTS.md) — schemas, identities, provenance
- [VALIDATION.md](VALIDATION.md) — stage gates and rejection rules
- [GOVERNANCE.md](GOVERNANCE.md) — GitHub workflow and change control
- [../metadata/README.md](../metadata/README.md) — verified sample/cohort provenance

## 1. Scientific objective

Reanalyse:

`GSE89223 / SRP092131 / PRJNA350714`

with the primary workflow:

```text
FASTQ
→ QC
→ Salmon
→ tximport
→ DESeq2
→ validation
→ comparison with original publication
```

The publication's:

```text
FastQC → Cutadapt → STAR → HTSeq → edgeR
```

workflow is a **comparator only** and is not rerun in the primary project.

## 2. Dataset identity

GSE89223 is:
- human prostate tissue;
- FFPE;
- Ion Torrent Proton;
- single-end;
- total-RNA / rRNA-depleted RNA-seq;
- not strict Poly(A)+ mRNA-seq.

Therefore the primary analysis is a **whole-transcriptome expression analysis**. The primary reference must preserve non-protein-coding transcript classes represented in the publication.

## 3. Analysis tracks

### Track A — publication-comparison

Purpose: compare with the paper's final DE analysis.

Cohort:
- 10 tumor;
- 12 control.

Control composition:
- 9 PCa adjacent-normal;
- 1 normal BPH tissue;
- 2 hyperplastic BPH tissues.

Design:

```r
~ group
```

Contrast:

```text
tumor vs control
```

### Track B — paired PCa sensitivity

Purpose: test robustness using matched patients.

Cohort:
- 9 matched PCa patients;
- 9 tumor;
- 9 adjacent-normal;
- BPH controls excluded;
- unpaired CP2 excluded.

Design:

```r
~ patient + condition
```

Contrast:

```text
tumor vs adjacent_normal
```

Track A and Track B are separate analyses and must never be silently mixed.

## 4. End-to-end architecture

```text
Stage 0  Architecture freeze
   ↓
Stage 1  Sample / cohort lock
   ↓
Stage 2  Reproducible environment
   ↓
Stage 3  Reference bundle
   ↓
Stage 4  Raw reads + QC / preprocessing decision
   ↓
Stage 5  Salmon pilot
   ↓
Stage 6  Full Salmon quantification
   ↓
Stage 7  tximport / gene-level import
   ↓
Stage 8  DESeq2
   ↓
Stage 9  Validation
   ↓
Stage 10 Publication benchmark
   ↓
Stage 11 Final report / reproducibility closure
```

Dependent stages cannot be accepted before their required gate passes.\n\nThe execution/orchestration layer that progressively automates these stages is defined in [AUTOMATION-PIPELINE.md](AUTOMATION-PIPELINE.md). The first implementation module is the resumable cross-platform Acquisition / Download Manager.

## 5. Stage 0 — Architecture freeze

### Inputs
- project objective;
- dataset choice;
- publication;
- current repository state.

### Required outputs
- project specification;
- architecture;
- decision record;
- data contracts;
- validation architecture;
- governance model.

### Gate G0
Pass when:
- question is explicit;
- tracks are explicit;
- reference strategy is explicit;
- Salmon strategy is explicit;
- statistical designs are explicit;
- storage/provenance are explicit;
- completion criteria are explicit.

## 6. Stage 1 — Sample / cohort lock

### Authoritative inputs
- GEO GSE89223;
- SRA SRP092131;
- BioProject PRJNA350714;
- paper and supplements.

### Identity chain

```text
GSM → SRX → SRR → BioSample → patient → diagnosis → tissue → analysis group
```

### Canonical artifact

`metadata/derived/GSE89223_sample_manifest.tsv`

### Rules
- no grouping from filename order;
- no grouping from prefix alone;
- no biological relabeling from PCA;
- no post-hoc relabeling to improve agreement.

### Gate G1
Pass when:
- all included runs map to metadata;
- Track A membership is explicit;
- Track B membership is explicit;
- pairing is explicit;
- exclusions are documented;
- metadata discrepancies remain visible.

## 7. Stage 2 — Reproducible environment

### Required tool families
CLI:
- Salmon 2.x;
- FastQC;
- MultiQC;
- raw-read retrieval tool;
- checksum utilities;
- reference construction utilities.

R/Bioconductor:
- R;
- tximport;
- DESeq2;
- annotation/parsing packages;
- plotting/reporting packages used by the final workflow.

### Rules
- exact versions recorded;
- exact binary identity recorded;
- install source/channel recorded;
- environment specification committed;
- version-sensitive options checked against exact installed version.

### Salmon compatibility
Salmon 1.x indexes must not be reused with Salmon 2.x.

### Gate G2
Pass when the environment is reproducible and executable identities are unambiguous.

## 8. Stage 3 — Reference bundle

### Primary context
- GRCh37.p13 / hg19-era benchmark context;
- GENCODE Release 19 comprehensive GTF;
- matching GRCh37.p13 genome;
- comprehensive transcript FASTA derived from genome + comprehensive GTF;
- matching genome decoys;
- tx2gene derived from same GTF;
- gene/transcript biotype mappings from same GTF.

### Why comprehensive
The publication's DE result included protein-coding and non-coding/other classes. A protein-coding-only reference would change the biological search space and invalidate the primary benchmark.

### Decoy-aware index

```text
comprehensive transcript FASTA
+
matching genome sequences
+
decoys.txt
→ Salmon selective-alignment index
```

Primary mode:
- selective alignment;
- decoy-aware;
- no `--sketch`.

Primary k-mer behavior:
- normal k=31 baseline unless read-length QC justifies a separate sensitivity index.

### Reference identity must include
- source URLs;
- checksums;
- build/release;
- derivation commands;
- transcript/gene counts;
- biotype summary;
- tx2gene checksum;
- decoy checksum;
- index identity/hash;
- Salmon version used for index construction.

### Gate G3
Pass when all reference components reconcile and no annotation-release mixing exists.

## 9. Stage 4 — Raw data + QC

### Raw input
Only run accessions approved by the manifest.

### Storage
Raw FASTQ/SRA are immutable external inputs and stay outside Git.

### Per-sample provenance
Record:
- accession;
- source;
- local path;
- size;
- checksum;
- read/spot count;
- acquisition provenance.

### QC
FastQC per sample + MultiQC across cohort.

Review:
- read count;
- read-length distribution;
- per-base quality;
- per-sequence quality;
- GC distribution;
- adapters/overrepresented sequences;
- duplication indicators;
- abnormal composition.

### Dataset-specific expectations
- FFPE degradation;
- variable single-end read lengths;
- possible fragmentation/3′ bias;
- Ion Torrent homopolymer/indel-related error characteristics.

These require review, not automatic sample exclusion.

### Trimming
Conditional only:

```text
QC acceptable → retain raw reads
QC shows material adapter/quality issue → predefined Cutadapt rule → post-trim QC
```

No trimming rule is tuned against DE overlap.

### Gate G4
Pass when raw identity/integrity is verified and preprocessing policy is frozen.

## 10. Stage 5 — Salmon pilot

Purpose: freeze the technical Salmon configuration before the full run.

### Pilot sample selection
Use a small representative tumor/control subset with ordinary depth/quality characteristics.

Selection must not depend on desired biological outcome.

### Pilot resolves
- library type;
- version-specific single-end fragment-length behavior;
- sequence/GC/positional bias options;
- reference compatibility;
- mapping/assignment diagnostics;
- required retained outputs.

### Single-end fragment length
Do not blindly copy legacy `--fldMean`/`--fldSD` recipes.

At execution time:
- inspect exact Salmon 2.x behavior;
- record fragment-length metadata emitted by Salmon;
- use a biologically plausible prior only if the exact version supports/requires it;
- sensitivity-test technical assumptions when justified.

No fragment-length choice may be selected by maximizing paper agreement.

### Gene aggregation
Primary gene aggregation is downstream in tximport, not via Salmon `--geneMap`.

### Inferential replicates
Not required for the primary gene-level tximport → DESeq2 project.

### Required retained Salmon outputs
At minimum:
- `quant.sf`;
- `cmd_info.json`;
- `lib_format_counts.json`;
- `aux_info/meta_info.json`;
- `aux_info/ambig_info.tsv`;
- `libParams/flenDist.txt`;
- `logs/salmon_quant.log`.

### Gate G5
Pass only when one primary Salmon configuration is frozen in version-controlled configuration.

## 11. Stage 6 — Full Salmon quantification

### Invariants
Every sample in one analysis_id uses:
- one reference bundle;
- one index;
- one preprocessing policy;
- one Salmon version;
- one frozen primary configuration.

### Quantification manifest
Records:
- analysis_id;
- sample_id;
- SRR;
- input checksum;
- preprocessing ID;
- reference ID;
- index ID/hash;
- Salmon version;
- config ID/hash;
- output path;
- completion/warning/exclusion state.

### Hard failures
Examples:
- non-zero execution;
- missing/malformed `quant.sf`;
- wrong index;
- sample/input mismatch;
- fatal Salmon diagnostic.

### Review-required warnings
Examples:
- mapping-rate outlier;
- unexpected library type;
- high decoy fraction;
- anomalous read count;
- unusual fragment-length metadata.

Warnings do not automatically remove samples.

### Gate G6
Pass when every expected sample is quantified or explicitly excluded with justified provenance.

## 12. Stage 7 — tximport / gene level

### Primary route

```text
quant.sf × samples
→ tximport(type="salmon", tx2gene=...)
→ gene-level counts + abundance + length information
→ DESeqDataSetFromTximport(...)
```

### Rules
- tx2gene comes from the exact GENCODE v19 GTF;
- transcript version handling is explicit;
- sample order must match metadata;
- unmatched transcripts are quantified;
- TPM is not used directly as DESeq2 count input.

### Gate G7
Pass when the gene-level object is complete, traceable and internally consistent.

## 13. Stage 8 — DESeq2

### Track A
`design = ~ group`

Reference: control  
Contrast: tumor vs control

### Track B
`design = ~ patient + condition`

Reference: adjacent_normal  
Contrast: tumor vs adjacent_normal

### Primary DEG threshold

```text
BH-adjusted p-value < 0.05
```

No mandatory absolute log2FC threshold is added to the primary benchmark definition.

### Filtering
- all-zero genes may be removed;
- documented independent filtering may be used;
- arbitrary post-hoc filters are prohibited.

### Visualization transforms
VST/rlog-style transformed values may be used for PCA, heatmaps and distances, but not as raw DE input.

### LFC shrinkage
Optional for reporting/ranking; it does not redefine primary significance.

### Outliers
Sample deletion is never automatic.

### Gate G8
Pass when model matrix, metadata order, contrasts, PCA, sample distances, dispersion and outlier diagnostics are reviewed.

## 14. Stage 9 — Validation

Four layers:

### Technical
- input integrity;
- QC;
- reference consistency;
- index identity;
- Salmon completion;
- library type;
- assignment/mapping diagnostics;
- tximport completeness.

### Statistical
- PCA;
- sample distances;
- dispersion;
- normalization factors;
- p-value/padj behavior;
- independent filtering;
- outlier diagnostics.

### Biological sanity
Post-analysis only.

Examples:
- PCA3;
- AMACR;
- ANKRD34B;
- NEK5;
- KCNG3;
- PTPRT.

These markers must never tune the workflow.

### Sensitivity
- Track B required;
- additional technical sensitivity only when technically justified.

### Gate G9
Pass when no unresolved critical inconsistency remains.

See [VALIDATION.md](VALIDATION.md).

## 15. Stage 10 — Publication benchmark

### Primary comparator
Paper final 22-sample DE analysis / Supplementary Table 1.

Published headline:
- 3,384 DE genes at FDR < 0.05;
- 3,013 protein-coding;
- 371 non-coding/other;
- 1,490 upregulated;
- 1,894 downregulated.

### Identifier harmonization
- preserve internal canonical IDs;
- create separate normalized Ensembl gene-ID comparison key;
- strip version suffixes only in comparison key;
- symbols are secondary.

### Required comparison metrics
1. tested-gene count;
2. significant-gene count;
3. overlap;
4. union;
5. Jaccard;
6. published-DE recovery fraction;
7. direction concordance;
8. comparable log2FC correlation;
9. comparable rank correlation;
10. biotype composition;
11. top-gene comparison;
12. marker consistency;
13. pathway comparison if performed.

### Difference attribution categories
- cohort;
- reference/annotation;
- transcript-vs-genome quantification;
- multimapping/gene-family;
- effective length / isoform composition;
- DE statistical model;
- FFPE degradation;
- Ion Torrent profile;
- preprocessing;
- unresolved.

### Important limitation
This project compares complete pipelines:

```text
STAR/HTSeq/edgeR
vs
Salmon/tximport/DESeq2
```

It does not isolate Salmon-vs-STAR effects without a separate future factorial bridge analysis.

### Gate G10
Pass when required metrics and methodological explanations are complete.

## 16. Stage 11 — Final report / closure

Final report includes:
1. project question;
2. dataset description;
3. whole-transcriptome/rRNA-depleted caveat;
4. cohort construction;
5. reference construction;
6. QC decision;
7. Salmon configuration;
8. Salmon diagnostics;
9. tximport strategy;
10. Track A results;
11. Track B results;
12. technical validation;
13. statistical validation;
14. biological sanity checks;
15. publication benchmark;
16. causes of agreement/disagreement;
17. limitations;
18. reproducibility identity;
19. conclusion.

## 17. Reproducibility identity

Every final result must resolve to:

```text
sample manifest
+ cohort revision
+ input checksum
+ reference checksum
+ index identity
+ Salmon version/config
+ tx2gene identity
+ R/Bioconductor versions
+ DE design
+ repository commit
= analysis_id
```

Different analysis IDs must never be mixed silently.

## 18. Storage architecture

External heavy workspace:

```text
<project-data>/
├── raw/
├── references/
├── indexes/
├── qc/
├── salmon/
├── intermediate/
├── logs/
└── tmp/
```

Repository target:

```text
RNASeqAnalysisWithSalmon/
├── README.md
├── AGENTS.md
├── docs/
│   ├── PROJECT-SPEC.md
│   ├── ARCHITECTURE.md
│   ├── DECISIONS.md
│   ├── DATA-CONTRACTS.md
│   ├── VALIDATION.md
│   └── GOVERNANCE.md
├── metadata/
├── config/
├── environment/
├── scripts/
├── results/
│   ├── qc_summary/
│   ├── salmon_summary/
│   ├── track_a/
│   ├── track_b/
│   └── comparison/
└── reports/
```

Heavy raw/reference/index/intermediate data remain outside Git.

## 19. Configuration architecture

Planned configuration surfaces:

```text
config/
├── project.yaml
├── cohorts.tsv
├── reference.yaml
├── preprocessing.yaml
├── salmon.yaml
├── deseq2.yaml
└── comparison.yaml
```

Scientific decisions belong in configuration, not hidden script constants.

## 20. Failure classes

### BLOCKER
Stops dependent work.

### REVIEW_REQUIRED
Requires explicit review before acceptance.

### INFO
Recorded but non-blocking.

Exact validation examples and rejection rules are defined in [VALIDATION.md](VALIDATION.md).

## 21. Non-goals

Primary completion does not require:
- rerunning STAR/HTSeq/edgeR;
- replacing GSE89223 with TCGA;
- treating the paper as ground truth;
- protein-coding-only reference;
- exact reproduction of 3,384 DE genes;
- relabeling samples from clustering;
- GRCh38 rerun;
- transcript-level DE;
- inferential-replicate DE;
- storing raw data in Git;
- visualization without an analytical question.

## 22. Architecture change control

Material changes require:
- a decision update in [DECISIONS.md](DECISIONS.md);
- architecture/spec update when relevant;
- review through PR before dependent execution.

## 23. Completion definition

Project complete means:
- cohort provenance complete;
- environment reproducible;
- reference pinned/checksummed;
- raw integrity verified;
- QC/preprocessing frozen;
- Salmon pilot frozen;
- full quantification complete;
- tximport validated;
- Track A complete;
- Track B complete;
- validation passed;
- publication benchmark complete;
- disagreements explained;
- compact provenance/config/results committed;
- final report reproducible.

A successful Salmon run alone is **not** project completion.

## 24. Architecture freeze

Architecture v2.0 is the execution baseline.

New evidence may justify revision, but every material revision must be explicit, reviewable and traceable.
