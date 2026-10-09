# Project Governance — Architecture v2.1

GitHub is the source of truth for execution state.

## Authoritative ownership

- Project intent: `docs/PROJECT-SPEC.md`
- Technical architecture: `docs/ARCHITECTURE.md`
- Scientific/engineering decisions: `docs/DECISIONS.md`
- Data/provenance schemas: `docs/DATA-CONTRACTS.md`
- Validation/acceptance: `docs/VALIDATION.md`
- Active work: GitHub Issues
- Implementation identity: branches, commits and PRs

## Workflow

```text
Architecture
  ↓
Issue
  ↓
bounded branch
  ↓
implementation/evidence
  ↓
validation gate
  ↓
pull request
  ↓
review
  ↓
main
  ↓
issue reconciliation
```

A local successful command is not by itself accepted project evidence.

## Workstream architecture

1. Metadata/cohort provenance
2. Reproducible environment
3. Reference bundle + Salmon index
4. Raw-read acquisition + QC/preprocessing
5. Salmon pilot + full quantification
6. tximport + DESeq2
7. Validation
8. Publication benchmark
9. Final report + reproducibility closure

## Dependency architecture

```text
Architecture
 ↓
Cohort
 ↓
Environment
 ↓
Reference
 ↓
Raw/QC
 ↓
Salmon pilot
 ↓
Full Salmon
 ↓
tximport/DESeq2
 ↓
Validation
 ↓
Publication benchmark
 ↓
Final report
```

Dependent work waits for its required gate.

## Branch naming

Recommended:
- `docs/<topic>`
- `metadata/<topic>`
- `env/<topic>`
- `reference/<topic>`
- `qc/<topic>`
- `salmon/<topic>`
- `analysis/<topic>`
- `validation/<topic>`
- `benchmark/<topic>`
- `report/<topic>`

## Pull-request contract

Substantive PRs state:
- linked Issue;
- scope;
- inputs affected;
- outputs affected;
- architecture decision affected, if any;
- validation performed;
- known limitations.

## Architecture change rule

Material scientific changes require:
1. update to `docs/DECISIONS.md`;
2. architecture/spec update where needed;
3. PR review before dependent execution.

## Result acceptance

A result is accepted only when:
- analysis_id exists;
- provenance is complete;
- relevant validation gate passed;
- compact outputs are consistent;
- Issue acceptance criteria are reconciled.

## No hidden scientific decisions

Decisions must not live only in:
- chat;
- shell history;
- uncommitted notes;
- one-off notebooks;
- hidden script constants.

Durable scientific choices belong in docs/configuration.

## Data safety

Never:
- commit raw FASTQ/SRA;
- commit large indexes/reference copies;
- commit credentials;
- delete raw data before required derived outputs/provenance are verified;
- overwrite unrelated user data;
- force-clean an uncertain worktree.

## Review rules

Review checks:
- architecture compliance;
- scientific correctness;
- reproducibility;
- identity/provenance;
- no silent scope expansion;
- no post-hoc parameter tuning;
- no cohort mixing;
- no annotation-release mixing.

## Completion rule

A merged PR is not project completion.

Project completion requires all criteria in the project specification, architecture and validation/reproducibility closure.

## Current execution phase

The authorized project is a scientific GSE89223 reanalysis:
immutable FASTQ inputs + QC → Salmon → tximport → DESeq2 → validation.

The desktop Rust/Tauri Download Manager is separate software and has
been removed from the main tree. No scientific gate depends on its GUI
features, installation or release, but all actual FASTQ provenance and QC
requirements remain mandatory.

G4 requires actual source/run/size/checksum/QC evidence and a documented
preprocessing decision; G5 freezes the scientific Salmon configuration
in reviewed code; G6 must use one index/config and produce per-sample
validated, restart-safe outputs with warnings preserved.

The former Module A Issue should be closed as out of scope once this
change is integrated, preserving its GitHub discussion for any future
independent project.
