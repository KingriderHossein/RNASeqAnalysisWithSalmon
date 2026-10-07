# Metadata provenance

## Authoritative raw metadata

The manifest is built from two source files:

- `GSE89223_family.soft.gz` — GEO family SOFT, accession `GSE89223`.
- `SRP092131_runinfo.csv` — NCBI SRA RunInfo, study `SRP092131`.

The source files are retained locally for provenance and are not committed. Recreate them with `bash scripts/fetch_metadata.sh`, then rebuild the derived manifest with `python3 scripts/build_manifest.py`. The derived TSV is suitable for version control.

## Derived manifest

`metadata/derived/GSE89223_sample_manifest.tsv` maps GEO sample, patient identifier, diagnosis/tissue, SRA experiment/run, BioSample, library metadata, and analysis-cohort membership.

### Verified study structure

- 32 total samples/runs.
- 15 PCa tumor samples.
- 14 PCa adjacent-normal samples.
- 2 BPH adenomatous samples.
- 1 BPH normal sample.
- All 32 SRA runs are single-end Ion Torrent Proton RNA-seq.

### Original-paper final DE cohort

Nikitina et al. excluded histologically problematic/outlier samples after MDS and repeat pathology review. Their final differential-expression dataset contained 22 samples:

- tumor group: 10 PCa tumor samples;
- control group: 12 samples composed of 9 PCa adjacent-normal, 1 BPH normal, and 2 BPH hyperplastic tissues.

The paper explicitly reports that BP3/BN3 had initially been documented as BPH, but review showed the patient actually had PCa; BP3 was retained in the tumor group.

Excluded pairs were also recorded with their pathology-based reasons: CN5/CP5 (seminal-vesicle contamination), CP6/CN6 and CP7/CN7 (insufficient tumor fraction / high normal-tissue contribution), CP9/CN9 (infiltrative growth prevented clean tumor-normal separation), and CP10/CN10 (substantial normal tissue in the tumor section).

One metadata discrepancy is preserved rather than silently corrected: GEO reports PSA 13.5 ng/ml for CP2, while Table 1 of the paper reports 8.6 ng/ml. The expression analysis does not depend on this PSA value, but provenance requires that both values be noted.

### Planned project contrasts

1. **Paper-reproduction contrast:** the paper's 10 tumor vs 12 control final cohort. This is required for the most direct comparison with the reported 3,384 DE genes.
2. **Paired PCa-only sensitivity contrast:** 9 matched PCa tumor/adjacent-normal patient pairs retained in the paper final set. This removes the BPH controls and the unpaired CP2 sample, allowing a paired design (`~ patient + condition`).

These contrasts answer different questions and must not be conflated.