# GSE89223 — prespecified G4/G5 pilot baseline (2026-10-09)

**Decision ID:** `GSE89223_G4_baseline_v1`  
**State:** BASELINE SELECTED FOR TECHNICAL PILOT; **Gate G4 remains OPEN** until Issue #4 acceptance and Module A provenance dependency #14 are satisfied.

## Declared primary technical baseline

- Use the unchanged, validated single-end raw FASTQ for each accession.
- No unconditional adapter or quality trimming; no hard length truncation.
- No automatic in silico rRNA filtering, read deduplication, or QC-only sample exclusion.
- Preserve input read identities and count. All later alternatives must write separate outputs and label sensitivity analysis.

## Empirical basis

Raw FastQC/MultiQC complete for 32/32 GSE89223 runs; four representative nuclear + two mitochondrial rRNA sequences showed a substantial rRNA assignment signal in separately documented full-file stratified sample reads and a competing GENCODE v19 transcript reference. The data are Ion Torrent Proton single-end FFPE and variable-read-length. FastQC WARN/FAIL flags are not biological grounds for trimming/exclusion. The rRNA mapping screen is not an independently validated full-run contamination fraction, nor a reason to remove those reads *before* measuring Salmon behavior.

## What this does not settle

The policy does not waive evidence of a real adapter, severe terminal artifact, or library-related bias. Such evidence may trigger a **documented, predefined sensitivity branch** after the no-trim technical pilot. Run quantification without tuning flags to reproduce the original publication.

## G5 exploratory pilot guardrails

- Pilot accession: `SRR4453804`, paper Track A tumor, single-end Ion Torrent Proton, 5,575,676 validated FASTQ records.
- Index: **completed and independently validated** pinned GENCODE v19 comprehensive transcriptome + GRCh37.p13 full-genome decoys, Salmon 2.8.0 k=31 `--keepDuplicates`. Do not run on a partially built index.
- Map `-r` single-end reads with explicitly declared library assumption `-l U` (unstranded). This is an **assumption to validate**, not proven protocol strandedness.
- Fragment length prior: start with `--fldMean 150 --fldSD 50` solely as an exploratory, reproducible reference parameter; quantify sensitivity to plausible alternative priors and document the library-specific interpretation before freezing G5. Salmon v2.8.0 deterministic mode behavior must be examined from output metadata.
- Initially disable `--seqBias`, `--gcBias`, `--posBias` until pilot diagnostics motivate separate comparisons.
- Required observed artifacts: `quant.sf`, `cmd_info.json`, `lib_format_counts.json`, `aux_info/meta_info.json`, `aux_info/ambig_info.tsv`, `libParams/flenDist.txt`, `logs/salmon_quant.log`.
- Compare output transcript IDs with the same `tx2gene.tsv`. Map/read rates and decoy absorption require interpretation; no biological differential-expression claim from one library.
- Keep pilot outputs off GitHub; commit only scripts, configs, checksums and small summaries.

**Formal acceptance:** technical exploratory run does not itself close G3/G4/G5 or permit cohort-wide processing.
