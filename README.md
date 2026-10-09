# RNASeqAnalysisWithSalmon

Reproducible **Salmon-first reanalysis of GSE89223 prostate-cancer RNA-seq**.

## In one line

```text
GSE89223 → acquisition → QC → Salmon → tximport → DESeq2 → validation → publication comparison
```

The original `STAR → HTSeq → edgeR` workflow is comparator-only.

## Dataset caveat

GSE89223 is FFPE, Ion Torrent Proton, single-end, total-RNA/rRNA-depleted RNA-seq. It is **not strict Poly(A)+ mRNA-seq**.

The primary project is therefore a **whole-transcriptome expression reanalysis**.

## Tracks

### Track A
10 tumor vs 12 control, matching the paper's final comparison cohort.

### Track B
9 matched PCa tumor/adjacent-normal pairs.

## Stage map

```text
0  Architecture freeze
1  Sample/cohort lock
2  Reproducible environment
3  GENCODE v19 / GRCh37 reference
4  Raw reads + QC/preprocessing decision
5  Salmon pilot
6  Full Salmon quantification
7  tximport → gene level
8  DESeq2
9  Validation
10 Publication benchmark
11 Final report / reproducibility closure
```

## Canonical documentation

| Document | Purpose |
|---|---|
| [PROJECT-SPEC](docs/PROJECT-SPEC.md) | Goal, scope, deliverables, completion |
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Full scientific end-to-end architecture |
| [DECISIONS](docs/DECISIONS.md) | Frozen scientific/engineering decisions |
| [DATA-CONTRACTS](docs/DATA-CONTRACTS.md) | Schemas, identities, provenance |
| [VALIDATION](docs/VALIDATION.md) | Gates and rejection rules |
| [GOVERNANCE](docs/GOVERNANCE.md) | GitHub workflow and change control |
| [Metadata provenance](metadata/README.md) | Sample/cohort provenance |
| [AGENTS](AGENTS.md) | Mandatory implementation rules |

## Primary reference

```text
GRCh37.p13
+
GENCODE Release 19 comprehensive GTF
+
comprehensive transcript FASTA derived from genome + GTF
+
matching genome decoys
```

A protein-coding-only reference is not sufficient for the primary benchmark.

## Primary DE designs

Track A:

```r
~ group
```

Track B:

```r
~ patient + condition
```

Primary DEG definition:

```text
BH-adjusted p-value < 0.05
```

## GitHub workflow

```text
Issue → branch → implementation/evidence → validation → PR → review → main
```

## Storage policy

Commit:
- metadata;
- configuration;
- scripts;
- checksums/provenance;
- compact QC/results;
- reports.

Do not commit:
- FASTQ/SRA;
- large reference files;
- Salmon indexes;
- large intermediates;
- caches;
- credentials.

## Completion

A successful Salmon run is not project completion.

Completion requires:
- verified cohort;
- reproducible environment;
- pinned/checksummed reference;
- frozen QC/preprocessing;
- frozen Salmon pilot;
- full quantification;
- validated tximport;
- Track A + Track B DESeq2;
- validation;
- publication benchmark;
- explained differences;
- reproducible final report.

## Primary study

Nikitina AS et al. *Novel RNA biomarkers of prostate cancer revealed by RNA-seq analysis of formalin-fixed samples obtained from Russian patients.* Oncotarget. 2017. DOI: 10.18632/oncotarget.16518

## Current state

This is the scientific GSE89223 reanalysis repository, not a general-purpose
acquisition/desktop application. The 32 raw sample FASTQs, validation records,
FastQC/MultiQC and scientific pilot evidence are stored outside Git; Salmon
G5/G6 integration follows the documented scientific gates.

At the owner's request on 2026-10-09, the former Rust/Tauri Download Manager
was removed from the current tree for later migration to a *separate repository*.
Recover its complete source at historical Git commit
`6ee4dfa99d20c9b593f92525a191cef641eb3f75`.
No raw FASTQ, Salmon index or scientific data is deleted.
