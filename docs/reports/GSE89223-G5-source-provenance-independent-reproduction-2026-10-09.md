# Independent provenance and scientific correction audit — GSE89223 G5 (2026-10-09)

## 1. Verified real Salmon execution and reproducibility

On Linux host `kingrider`, resolved executable: `/home/kingrider/anaconda3/envs/rnaseq/bin/salmon`; Bioconda `salmon 2.8.0 hfa8f182_0`. Actual ELF binary SHA-256: `865d2f7064d36c8638c7c9bffffa8f630ecd549be2ad120db645537bc094ec9e`. Existing pilot run provenance contains full `command.txt`, start/end time, stderr logs, `exit_code.txt=0`, and `quant.sf` for SRR4453804 (U & SF), SRR4453814 (SF).

**Independent rerun** on the existing stratified 10,000-read SRR4453804 FASTQ (`SHA-256 3b81155272812b24d333bad62657ba5000164aee01d4f08e1ad0e9e0493ecb90`) and pinned keepDuplicates GRCh37.p13/GENCODE v19 index, `-l SF --fldMean 150 --fldSD 50 --minScoreFraction 0.65`, regenerated `quant.sf` byte-identical to previous source experiment. Both SHA-256: `120260ef9e02377c14fd7346c52fa7f10722fca4b930f5fd34e224b6bfd6fe50`. Both: 10,000 processed, 1,318 mapped, 5,491 genome-decoy classifications, 2,736 filtered_vm, index sequence hash match, EM converged. This proves computational reproducibility of this experiment, NOT biological truth.

## 2. Source-grounded scientific design correction

Published source: Nikitina et al., *Novel RNA biomarkers of prostate cancer revealed by RNA-seq analysis of formalin-fixed samples obtained from Russian patients* (Oncotarget, 2017), https://pmc.ncbi.nlm.nih.gov/articles/PMC5464844/ . The paper explicitly: FFPE prostate tissue, Ion Proton, RiboMinus rRNA depletion protocol, FastQC, **Cutadapt read filtering/trimming**, genome mapping **STAR to hg19**, **HTSeq** gene counting using GENCODE v19 / GRCh37.p13, excludes ribosomal RNA mapped reads from subsequent analyses, edgeR DE. Our Salmon pilot deliberately processes raw FASTQ without trimming, on a transcript-only quantification competition against **whole-genome decoys**. **Its 7–13% transcript-assignment mapping rate is not equivalent to a STAR genomic alignment rate**. The decoy classification excludes genome-like reads from transcript abundance but cannot be naively called rRNA contamination: they need read-level validation/reference attribution. RNA fragmentation, library composition and transcriptome-target scope are possible contributors.

Salmon 2.8.0 is real official Rust v2 implementation (major-version rewrite and index-format difference): https://github.com/COMBINE-lab/salmon ; https://bioconda.github.io/recipes/salmon/README.html . Passing software execution is NOT sufficient to establish original-paper method replication; treat Salmon as a *distinct analytical comparison*.

## 3. FastQC review and quality-trim subset sensitivity

Source FastQC reports for SRR4453804 and SRR4453814: both **Adapter Content PASS**, both **Per-base sequence quality FAIL**, **Sequence Duplication Levels FAIL**, **Overrepresented Sequences WARN**; length 25–363 and 25–368 respectively. Adapter PASS means adapter is not evidenced by that module, NOT a proof that all possible Ion Torrent/adapters are absent. The paper used Cutadapt; this host's existing conda environment has **no cutadapt executable/module** as of audit. Avoid changing frozen environment or installing by stealth.

A *limited exploratory Q20 3'-end-tail clipping*, retaining only reads >=31nt, was implemented separately on the same 10k deterministic FASTQs, without modifying originals or claiming equivalence to Cutadapt:

| Sample | Raw mapped / 10k | Q20 clipped reads kept | Dropped below 31nt | Trimmed mapped | Trimmed map % of retained | Trimmed mapped / original 10k |
|---|---:|---:|---:|---:|---:|---:|
| SRR4453804 | 1318 | 9864 | 136 | 1329 | 13.4732% | 13.29% |
| SRR4453814 | 766 | 9771 | 229 | 763 | 7.8088% | 7.63% |

Improvement relative to all ORIGINAL FASTQ reads is tiny (+11/10k tumor, -3/10k control), with short-read exclusions. Thus **quality tail-clipping alone did not rescue low transcript-mapping rates** on these subsets. No recommendation to apply this heuristic to 32 runs.

## 4. Explicit scientific concerns / next decisive experiment

- Distinguish transcript-target mapping, full-genome-decoy assignment, and **true rRNA identity** through a controlled competitively aligned reference and read-level evidence; don't equate two tools' percentages with overlap.
- Quantify how much of the nontranscript alignment signal is rRNA and how much lies outside GENCODE annotated transcript targets. A genome alignment control on same subset with an appropriate aligner and explicit index is more informative than endless threshold tuning. Confirm published STAR settings and available tools before attempting any full genomic alignment.
- For an *actual paper-method comparison*, include a bounded Cutadapt-based filtered sample branch, with documented adapter specifications and QC, and classify as a secondary sensitivity analysis until choice is frozen.
- Watch transcript/gene identifiability due to 1,633 duplicate exact-sequence transcripts in comprehensive reference, 913 cross-gene duplicate pairs, and 39 shorter-than-k transcript ordering semantics. Audit G3 built index is structurally valid but not proof every gene is identifiable.
- Freeze only after judging whether low assignment is expected for this library preparation and study aim, the fragment-length prior sensitivity, confirmed strandedness and transcriptome target coverage.

## Transparent monitor and evidence

Live monitor: `http://127.0.0.1:8766` on Linux.
Host-local immutable evidence paths:
`/home/kingrider/GSE89223_download/audits/G5_source_verification_20261009/01_binary_and_provenance.log`
`.../02_reproduction.log`
`.../03_qc_review.log`
`.../04_quality_sensitivity.log`
and isolated quant output `.../independent_SRR4453804_SF_10k_reproduction/` and `.../quality_tail_q20_len31_v1/`.
No raw FASTQ or shared index mutation. **Full 32-sample run not started.** Formal G4/G5 gates remain open.
