# AGENTS.md

## Project boundary

Work only on the reproducible GSE89223 reanalysis defined by:

- \`docs/PROJECT-SPEC.md\` for project-level scope
- \`docs/ARCHITECTURE.md\` for the technical pipeline and gates

## Required workflow

1. Read the relevant GitHub Issue before starting work.
2. Follow the Salmon-first architecture; do not introduce STAR/HTSeq/edgeR into the primary execution path.
3. Do not infer sample groups or patient pairing from file order.
4. Prefer accession-driven, scripted operations.
5. Pin reference and annotation versions before building indexes.
6. Record tool versions and the exact analysis configuration used for results.
7. For single-end Salmon quantification, do not silently accept fragment-length assumptions; pass the architecture's pilot gate first.
8. Keep changes bounded to the active Issue.
9. Use a branch/PR for substantive repository changes.

## Data rules

- Never commit FASTQ, SRA, BAM, CRAM, Salmon indexes, large reference FASTA/GTF files, or package caches.
- Do not delete raw data until the required derived outputs are verified.
- Do not modify or mount unrelated storage merely for convenience.
- Do not include credentials, access tokens, or private machine configuration in the repository.

## Scientific rules

- Separate source metadata from analyst inference.
- Preserve BPH, PCa tumor, and adjacent-normal labels explicitly.
- Keep the paper-comparison track and paired sensitivity track distinct.
- Use \`tx2gene\` from the same annotation release that defines the indexed transcripts.
- Do not feed TPM directly to DESeq2.
- Report exclusions and outliers with a reason.
- Do not tune Salmon parameters to maximize agreement with the original paper.
- Do not claim reproduction of the paper unless cohort, reference, annotation, quantification, and statistical differences are documented.
