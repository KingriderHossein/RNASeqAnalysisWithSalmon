# GSE89223 — G3 comprehensive-reference validation (in progress)

**Report date:** 2026-10-09  
**Owning work item:** [Issue #3](https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/3)  
**Branch:** `reference/g3-validation-20261009`  
**Stage:** G3, not yet accepted; awaiting completed Salmon index and duplicate-ID policy.

## Design authority

The project uses an exact GRCh37.p13/GENCODE release-19 combination for GSE89223; the **primary** index includes all transcript types, not a protein-coding-only subset, and includes full-genome decoys. Single-end quantification is a later G5 work item. These rules are recorded in `docs/ARCHITECTURE.md`, `docs/DECISIONS.md`, `AGENTS.md`, and Issue #3.

## Downloaded inputs (EBI GENCODE 19 release directory)

| Input | URL | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| GTF | https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_19/gencode.v19.annotation.gtf.gz | 37,991,892 | `9909e8123a40eca090c3f8e501633f7cc65cd6c4adb83b95009745eca3564d70` |
| GRCh37.p13 genome | https://ftp.ebi.ac.uk/pub/databases/gencode/Gencode_human/release_19/GRCh37.p13.genome.fa.gz | 804,605,548 | `a37049f50cd181ffd057e8edde5f8d5a39afd3fed89ef17eb423323507ae2dd6` |

Both gzip archives passed `gzip -t`. The GRCh37 and GTF contig naming is compatible in the examined and full extracted data. Local compressed sources remain unchanged.

## Transcript generation

The machine-local `gffread` executable is **0.12.7**, installed by unpacking Ubuntu gffread and its libgclib dependency into the project's tool directory, without system-wide package changes. Used with uncompressed GTF because the first attempt passed compressed input to this binary and failed. The corrected extraction used:

```bash
gffread gencode.v19.annotation.gtf -g GRCh37.p13.genome.fa -w gencode.v19.comprehensive.derived_transcripts.fa -F
```

**Verified:** 196,520 transcript records / 196,520 unique GTF transcript IDs, **0** unexpected, **0** unextracted; 57,820 genes; 25 annotated contigs; 297 genome FASTA sequence records.

`gencode.v19.comprehensive.derived_transcripts.fa` SHA-256: `372ea593f52f4d5141c67467f3f7b9abcf53a936e233bcc2e8a911c8eb695d49`.

GTF-derived `derived/tx2gene.tsv`: exactly 196,520 transcript mappings, SHA-256 `31391eaf0186d3b49e92de796c0aeb7b7e979d7018f388821b2e415b64f5b4f1`.

`derived/transcript_biotypes.tsv`: SHA-256 `399c002df66700303229867bb6b0ca50b5142a07e27688edba5644daa886983c`.

## Genome decoys and Salmon build

Constructed the 297-name `derived/decoys.txt` from the same genome FASTA, retaining genome record order and appending all genome sequences **after** the 196,520 transcript sequences in the combined FASTA:

- `derived/decoys.txt` SHA-256: `bfb0fd5b21b2675e1ba6e1756bf85094514f3fd9814c0d93186bee5cc54be8b7`
- `derived/gencode_v19_transcripts_plus_GRCh37p13_decoys.fa` SHA-256: `807588027866b52fd7f690f84b4ccf1b4ff0e7c52663744ab96446be966fabf0`

At report drafting, `salmon index` **was still running**, using Salmon **2.8.0**, selective-alignment-capable decoy-aware k=31 index, no `--sketch`:

```bash
salmon index \
  --transcripts derived/gencode_v19_transcripts_plus_GRCh37p13_decoys.fa \
  --decoys derived/decoys.txt \
  --index salmon_v2.8.0_decoyaware_k31 \
  --kmerLen 31 --threads 6 --ramLimit 12 --gencode
```

**Do not use this incomplete index for quantification until the zero exit status, structural checks and immutable identity manifest are recorded.**

## Exact-sequence duplicate ambiguity — material finding

The Salmon index builder emitted `duplicate_clusters.tsv` covering **1,633 collapsed exact-sequence duplicate transcripts**. Joining both IDs in this file to the same pinned `tx2gene.tsv` gave:

| Comparison | Result |
|---|---:|
| Duplicated transcript pairs within one gene | 720 |
| Duplicated transcript pairs **between distinct genes** | 913 |
| Transcripts retained under this collapse model | 194,887 |
| Genes lacking any retained transcript if each removed duplicate is absent from quantification | **469** |
| Of these gene entries, annotated as protein-coding | **189** |

This issue **does not mean source/annotation corruption**. Transcript sequence identity cannot distinguish the underlying gene, and collapsing to one representative can remove gene labels even when original reference FASTA was comprehensive. Conversely, `--keepDuplicates` preserves identities but does not magically resolve non-identifiable sequence evidence.

**Acceptance risk:** Before declaring the primary transcript universe fixed or interpreting `tximport` gene expression, decide and document whether to build a primary `--keepDuplicates` index, and quantify how uncertain/indistinguishable gene groups are handled. Do not silently treat missing identifiers as biological zero. The initially built default-collapsing index must not be silently promoted as an accepted G3 reference.

## Evidence and next required checks

- Read-only validation program: `scripts/validate_reference_g3.py` (this PR), which checks pinned source hashes, all transcript IDs, complete `tx2gene` coverage, genome/decoy order, combined FASTA order, and duplicate-cluster cross-gene frequency. Its exit code remains **2** while the build lacks a verified successful termination marker.
- Require exact final index file names, Salmon version, model/index metadata, SHA-256 identity, and a reproducible manifest of the completed index.
- Review the duplicate handling policy and confirm whether `quant.sf` contains every required transcript or an explicitly documented representative subset.
- Gate **G3 remains OPEN**. Gate G4 remains independently OPEN; no accepted G5 Salmon pilot or full run has been initiated.

All large source/archive/FASTA/index data and FASTQs remain **off GitHub** on the user's analysis machine. No 3-TB HDD mutation, deletion, raw FASTQ modification, or re-download is required.
