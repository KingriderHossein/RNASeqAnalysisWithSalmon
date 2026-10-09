#!/usr/bin/env python3
import csv, json, math, statistics
from pathlib import Path
from collections import Counter, defaultdict
import openpyxl
R=Path("/home/kingrider/GSE89223_download")
A=R/"audits/G10_publication_20261009"
O=A/"g10_discordance_v2"
O.mkdir(exist_ok=False)
norm=lambda s:str(s).strip().split(".")[0]
def num(v):
 try:
  x=float(v);return x if math.isfinite(x) else None
 except (TypeError,ValueError):return None
pub={}
ws=openpyxl.load_workbook(A/"reference/oncotarget-08-32990-s002.xlsx",read_only=True,data_only=True).active
for row in list(ws.values)[1:]:
 if not row[1]:continue
 k=norm(row[1])
 assert k not in pub
 pub[k]={"symbol":row[0],"lfc":num(row[2]),"p":num(row[3]),"padj":num(row[4]),"type":row[5]}
ours={}
with (R/"audits/G8_DESeq2_20261009/track_A/deseq2_full_results.csv").open() as f:
 for v in csv.DictReader(f):
  k=norm(v["gene_id"]);assert k not in ours
  ours[k]={"versioned_id":v["gene_id"],"lfc":num(v["log2FoldChange"]),"p":num(v["pvalue"]),"padj":num(v["padj"]),"baseMean":num(v["baseMean"]),"sig":v["significant"]=="TRUE"}
types={}
with (R/"reference/G3_GENCODEv19_GRCh37p13/derived/transcript_biotypes.tsv").open() as f:
 for v in csv.DictReader(f,delimiter="\t"):
  k=norm(v["gene_id"]);typ=v["gene_type"]
  if k in types and types[k]!=typ:raise ValueError(("inconsistent gene_type",k))
  types[k]=typ
assert len(pub)==3384 and sum(v["sig"] for v in ours.values())==2093
P=set(pub);D={k for k,v in ours.items() if v["sig"]};both=P&D
onlypub=P-D;onlyour=D-P
def cat(k):
 v=ours.get(k)
 if v is None:return "absent_from_our_nonzero_gene_universe"
 if v["p"] is None:return "our_raw_p_unavailable"
 if v["padj"] is None:return "our_BH_padj_unavailable"
 if v["padj"]>=.05 and v["p"]<.05:return "our_nominal_p_below_0.05_but_BH_not"
 if v["padj"]>=.05:return "our_not_significant_nominal"
 return "our_significant"
def direction(k):return (pub[k]["lfc"] or 0)*(ours[k]["lfc"] or 0)>0
def pearson(pairs):
 if len(pairs)<2:return None
 x,y=zip(*pairs)
 return statistics.correlation(x,y) if statistics.pstdev(x)>0 and statistics.pstdev(y)>0 else None
def rank(vals):
 ids=sorted(range(len(vals)),key=vals.__getitem__);out=[0.]*len(vals);i=0
 while i<len(ids):
  j=i+1
  while j<len(ids) and vals[ids[j]]==vals[ids[i]]:j+=1
  for k in ids[i:j]:out[k]=(i+1+j)/2
  i=j
 return out
def spear(pairs):
 if len(pairs)<2:return None
 x,y=zip(*pairs);return pearson(list(zip(rank(x),rank(y))))
rows=[]
for k in sorted(P|D):
 p=pub.get(k);o=ours.get(k);status="shared_DE" if k in both else "published_only" if k in onlypub else "ours_only"
 rows.append(dict(ensembl_id=k,gene_name=p["symbol"] if p else "",status=status,category=cat(k) if p else "not_in_published_DE_table",publication_biotype=p["type"] if p else "",gencode_v19_biotype=types.get(k,"unannotated"),publication_logFC=p["lfc"] if p else "",publication_FDR=p["padj"] if p else "",ours_gene_id=o["versioned_id"] if o else "",ours_baseMean=o["baseMean"] if o else "",ours_log2FC=o["lfc"] if o else "",ours_pvalue=o["p"] if o else "",ours_padj=o["padj"] if o else "",direction_same=direction(k) if p and o and p["lfc"] is not None and o["lfc"] is not None else "unavailable"))
with (O/"discordant_gene_classification.csv").open("x",newline="") as f:
 w=csv.DictWriter(f,fieldnames=rows[0].keys());w.writeheader();w.writerows(rows)
pubcats=Counter(cat(k) for k in onlypub)
pubbiotype=Counter(v["type"] for v in pub.values())
ourbiotype=Counter(types.get(k,"unannotated") for k in D)
onlyourbio=Counter(types.get(k,"unannotated") for k in onlyour)
matchpub=Counter(pub[k]["type"] for k in both)
paired=[(pub[k]["lfc"],ours[k]["lfc"]) for k in P&set(ours) if pub[k]["lfc"] is not None and ours[k]["lfc"] is not None]
matched_dir=sum(x*y>0 for x,y in paired)
print("PUBLISHED_ONLY_CAT",dict(pubcats),flush=True)
print("BIOTYPE_PUBLICATION",dict(pubbiotype),flush=True)
print("BIOTYPE_OUR",dict(ourbiotype),flush=True)
print("BIOTYPE_OUR_ONLY",dict(onlyourbio),flush=True)
print("BIOTYPE_SHARED",dict(matchpub),flush=True)
print("DIRECTION_ALL_PUBLICATION_WITH_OUR",len(paired),matched_dir,flush=True)
details={"published_only_categories":dict(pubcats),"published_biotypes":dict(pubbiotype),"our_significant_gencode_biotypes":dict(ourbiotype),"our_only_gencode_biotypes":dict(onlyourbio),"shared_published_biotypes":dict(matchpub),"published_de_identified_in_our_nonzero_universe":len(P&set(ours)),"published_with_our_effect_pairs":len(paired),"direction_consistent_all_pairs":matched_dir,"direction_consistent_all_pairs_fraction":matched_dir/len(paired),"lfc_pearson_all_pairs":pearson(paired),"lfc_spearman_all_pairs":spear(paired),"published_only_total":len(onlypub),"our_only_total":len(onlyour),"shared_total":len(both),"versioned_ensembl_join":"suffix stripped only in comparison key","critical_comparator_limitation":"published workbook includes FDR-significant genes only, not full published tested universe; ours-only genes lack published p/FC"}
with (O/"discordance_summary.json").open("x") as f:json.dump(details,f,indent=2);f.write("\n")
assert len(rows)==3741 and sum(pubcats.values())==1648 and sum(ourbiotype.values())==2093
print("G10_DISCORDANCE_PASS",len(rows),flush=True)