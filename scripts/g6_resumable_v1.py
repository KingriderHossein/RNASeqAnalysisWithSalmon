#!/usr/bin/env python3
"""G6 sequential, recoverable Salmon runner. No deletion/overwrite of existing results."""
import argparse,csv,fcntl,hashlib,json,os,shutil,subprocess,sys,time
from pathlib import Path
R=Path("/home/kingrider/GSE89223_download")
A=R/"audits/G6_release_20261009"
CONF=A/"salmon_config_candidate.json"
REQ=["quant.sf","cmd_info.json","lib_format_counts.json","aux_info/meta_info.json","aux_info/ambig_info.tsv","libParams/flenDist.txt","logs/salmon_quant.log"]
def emit(s):
    print(time.strftime("%Y-%m-%dT%H:%M:%S%z"),s,flush=True)
def sha(path):
    h=hashlib.sha256()
    with path.open("rb") as f:
        for b in iter(lambda:f.read(8*1024*1024),b""):h.update(b)
    return h.hexdigest()
def validate(out,cfg,expected_reads):
    for p in REQ:
        q=out/p
        if not q.is_file() or not q.stat().st_size:raise RuntimeError("missing/empty "+p)
    m=json.loads((out/"aux_info/meta_info.json").read_text())
    if m.get("num_processed")!=expected_reads:raise RuntimeError("read-count disagreement")
    if m.get("index_seq_hash")!=cfg["index_seq_hash"]:raise RuntimeError("index identity mismatch")
    if m.get("num_valid_targets")!=196520:raise RuntimeError("transcript target count mismatch")
    if m.get("library_types")!=["SF"]:raise RuntimeError("wrong library mode")
    if m.get("quant_errors") or not m.get("em_converged"):raise RuntimeError("Salmon internal error/no EM convergence")
    if m.get("num_mapped",0)<=0:raise RuntimeError("no transcript mapping")
    seen=set();total_tpm=0.;n=0
    with (out/"quant.sf").open() as f:
        for row in csv.DictReader(f,delimiter="\t"):
            n+=1;name=row["Name"]
            if name in seen:raise RuntimeError("duplicate transcript "+name)
            seen.add(name);total_tpm+=float(row["TPM"])
            if float(row["NumReads"])<0:raise RuntimeError("negative estimated counts")
    if n!=196520 or abs(total_tpm-1000000)>0.5:raise RuntimeError("quant.sf target count or TPM sum error")
    return {"processed":m["num_processed"],"mapped":m["num_mapped"],"mapped_pct":m["percent_mapped"],"decoy":m["num_decoy_fragments"],"warnings":m.get("diagnostics",[]),"tpm_sum":total_tpm}
def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--dry-run",action="store_true")
    ap.add_argument("--pilot-only",action="store_true",help="One real FASTQ (SRR4453783) only")
    ap.add_argument("--batch",action="store_true",help="Requires formal acceptance marker")
    args=ap.parse_args()
    if not any((args.dry_run,args.pilot_only,args.batch)):ap.error("Choose mode")
    if sum((args.dry_run,args.pilot_only,args.batch))!=1:ap.error("One mode only")
    cfg=json.loads(CONF.read_text())
    if cfg["salmon_version"]!="salmon 2.8.0":raise RuntimeError("version freeze invalid")
    index=Path(cfg["index"])
    if sha(index/"info.json")!=cfg["index_info_sha256"]:raise RuntimeError("index info SHA changed")
    inf=json.loads((index/"info.json").read_text())
    if inf["seq_hash"]!=cfg["index_seq_hash"]:raise RuntimeError("index sequence hash changed")
    s=subprocess.run(["/home/kingrider/anaconda3/envs/rnaseq/bin/salmon","--version"],capture_output=True,text=True,check=True).stdout.strip()
    if s!=cfg["salmon_version"]:raise RuntimeError("salmon version differs")
    pre=json.loads(Path(cfg["preflight"]).read_text())
    if pre["status"]!="PASS" or len(pre["samples"])!=32:raise RuntimeError("preflight failed")
    if args.batch:
        gate=A/"FORMAL_G4_G5_ACCEPTED.json"
        if not gate.exists() or json.loads(gate.read_text()).get("accepted") is not True:
            raise RuntimeError("G4/G5 formal acceptance missing; 32-sample batch forbidden")
    root=Path(cfg["output_root"])
    expected=pre["samples"]
    if args.pilot_only:expected=expected[:1]
    emit("PLAN mode="+("dry" if args.dry_run else "pilot" if args.pilot_only else "batch")+" n="+str(len(expected)))
    for row in expected:
        p=Path(row["path"])
        if not p.exists() or p.stat().st_size!=row["size_bytes"]:raise RuntimeError("source mutated: "+row["accession"])
    if args.dry_run:return
    if shutil.disk_usage(root.parent if root.parent.exists() else R).free < cfg["min_free_gib"]*1024**3:raise RuntimeError("low disk")
    root.mkdir(parents=True,exist_ok=True)
    with (root/"runner.lock").open("a+") as lk:
        try:fcntl.flock(lk,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:raise RuntimeError("other G6 runner active")
        for i,row in enumerate(expected,1):
            acc=row["accession"];src=Path(row["path"])
            final=root/acc;work=root/(acc+".working")
            emit("SAMPLE "+str(i)+"/"+str(len(expected))+" "+acc)
            if final.exists():
                evidence=final/"validation.json"
                if not evidence.exists():raise RuntimeError("existing result cannot be silently overwritten: "+str(final))
                e=json.loads(evidence.read_text())
                if e.get("config_sha")!=sha(CONF) or e.get("fastq_sha")!=row["checksum_sha256_verified_prior"]:raise RuntimeError("existing result provenance conflict")
                validate(final,cfg,row["reads"])
                emit("VALIDATED_SKIP "+acc);continue
            if work.exists():raise RuntimeError("interrupted working dir preserved; manual inspect before retry: "+str(work))
            if sha(src)!=row["checksum_sha256_verified_prior"]:raise RuntimeError("FASTQ checksum mismatch: "+acc)
            work.mkdir()
            cmd=["/home/kingrider/anaconda3/envs/rnaseq/bin/salmon","quant","-i",str(index),"-l","SF","-r",str(src),"-p",str(cfg["threads"]),"--fldMean",str(cfg["fragment_length_prior_mean"]),"--fldSD",str(cfg["fragment_length_prior_sd"]),"-o",str(work)]
            (work/"command.json").write_text(json.dumps(cmd)+"\n")
            emit("SALMON_START "+acc+" "+" ".join(cmd))
            with (root/(acc+".run.log")).open("a") as log:
                rc=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT).returncode
            emit("SALMON_EXIT "+acc+" "+str(rc))
            if rc!=0:raise RuntimeError("Salmon failed, partial preserved: "+acc)
            result=validate(work,cfg,row["reads"])
            evidence={"accession":acc,"config_sha":sha(CONF),"fastq_sha":row["checksum_sha256_verified_prior"],"input":str(src),"validated_at":time.strftime("%Y-%m-%dT%H:%M:%S%z"),"metrics":result}
            (work/"validation.json").write_text(json.dumps(evidence,indent=2)+"\n")
            work.rename(final)
            emit("VALIDATED_FINAL "+acc+" "+json.dumps(result))
    emit("ALL_REQUESTED_COMPLETED")
if __name__=="__main__":
    try:main()
    except Exception as exc:emit("ERROR "+repr(exc));sys.exit(2)
