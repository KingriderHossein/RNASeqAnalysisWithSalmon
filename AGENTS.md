# AGENTS.md

## Canonical project authority

Read these before implementation:
- `docs/PROJECT-SPEC.md`
- `docs/ARCHITECTURE.md`
- `docs/DECISIONS.md`
- `docs/DATA-CONTRACTS.md`
- `docs/VALIDATION.md`
- `docs/GOVERNANCE.md`

## Mandatory scientific rules

1. Primary pipeline: `FASTQ → QC → Salmon → tximport → DESeq2`.
2. STAR/HTSeq/edgeR is comparator-only unless scope changes explicitly.
3. GSE89223 is total-RNA/rRNA-depleted whole-transcriptome data, not strict Poly(A)+ mRNA-seq.
4. Track A and Track B stay separate.
5. Primary reference is GENCODE v19 comprehensive / GRCh37.p13.
6. Never mix annotation releases.
7. tx2gene comes from the same GTF as the reference.
8. TPM is not DESeq2 raw count input.
9. Full Salmon quantification requires pilot Gate G5.
10. Never tune parameters against publication overlap.
11. Never exclude samples merely to improve PCA, mapping or agreement.
12. Published results are comparator evidence, not ground truth.
13. Every result must have analysis_id and provenance.

## Data rules

Never commit:
- FASTQ/SRA;
- large reference FASTA/GTF copies;
- Salmon indexes;
- large temporary/intermediate data;
- credentials;
- package caches.

## Work rules

- read the relevant Issue;
- obey stage dependencies;
- use branch + PR for substantive changes;
- record material architecture changes in `docs/DECISIONS.md`;
- pass the relevant validation gate before dependent work;
- never infer biology from filename/order/clustering alone;
- do not hide scientific decisions in scripts or chat.

## Current execution boundary

This repository owns ONLY the scientific GSE89223 workflow, its frozen
reference and reproducible batch/CLI execution. Rust/Tauri GUI software,
cross-platform Download Manager deliverables and their Rust Core/CLI are
independent and out of scope.

G4 accepts verifiable FASTQ input identity, checksums, provenance, QC and an
explicit preprocessing decision regardless of the software that acquired it;
a separate app's feature/release status is **not** a G4 dependency.
G5 must freeze a defensible Salmon configuration with the known mapping/decoy
warnings investigated and documented; G6 must respect its own G4/G5 scientific
gates and safe no-overwrite output rules. G7 onwards use tximport/DESeq2.

No scientific stage is implicitly accepted by removing an unrelated
software dependency.
