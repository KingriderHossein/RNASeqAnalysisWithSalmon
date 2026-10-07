# Metadata provenance

## Authoritative raw metadata

The manifest is built from two source files:

- `GSE89223_family.soft.gz` — GEO family SOFT, accession `GSE89223`.
- `SRP092131_runinfo.csv` — NCBI SRA RunInfo, study `SRP092131`.

The source files are retained locally for provenance. The derived TSV is suitable for version control.

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

### Planned project contrasts

1. **Paper-reproduction contrast:** the paper's 10 tumor vs 12 control final cohort. This is required for the most direct comparison with the reported 3,384 DE genes.
2. **Paired PCa-only sensitivity contrast:** 9 matched PCa tumor/adjacent-normal patient pairs retained in the paper final set. This removes the BPH controls and the unpaired CP2 sample, allowing a paired design (`~ patient + condition`).

These contrasts answer different questions and must not be conflated.