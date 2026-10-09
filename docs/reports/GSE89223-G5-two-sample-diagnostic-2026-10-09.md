# G5 two-sample diagnostic gate — 2026-10-09

**Scope:** Pilot-only, do not quantify all 32 yet. User-approved diagnostic follow-up to low mapping / high decoy assignment. Both runs use the same Salmon 2.8.0 GRCh37.p13/GENCODE v19 full-genome decoy-aware `--keepDuplicates` index (k=31), raw single-end FASTQ, `-l SF`, and exploratory fragment-length prior 150 ± 50. Both emit 196,520 unique transcript IDs, TPM sum 1,000,000, matching index sequence hash, EM converged and no quant errors.

| Metric | SRR4453804 (tumor) | SRR4453814 (control) |
| --- | ---: | ---: |
| Processed | 5,575,676 | 7,940,368 |
| Mapped | 749,597 (13.4441%) | 625,965 (7.8833%) |
| Decoy-assigned | 3,049,750 (54.6974%) | 3,879,343 (48.8560%) |
| `num_fragments_filtered_vm` | 1,535,691 (27.5427%) | 2,873,111 (36.1836%) |
| Remainder after these three categories | 240,638 | 561,949 |
| Salmon-detected strand | SF | SF |
| Salmon warning | low_mapping_rate | very_low_mapping_rate |

**Careful terminology:** `num_fragments_filtered_vm` is Salmon's internal selective-alignment filtration count; do not label every instance definitively as a separate low-quality read. `num_alignments_below_threshold_for_mapped_fragments_vm` is an alignment count and must not be added to the fragment partition. The decoy count measures Salmon's classification and **does not prove rRNA identity**.

**QC context:** FastQC/MultiQC was run on all 32 sample FASTQs. SRR4453804: 5,575,676 reads, length 25–363, GC 51%. SRR4453814: 7,940,368 reads, length 25–368, GC 54%. Earlier stratified and competing reference rRNA screens on 32 runs suggest substantial rRNA-related signal but do not isolate all decoy classes.

**Decision:** G3 index is technically validated; 2 pilot quantifications are technically valid. `SF` is favored over `U` by detected orientation and mismatch warnings, but the exceptionally low mapping in independent control demands investigation before acceptance G5/G6. Preserve original FASTQs and archive; no mass quantification, global threshold relaxation or automatic trimming/rRNA removal. Next discriminating test: controlled diagnostic map of representative read subsets against mature rRNA references and genome versus transcriptome, plus a low-score alignment sensitivity test with explicitly distinct output; verify per-class fractions and read-level overlap without conflating filter counts.

**Local evidence:** `/home/kingrider/GSE89223_download/pilots/G5_SRR4453804_keepDuplicates_raw_SF_fld150_50_v1/` and `.../G5_SRR4453814_keepDuplicates_raw_SF_fld150_50_v1/`; FASTQ checksum for new independent control was checked against source conversion provenance before running. Formal G4 and G5 gates remain open.
