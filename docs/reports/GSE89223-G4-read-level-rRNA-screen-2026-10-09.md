# GSE89223 G4 — first-50k-read rRNA mapping screen (2026-10-09)

**Issue:** #4; **PR:** #51; **Gate:** G4 OPEN. **Analysis:** `GSE89223_G4_rRNA_read_mapping_screen_v1`.

## Method and provenance

On the Linux host, downloaded the official minimap2 2.31 x86-64 release binary archive from `https://github.com/lh3/minimap2/releases/download/v2.31/minimap2-2.31_x64-linux.tar.bz2`; downloaded archive SHA-256: `300bc287f05eb890c6211fa7db043ce98320a401621fadd1cfdbeabd1a6e4ab5`. Verified binary reports `2.31-r1302`.

Constructed a six-record *representative* mature human rRNA reference: NCBI RefSeq 5S `NR_023363.1` (121 nt), 5.8S `NR_003285.3` (157 nt), 18S `NR_003286.4` (1869 nt), 28S `NR_003287.4` (5070 nt), mitochondrial MT-RNR1 12S (`NC_012920.1:648–1601`, 954 nt) and MT-RNR2 16S (`NC_012920.1:1671–3229`, 1559 nt). Combined FASTA SHA-256: `54abc9d0edffa981ea250497b2cfa96edc682cf5d0393b93f5596d677d004713`. This panel is **not exhaustive** (paralogs, pseudogenes, variants, external contamination, full genome competitors are absent).

Extracted the **first 50,000 unmodified reads**, in original file order, from each of the 32 verified single-end GSE89223 FASTQs. Each first-50k subset was separately recorded with SHA-256. Ran `minimap2 -ax sr --secondary=no -t 4 <representative-rRNA-FASTA> <first-50k-FASTQ>`. Parsed primary SAM alignments; counted mapped reads and separately MAPQ >=20. All 32 jobs returned successfully. No FASTQ original was modified.

**Selection caveat:** first 50,000 reads **are not random** and cannot establish full-run rRNA prevalence or statistical group differences. MAPQ>=20 is aligner-specific and, without competing genome/transcriptome sequences, does not establish unique rRNA origin.

## Empirical results

| Quantity | Result |
| --- | --- |
| Runs examined | 32/32 |
| Reads examined per run | 50,000 |
| Total screened reads | 1,600,000 |
| Fraction mapping to representative panel across samples (minimum / median / maximum) | 18.032% / **50.923%** / 74.692% |
| Fraction mapped with MAPQ >=20, of all sampled reads (minimum / median / maximum) | 13.518% / **43.247%** / 63.632% |
| Original Track A tumor first-50k mapping median | 42.877% (n=10) |
| Original Track A control first-50k mapping median | 51.593% (n=12) |
| Runs excluded from original Track A, first-50k mapping median | 55.056% (n=10) |

Sample-level examples: `SRR4453800` 18.032% (Track A tumor), `SRR4453785` 22.852% (Track A tumor), `SRR4453801` 25.488% (Track A control); `SRR4453793` 74.692% and `SRR4453792` 74.502% (both already excluded from Track A by the cohort manifest), `SRR4453814` 71.214% (Track A control).

The earlier 50k-read pilot sample `SRR4453804` showed 29,023/50,000 primary reads mapped (**58.046%**), 26,404/50,000 (52.808%) MAPQ>=20. Counts by reference in the pilot: 28S 16,733; 18S 10,945; mitochondrial 16S 966; mitochondrial 12S 208; 5.8S 162; 5S 9.

## Assessment

This is substantially stronger evidence of rRNA-related sequence in the raw FASTQs than FastQC peak lists, but **these numbers remain screening alignments rather than validated rRNA contamination fractions**. Short-read ambiguity, missing decoy competitors, platform-specific indels and unrepresentative first-read selection must be addressed. Reported tumor-control medians are **descriptive only**, not an inferential group difference.

**No preprocessing decision can be frozen from this screen alone.** In particular:
- No sample exclusion or automatic read removal, deduplication, trimming or fastq rewriting.
- Do not adopt the apparent 18–75% mapping range as full-run rRNA contamination estimates.
- Next: prespecified, reproducible whole-read/representative *random* sampling across each run, a more comprehensive pinned nuclear + mitochondrial rRNA panel plus specificity checks against competing genome/transcriptome sequences, and per-run classification with uncertainty.
- Evaluate group balance separately for Track A and B; never tune preprocessing to reproduce the publication's DE result.

## Machine-local outputs

`/home/kingrider/GSE89223_download/qc/gse89223_raw_v1_20261009/rrna_read_mapping_v1/`:
- `reference_manifest.json` and `human_rrna_representatives_v1.fasta`;
- `pilot_summary.json`;
- `all32_first_50000_reads.tsv`;
- first-50k FASTQ *diagnostic subsets*, with each subset's SHA-256 captured in TSV; originals unchanged;
- execution log `all32_screen.log`.

Scripts on Linux: `g4_build_rrna_mapping_reference_v1.py`, `g4_rrna_pilot_v1.py`, and `g4_rrna_all32_screen_v1.py`. Raw FASTQs and derived large artifacts are not in Git.
