# G6 staged pilot execution and safety tests — 2026-10-09

## Completed on real Linux host

**First new cohort sample under the G6 runner**: `SRR4453783`, full original single-end raw FASTQ (14,930,696 reads). Real Salmon 2.8.0 run (SF, single-end prior 150±50, GENCODE v19+GRCh37.p13 full-genome decoy-aware `--keepDuplicates` k31, 6 threads) started 08:55:10 and exited 0 at 08:56:59 (system locale). Result formally checked by automated validator and promoted from `.working` to final:
`/home/kingrider/GSE89223_download/salmon/G6_SF_raw_v1/SRR4453783/`.

Metrics **from real `aux_info/meta_info.json`**:
- Processed: **14,930,696**; accepted transcript-mapped: **3,087,405**; mapping: **20.6782389782767%**.
- Decoy-assigned: **5,863,240**; warning `low_mapping_rate` retained; not a biological quality pass.
- All required output files nonempty, `quant.sf` contains 196,520 unique versioned transcripts, TPM sum **999999.9999749971**, index sequence hash matches pinned config, EM converged, no internal quant error, input SHA-256 verified before execution; `validation.json` persists input/config provenance and QC.

**Recovery test**: re-ran `--pilot-only`; emitted `VALIDATED_SKIP SRR4453783`, no Salmon re-run or overwritten result, exit 0. **Gate test:** `--batch` with no formal gate marker fails (exit 2), with explicit message `G4/G5 formal acceptance missing; 32-sample batch forbidden`. Test suite `SAFETY_TESTS_PASS`.

**Implementation note:** initial runner trial encountered a missing output-parent directory before Salmon ran; fixed in isolated working copy; saved initial failure and subsequent passing `--dry-run`, then full sample validation and gate tests. The *GitHub version* of `scripts/g6_resumable_v1.py` was uploaded after this fix. Pilot run is validated and durable; unrelated existing cohort outputs were never overwritten.

## Precise status / gate decision

- **G4 technical cohort evidence:** PASS (32 canonical raw FASTQs, provenance state records, 32 FastQC/MultiQC, matching manifest, G3 index PASS). Still **G4 FORMAL OPEN** because project architecture and Issue #4 require Issue #14 acceptance, not satisfied. Do not conflate completed study-specific acquisition artifacts with completed cross-platform Download Manager engineering.
- **G5 technical pilots:** PASS for test execution/config consistency; scientific warnings recorded, single-end length prior is a sensitivity-tested model assumption not physical measurement. **G5 CONFIG STAGED ON PR #52**, not a formally accepted/integrated freeze as of this report.
- **G6 staged pilot:** one sample `SRR4453783` quantified and fully validated. **G6 full batch: NOT STARTED**; deliberate gate interlock is functioning. No false claim of 32 results.
- Recovery granularity is **per sample only**, not within a truncated Salmon process. An incomplete `.working` is preserved and blocks unreviewed automatic resume.

## Follow-up to enable the 32-run batch

1. Reconcile final requirements of Issue #14 / G4 provenance and accept G4 explicitly without bypassing frozen architecture, or enact a deliberate project-level policy change through change control if scientifically justified.
2. Review and integrate frozen candidate in PR #52, record formal G5 acceptance with explicit SF / prior / bias flags and known warnings.
3. Create the formal acceptance record required by the guarded runner only after both gates genuinely pass and have GitHub provenance; then execute `--batch` with published live monitor.
4. Stage-wise review warnings; 32 input samples remain in the cohort; never automatically exclude because of low mapping.

## Monitor

```bash
tail -f ~/GSE89223_download/audits/G6_release_20261009/02_runner_one_sample_retry.log
tail -f ~/GSE89223_download/salmon/G6_SF_raw_v1/SRR4453783.run.log
tail -f ~/GSE89223_download/audits/G6_release_20261009/03_resume_and_gate_tests.log
```

No whole-cohort STAR/HTSeq work here; their numbers are not needed for the current Salmon track.
