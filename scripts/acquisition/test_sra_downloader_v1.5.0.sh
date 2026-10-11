#!/usr/bin/env bash
# Offline integration tests: stub NCBI tools, never access network.
set -Eeuo pipefail

HERE="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
APP="$HERE/sra_downloader_v1.5.0.sh"
ROOT="$(mktemp -d)"
trap 'rm -rf -- "$ROOT"' EXIT
mkdir -p "$ROOT/mockbin"

cat > "$ROOT/mockbin/prefetch" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == '--version' ]]; then echo 'prefetch 3.4.1-stub'; exit 0; fi
if [[ "${1:-}" == '--help' ]]; then
    printf '%s\n' '--resume' '--verify' '--progress' '--max-size' '--output-directory' '--heartbeat'
    exit 0
fi
out=''
acc=''
args=("$@")
for (( i=0; i<$#; i++ )); do
    case "${args[i]}" in
        --output-directory) out="${args[i+1]}" ;;
        SRR[0-9]*|ERR[0-9]*|DRR[0-9]*) acc="${args[i]}" ;;
    esac
done
[[ -n "$out" && -n "$acc" ]] || exit 100
[[ "$out" == */"$acc" ]] || exit 101
printf '%s\n' "$acc|$out" >> "$MOCK_PREFETCH_CALLS"
mkdir -p -- "$out"
# First call to a configured ID fails after creating a partial file, second resumes.
if [[ "${MOCK_FAIL_ONCE:-}" == "$acc" ]]; then
    if [[ ! -f "$out/.failed-once" ]]; then
        printf 'partial\n' > "$out/.failed-once"
        exit 32
    fi
fi
if [[ "${MOCK_ALWAYS_FAIL:-}" == "$acc" ]]; then
    echo 'intentional failure' >&2
    exit 34
fi
if [[ "${MOCK_SLEEP_ID:-}" == "$acc" ]]; then
    printf 'partial data\n' > "$out/partial.bin"
    sleep 30
fi
printf 'fake sra data for %s\n' "$acc" > "$out/$acc.sra"
echo 'mock prefetch success'
STUB

cat > "$ROOT/mockbin/vdb-validate" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == '--version' ]]; then echo 'vdb-validate 3.4.1-stub'; exit 0; fi
path="${1:-}"
printf '%s\n' "$path" >> "$MOCK_VALIDATOR_CALLS"
acc="$(basename -- "$path")"
if [[ "${MOCK_VALIDATION_FAIL:-}" == "$acc" ]]; then exit 9; fi
[[ -s "$path/$acc.sra" ]] || exit 7
[[ "$(cat -- "$path/$acc.sra")" == "fake sra data for $acc" ]] || exit 8
STUB
chmod +x "$ROOT/mockbin/"*

export PATH="$ROOT/mockbin:$PATH"
export MOCK_PREFETCH_CALLS="$ROOT/prefetch_calls"
export MOCK_VALIDATOR_CALLS="$ROOT/validator_calls"
: > "$MOCK_PREFETCH_CALLS"
: > "$MOCK_VALIDATOR_CALLS"

pass=0
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "PASS: $*"; pass=$((pass + 1)); }
expect_status() {
    local want="$1"; shift
    local got=0
    "$@" > "$ROOT/stdout" 2> "$ROOT/stderr" || got=$?
    [[ "$got" == "$want" ]] || {
        cat "$ROOT/stdout" "$ROOT/stderr" >&2
        fail "wanted exit $want but received $got: $*"
    }
}
count_calls() { grep -c -- '^' "$MOCK_PREFETCH_CALLS" || true; }

# 1: dry-run creates no directories.
expect_status 0 "$APP" --run SRR000001 --output "$ROOT/dry" --dry-run
[[ ! -e "$ROOT/dry" ]] || fail 'dry-run changed filesystem'
ok 'dry-run is side-effect free'

# 2: first download with correct accession-dir output, validation and manifests.
expect_status 0 "$APP" --run SRR000001 --output "$ROOT/output" --min-free-gb 0
[[ -s "$ROOT/output/SRR000001/SRR000001.sra" ]] || fail 'missing SRA file'
[[ -s "$ROOT/output/.checksums/SRR000001.sha256" ]] || fail 'missing manifest'
[[ -s "$ROOT/output/.complete/SRR000001.ok" ]] || fail 'missing marker'
[[ -s "$(find "$ROOT/output/.reports" -name '*.tsv' | head -n 1)" ]] || fail 'missing TSV report'
[[ "$(count_calls)" == 1 ]] || fail 'unexpected number of downloads'
ok 'download, path, validation, and atomic metadata'

# 3: rerun reuses checksum and skips prefetch + redundant validation.
old_validations="$(wc -l < "$MOCK_VALIDATOR_CALLS")"
expect_status 0 "$APP" --run SRR000001 --output "$ROOT/output" --min-free-gb 0
[[ "$(count_calls)" == 1 ]] || fail 're-downloaded completed run'
[[ "$(wc -l < "$MOCK_VALIDATOR_CALLS")" == "$old_validations" ]] || fail 'needlessly reran vdb-validate'
ok 'checksum-only reuse avoids repeated validation and download'

# 4: opt-in full mode validates complete Run again.
expect_status 0 "$APP" --run SRR000001 --output "$ROOT/output" --min-free-gb 0 --verify-existing full
[[ "$(wc -l < "$MOCK_VALIDATOR_CALLS")" == $((old_validations + 1)) ]] || fail 'full mode did not validate'
ok 'full reuse performs vdb-validate'

# 5: changed data cannot be silently accepted or overwritten.
original_manifest="$(sha256sum "$ROOT/output/.checksums/SRR000001.sha256")"
printf 'corruption\n' >> "$ROOT/output/SRR000001/SRR000001.sra"
expect_status 2 "$APP" --run SRR000001 --output "$ROOT/output" --min-free-gb 0
[[ "$(sha256sum "$ROOT/output/.checksums/SRR000001.sha256")" == "$original_manifest" ]] || fail 'trusted checksum was overwritten'
[[ "$(count_calls)" == 1 ]] || fail 'prefetch ran despite integrity conflict'
grep -q 'INTEGRITY CONFLICT' "$ROOT/stderr" || fail 'integrity diagnostic missing'
ok 'integrity conflict fails closed and preserves evidence'

# 6: simulate interrupted prefetch and retry/resume in same exact folder.
export MOCK_FAIL_ONCE='ERR000002'
expect_status 0 "$APP" --run ERR000002 --output "$ROOT/output" --min-free-gb 0 --base-backoff 0 --max-backoff 0 --max-attempts 3
[[ -s "$ROOT/output/ERR000002/ERR000002.sra" ]] || fail 'failed to resume partial download'
[[ -f "$ROOT/output/ERR000002/.failed-once" ]] || fail 'partial file vanished'
[[ "$(grep -c 'ERR000002|' "$MOCK_PREFETCH_CALLS")" == 2 ]] || fail 'retry count not 2'
unset MOCK_FAIL_ONCE
ok 'retry uses preserved data and exact Run directory'

# 7: --continue-on-failure saves successful later Runs and returns failure.
export MOCK_ALWAYS_FAIL='DRR000003'
expect_status 2 "$APP" --run DRR000003 --run SRR000004 --output "$ROOT/output" --min-free-gb 0 --max-attempts 2 --base-backoff 0 --max-backoff 0 --continue-on-failure
[[ -s "$ROOT/output/SRR000004/SRR000004.sra" ]] || fail 'later run did not proceed'
unset MOCK_ALWAYS_FAIL
ok 'continue-on-failure keeps partial failure and processes later Runs'

# 8: the lock must refuse a second process targeting the same output root.
exec 8>"$ROOT/output/.sra-downloader.lock"
flock -n 8 || fail 'cannot create test lock'
expect_status 1 "$APP" --run SRR000005 --output "$ROOT/output" --min-free-gb 0
grep -q 'Another instance' "$ROOT/stderr" || fail 'lock error not reported'
flock -u 8
ok 'process lock protects shared output'

# 9: strict parse, CRLF, comments and duplicates.
printf '#comment\r\nSRR000006\r\nSRR000006\r\n\r\nERR000007\r\n' > "$ROOT/ids.txt"
expect_status 0 "$APP" --input "$ROOT/ids.txt" --output "$ROOT/out2" --min-free-gb 0
[[ "$(grep -c '^SRR000006|' "$MOCK_PREFETCH_CALLS")" == 1 ]] || fail 'duplicate run downloaded'
[[ -s "$ROOT/out2/ERR000007/ERR000007.sra" ]] || fail 'missing second run'
printf 'SRR000006\ninvalid_accession\n' > "$ROOT/bad.txt"
expect_status 1 "$APP" --input "$ROOT/bad.txt" --output "$ROOT/out3" --min-free-gb 0
[[ ! -e "$ROOT/out3" ]] || fail 'created output for invalid input'
ok 'strict input validation and duplicate filtering'

# 10: guard refuses start if reserve is larger than available disk space.
expect_status 2 "$APP" --run SRR000008 --output "$ROOT/out2" --min-free-gb 90000000
[[ ! -d "$ROOT/out2/SRR000008" ]] || fail 'started download despite reserve'
ok 'preflight disk reserve prevents download'

# 11: failure must not delete or move other existing files.
mkdir -p "$ROOT/out2/DRR000009"
printf 'user data\n' > "$ROOT/out2/DRR000009/my-notes.txt"
export MOCK_ALWAYS_FAIL='DRR000009'
expect_status 2 "$APP" --run DRR000009 --output "$ROOT/out2" --min-free-gb 0 --max-attempts 1
[[ "$(cat "$ROOT/out2/DRR000009/my-notes.txt")" == 'user data' ]] || fail 'user data modified'
unset MOCK_ALWAYS_FAIL
ok 'existing user files preserved on failed transfer'

# 12: success from prefetch but failure in scientific-file validation preserves data.
export MOCK_VALIDATION_FAIL='SRR000010'
expect_status 2 "$APP" --run SRR000010 --output "$ROOT/out2" --min-free-gb 0 --max-attempts 1
[[ -s "$ROOT/out2/SRR000010/SRR000010.sra" ]] || fail 'validation failure lost downloaded SRA'
[[ ! -e "$ROOT/out2/.complete/SRR000010.ok" ]] || fail 'invalid SRA marked complete'
unset MOCK_VALIDATION_FAIL
ok 'failed vdb-validation never marks a Run as complete'

# 13: support output paths with spaces and preserve quoted arguments.
expect_status 0 "$APP" --run SRR000011 --output "$ROOT/out folder with spaces" --min-free-gb 0
[[ -s "$ROOT/out folder with spaces/SRR000011/SRR000011.sra" ]] || fail 'spaces in path not supported'
ok 'output paths with spaces are supported'

# 14: migrate verified older checksum metadata without re-downloading.
expect_status 0 "$APP" --run SRR000012 --output "$ROOT/out2" --min-free-gb 0
old_calls="$(count_calls)"
cat > "$ROOT/out2/.complete/SRR000012.ok" <<'OLD'
accession=SRR000012
status=VALIDATED
checksum_policy=stable-files-v1
OLD
expect_status 0 "$APP" --run SRR000012 --output "$ROOT/out2" --min-free-gb 0
[[ "$(count_calls)" == "$old_calls" ]] || fail 'legacy migration unexpectedly re-downloaded'
grep -q '^policy=stable-files-v2$' "$ROOT/out2/.complete/SRR000012.ok" || fail 'legacy marker not updated'
ok 'legacy checksum policy migrated without network activity'

# 15: stall timeout stops sleeping downloader but leaves its partial file.
export MOCK_SLEEP_ID='SRR000013'
expect_status 2 "$APP" --run SRR000013 --output "$ROOT/out2" --min-free-gb 0 --stall-timeout 2 --stall-poll 1 --max-attempts 1
[[ -s "$ROOT/out2/SRR000013/partial.bin" ]] || fail 'stall watchdog deleted partial data'
[[ ! -e "$ROOT/out2/.complete/SRR000013.ok" ]] || fail 'stalled Run marked complete'
unset MOCK_SLEEP_ID
ok 'opt-in stall watchdog preserves interrupted data'

# 16: crash between manifest and marker writes can recover safely without redownload.
expect_status 0 "$APP" --run SRR000014 --output "$ROOT/out2" --min-free-gb 0
old_calls="$(count_calls)"
rm "$ROOT/out2/.complete/SRR000014.ok"
expect_status 0 "$APP" --run SRR000014 --output "$ROOT/out2" --min-free-gb 0
[[ "$(count_calls)" == "$old_calls" ]] || fail 'metadata-only interrupted state re-downloaded'
[[ -s "$ROOT/out2/.complete/SRR000014.ok" ]] || fail 'marker not repaired'
ok 'interrupted manifest/marker publication recovers without redownload'

# 17: required mountpoint guard rejects unmounted disk before mkdir.
mkdir -p "$ROOT/unmounted_drive"
expect_status 1 "$APP" --run SRR000015 --output "$ROOT/unmounted_drive/sra" --require-mountpoint "$ROOT/unmounted_drive" --min-free-gb 0
[[ ! -e "$ROOT/unmounted_drive/sra" ]] || fail 'created data directory on unmounted storage path'
ok 'missing external disk detected before output folder creation'

printf '\nALL %d OFFLINE TESTS PASSED\n' "$pass"
