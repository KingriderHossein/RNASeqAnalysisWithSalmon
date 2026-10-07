# Validation Architecture — v2.0

This document defines the validation gates and severity rules.

## Severity classes

### BLOCKER
Dependent work stops.

Examples:
- corrupt or missing input;
- ambiguous sample identity;
- wrong reference;
- incompatible index;
- malformed Salmon output;
- tx2gene/reference mismatch;
- invalid statistical design.

### REVIEW_REQUIRED
Requires explicit review before dependent acceptance.

Examples:
- mapping-rate outlier;
- unexpected library type;
- PCA outlier;
- high decoy fraction;
- unusual fragment-length behavior.

### INFO
Record but do not block.

Examples:
- expected depth variation;
- benign runtime warnings;
- non-material timing differences.

## G0 — Architecture validation

Must verify:
- scientific question;
- Track A and Track B;
- reference decision;
- Salmon strategy;
- statistical designs;
- data/storage model;
- validation plan;
- completion definition.

## G1 — Cohort validation

Checks:
- all included SRRs exist in manifest;
- GSM/SRR relationships reconcile;
- Track A = 22 samples;
- Track A = 10 tumor + 12 control;
- Track B = 18 samples;
- Track B = 9 patients;
- no BPH sample in Track B;
- no paper-excluded sample silently re-enters Track A;
- every exclusion has a reason.

## G2 — Environment validation

Checks:
- exact Salmon version;
- exact Salmon executable identity;
- FastQC/MultiQC versions;
- raw-read retrieval-tool version;
- R version;
- tximport version;
- DESeq2 version;
- environment recreation specification;
- install source/channel when relevant.

Unexpected command resolution is a blocker.

## G3 — Reference validation

Checks:
- GRCh37.p13 identity;
- GENCODE v19 comprehensive GTF identity;
- source checksums;
- genome/GTF chromosome naming compatibility;
- transcript extraction success;
- unique transcript identifiers;
- transcript count;
- gene count;
- gene/transcript biotype distribution;
- tx2gene mapping completeness;
- decoy list correctness;
- Salmon index identity;
- no Salmon 1.x index reuse with Salmon 2.x.

## G4 — Raw/QC validation

Per sample:
- expected file exists;
- checksum/provenance recorded;
- read count plausible;
- read-length distribution recorded;
- no file corruption/truncation;
- FastQC complete.

Cohort:
- MultiQC complete;
- adapter/quality issues reviewed;
- preprocessing decision frozen.

Sample exclusion requires independent justification and review.

## G5 — Salmon pilot validation

Must demonstrate:
- successful quantification;
- correct index identity;
- plausible/investigated library type;
- no systematic reference incompatibility;
- no severe mapping/decoy anomaly;
- fragment-length behavior understood for the exact Salmon version;
- bias configuration frozen;
- required output files produced.

Technical variants are judged on technical behavior, never DE overlap.

## G6 — Full quantification validation

For every sample:
- `quant.sf` exists and parses;
- required metadata/log files exist;
- analysis_id matches;
- index/config IDs match;
- no fatal diagnostics.

Cohort review:
- mapping-rate distribution;
- decoy-related metrics where available;
- assigned/ambiguous behavior;
- library-format consistency;
- read-count/depth outliers;
- sample-level warnings.

## G7 — tximport validation

Checks:
- all expected samples imported;
- sample order exactly matches metadata;
- transcript IDs reconcile;
- unmatched transcript count quantified;
- no unexplained duplicate transcript keys;
- gene count plausible;
- no accidental TPM-to-DESeq2 route.

## G8 — DESeq2 validation

Checks:
- design matrix full rank;
- factor reference levels correct;
- contrasts correct;
- complete result table retained;
- PCA reviewed;
- sample distances reviewed;
- dispersion reviewed;
- normalization factors reviewed;
- p-value/padj behavior reviewed;
- Cook's-distance/outlier behavior reviewed.

Track A and Track B remain separate.

## G9 — Biological/sensitivity validation

Biological sanity checks occur only after analysis.

Examples from the publication:
- PCA3;
- AMACR;
- ANKRD34B;
- NEK5;
- KCNG3;
- PTPRT.

These genes must never be used to tune the workflow.

Sensitivity:
- Track B is mandatory;
- technical sensitivity tests require pre-specified technical justification.

## G10 — Publication-comparison validation

Must produce:
- tested-gene count;
- significant-gene count;
- overlap;
- union;
- Jaccard index;
- recovery fraction of published DE genes;
- direction concordance;
- fold-change correlation where comparable;
- rank correlation where comparable;
- biotype comparison;
- top-gene comparison;
- pathway comparison if included.

Identifier harmonization must be auditable.

## Rejection rules

A result is invalid if:
- identity cannot be proven;
- provenance is missing;
- configuration is unknown;
- annotation/reference releases are mixed;
- labels are inferred post hoc;
- parameters were tuned to match the publication;
- Track A and Track B outputs were mixed;
- primary significance criteria changed after viewing overlap.

## Reproducibility closure

Final validation passes only when a competent analyst can determine:
- exact cohort;
- exact inputs;
- exact reference;
- exact Salmon configuration;
- exact tx2gene;
- exact DE design;
- exact threshold;
- exact analysis_id;
- exact repository commit.
