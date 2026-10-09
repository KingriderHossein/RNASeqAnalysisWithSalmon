# G3 duplicate-ID handling: executable synthetic Salmon 2.8.0 check

**Issue #3; 2026-10-09; draft PR #52; G3 OPEN.**

## Context and risk

The comprehensive GENCODE v19/GRCh37.p13 reference has 196,520 unique versioned transcript IDs and 57,820 GTF gene IDs. Under the default Salmon 2.8.0 indexer settings, 1,633 exact-duplicate transcript pairs appear in `duplicate_clusters.tsv`. Of these, 913 pair a retained and removed transcript from **different** gene IDs. Using the actual `tx2gene.tsv`, 469 genes have no retained transcript IDs if duplicates are collapsed, 189 of them annotated `protein_coding`. Thus default exact-sequence deduplication changes the effective target universe.

## Executed technical discriminating test

On authorized Linux host, executed a fully *synthetic* reference and FASTQ fixture using locally installed Salmon **2.8.0** (no biological FASTQ was modified):

- Three transcript IDs: `ENST_A.1` and `ENST_B.1` with exact identical sequence; `ENST_C.1` with distinct sequence; one decoy `chrTest`.
- Built k=31 indexes in two isolated directories, one default and one with `--keepDuplicates`.
- Ran single-end read quantification into two isolated output directories.
- **Default index:** `quant.sf` contains `ENST_A.1`, `ENST_C.1`; the second duplicate ID `ENST_B.1` is absent.
- **`--keepDuplicates` index:** `quant.sf` contains all three IDs, `ENST_A.1`, `ENST_B.1`, `ENST_C.1`.
- Both builds still emit a `duplicate_clusters.tsv` describing the duplicate relationship; the presence of this file is **not** proof that the duplicates were actually discarded.

This test directly demonstrates identifier preservation on the installed Salmon version, **not** an identifiable allocation of reads between identical transcripts from different genes. Such expression estimates remain ambiguous and require transparent reporting or gene-level sensitivity policy.

## Interpretation / recommended primary index

To retain the declared comprehensive transcript universe for subsequent exact-version `tximport`, **`--keepDuplicates` is a scientifically well-motivated primary candidate**, with a separate downstream warning/flag for cross-gene identical-sequence clusters. No change in the architecture's GENCODE release, genome build, selective-alignment mode or decoy policy is proposed.

The existing *default-collapsing* full-genome index build was already running. It is preserved as technical comparator, not silently promoted as the primary accepted G3 reference. Once it finishes and is checked, build the `--keepDuplicates` candidate in an independently named output directory, subject to available disk and memory, then run the reference audit and bounded G5 pilot on the accepted G3 index.

## Provenance and artifacts

Synthetic test script `/home/kingrider/GSE89223_download/g3_salmon_duplicate_fixture_v1.py`; local synthetic FASTQ/index/quant logs under `reference/G3_GENCODEv19_GRCh37p13/derived/duplicate_fixture_v1/`.

The original biological FASTQ and SRA are untouched; no raw or large reference files are committed. **G3 remains OPEN until reference variant selected, full-size index completed, exact index file checksum/manifest and quantitative configuration validated.** G4 and G5 gates are independently OPEN.
