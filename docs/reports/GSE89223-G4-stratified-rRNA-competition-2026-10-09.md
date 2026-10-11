# GSE89223 G4 — full-file stratified rRNA screening and transcript competition

**Run date:** 2026-10-09. **Issue:** #4. **Draft PR:** #51. **Gate G4:** OPEN.  
**Read safety:** 32 original FASTQs and archival SRA untouched. No clipping, deduplication, exclusions or Salmon quantification.

## Why a new experiment was necessary

The earlier 32-run screening read only the *first* 50,000 sequences of each FASTQ, risking ordering bias. The 18.032–74.692% rates from that experiment were never valid full-file rRNA abundance estimates. This experiment systematically samples across the entire original FASTQ and adds a transcriptome competitor.

## Design A: whole-file systematic sampling

- 32 validated single-end GSE89223 FASTQs.
- **100,000 sampled reads per sample**, **3,200,000 screened reads** in total.
- Each FASTQ is divided by its recorded read count into 100,000 equal-count strata. Within each stratum, exactly one read index is selected using a deterministic 64-bit integer from `SHA256("G4v1|SRR|stratum")`. This is reproducible spatially distributed sampling, not independently drawn reads.
- All sampled subsets and SHA-256 identities are retained separately; originals untouched. Validated FASTQ header/plus structure on selected records. Sample records reconciled against all 32 accession records.
- Minimap2 **2.31-r1302**, `-ax sr --secondary=no -t 4`, with the **six representative rRNA reference** from prior report: 5S, 5.8S, 18S, 28S and mitochondrial 12S/16S.
- Representative rRNA FASTA SHA-256: `54abc9d0edffa981ea250497b2cfa96edc682cf5d0393b93f5596d677d004713`.
- Rates count primary SAM alignments; a separate stricter measure counts alignments with MAPQ ≥20. The rRNA-only reference lacks genomic/transcript competitor sequences.

**Result (sample-wise min / median / max):**

| Metric | Minimum | Median | Maximum |
|---|---:|---:|---:|
| Fraction primarily mapped to six-rRNA-only reference | 17.983% | **49.757%** | 74.276% |
| Fraction mapped with MAPQ ≥20, all sampled reads as denominator | 13.361% | **41.948%** | 62.651% |

## Design B: alternative-transcript competition diagnostic

- Downloaded from **GENCODE human release 19**, EBI mirror:
  - `gencode.v19.pc_transcripts.fa.gz`, SHA-256 `450a2af39f2a812978fee2478dfd1f21dba8fb35b6d7b12eb5fb761fe4527153` — 95,309 transcript FASTA records.
  - `gencode.v19.lncRNA_transcripts.fa.gz`, SHA-256 `0cdf314fa04f8fea2d909bad723ac84f92571ffe2dab80931a80c4939b7d5e6f` — 23,898 records.
- Appended six rRNA representatives. **119,213 total references**, 219,371,166 nucleotides. Prefixes identify RRNA vs GENCODE_PC vs GENCODE_LNCRNA; combined FASTA SHA-256 `fe56c06fcb115ee2f7e1e9fdac1cb872943c32fbb6905a3b94fb28caf715f98c`. Indexed using minimap2 `-x sr -d`.
- To contain resources while assessing competition, deterministically selected every tenth read (the sixth in every group of ten) from the existing 100,000-read spatially distributed samples: **10,000 reads per run, 320,000 total**.
- Mapped to the **combined** reference with minimap2 `-ax sr --secondary=no -t 3`, counted only primary SAM alignments, and assigned category by primary reference; MAPQ ≥20 was tracked separately.
- Competitor sequences exclude other GENCODE classes and the genomic reference, and minimap2 primary-category selection may favor one of multiple equivalent targets. This is a **screening comparison**, not a complete read origin assignment.

**Result (sample-wise min / median / max):**

| Metric | Minimum | Median | Maximum |
|---|---:|---:|---:|
| Primary rRNA assignment among 10,000 competition-mapped input reads | **16.170%** | **47.410%** | **72.830%** |
| Primary rRNA assignment with MAPQ ≥20 (all input reads denominator) | 12.500% | 40.965% | 61.900% |

Median per-sample primary counts out of 10,000: rRNA **4,741**; protein-coding **1,562.5**; lncRNA **563**; unmapped **2,975.5**. Per-sample medians are calculated independently and do not sum to 10,000.

## Group review (DESCRIPTIVE ONLY)

Cohort membership was drawn from the authoritative sample manifest, never inferred from QC results.

| Existing paper group | n | rRNA-only mapping median, 100k/read | Competitive rRNA primary mapping median, 10k/read |
| --- | ---: | ---: | ---: |
| Track A tumor | 10 | 42.403% | **40.155%** |
| Track A control | 12 | 50.742% | **48.580%** |
| Not in Track A | 10 | 54.174% | **51.625%** |

Example competition extremes:
- Lowest: `SRR4453800` **16.17%** (Track A tumor), `SRR4453785` **20.93%** (Track A tumor).
- Highest: `SRR4453793` **72.83%** and `SRR4453792` **72.20%** (both excluded from Track A by pre-existing cohort rules), `SRR4453814` **67.61%** (Track A control).

These medians cannot support a clinical or biological claim about tumor vs control because of possible library batch, sample degradation, sequence composition, competing alignment ambiguity, read length and sample design effects. The two screening experiments also use **different numbers of reads**, so the 1.849 percentage-point median per-run difference is a *descriptive comparison*, not an exclusively competition-caused difference.

## Scientific assessment

**Confirmed**: (1) many overrepresented FastQC 50-mers exactly match nuclear 18S/28S rRNA; (2) substantial rRNA-related mapping signal remains across the whole-file spatially stratified samples; and (3) this signal persists with a large protein-coding/lncRNA competitor panel.

**Not yet established**: a defensible genome-competitive, comprehensive full-run rRNA contamination rate; whether non-rRNA reads should be physically removed; the direction or magnitude of mapping bias in the future Salmon reference. The full genome / rDNA arrays / pseudogene repertoire, alternative rRNA copies, indel/multimapping ambiguities, and protocol-specific artifacts remain incompletely resolved. These screens must not be used as de facto biological group-difference tests.

**Recommendation:** proceed with a controlled G4 decision on *retaining raw reads as the default*; do not deduplicate, hard-truncate, drop samples, or mask rRNA without a traceable comparison with G3's actual transcript/reference strategy and Salmon pilot QC. Set a *technical watch* for high-rRNA libraries at the pilot step rather than arbitrary exclusion. Freeze G4 only after explicitly reviewing experiment-level risks and documenting the accepted preprocessing configuration.

## Files and provenance (machine-local, not committed)

Root: `/home/kingrider/GSE89223_download/qc/gse89223_raw_v1_20261009/rrna_read_mapping_v1`.

- `all32_systematic_100k_v1.tsv`: 32 complete rows with per-run subset SHA-256 and original FASTQ checksum.
- `competition_all32_10k.tsv`: 32 complete rows with primary category classification and sampled subset SHA-256.
- `systematic_100k.log`, `competition_all32.log`.
- `competitor_gencode19/competition_manifest.json`, pinned source FASTAs, combined FASTA and minimap2 index.
- Extraction and evaluation code: `g4_systematic_100k_v1.py`, `g4_competition_reference_v1.py`, `g4_competition_screen_v1.py`, retained alongside earlier QC scripts in the analysis root.

No large FASTQ, index, or raw reads were committed to GitHub. This is preliminary scientific evidence pending G4 acceptance.
