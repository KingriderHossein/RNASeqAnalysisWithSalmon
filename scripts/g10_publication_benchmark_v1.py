#!/usr/bin/env python3
import csv,hashlib,json,math,statistics
from pathlib import Path
from collections import Counter
import openpyxl
R=Path("/home/kingrider/GSE89223_download")
A=R/"audits/G10_publication_20261009"
PUB=A/"reference/oncotarget-08-32990-s002.xlsx"
OUR=R/"audits/G8_DESeq2_20261009/track_A/deseq2_full_results.csv"
def norm(s):return str(s).split(".",1)[0].strip() if s else ""
def number(s):
 try:
  x=float(s)
  return x if math.isfinite(x) else None
 except (TypeError,ValueError):return None
w=openpyxl.load_workbook(PUB,read_only=True,data_only=True)
s=w["results_de_plus"]
it=s.values
head=next(it)
expected=("gene_name","ensemble_id","logFC","PValue","FDR","type")
assert tuple(head[:6])==expected,head
pub={}
for row in it:
 key=norm(row[1])
 if not key:continue
 if key in pub:raise RuntimeError("duplicated public id: "+key)
 pub[key]={"id":key,"symbol":str(row[0]),"published_lfc":number(row[2]),"published_p":number(row[3]),"published_fdr":number(row[4]),"gene_type":str(row[5])}
assert len(pub)==3384
assert all(x["published_fdr"] is not None and x["published_fdr"]<.05 for x in pub.values())
ours={}
with OUR.open() as f:
 reader=csv.DictReader(f)
 for row in reader:
  key=norm(row["gene_id"])
  if key in ours:raise RuntimeError("our duplicated comparison id "+key)
  ours[key]={"id":row["gene_id"],"lfc":number(row["log2FoldChange"]),"padj":number(row["padj"]),"pvalue":number(row["pvalue"]),"sig":row["significant"]=="TRUE"}
our_de={k for k,x in ours.items() if x["sig"] and x["padj"] is not None and x["padj"]<.05}
pub_de=set(pub)
overlap=our_de&pub_de
union=our_de|pub_de
pub_in_tested=set(pub)&set(ours)
upub=Counter("up" if z["published_lfc"]>0 else "down" for z in pub.values())
all_pub_in_our=len(pub_in_tested)
both=[(pub[k]["published_lfc"],ours[k]["lfc"]) for k in overlap if pub[k]["published_lfc"] is not None and ours[k]["lfc"] is not None]
concord=sum(1 for x,y in both if x*y>0)
opposed=sum(1 for x,y in both if x*y<0)
zero=len(both)-concord-opposed
def pearson(values):
 if len(values)<2:return None
 a,b=zip(*values)
 if statistics.pstdev(a)==0 or statistics.pstdev(b)==0:return None
 return statistics.correlation(a,b)
def ranks(vals):
 order=sorted(range(len(vals)),key=lambda i:vals[i])
 r=[0.0]*len(vals)
 st=0
 while st<len(order):
  en=st+1
  while en<len(order) and vals[order[en]]==vals[order[st]]:en+=1
  mid=(st+1+en)/2
  for j in range(st,en):r[order[j]]=mid
  st=en
 return r
def spearman(values):
 if len(values)<2:return None
 a,b=zip(*values)
 return pearson(list(zip(ranks(a),ranks(b))))
fields=("ensembl_id","published_gene_name","published_logFC","published_FDR","published_gene_type","ours_gene_id","ours_log2FC","ours_padj","ours_significant","in_both_significant","direction_agrees")
with (A/"gene_level_comparison.csv").open("x",newline="") as f:
 writer=csv.DictWriter(f,fieldnames=fields);writer.writeheader()
 for k in sorted(union):
  p=pub.get(k,{});o=ours.get(k,{})
  writer.writerow(dict(ensembl_id=k,published_gene_name=p.get("symbol",""),published_logFC=p.get("published_lfc",""),published_FDR=p.get("published_fdr",""),published_gene_type=p.get("gene_type",""),ours_gene_id=o.get("id",""),ours_log2FC=o.get("lfc",""),ours_padj=o.get("padj",""),ours_significant=k in our_de,in_both_significant=k in overlap,direction_agrees=(p.get("published_lfc",0)*o.get("lfc",0)>0) if k in overlap else ""))
res={"source_filename":PUB.name,"published_sha256":hashlib.sha256(PUB.read_bytes()).hexdigest(),"published_de":len(pub_de),"published_up":upub["up"],"published_down":upub["down"],"our_de":len(our_de),"our_tested_nonzero":len(ours),"published_ids_in_our_tested":all_pub_in_our,"overlap":len(overlap),"union":len(union),"jaccard":len(overlap)/len(union),"published_recovery":len(overlap)/len(pub_de),"our_precision_as_overlap_fraction":len(overlap)/len(our_de),"direction_agree":concord,"direction_disagree":opposed,"direction_zero":zero,"direction_agreement_fraction":concord/len(both) if both else None,"lfc_pearson_overlap":pearson(both),"lfc_spearman_overlap":spearman(both),"published_protein_coding":sum(z["gene_type"]=="protein_coding" for z in pub.values()),"published_marker_in_overlaps":[z["symbol"] for k,z in pub.items() if k in overlap and z["symbol"] in ("PCA3","AMACR","ANKRD34B","NEK5","KCNG3","PTPRT")],"published_only":len(pub_de-overlap),"ours_only":len(our_de-overlap)}
with (A/"benchmark_summary.json").open("x") as f:json.dump(res,f,indent=2);f.write("\n")
print("G10_BENCHMARK",json.dumps(res,ensure_ascii=False),flush=True)
print("G10_DONE",flush=True)