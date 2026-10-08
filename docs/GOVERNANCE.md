# Project Governance — Architecture v2.0

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

Architecture v2.0 and the Module A Download Manager execution contract are accepted.

Current authorized engineering phase: **Module A implementation**.

This permits Rust/Tauri scaffolding, the shared core, CLI/GUI integration, persisted state/recovery behavior, storage checks, tool adapters, tests, and packaging work for the Download Manager.

Actual accepted-project raw-data acquisition and downstream scientific execution remain controlled by their Issues and validation gates. Reference/index construction, Salmon quantification, tximport, DESeq2 and downstream analysis are not implicitly authorized by this phase transition.
