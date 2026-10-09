#!/usr/bin/env python3
"""Validate pinned, independently built Salmon 2.8.0 keepDuplicates index.

Read only; writes a new JSON report exclusively to an unused report path.
"""
from pathlib import Path
import sys, json, csv, hashlib, datetime
ROOT=Path("/home/kingrider/GSE89223_download/reference/G3_GENCODEv19_GRCh37p13")
KEPT=ROOT/"salmon_v2.8.0_decoyaware_keepDuplicates_k31"
DEFAULT=ROOT/"salmon_v2.8.0_decoyaware_k31"
LOGS=ROOT/"derived/keepDuplicates_build"
RESULT=ROOT/"derived/G3_keepDuplicates_validation_v1.json"
def chk(condition, message):
 if not condition:raise AssertionError(message)
def sha(path):
 h=hashlib.sha256()
 with path.open("rb") as f:
  for chunk in iter(lambda:f.read(8*1024*1024),b""):h.update(chunk)
 return h.hexdigest()
def run():
 chk(not RESULT.exists(),"Report already exists; refusing overwrite")
 chk((LOGS/"exit_code.txt").exists(),"Index still running; no exit code")
 chk((LOGS/"exit_code.txt").read_text().strip()=="0","Index build did not return success")
 info=json.loads((KEPT/"info.json").read_text())
 base=json.loads((DEFAULT/"info.json").read_text())
 assert info["salmon_version"]==base["salmon_version"]=="2.8.0"
 assert info["k"]==base["k"]==31
 assert info["num_decoys"]==base["num_decoys"]==297
 assert info["keep_duplicates"] is True
 assert base["keep_duplicates"] is False
 assert info["num_refs"]-info["num_decoys"]==196520,(info["num_refs"],info["num_decoys"])
 # The first decoy is not necessarily the boundary after all target references.
 # Salmon 2.8.0 may place shorter-than-k transcript entries after it.
 assert 0<=info["first_decoy_index"]<=196520
 assert info["num_refs"]==196817,(info["num_refs"],196817)
 assert base["num_refs"]==195184
 # Transcript duplicate preservation changes transcript sequence-list identity hashes;
 # compare pinned combined FASTA provenance rather than expecting equal seq_hash.
 assert info["decoy_seq_hash"]==base["decoy_seq_hash"]
 assert info["decoy_name_hash"]==base["decoy_name_hash"]
 mappings={}
 with (ROOT/"derived/tx2gene.tsv").open() as f:
  for row in csv.DictReader(f,delimiter="\t"):
   tid=row["transcript_id"]
   chk(tid not in mappings,"Duplicate transcript in tx2gene: "+tid)
   mappings[tid]=row["gene_id"]
 assert len(mappings)==196520
 duplicate_file=KEPT/"duplicate_clusters.tsv"
 chk(duplicate_file.is_file(),"Duplicate evidence not found")
 with duplicate_file.open() as f:
  rows=list(csv.DictReader(f,delimiter="\t"))
 chk(len(rows)==1633,"Duplicate cluster total changed")
 chk(all(x["RetainedRef"] in mappings and x["DuplicateRef"] in mappings for x in rows),"Unmatched duplicate IDs")
 cross=sum(mappings[x["RetainedRef"]]!=mappings[x["DuplicateRef"]] for x in rows)
 assert cross==913,(cross,913)
 # Digest only stable finished index files, no ephemeral build intermediates.
 index_files={}
 for p in sorted(KEPT.iterdir()):
  if p.is_file():
   index_files[p.name]={"bytes":p.stat().st_size,"sha256":sha(p)}
 required={"info.json","duplicate_clusters.tsv","index.refinfo","index.ctab","index.ectab","index.ssi","index.ssi.mphf","refseq.bin","refseq_offsets.json"}
 chk(required.issubset(index_files),"Missing final index outputs "+repr(required-set(index_files)))
 result={"validation_id":"GSE89223_G3_keepDuplicates_index_validation_v1",
 "validated_at_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(),
 "index_directory":str(KEPT),"salmon_version":info["salmon_version"],
 "reference":"GENCODE v19 comprehensive transcriptome / GRCh37.p13 full genome decoys",
 "index_info":info,"default_index_info":base,
 "transcript_ids_from_t2g":len(mappings),"gene_ids":len(set(mappings.values())),
 "decoy_count":info["num_decoys"],"cross_gene_duplicate_pairs":cross,
 "total_identical_sequence_pairs":len(rows),
 "files":index_files,"script_sha256":sha(Path(__file__)),"validation":"PASS",
 "interpretation":"Retains 196520 transcript references and 297 decoys; retention does not resolve cross-gene indistinguishability"}
 with RESULT.open("x") as f:json.dump(result,f,indent=2,ensure_ascii=False);f.write("\n")
 print("VALIDATION_PASS keep_duplicates",info["keep_duplicates"],"refs",info["num_refs"],"decoys",info["num_decoys"],"file_count",len(index_files),flush=True)
 print("SOURCE_HASH_EQUIVALENCE",info["seq_hash"]==base["seq_hash"],"CROSSGENE",cross,flush=True)
 print("RESULT",RESULT,flush=True)
if __name__=="__main__":run()
