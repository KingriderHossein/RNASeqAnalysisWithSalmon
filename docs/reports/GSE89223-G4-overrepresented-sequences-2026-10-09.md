# GSE89223 G4 supplementary review — overrepresented sequences and read-length tails

**Date:** 2026-10-09. **Status:** evidence collected; rRNA identity NOT verified; preprocessing not frozen. **Issue:** #4. **Draft PR:** #51.

## Input and method
Parsed all 32 FastQC 0.13.0 ZIP reports (`fastqc_data.txt`) generated from the 32 verified single-end FASTQs. Extracted each FastQC **Overrepresented sequences** table and the binned **Sequence Length Distribution**; compared exact reported 50-mer strings across SRR samples. This is *FastQC peak-list analysis*, **not** a whole-read prevalence or contamination-rate estimate. All source FASTQs remain untouched.

## Observations
- **32 samples**, **474 FastQC overrepresented-sequence rows**, representing **112 distinct reported 50-mers**. FastQC labels all 474 rows `No Hit`; this is not a reference-based rRNA assignment.
- Exact 50-mer `GTGGAGAAGGGTTCCATGTGAACAGCAGTTGAACATGGGTCAGTCGGTCC` appears in **28/32** reports.
- `GAAGGTGGTTTTCCCAGGGCGAGGCTTATCCATTGCACTCCGGATGTGCT` appears in **21/32** reports.
- Several reported sequences overlap by shifted windows, suggesting a related sequence family; the evidence **does not determine its biological origin**.
- FastQC 50-mer peak lists can underrepresent total abundant sequence families, and overlapping peak percentages are not additive. A FastQC `No Hit` result is neither proof nor refutation of rRNA origin.

Using FastQC **binned** read-length distributions (bins with lower boundary >= threshold, thus approximate):
- Fraction with length >=200 bp: median **1.5425%**, range **0.702–7.238%**.
- Fraction with length >=250 bp: median **0.060%**, range **0.017–0.632%**.
- SRR4453785: >=200 ~4.119%; >=250 ~0.288%.
- SRR4453804: >=200 ~1.893%; >=250 ~0.064%.
- SRR4453807: >=200 ~0.702%; >=250 ~0.030%.

## Interpretation
**A shared, reproducible sequence signal exists across samples, but its origin is unknown.** rRNA carryover, abundant biological RNAs, and library/technology-specific artifacts remain candidate explanations. Neither `Adapter Content PASS` in 32/32 nor these FastQC peak lists establishes contaminant absence. Reported sequence duplication alone cannot distinguish biological abundance from technical PCR duplicates.

Low-quality long-read tails deserve coverage-weighted review; as very long reads are uncommon, arbitrary fixed-length truncation would discard valid signal without sufficient justification.

## Next discriminating test (not yet executed)
Use a *versioned, cited* human 5S / 5.8S / 18S / 28S and mitochondrial rRNA reference, with a separately characterized adapter/artifact reference, to assign **representative exact 50-mers** (and reverse complements) by documented stringent alignment. Then estimate read-level rRNA fractions with a reproducible method across all 32 samples, contrasting Track A/Track B and reporting uncertainty. A hit from a short conserved 50-mer alone may remain ambiguous; sequence-family or full-read confirmation is necessary.

## Safe processing decision
**No automatic trimming, deduplication, read exclusion, or sample exclusion.** Keep FASTQs immutable, record all provenance, and postpone final G4 acceptance until taxonomic/biological origin and quality effects have been reviewed. Do not optimize results against publication differential-expression overlap. Salmon reference/index and quantification remain separately gated.

## Local evidence
- `/home/kingrider/GSE89223_download/g4_overrepresented_review_v2.py` — descriptive extraction script.
- `/home/kingrider/GSE89223_download/qc/gse89223_raw_v1_20261009/g4_overrepresented_review.tsv` — per-run peak table.
- 32 FastQC ZIPs and MultiQC combined report under the same QC folder.

*The repository contains this compact supplementary report, not the 94 GB FASTQ data nor source ZIP files.*
