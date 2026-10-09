# GSE89223 G5 technical pilot audit — 2026-10-09

**Status:** real pilot runs completed; preliminary configuration comparison, **not** G5/G6 gate acceptance. Source FASTQ immutable.

## Audited original single-end run

- SRR4453804, 5,575,676 reads; Salmon 2.8.0, reference GRCh37.p13/GENCODE v19, full decoys, k31, `--keepDuplicates`, 196,520 versioned quant.sf transcript rows.
- Raw, untrimmed reads; `-l U --fldMean 150 --fldSD 50` as initial assumption.
- 761,958 / 5,575,676 mapped **13.665751%**; `num_decoy_fragments=3,049,750`; 196,520 unique quant.sf IDs matching reference/tx2gene; VB/EM converged, no quant errors, sum TPM 1,000,000.
- Salmon diagnostic codes: `low_mapping_rate`, `library_type_mismatch`, `strand_bias_unstranded`. Measured strand mapping bias ~0.970745, detected `SF`. A high number of decoy-attributed reads is consistent with earlier G4 rRNA evidence but not proven to be rRNA-specific merely by count.

## Controlled independent strandedness test

Kept identical index/FASTQ/fragment-length priors (150/50)/threads/bias options but replaced library type `U` with `SF`. Output isolated at `pilots/G5_SRR4453804_keepDuplicates_raw_SF_fld150_50_v1`.

- `-l SF`: 749,597 mapped, **13.444056%**, 3,049,750 decoy fragments, 196,520 unique quant.sf transcript IDs, TPM sum 1,000,000, EM converged and no quant errors.
- Diagnostics with `SF`: only `low_mapping_rate`; the `library_type_mismatch` and `strand_bias_unstranded` warnings disappeared.
- Change in apparent mapped-read count: 12,361 fewer assigned fragments with `SF` (orientation restriction); low mapping therefore is **not resolved** by changing library type.

## Interpretation

This strengthens the hypothesis of a forward-stranded library, but library chemistry/source publication and an independent second-sample check are needed before globally freezing strand orientation. Non-mapping and decoy-vs-transcript classification require separate investigation. The `num_decoy_fragments` field is a Salmon classification count, not a verified rRNA fraction. The unstranded baseline was a provisional trial, not the accepted biological configuration.

The choice of `--fldMean 150 --fldSD 50` is explicitly prior-driven in this version (`fragment_length_source=prior`); carry out a bounded sensitivity analysis instead of treating an assumed prior as experimentally measured fragment lengths.

## Decision / gates

- G3 duplicate-preserving index has completed and passed technical validation (297 decoys and 196,520 quantification targets). Cross-gene indistinguishable transcripts remain flagged.
- Technical pilot validation **passed structurally**; its primary biological configuration is **not frozen** because source-confirmed strandedness, fragment-prior sensitivity and low mapping/decoy diagnostics are pending.
- G4 formal gate remains open, including Issue #14 acquisition provenance dependency.
- **Do not launch cohort-wide Salmon quantification yet.** 
