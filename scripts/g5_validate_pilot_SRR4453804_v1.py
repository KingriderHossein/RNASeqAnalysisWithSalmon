#!/usr/bin/env python3
"""Read-only GSE89223 G5 one-sample Salmon 2.8.0 technical pilot audit."""
from pathlib import Path
import csv,json,math,hashlib,datetime,sys
ROOT=Path("/home/kingrider/GSE89223_download")
D=ROOT/"pilots/G5_SRR4453804_keepDuplicates_raw_v1"
REF=ROOT/"reference/G3_GENCODEv19_GRCh37p13"
def check(x,msg):
 if not x:raise RuntimeError(msg)
check((D/"exit_code.txt").exists(),"pilot still incomplete")
check((D/"exit_code.txt").read_text().strip()=="0","pilot failed")
required=["quant.sf","cmd_info.json","lib_format_counts.json","aux_info/meta_info.json","aux_info/ambig_info.tsv","libParams/flenDist.txt","logs/salmon_quant.log"]
for p in required:check((D/p).is_file() and (D/p).stat().st_size>0,"missing required artifact: "+p)
met=json.loads((D/"aux_info/meta_info.json").read_text())
idx=json.loads((REF/"salmon_v2.8.0_decoyaware_keepDuplicates_k31/info.json").read_text())
raw=json.loads((ROOT/"metadata/fastq_conversion_state/SRR4453804.json").read_text())
validation=json.loads((REF/"derived/G3_keepDuplicates_validation_v1.json").read_text())
check(validation["validation"]=="PASS" and idx["keep_duplicates"] is True,"reference identity acceptance missing")
check(met["salmon_version"]=="2.8.0","salmon version mismatch")
check(met.get("keep_duplicates") is True,"pilot reference duplicates unexpectedly collapsed")
check(met.get("num_processed")==raw["reads"],"processed FASTQ read count mismatch")
check(met.get("index_seq_hash")==idx["seq_hash"],"pilot index hash mismatch")
check(met.get("num_valid_targets")==idx["num_refs"]-idx["num_decoys"],"quant targets and reference target count mismatch")
with (D/"quant.sf").open() as f:
 q=list(csv.DictReader(f,delimiter="\t"))
with (REF/"derived/tx2gene.tsv").open() as f:
 tids={x["transcript_id"] for x in csv.DictReader(f,delimiter="\t")}
check(len(q)==len(tids)==196520,"quant rows differ from comprehensive transcriptome")
names={r["Name"] for r in q}
check(names==tids,"quant IDs and GENCODE-v19 tx2gene mismatch")
for r in q:
 for k in ("TPM","NumReads","EffectiveLength"):
  val=float(r[k]);check(math.isfinite(val) and val>=0,"bad numeric "+k)
total_reads=sum(float(x["NumReads"]) for x in q)
summary={"status":"PASS_TECHNICAL_PILOT_NOT_FORMAL_G5","run":"SRR4453804",
"observed_at_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(),
"input_reads":met["num_processed"],"mapped_reads":met["num_mapped"],
"percent_mapped":met.get("percent_mapped"),"decoy_reads":met.get("num_decoy_fragments"),
"num_transcript_rows":len(q),"reference_gene_ids":57820,
"sum_estimated_fragment_counts":total_reads,
"salmon_version":met["salmon_version"],"index_name_hash":met.get("index_name_hash"),
"fragment_length_source":met.get("frag_length_source"),
"em_converged":met.get("em_converged"),"library_types":met.get("library_types"),
"detected_library_type":met.get("detected_library_type"),
"warnings":met.get("diagnostics",[]),"checks":required}
report=D/"pilot_validation_v1.json"
check(not report.exists(),"validation report exists, refusing overwrite")
report.write_text(json.dumps(summary,indent=2,ensure_ascii=False)+"\n")
print("G5_PILOT_TECHNICAL_PASS",json.dumps({k:summary[k] for k in ("input_reads","mapped_reads","percent_mapped","decoy_reads","num_transcript_rows","em_converged","detected_library_type")}),flush=True)
