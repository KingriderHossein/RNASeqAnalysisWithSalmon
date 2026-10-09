# Contributing

This repository owns the scientific GSE89223 reanalysis. Contributions should make the workflow easier to inspect, reproduce or validate. English and Persian issue descriptions are welcome.

## Before changing files

1. Search [open Issues](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues) and [pull requests](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/pulls) for related work.
2. Read [AGENTS.md](AGENTS.md) and its linked scientific documents. Start with the [documentation index](docs/README.md) if you are new to the project.
3. For substantive work, use a focused Issue, branch and PR. Describe the problem, inputs, expected outputs and relevant validation gate.

Use `docs/<topic>`, `metadata/<topic>`, `qc/<topic>`, `analysis/<topic>` or another branch prefix from [Governance](docs/GOVERNANCE.md). Small documentation corrections can use one focused PR without creating duplicate tracking work.

## Scientific changes

- Keep Track A and Track B distinct.
- Preserve the GENCODE v19 / GRCh37.p13 reference contract and matching tx2gene.
- Do not pass TPM directly to DESeq2 as raw counts.
- Do not tune parameters or remove samples to improve publication overlap.
- Preserve warnings, failed attempts and limitations that affect interpretation.
- Record new scientific choices in [DECISIONS.md](docs/DECISIONS.md) before dependent execution.
- Use a new script/config version, analysis ID or output path when behavior changes. Retain provenance for earlier results.

For a scientific concern, provide the analysis ID, stage, report or file, exact evidence, and effect on the conclusion. Keep observations separate from proposed explanations.

## Local validation

From the repository root:

```bash
python3 scripts/check_repository_v1.py
for script in scripts/*.sh; do
  bash -n "$script" || exit 1
done
```

These are static repository checks. For an analysis change, also supply the evidence required by [VALIDATION.md](docs/VALIDATION.md). Say which checks were run and which were not. A green CI run does not close a scientific gate.

Review changed links, figures and commands. For Persian HTML, preserve RTL layout and Vazirmatn. GitHub controls Markdown typography; do not add CSS that its renderer cannot apply.

## Pull requests

The PR template asks for purpose, scope, input/output impact, validation and limits. Keep changes small enough to review as one coherent result. Include the Issue reference and exact analysis/config identity when applicable. Do not mix documentation work with an unreviewed scientific rerun.

## Data and credentials

Never commit raw FASTQ/SRA, large reference sequences, Salmon indexes, package caches or credentials. Keep large data outside Git and record identities as required by [DATA-CONTRACTS.md](docs/DATA-CONTRACTS.md). Use compact synthetic fixtures for code checks when possible. Redact personal paths or confidential content from new issue attachments as appropriate; never attach credentials.

## License status

The repository does not currently contain a project license. No new reuse permission or contributor license agreement is introduced by this guide. A license selection requires an explicit maintainer decision. Dependencies and source datasets retain their own terms.
