# Data Contracts — Architecture v2.0

This document defines the required shape, identity, provenance, and allowed transformations for every major data object.

## Sample manifest

Canonical artifact: `metadata/derived/GSE89223_sample_manifest.tsv`.

Required information:
- sample title / sample_id;
- GSM, SRX, SRR, BioSample;
- patient_id;
- diagnosis and tissue;
- analysis_group;
- pair_status;
- Track A / paper-final membership;
- paper group;
- Track B / paired-sensitivity membership;
- exclusion reason;
- library strategy/selection/source/layout;
- platform/model;
- read/spot/bases/average-length/archive-size fields;
- provenance/review flags.

Rules:
- one unambiguous sample/run identity per row;
- duplicate SRR/GSM identities are not accepted silently;
- cohort membership is explicit;
- biology is never inferred from filename order, name prefix, PCA or desired outcome.

## Cohort contract

### Track A
Must resolve to exactly:
- 10 tumor;
- 12 control.

### Track B
Must resolve to exactly:
- 9 matched patients;
- 9 tumor;
- 9 adjacent-normal.

A change requires architecture review.

## Raw-read contract

Each approved input records:
- accession;
- acquisition source;
- local path;
- size;
- checksum when available/created;
- acquisition date;
- read/spot count;
- compression;
- single-end layout;
- source manifest identity.

Raw reads are immutable. A trimmed derivative receives a new path and provenance record; it never overwrites raw input.

G4 acceptance evaluates file-level provenance and QC independently of
the release status of a separate download application.


## QC contract

Per sample:
- FastQC report identity;
- software version;
- input checksum;
- core metrics;
- pass/warn/fail evidence.

Cohort:
- MultiQC report;
- included-sample list;
- QC review decision;
- preprocessing decision.

QC cannot redefine biological labels.

## Reference bundle contract

The reference identity binds:
- GRCh37.p13 assembly;
- GENCODE v19 comprehensive GTF;
- genome FASTA source/checksum;
- GTF source/checksum;
- transcript FASTA derivation command/checksum;
- transcript count;
- gene count;
- biotype summary;
- decoy list/checksum;
- tx2gene/checksum;
- gene-biotype table/checksum;
- Salmon index identity/hash;
- Salmon version used to build index;
- k-mer configuration;
- creation date;
- source URLs.

No file from another release may enter the bundle silently.

## Transcript FASTA contract

The primary transcript FASTA is derived from:
- matching GRCh37.p13 genome FASTA;
- GENCODE v19 comprehensive GTF.

Each transcript header must retain a stable identifier compatible with tx2gene.

Duplicate transcript IDs are a blocker unless explicitly understood and resolved.

## tx2gene contract

Required columns:
- transcript_id;
- gene_id.

Recommended:
- transcript_type;
- gene_type;
- gene_name.

Rules:
- one transcript maps to one canonical gene;
- transcript-version handling is explicit;
- source GTF is identical to the reference annotation.

## Salmon index contract

Identity includes:
- reference bundle ID;
- Salmon version;
- selective-alignment mode;
- decoy policy;
- k-mer setting;
- index path;
- index metadata/hash.

One analysis may not mix indexes.

## Salmon configuration contract

Version-controlled configuration records:
- Salmon version target;
- mapping mode;
- library-type strategy;
- bias-correction flags;
- fragment-length handling policy;
- thread policy;
- index/reference ID;
- version-specific options;
- pilot decision rationale;
- configuration ID/hash.

## Salmon sample-output contract

Retain at minimum:
- `quant.sf`;
- `cmd_info.json`;
- `lib_format_counts.json`;
- `aux_info/meta_info.json`;
- `aux_info/ambig_info.tsv`;
- `libParams/flenDist.txt`;
- `logs/salmon_quant.log`.

Each output maps to exactly one sample_id and analysis_id.

## Quantification manifest

Required fields:
- analysis_id;
- sample_id;
- SRR;
- input checksum;
- preprocessing ID;
- reference ID;
- index ID/hash;
- Salmon version;
- Salmon config ID/hash;
- output path;
- completion state;
- warning state;
- exclusion state/reason.

## tximport contract

The imported gene-level object retains:
- counts;
- abundance;
- sample-specific average transcript lengths;
- exact sample names/order;
- source quant paths;
- tx2gene identity.

The object must be reconstructable from repository configuration plus Salmon outputs.

## DESeq2 metadata contract

Track A:
- sample_id;
- group = tumor/control.

Track B:
- sample_id;
- patient;
- condition = tumor/adjacent_normal.

Reference levels are explicit.

## Differential-expression result contract

For each track retain a complete tested-gene table, not only significant genes.

Required columns:
- gene_id;
- gene_name when available;
- gene_type when available;
- baseMean;
- log2FoldChange;
- lfcSE;
- stat;
- pvalue;
- padj;
- tested/significant flag;
- analysis_id;
- contrast ID.

## Publication-comparison contract

Required matching fields include:
- internal canonical gene_id;
- normalized comparison gene_id;
- publication gene_id;
- gene symbol;
- gene type;
- our log2FC;
- publication direction/logFC when available;
- our padj;
- publication FDR/p-value when available;
- overlap status;
- direction concordance.

## Analysis ID contract

An analysis_id binds:
- cohort revision;
- input revision;
- preprocessing revision;
- reference revision;
- Salmon configuration revision;
- tx2gene revision;
- DE design revision;
- repository commit.

Changing one of these creates a new analysis identity or explicit versioned rerun.

## Provenance rule

Every derived artifact must answer:
- what produced me?
- from which exact input?
- with which versions/configuration?
- under which analysis_id?
- from which repository commit?
- where is my machine-readable provenance?

## Overwrite rule

No authoritative result is silently overwritten.

New runs use a new analysis ID, versioned path, or explicitly reviewed replacement with retained provenance.
