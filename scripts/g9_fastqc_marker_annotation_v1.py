#!/usr/bin/env python3
"""G9: non-destructive FastQC and GENCODE-v19 marker annotations."""
from pathlib import Path
import csv, json, re, zipfile, statistics, hashlib
R=Path("/home/kingrider/GSE89223_download")
A=R/"audits/G9_closure_20261009"
A.mkdir(exist_ok=False)
MAN=Path("/home/kingrider/Projects/RNASeqAnalysisWithSalmon/metadata/derived/GSE89223_sample_manifest.tsv")
REF=R/"reference/G3_GENCODEv19_GRCh37p13/gencode.v19.annotation.gtf"
NAMES=("PCA3","AMACR","ANKRD34B","NEK5","KCNG3","PTPRT","KLK3")
record={}
pat={k:re.compile(r'\b'+k+r' "([^"]+)"') for k in ("gene_id","gene_name","gene_type")}
with REF.open() as f:
    for line in f:
        if line.startswith("#"):continue
        fields=line.split("\t",8)
        if len(fields)!=9 or fields[2]!="gene":continue
        if not any((f'gene_name "{target}"' in fields[8]) for target in NAMES):continue
        vals={k:p.search(fields[8]) for k,p in pat.items()}
        if any(v is None for v in vals.values()):continue
        gd={k:v.group(1) for k,v in vals.items()}
        if gd["gene_name"] not in NAMES:continue
        record.setdefault(gd["gene_name"],[]).append(gd)
print("GENCODE_SOURCE",REF,flush=True)
print("GENCODE_MARKER_COUNTS",{n:len(record.get(n,[])) for n in NAMES},flush=True)
with (A/"marker_annotation_v19.csv").open("x",newline="") as f:
    w=csv.DictWriter(f,fieldnames=["gene_name","gene_id","gene_type"])
    w.writeheader()
    for n in NAMES:
        for item in record.get(n,[]):w.writerow(item)
assert all(len(record.get(n,[]))==1 for n in NAMES),"Gene symbols absent or ambiguous"
manifest=list(csv.DictReader(MAN.open(),delimiter="\t"))
assert len(manifest)==32 and len(set(x["run"] for x in manifest))==32
qc=[]
for m in manifest:
    acc=m["run"]
    p=R/"qc/gse89223_raw_v1_20261009/fastqc"/(acc+"_fastqc.zip")
    assert p.is_file(),p
    with zipfile.ZipFile(p) as z:
        prefix=acc+"_fastqc/"
        summaries=[row.split("\t") for row in z.read(prefix+"summary.txt").decode().splitlines()]
        status={item[1]:item[0] for item in summaries}
        d=z.read(prefix+"fastqc_data.txt").decode().splitlines()
    vals={}
    active=None
    dedup=None
    for row in d:
        if row.startswith(">>"):
            active=None if row.startswith(">>END") else row[2:].split("\t")[0]
        elif active=="Basic Statistics" and "\t" in row and not row.startswith("#"):
            k,v=row.split("\t",1)
            vals[k]=v
        elif active=="Sequence Duplication Levels" and row.startswith("#Total Deduplicated Percentage"):
            dedup=float(row.split("\t")[1])
    assert int(vals["Total Sequences"])==int(m["spots"]),acc
    metadata=json.loads((R/"salmon/G6_SF_raw_v1"/acc/"aux_info/meta_info.json").read_text())
    assert metadata["num_processed"]==int(m["spots"]),acc
    qc.append({
        "run":acc,"group":m["analysis_group"],"patient_id":m["patient_id"],
        "paper_final_set":m["paper_final_set"],"paired_pca_sensitivity":m["paired_pca_sensitivity"],
        "reads":int(m["spots"]),"mean_length_fastqc":float(vals["Mean Length"]),
        "read_length_range":vals["Sequence length"],"gc_pct":float(vals["%GC"]),
        "fastqc_deduplicated_pct":dedup,
        "fastqc_base_quality":status.get("Per base sequence quality","unknown"),
        "fastqc_adapter":status.get("Adapter Content","unknown"),
        "fastqc_duplication":status.get("Sequence Duplication Levels","unknown"),
        "salmon_mapping_pct":float(metadata["percent_mapped"]),
        "salmon_decoy_pct":100*metadata["num_decoy_fragments"]/metadata["num_processed"],
        "salmon_mapped_reads":int(metadata["num_mapped"]),
    })
with (A/"fastqc_salmon_samples_v1.csv").open("x",newline="") as f:
    w=csv.DictWriter(f,fieldnames=qc[0].keys());w.writeheader();w.writerows(qc)
print("ALL_SAMPLES_FASTQC_SALMON_JOINED",len(qc),flush=True)
print("FASTQC_BASE_QUALITY_STATUS",{s:sum(x["fastqc_base_quality"]==s for x in qc) for s in ("PASS","WARN","FAIL")},flush=True)
print("FASTQC_ADAPTER_STATUS",{s:sum(x["fastqc_adapter"]==s for x in qc) for s in ("PASS","WARN","FAIL")},flush=True)
for acc in ("SRR4453804","SRR4453805","SRR4453794","SRR4453814"):
    x=next(z for z in qc if z["run"]==acc)
    print("SPECIAL",json.dumps(x,ensure_ascii=False),flush=True)
print("G9_INPUTS_READY",flush=True)