#!/usr/bin/env bash
set -euo pipefail
ROOT=/home/kingrider/GSE89223_download
D="$ROOT/reference/G3_GENCODEv19_GRCh37p13"
STATUS="$D/derived/keepDuplicates_build/exit_code.txt"
LOG="$ROOT/g3_to_g5_provisional_orchestration_v1.log"
if test -e "$LOG"; then echo "Refusing to overwrite existing orchestration log" >&2; exit 34; fi
{
 echo "WORKFLOW_STARTED $(date -Is)"
 echo "WAIT_FOR_KEEPDUPLICATES_INDEX"
 for ((step=0;step<180;step++)); do
   if test -f "$STATUS"; then break; fi
   if ! ps -eo args | grep -F 'salmon index --transcripts' | grep -F -- '--keepDuplicates' | grep -v grep >/dev/null; then
     echo "ERROR: index process not running and no exit result"; exit 41
   fi
   sleep 10
 done
 if ! test -f "$STATUS"; then echo "TIMEOUT: index not complete within 1800s"; exit 42; fi
 if ! test "$(cat "$STATUS")" = "0"; then echo "ERROR: index build exit $(cat "$STATUS")"; exit 43; fi
 echo "KEEP_INDEX_BUILD_EXIT_ZERO $(date -Is)"
 python3 "$ROOT/g3_validate_keepduplicates_v1.py"
 test -s "$D/derived/G3_keepDuplicates_validation_v1.json" || exit 44
 echo "G3_KEEPDUPLICATES_PASS $(date -Is)"
 bash "$ROOT/g5_pilot_SRR4453804_v1.sh"
 echo "G5_PILOT_RUN_RETURNED_ZERO $(date -Is)"
} > "$LOG" 2>&1
