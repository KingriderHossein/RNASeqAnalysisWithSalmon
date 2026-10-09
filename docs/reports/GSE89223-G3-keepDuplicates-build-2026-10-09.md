# GSE89223 G3 keepDuplicates candidate build (2026-10-09)

The default index from the pinned comprehensive GENCODE v19 / GRCh37.p13 reference **completed successfully** on 2026-10-09 07:09:52 host-local. A read-only audit passed with 196,520 GTF/FASTA/tx2gene transcript identities, 57,820 genes and 297 exact matching genome decoys; index `info.json` reports Salmon 2.8.0, k=31, `keep_duplicates=false`, 195,184 total indexed references and 297 decoys. Source FASTQs unchanged.

The default index dropped 1,633 exact duplicate transcript identifiers; 913 pairs cross gene IDs and the resulting catalog lacks transcript representation for 469 gene IDs (189 protein-coding). Salmon 2.8.0 synthetic fixture independently confirmed `--keepDuplicates` retains the duplicate identifiers in `quant.sf` while default index does not. This is an identity-preservation choice, not a claim that reads can distinguish identical genes.

**Isolated alternative index build started** at 2026-10-09 07:28:31 local host time, path:
`/home/kingrider/GSE89223_download/reference/G3_GENCODEv19_GRCh37p13/salmon_v2.8.0_decoyaware_keepDuplicates_k31`.

Command:
```bash
salmon index --transcripts derived/gencode_v19_transcripts_plus_GRCh37p13_decoys.fa \
  --decoys derived/decoys.txt \
  --index salmon_v2.8.0_decoyaware_keepDuplicates_k31 \
  --kmerLen 31 --threads 6 --ramLimit 12 --gencode --keepDuplicates
```

The new index is **building, not complete**. Its log expressly reports **1,633 exact-sequence duplicates retained**. The run records `started_at.txt`, `salmon_stderr.log`, `salmon_stdout.log`, `exit_code.txt` on completion. Validation must inspect successful exit, `info.json.keep_duplicates=true`, reference-name coverage, 297 decoys, structural metadata, a complete index digest, and bounded pilot quantification. The complete G3 acceptance remains open.

**G4**: proposed baseline is unchanged raw single-end FASTQ, without blanket trimming, deduplication, rRNA filtering or sample exclusion. The original project Issue #4 explicitly depends on the still-open Module A #14 acquisition automation workstream for full formal G4 acceptance. Therefore a scientific pilot may be treated as **provisional validation** if run after index validation, but it must not be misrepresented as closing the integrated G4/G5 project gates.

Disk free at build initiation: ~200 GiB; no removal or overwrite of default index, raw files or USB archive.
