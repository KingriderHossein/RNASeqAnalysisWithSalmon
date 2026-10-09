#!/usr/bin/env python3
"""GSE89223 G3 locked-reference audit, read-only; no data mutation.

Run with: python3 g3_validate_reference_v1.py ROOT_DIR
Print JSON to stdout. "index_complete" stays false until the exact build returns 0.
"""
import collections, csv, hashlib, json, re, sys
from pathlib import Path
def digest(p):
    h=hashlib.sha256()
    with p.open("rb") as f:
        for x in iter(lambda:f.read(8*1024*1024),b""):h.update(x)
    return h.hexdigest()
def fasta_ids(p):
    ids=[]
    with p.open("rb") as f:
        for raw in f:
            if raw.startswith(b">"):
                ids.append(raw[1:].split(maxsplit=1)[0].decode("ascii"))
    return ids
def require(cond,msg):
    if not cond: raise AssertionError(msg)
def main(directory):
    d=Path(directory).resolve()
    assert d.is_dir()
    gt=d/"gencode.v19.annotation.gtf.gz"
    genomegz=d/"GRCh37.p13.genome.fa.gz"
    genome=d/"GRCh37.p13.genome.fa"
    tx=d/"gencode.v19.comprehensive.derived_transcripts.fa"
    dt=d/"derived"
    mapping=dt/"tx2gene.tsv"
    dec=dt/"decoys.txt"
    mixed=dt/"gencode_v19_transcripts_plus_GRCh37p13_decoys.fa"
    idx=d/"salmon_v2.8.0_decoyaware_k31"
    expected={"gencode.v19.annotation.gtf.gz":"9909e8123a40eca090c3f8e501633f7cc65cd6c4adb83b95009745eca3564d70",
              "GRCh37.p13.genome.fa.gz":"a37049f50cd181ffd057e8edde5f8d5a39afd3fed89ef17eb423323507ae2dd6"}
    for p in (gt,genomegz,genome,tx,mapping,dec,mixed):
        require(p.is_file() and p.stat().st_size>0,"Missing: "+str(p))
    for n,v in expected.items(): require(digest(d/n)==v,"Input checksum mismatch: "+n)
    gnames=fasta_ids(genome)
    trnames=fasta_ids(tx)
    mixnames=fasta_ids(mixed)
    decnames=dec.read_text().splitlines()
    require(len(gnames)==len(set(gnames))==len(decnames),"Genome/decoy count or duplicates")
    require(gnames==decnames,"Genome and decoy names/order mismatch")
    require(len(trnames)==len(set(trnames))==196520,"Unexpected transcript counts/duplicate names")
    require(set(gnames).isdisjoint(trnames),"Decoy names clash with transcripts")
    require(mixnames==trnames+gnames,"Combined FASTA headers/order mismatch")
    with mapping.open() as f:
        rows=list(csv.DictReader(f,delimiter="\t"))
    require(len(rows)==len(trnames),"tx2gene row count mismatch")
    m={r["transcript_id"]:r["gene_id"] for r in rows}
    require(len(m)==len(rows) and set(m)==set(trnames),"tx2gene transcript identity differs from FASTA")
    require(all(x.startswith("ENSG") for x in m.values()),"Unexpected gene ID")
    audit=json.loads((dt/"gtf_transcript_audit.json").read_text())
    require(audit["fasta_transcript_count"]==len(trnames) and audit["missing_transcripts"]==audit["unexpected_transcripts"]==0,"GTF audit mismatch")
    require(audit["gtf_sha256"]==expected[gt.name],"GTF differs from audit identity")
    dup=idx/"duplicate_clusters.tsv"
    duplicates=[]
    if dup.is_file():
        with dup.open() as f:duplicates=list(csv.DictReader(f,delimiter="\t"))
    bad=[r for r in duplicates if r["RetainedRef"] not in m or r["DuplicateRef"] not in m]
    require(not bad,"Duplicate cluster names not in tx2gene")
    cross=sum(m[r["RetainedRef"]]!=m[r["DuplicateRef"]] for r in duplicates)
    log=(dt/"index_build.log").read_text()
    complete="INDEX_BUILD_RETURNED_0" in log
    indexfiles=sorted(str(p.relative_to(idx)) for p in idx.rglob("*") if p.is_file())
    results={
      "reference":"GSE89223_GENCODE_v19_GRCh37p13",
      "input_sha256":expected,
      "gtf_genes":audit["gtf_gene_count"],"gtf_transcripts":audit["gtf_transcript_count"],
      "derived_transcripts":len(trnames),"tx2gene_rows":len(m),
      "mapped_genes":len(set(m.values())),"genome_decoys":len(gnames),
      "transcript_fasta_sha256":digest(tx),
      "tx2gene_sha256":digest(mapping),"decoy_names_sha256":digest(dec),
      "combined_fasta_sha256":digest(mixed),
      "exact_sequence_duplicates_collapsed":len(duplicates),
      "cross_gene_duplicate_pairs":cross,
      "index_complete":complete,"index_files":indexfiles,
      "needs_duplicate_policy":cross>0}
    print(json.dumps(results,indent=2))
    return 0 if complete else 2
if __name__=="__main__":
    sys.exit(main(sys.argv[1]))
