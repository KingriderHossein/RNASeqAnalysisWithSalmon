# G5 independent genome-control metric contract (2026-10-09)

The user's required three *separately labeled* metrics are:

1. **STAR genome alignment rate**: uniquely mapped + accepted multi-locus mapped READS (not SAM records) divided by all input reads; additionally list unique/multi/too-many/unmapped separately, from `Log.final.out`.
2. **STAR → HTSeq gene assignment rate**: reads actually assigned to a specific annotated gene by HTSeq divided by *all ORIGINAL input reads*; also report assigned divided by reads eligible at HTSeq input, ambiguous/no_feature/low_quality/alignment_not_unique and protocol settings. Do not conflate genome-mapping rate and gene-count assignment.
3. **Salmon transcript mapping rate**: `num_mapped / num_processed` from `aux_info/meta_info.json` using same input subset, index and sample accession.

**Comparability:** same deterministic 10k FASTQs (SRR4453804 tumor / SRR4453814 adjacent-normal), same GRCh37.p13 genome and GENCODE v19 GTF, same original denominator, preserve strand/library handling and multimap definitions explicitly. No fabrication: report N/A for unexecuted STAR and HTSeq.

**Resource gate:** Remote Linux machine 30 GiB total RAM / ~23 GiB available at preflight, 193 GiB free SSD. Default human STAR SA needs ~27-30 GiB; cannot safely build default with unbounded RAM. Adopt separately isolated sparse index for human reference with `--genomeSAsparseD 3 --genomeSAindexNbases 12 --limitGenomeGenerateRAM 15000000000` as documented by STAR author; may alter speed but should retain accuracy. Verify options/version, avoid OOM, reuse existing genome and GTF, do not modify frozen Salmon index or source FASTQs. STAR and HTSeq initially missing; isolated conda prefix installation initiated at `audits/G5_genome_control_20261009/conda_STAR_HTSeq`, log `01_tools_install.log`. Never claim tools or index completed without exit/proof. Installation may be pending.

**User monitor commitment:** announce exact command and tail log path before each material host process; capture time, full command, exit status and output for every step.

Evidence: `/home/kingrider/GSE89223_download/audits/G5_genome_control_20261009/00_preflight.log`. Initial preflight observed source FASTA 3.23 GB, matching GTF 1.17 GB, index STAR absent, STAR/HTSeq executables absent, 30GiB RAM and 193GB free. Full 32-run remains blocked until G4/G5 formal decision.

STAR resource sources: https://pmc.ncbi.nlm.nih.gov/articles/PMC4631051/ and https://github.com/alexdobin/STAR/discussions/1247 .
