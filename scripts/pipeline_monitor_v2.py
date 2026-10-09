#!/usr/bin/env python3
"""Read-only live project timeline; snapshots are derived from artifacts and logs.
Bound to localhost only. No filesystem-changing endpoints.
"""
import datetime as dt
import importlib.util
import json
import os
import re
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
import argparse
ROOT=Path("/home/kingrider/GSE89223_download")
REF=ROOT/"reference/G3_GENCODEv19_GRCh37p13"
QC=ROOT/"qc/gse89223_raw_v1_20261009"
HTML=ROOT/"pipeline_monitor_v2.html"
spec=importlib.util.spec_from_file_location("legacy_monitor",ROOT/"pipeline_monitor.py")
legacy=importlib.util.module_from_spec(spec)
spec.loader.exec_module(legacy)
def read(p,limit=180000):
 return legacy.safe_read_text(p,limit)
def exists(p): return p.is_file() and p.stat().st_size>0
def count(p,pattern):return legacy.count_named(p,pattern)
def item(text,ok=True):return {"text":text,"ok":ok}
def build():
 b=legacy.state()
 events=[]
 def event(t,source="شواهد فایل و لاگ"):events.append({"text":t,"source":source})
 stages={s["key"]:s for s in b["stages"]}
 for s in stages.values():
  s["details"]=[];s["next"]="";s["evidence"]=[];s["gate"]="";s["progress_note"]=""
 def fill(k,details,next="",evidence=None,gate=""):
  s=stages[k];s["details"]=details;s["next"]=next;s["evidence"]=evidence or [];s["gate"]=gate
  return s
 fill("raw",[item("آرشیو مستقل ۳۲ SRA با کنترل SHA-256") ,item("نسخه‌های SRA در SSD پس از تأیید حذف شده‌اند؛ FASTQ محفوظ است")],evidence=["گزارش‌های archive و cleanup"],gate="G1/G4")
 fill("fastq",[item("تبدیل ۳۲ اجرای SRA به single-end FASTQ"),item("رکوردهای مستقل provenance برای هر SRR")],gate="G4",evidence=["metadata/fastq_conversion_state"])
 fill("qc",[item("FastQC 0.13.0 روی ۳۲ نمونه"),item("پرچم FAIL/WARN مربوط به ماژول‌هاست، نه تعداد نمونه‌های ردشده")],gate="G4",evidence=["fastqc ZIP + provenance"])
 fill("multiqc",[item("گزارش MultiQC 1.35 و مقایسهٔ نتایج QC")],gate="G4",evidence=["multiqc/GSE89223_multiqc.html"])
 rRNA=QC/"rrna_read_mapping_v1"
 systematic=exists(rRNA/"all32_systematic_100k_v1.tsv")
 competitive=exists(rRNA/"competition_all32_10k.tsv")
 reviews=[item("بازبینی الگوهای GC، کیفیت، توالی پرتکرار و duplication"),item("تطبیق دقیق ۸۶/۱۱۲ توالی پرتکرار با rRNA انسانی"),item("نمونه‌گیری ۱۰۰ هزار Read از سراسر هر یک از ۳۲ FASTQ",systematic),item("کنترل رقابتی با GENCODE v19 روی ۱۰ هزار Read از هر نمونه",competitive),item("تصمیم رسمی دربارهٔ preprocessing هنوز ثبت نشده است",b["g4_accepted"])]
 rv=fill("review",reviews,"تصویب سیاست preprocessing بر پایهٔ شواهد و بدون حذف خودکار داده‌ها",["QC rRNA diagnostic","qc_review_decision.json"],"G4")
 rv["status"]="done" if b["g4_accepted"] else "attention"
 rv["completed"]=sum(x["ok"] for x in reviews[:4]);rv["total"]=5
 gtf=exists(REF/"gencode.v19.annotation.gtf.gz")
 genome=exists(REF/"GRCh37.p13.genome.fa.gz")
 tx=exists(REF/"gencode.v19.comprehensive.derived_transcripts.fa")
 tx2=exists(REF/"derived/tx2gene.tsv")
 dec=exists(REF/"derived/decoys.txt")
 indexdir=REF/"salmon_v2.8.0_decoyaware_k31"
 log=read(REF/"derived/index_build.log")
 salmonlog=read(REF/"derived/salmon_index_stderr.log")
 indexdone="INDEX_BUILD_RETURNED_0" in log
 indexrunning=False
 if not indexdone and "INDEX_BUILD_BEGIN" in log:
  # Linux process visibility from /proc is observational; do not execute commands.
  try:
   for p in Path("/proc").iterdir():
    if not p.name.isdigit():continue
    try:
     cmd=(p/"cmdline").read_bytes().replace(b"\0",b" ").decode("utf-8","ignore")
     if "salmon index" in cmd and str(indexdir) in cmd and "bash -lc" not in cmd:
      indexrunning=True;break
    except (OSError,PermissionError):continue
  except OSError:pass
 phases=re.findall(r"Phase\s+[1-9]:[^<\n]+",salmonlog)
 lastphase=phases[-1].strip()[:170] if phases else ""
 duplicates=indexdir/"duplicate_clusters.tsv"
 dupcount=max(0,len(read(duplicates,130000).splitlines())-1) if exists(duplicates) else None
 refitems=[item("GENCODE v19 GTF دریافت و اعتبارسنجی شد",gtf),item("GRCh37.p13 genome دریافت و اعتبارسنجی شد",genome),item("۱۹۶٬۵۲۰ رونوشت جامع از ژنوم/GTF استخراج شد",tx),item("tx2gene و biotype با ۰ شناسهٔ مغایر تولید شدند",tx2),item("۲۹۷ نام ژنومی decoy ثبت شد",dec),item("Salmon 2.8.0: ساخت index انتخابی k=31 با decoy",indexdone),item("ارزیابی ۱٬۶۳۳ رونوشت تکراری و سیاست keepDuplicates",False)]
 rs=fill("reference",refitems,"تکمیل و تأیید index؛ تعیین تکلیف شناسه‌های تکراری بین ژن‌ها",["G3 GTF/FASTA","derived/tx2gene.tsv","derived/decoys.txt","derived/salmon_index_stderr.log"],"G3")
 rs["status"]="done" if all(x["ok"] for x in refitems) else "running" if indexrunning else "attention"
 rs["completed"]=sum(x["ok"] for x in refitems);rs["total"]=len(refitems)
 rs["progress_note"]=lastphase if indexrunning else ("Index building returned success; further review required" if indexdone else "ساخت index در حال اجرا نیست؛ بررسی لازم است")
 if indexrunning:event("ساخت Salmon Index هنوز فعال است: "+(lastphase or "در حال پردازش"),"Salmon log / proc")
 if indexdone:event("ساخت اولیه Salmon Index به پایان رسید؛ اعتبارسنجی نهایی باقی است","index_build.log")
 if tx:event("استخراج ۱۹۶٬۵۲۰ رونوشت و تطبیق شناسه‌ها تکمیل شده است","G3 FASTA + audit")
 if competitive:event("آزمون رقابتی rRNA برای ۳۲ نمونه انجام شده؛ G4 هنوز نیازمند تصمیم علمی است","competition_all32_10k.tsv")
 if dupcount:event(f"Salmon {dupcount} زوج توالی تکراری گزارش کرده؛ مشکل انتساب بین ژن‌ها باید بررسی شود","duplicate_clusters.tsv")
 stage5=fill("salmon",[item("پایلوت single-end هنوز نتیجهٔ تأییدشده ندارد",False),item("کمی‌سازی کامل ۳۲ نمونه انجام نشده است",False)],"بعد از پذیرش G3 و G4، پایلوت G5 با index تثبیت‌شده","Issue #5".split(","),"G5/G6")
 stage5["status"]="pending"
 for k,title,gate in [("tximport","ورود خروجی Salmon به tximport","G7"),("deseq2","مدل‌های آماری Track A و Track B","G8"),("comparison","بررسی نتایج و مقایسه با مقاله","G9/G10")]:
  fill(k,[item("این مرحله هنوز اجرا نشده است",False)],"وابسته به پذیرش مراحل قبلی",[],gate)
  stages[k]["status"]="pending"
 b["stages"]=list(stages.values())
 b["events_detail"]=events[:12]
 b["current_stage"]="G3 — ساخت و اعتبارسنجی Salmon Index"
 b["current_detail"]=rs["progress_note"]
 b["reference"]={"sources_verified":gtf and genome,"transcripts":196520 if tx else None,"tx2gene":tx2,"decoys":297 if dec else 0,
  "index_running":indexrunning,"index_build_returned_zero":indexdone,"index_file_count":sum(1 for x in indexdir.iterdir() if x.is_file()) if indexdir.is_dir() else 0,
  "index_stage":lastphase,"duplicate_pairs":dupcount}
 b["github_links"]={"g3":"https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/3",
 "g4":"https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/issues/4",
 "g3_pr":"https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/pull/52",
 "g4_pr":"https://github.com/KingriderHossein/RNASeqAnalysisWithSalmon/pull/51"}
 b["read_only"]=True
 return b
class Handler(BaseHTTPRequestHandler):
 def send(self,n,body,typ):
  self.send_response(n);self.send_header("Content-Type",typ);self.send_header("Cache-Control","no-store");self.send_header("X-Content-Type-Options","nosniff");self.send_header("Content-Length",str(len(body)));self.end_headers();self.wfile.write(body)
 def do_GET(self):
  try:
   path=self.path.split("?",1)[0]
   if path in ("/","/index.html"):self.send(200,HTML.read_bytes(),"text/html; charset=utf-8")
   elif path=="/api/status":self.send(200,json.dumps(build(),ensure_ascii=False).encode(),"application/json; charset=utf-8")
   elif path=="/multiqc":
    p=QC/"multiqc/GSE89223_multiqc.html"
    self.send(200,p.read_bytes(),"text/html; charset=utf-8") if p.is_file() else self.send(404,b"Not found","text/plain")
   else:self.send(404,b"Not found","text/plain")
  except Exception as e:self.send(500,str(e).encode(),"text/plain")
 def do_POST(self):self.send(405,b"Read only","text/plain")
 def log_message(self,fmt,*args):pass
if __name__=="__main__":
 ap=argparse.ArgumentParser();ap.add_argument("--port",type=int,default=8766);ap.add_argument("--once",action="store_true");a=ap.parse_args()
 if a.once:print(json.dumps(build(),ensure_ascii=False,indent=2))
 else:
  assert HTML.is_file()
  print("GSE89223 extended monitor on http://127.0.0.1:%s"%a.port,flush=True)
  ThreadingHTTPServer(("127.0.0.1",a.port),Handler).serve_forever()
