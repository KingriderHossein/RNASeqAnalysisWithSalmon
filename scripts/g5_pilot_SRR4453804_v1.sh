#!/usr/bin/env bash
set -euo pipefail
ROOT=/home/kingrider/GSE89223_download
REF="$ROOT/reference/G3_GENCODEv19_GRCh37p13"
INDEX="$REF/salmon_v2.8.0_decoyaware_keepDuplicates_k31"
FASTQ="$ROOT/fastq/pilot/SRR4453804.fastq"
OUT="$ROOT/pilots/G5_SRR4453804_keepDuplicates_raw_v1"
LOGDIR="$ROOT/logs/G5_SRR4453804_keepDuplicates_raw_v1"
CONF="$ROOT/qc/gse89223_raw_v1_20261009/preprocessing_baseline_v1.json"
source /home/kingrider/anaconda3/etc/profile.d/conda.sh
conda activate rnaseq
test -f "$REF/derived/G3_keepDuplicates_validation_v1.json" || { echo "BLOCKED: G3 keepDuplicates index not validated"; exit 25; }
test -f "$CONF"
test -f "$FASTQ"
test ! -e "$OUT" || { echo "BLOCKED: pilot destination exists"; exit 26; }
test ! -e "$LOGDIR" || { echo "BLOCKED: pilot log destination exists"; exit 27; }
python3 - "$REF" "$CONF" "$FASTQ" <<'PY'
import sys,json
from pathlib import Path
ref=Path(sys.argv[1]);conf=json.loads(Path(sys.argv[2]).read_text());idx=json.loads((ref/"derived/G3_keepDuplicates_validation_v1.json").read_text());fastq=Path(sys.argv[3])
assert conf["status"]=="BASELINE_SELECTED_NOT_GATE_ACCEPTED"
assert idx["validation"]=="PASS" and idx["index_info"]["keep_duplicates"] is True
state=json.loads(Path("/home/kingrider/GSE89223_download/metadata/fastq_conversion_state/SRR4453804.json").read_text())
assert state["path"]==str(fastq) and fastq.stat().st_size==state["size_bytes"] and state["reads"]==5575676
print("G5_PREFLIGHT_PASS",fastq,flush=True)
PY
mkdir -p "$LOGDIR"
salmon --version > "$LOGDIR/salmon_version.txt"
date -Is > "$LOGDIR/started_at.txt"
printf '%s\n' "salmon quant --index $INDEX --libType U --unmatedReads $FASTQ --threads 6 --fldMean 150 --fldSD 50 --output $OUT" > "$LOGDIR/command.txt"
set +e
salmon quant --index "$INDEX" --libType U --unmatedReads "$FASTQ" --threads 6 --fldMean 150 --fldSD 50 --output "$OUT" > "$LOGDIR/stdout.log" 2> "$LOGDIR/stderr.log"
rc=$?
set -e
printf '%s\n' "$rc" > "$LOGDIR/exit_code.txt"
if test -d "$OUT"; then printf '%s\n' "$rc" > "$OUT/exit_code.txt"; fi
date -Is > "$LOGDIR/ended_at.txt"
echo "G5_PILOT_EXIT=$rc"
exit "$rc"
