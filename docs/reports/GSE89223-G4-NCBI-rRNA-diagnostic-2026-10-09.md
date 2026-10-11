# GSE89223 G4 — NCBI RefSeq rRNA peak-matching diagnostic (2026-10-09)

**Issue:** #4. **PR:** #51. **Status:** preliminary *positive sequence identity* evidence; G4 remains OPEN.  
**No raw FASTQ data modified. No samples dropped. No trimming performed.**

## Reference acquisition

On the authorized Linux workstation, fetched **version-pinned human NCBI RefSeq mature nuclear ribosomal RNAs** using E-utilities `efetch.fcgi?db=nuccore&id=<accession>&rettype=fasta&retmode=text`. Stored exact FASTA responses and SHA-256 checksums under `qc/gse89223_raw_v1_20261009/rrna_diagnostic_v1`.

| RNA | RefSeq accession | Length (nt) | FASTA SHA-256 |
| --- | --- | ---: | --- |
| 5S | `NR_023363.1` | 121 | `80dc7015aed93ae9c16ad32eedce2eb9f07a69705db54d26be6b5c82eee52528` |
| 5.8S | `NR_003285.3` | 157 | `778eceea0fa3ecc5d25b26921f08935badf8e5c9348423005e2f37f8c4206d93` |
| 18S | `NR_003286.4` | 1869 | `52dc74e97b9a3dae503e703ebdc0308e09856727bd9885f690d2cd3f68924f9d` |
| 28S | `NR_003287.4` | 5070 | `624251fbb017d9d08d52e0c81072a5b099e113ce15c30421237ebad050b70448` |

Reference records: https://www.ncbi.nlm.nih.gov/nuccore/NR_023363.1 ; https://www.ncbi.nlm.nih.gov/nuccore/NR_003285.3 ; https://www.ncbi.nlm.nih.gov/nuccore/NR_003286.4 ; https://www.ncbi.nlm.nih.gov/nuccore/NR_003287.4 .

## Test procedure

Source: `g4_overrepresented_review.tsv` generated from 32 FastQC ZIP files (FastQC 0.13.0). The FASTQ inputs had been separately validated and retained unchanged.

For each of **112 distinct FastQC-reported 50-mers**, evaluated its complete sequence and reverse complement at every reference start position, counting substitutions. Retained full-length matches with **zero or one substitution**; a mismatch or indel larger than this window is not identified by this specific screening test. This is a conservative *peak-identity test*, **not** a read-level rRNA fraction estimator and not a substitute for gapped alignment.

### Results

- **86 / 112 unique reported 50-mers** have an **exact (0-mismatch) match** to one of the pinned human rRNA reference sequences.
- **26 / 112** have **no match with <=1 substitution** in either orientation to the four reference representatives. **No sequence had exactly one mismatch as the best qualifying match.**
- Exact hits split into **51 unique 50-mers to 18S**, **35 to 28S**; no exact hit in the 5S or 5.8S representatives.
- Across the 474 sample/sequence report rows, **343** rows represent an exact-matching 50-mer.
- **31 of 32** FastQC sample peak lists contain at least one exact rRNA-matching 50-mer. This does **not** establish that the remaining sample has no rRNA.
- Top shared 50-mer `GTGGAGAAGGGTTCCATGTGAACAGCAGTTGAACATGGGTCAGTCGGTCC` occurs in **28/32 FastQC sample lists** and matches **28S `NR_003287.4` exactly**.
- Other widely shared exact matches occur in the 18S/28S reference.

### Interpretation and important limits

This is **positive evidence of abundant nuclear rRNA-derived sequence in the reported peak lists**, compatible with residual rRNA following depletion. It does not establish full-cohort rRNA read percentages, degree of depletion failure, the cause of variability, contamination source, or the need for trimming.

All sequences assessed are pre-selected for FastQC overrepresentation; therefore **86/112 and 343/474 are NOT percentages of sequencing reads**. Peak percentages are not additive, and lists can omit low-prevalence rRNA sequences. Short conserved regions may map elsewhere; representative reference matches are strong but not a comprehensive specificity test. This mini-reference omits rRNA paralogs, mitochondrial 12S/16S rRNA, pseudogenes, potential microbial sequences, and non-rRNA artifacts. Indel-prone Ion Torrent reads require an alignment approach that allows indels.

## Pending work before G4 acceptance

1. Add mitochondrial rRNA (and, if appropriate, a non-redundant expanded human rRNA panel), documenting the reference sequence origin, releases and SHA-256.
2. Use a validated aligner supporting indels and orientation to estimate **per-run read-level rRNA assignment** from all/representatively sampled reads, distinguishing ambiguous alignments; record tool version, parameters, input checksum, and confidence intervals for samples where sampling applies.
3. Join with fixed Track A/Track B sample identities; inspect group confounding and read-length/degradation trends.
4. Distinguish rRNA read assignment from trimming; decide preprocessing only with evidence and preserve raw FASTQ.
5. G4 stays open; do not proceed to accepted full Salmon quantification, tximport, or DESeq2 while relevant gates remain unmet.

## Machine-local reproducibility artifacts

- `g4_rrna_reference_match_v1.py`
- `qc/gse89223_raw_v1_20261009/rrna_diagnostic_v1/NR_*.fasta`
- `qc/gse89223_raw_v1_20261009/rrna_diagnostic_v1/reference_manifest.tsv`
- `qc/gse89223_raw_v1_20261009/rrna_diagnostic_v1/rrna_50mer_hits.tsv`
- `qc/gse89223_raw_v1_20261009/rrna_diagnostic_v1/summary.json`

Small summary only committed here; FASTQ archives and full per-sample reports remain outside Git.
