# AGENTS.md

## Project boundary

Work only on the reproducible reanalysis of GSE89223 described in `docs/PROJECT-SPEC.md`.

## Required workflow

1. Read the relevant GitHub Issue before starting work.
2. Do not infer sample groups or patient pairing from file order.
3. Prefer accession-driven, scripted operations.
4. Pin reference and annotation versions before building indexes.
5. Record tool versions used for results.
6. Keep changes bounded to the active Issue.
7. Use a branch/PR for substantive repository changes.

## Data rules

- Never commit FASTQ, SRA, BAM, CRAM, Salmon indexes, large reference FASTA/GTF files, or package caches.
- Do not delete raw data until the required derived outputs are verified.
- Do not modify or mount unrelated storage merely for convenience.
- Do not include credentials, access tokens, or private machine configuration in the repository.

## Scientific rules

- Separate source metadata from analyst inference.
- Preserve BPH, PCa tumor, and adjacent-normal labels explicitly.
- Report exclusions and outliers with a reason.
- Do not claim reproduction of the paper unless the comparison cohort, reference, and statistical differences are documented.
