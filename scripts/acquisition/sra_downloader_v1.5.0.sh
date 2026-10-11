#!/usr/bin/env bash
# SRA Sequential Downloader v1.5.0
# An operational wrapper around NCBI prefetch and vdb-validate.
# Linux (native or WSL2) + Bash 4.4+; does not download FASTQ or delete SRA data.

set -Eeuo pipefail
IFS=$'\n\t'
umask 022

readonly APP_NAME='sra-downloader'
readonly APP_VERSION='1.5.0'
readonly MANIFEST_POLICY='stable-files-v2'

INPUT_FILE=''
OUTPUT_ROOT='raw/sra'
OUTPUT_WAS_SET=0
REQUIRED_MOUNTPOINT=''
MAX_ATTEMPTS="${MAX_ATTEMPTS:-10}"
MIN_FREE_GB="${MIN_FREE_GB:-20}"
BASE_BACKOFF_SECONDS="${BASE_BACKOFF_SECONDS:-15}"
MAX_BACKOFF_SECONDS="${MAX_BACKOFF_SECONDS:-300}"
STALL_TIMEOUT_SECONDS="${SRA_STALL_TIMEOUT_SECONDS:-${STALL_TIMEOUT_SECONDS:-0}}"
STALL_POLL_SECONDS="${STALL_POLL_SECONDS:-30}"
STOP_ON_FAILURE="${STOP_ON_FAILURE:-1}"
PREFETCH_MAX_SIZE="${PREFETCH_MAX_SIZE:-u}"
PREFETCH_HEARTBEAT_MINUTES="${PREFETCH_HEARTBEAT_MINUTES:-0}"
VERIFY_EXISTING='checksum'
DRY_RUN=0
LOG_DIR="${SRA_LOG_DIR:-${LOG_DIR:-}}"
CHECKSUM_DIR="${SRA_CHECKSUM_DIR:-${CHECKSUM_DIR:-}}"
COMPLETE_DIR=''
REPORT_DIR=''
REPORT_FILE=''
PREFETCH_MAX_SIZE_FLAG='--max-size'
PREFETCH_HAS_PROGRESS=0
PREFETCH_HAS_HEARTBEAT=0
PREFETCH_VERSION=''
VALIDATOR_VERSION=''
ACTIVE_PID=''
MONITOR_PID=''
ACTIVE_LABEL=''
RUN_STATUS=''
RUN_ATTEMPTS=0

declare -a CLI_RUNS=()
declare -a RUNS=()
declare -A RUN_SEEN=()

usage() {
    cat <<'HELP'
sra-downloader v1.5.0 -- Resumable SRA Run downloader for Linux / WSL2

USAGE
  sra-downloader --run SRR000001 [--run ERR123456] -o /data/sra
  sra-downloader --input runs.txt --output /data/sra
  sra-downloader runs.txt /data/sra                # legacy positional form
  sra-downloader --run SRR000001 --dry-run

INPUT
  -r, --run ACCESSION          SRR/ERR/DRR Run; repeat for multiple Runs
  -i, --input FILE             Text file with one Run ID per line;
                               blank lines and # comments are ignored

OUTPUT / RETRIES
  -o, --output DIR             Parent directory (default: raw/sra)
      --max-attempts N         Attempts per Run (default: 10)
      --min-free-gb N          Minimum free GiB before/during transfer (default: 20)
      --base-backoff N         Initial retry delay, seconds (default: 15)
      --max-backoff N          Maximum retry delay, seconds (default: 300)
      --max-size SIZE          prefetch limit, e.g. 20g, 500g, u (default: u)
      --continue-on-failure    Continue with other Runs on error
      --require-mountpoint DIR Abort unless DIR is mounted and output is beneath it

INTEGRITY / MONITORING
      --verify-existing MODE   checksum (default), full (also vdb-validate)
      --stall-timeout N        Kill if no directory growth for N seconds;
                               disabled by default (may produce false positives)
      --stall-poll N           Monitoring interval, seconds (default: 30)
      --heartbeat-minutes N    Native prefetch heartbeat if supported (default: 0)
      --dry-run                Print plan, do not create files or download
  -h, --help                   Show help
  -V, --version                Show version

ENVIRONMENT
  MAX_ATTEMPTS, MIN_FREE_GB, BASE_BACKOFF_SECONDS,
  MAX_BACKOFF_SECONDS, SRA_STALL_TIMEOUT_SECONDS, STALL_POLL_SECONDS,
  PREFETCH_MAX_SIZE, PREFETCH_HEARTBEAT_MINUTES, STOP_ON_FAILURE,
  SRA_LOG_DIR, SRA_CHECKSUM_DIR

EXIT STATUS
  0 = all requested Runs validated; 2 = one or more Runs failed;
  1 = invalid configuration or missing dependencies; 130 = interrupted.

NOTES
  * Uses NCBI prefetch and vdb-validate. Does not extract FASTQ.
  * Never removes downloaded Run data, even when a download fails.
  * Existing trusted checksum mismatches stop rather than being silently reset.
  * Re-run the same command after interrupted transfers to resume.
  * The script serializes its own runs using a lock per output root.
HELP
}

timestamp() { date '+%Y-%m-%d %H:%M:%S'; }
now_utc() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log() { printf '[%s] %s\n' "$(timestamp)" "$*"; }
error() { log "ERROR: $*" >&2; }
die() { error "$*"; exit 1; }
requires() { command -v "$1" >/dev/null 2>&1 || die "Missing required executable: $1"; }

run_id_valid() { [[ "$1" =~ ^(SRR|ERR|DRR)[0-9]+$ ]]; }
trim() {
    local x="$1"
    x="${x#"${x%%[![:space:]]*}"}"
    x="${x%"${x##*[![:space:]]}"}"
    printf '%s' "$x"
}

normalize_number() {
    local key="$1" value="$2" minimum="$3"
    [[ "$value" =~ ^[0-9]{1,9}$ ]] || die "$key must be a nonnegative integer (up to 9 digits)."
    local n=$((10#$value))
    (( n >= minimum )) || die "$key must be >= $minimum."
    printf '%d' "$n"
}

parse_args() {
    local -a positional=()
    while (( $# > 0 )); do
        case "$1" in
            -r|--run) (( $# >= 2 )) || die "$1 requires an ID"; CLI_RUNS+=("$2"); shift 2 ;;
            -i|--input) (( $# >= 2 )) || die "$1 requires a file"; [[ -z "$INPUT_FILE" ]] || die 'Only one input file is allowed'; INPUT_FILE="$2"; shift 2 ;;
            -o|--output) (( $# >= 2 )) || die "$1 requires a directory"; OUTPUT_ROOT="$2"; OUTPUT_WAS_SET=1; shift 2 ;;
            --max-attempts) (( $# >= 2 )) || die "$1 requires a number"; MAX_ATTEMPTS="$2"; shift 2 ;;
            --min-free-gb) (( $# >= 2 )) || die "$1 requires a number"; MIN_FREE_GB="$2"; shift 2 ;;
            --base-backoff) (( $# >= 2 )) || die "$1 requires a number"; BASE_BACKOFF_SECONDS="$2"; shift 2 ;;
            --max-backoff) (( $# >= 2 )) || die "$1 requires a number"; MAX_BACKOFF_SECONDS="$2"; shift 2 ;;
            --max-size) (( $# >= 2 )) || die "$1 requires a size"; PREFETCH_MAX_SIZE="$2"; shift 2 ;;
            --verify-existing) (( $# >= 2 )) || die "$1 requires checksum or full"; VERIFY_EXISTING="$2"; shift 2 ;;
            --stall-timeout) (( $# >= 2 )) || die "$1 requires a number"; STALL_TIMEOUT_SECONDS="$2"; shift 2 ;;
            --stall-poll) (( $# >= 2 )) || die "$1 requires a number"; STALL_POLL_SECONDS="$2"; shift 2 ;;
            --heartbeat-minutes) (( $# >= 2 )) || die "$1 requires a number"; PREFETCH_HEARTBEAT_MINUTES="$2"; shift 2 ;;
            --continue-on-failure) STOP_ON_FAILURE=0; shift ;;
            --require-mountpoint) (( $# >= 2 )) || die "$1 requires a directory"; REQUIRED_MOUNTPOINT="$2"; shift 2 ;;
            --dry-run) DRY_RUN=1; shift ;;
            -h|--help) usage; exit 0 ;;
            -V|--version) printf '%s\n' "$APP_VERSION"; exit 0 ;;
            --) shift; while (( $# > 0 )); do positional+=("$1"); shift; done ;;
            -*) die "Unknown argument: $1" ;;
            *) positional+=("$1"); shift ;;
        esac
    done
    if (( ${#positional[@]} > 0 )); then
        if (( ${#positional[@]} == 1 )) && run_id_valid "${positional[0]}"; then
            CLI_RUNS+=("${positional[0]}")
        elif (( ${#positional[@]} <= 2 )) && [[ -z "$INPUT_FILE" ]]; then
            INPUT_FILE="${positional[0]}"
            if (( ${#positional[@]} == 2 )); then
                (( OUTPUT_WAS_SET == 0 )) || die 'Output path specified twice'
                OUTPUT_ROOT="${positional[1]}"
            fi
        else
            die 'Unexpected positional arguments; see --help'
        fi
    fi
    if (( ${#CLI_RUNS[@]} == 0 )) && [[ -z "$INPUT_FILE" ]]; then
        if [[ -f metadata/runs.txt ]]; then INPUT_FILE='metadata/runs.txt'
        else die 'Provide --run ACCESSION or --input FILE (see --help)'; fi
    fi
}

validate_settings() {
    MAX_ATTEMPTS="$(normalize_number MAX_ATTEMPTS "$MAX_ATTEMPTS" 1)"
    MIN_FREE_GB="$(normalize_number MIN_FREE_GB "$MIN_FREE_GB" 0)"
    BASE_BACKOFF_SECONDS="$(normalize_number BASE_BACKOFF_SECONDS "$BASE_BACKOFF_SECONDS" 0)"
    MAX_BACKOFF_SECONDS="$(normalize_number MAX_BACKOFF_SECONDS "$MAX_BACKOFF_SECONDS" 0)"
    STALL_TIMEOUT_SECONDS="$(normalize_number STALL_TIMEOUT_SECONDS "$STALL_TIMEOUT_SECONDS" 0)"
    STALL_POLL_SECONDS="$(normalize_number STALL_POLL_SECONDS "$STALL_POLL_SECONDS" 1)"
    PREFETCH_HEARTBEAT_MINUTES="$(normalize_number PREFETCH_HEARTBEAT_MINUTES "$PREFETCH_HEARTBEAT_MINUTES" 0)"
    [[ "$STOP_ON_FAILURE" == 0 || "$STOP_ON_FAILURE" == 1 ]] || die 'STOP_ON_FAILURE must be 0 or 1'
    [[ "$VERIFY_EXISTING" == 'checksum' || "$VERIFY_EXISTING" == 'full' ]] || die '--verify-existing must be checksum or full'
    [[ "$PREFETCH_MAX_SIZE" =~ ^(u|[0-9]{1,15}[kKmMgGtT]?)$ ]] || die '--max-size must be u or a size such as 20g'
    (( MAX_BACKOFF_SECONDS >= BASE_BACKOFF_SECONDS )) || die '--max-backoff must be >= --base-backoff'
    [[ -n "$OUTPUT_ROOT" && "$OUTPUT_ROOT" != '/' ]] || die 'Unsafe output root'
}

add_run() {
    local id="$1"
    run_id_valid "$id" || die "Invalid Run accession: $id (expected SRR/ERR/DRR + digits)"
    if [[ ! -v RUN_SEEN["$id"] ]]; then
        RUN_SEEN["$id"]=1
        RUNS+=("$id")
    fi
}

load_input() {
    local id line n=0
    if [[ -n "$INPUT_FILE" ]]; then
        [[ -f "$INPUT_FILE" && -r "$INPUT_FILE" ]] || die "Cannot read input file: $INPUT_FILE"
        while IFS= read -r line || [[ -n "$line" ]]; do
            n=$((n + 1))
            line="${line%$'\r'}"
            id="$(trim "$line")"
            [[ -z "$id" || "${id:0:1}" == '#' ]] && continue
            run_id_valid "$id" || die "Invalid accession at $INPUT_FILE:$n: [$id]"
            add_run "$id"
        done < "$INPUT_FILE"
    fi
    for id in "${CLI_RUNS[@]}"; do add_run "$id"; done
    (( ${#RUNS[@]} > 0 )) || die 'No valid Run accessions were supplied'
}

absolute_dir() {
    local path="$1"
    mkdir -p -- "$path" || return 1
    (cd -- "$path" && pwd -P)
}

check_space() {
    local free_kb
    free_kb="$(df -Pk "$OUTPUT_ROOT" | awk 'NR == 2 {print $4}')" || return 1
    [[ "$free_kb" =~ ^[0-9]+$ ]] || return 1
    # df -P -k reports 1024-byte blocks: reserve is binary GiB.
    (( free_kb > MIN_FREE_GB * 1024 * 1024 ))
}

available_space_gib() {
    local kb
    kb="$(df -Pk "$OUTPUT_ROOT" | awk 'NR == 2 {print $4}')" || return 1
    [[ "$kb" =~ ^[0-9]+$ ]] || return 1
    printf '%d' "$((kb / 1024 / 1024))"
}

retry_delay() {
    local number="$1" delay="$BASE_BACKOFF_SECONDS" i
    for (( i=1; i<number; i++ )); do
        (( delay == 0 )) && break
        if (( delay >= MAX_BACKOFF_SECONDS || delay > MAX_BACKOFF_SECONDS / 2 )); then
            delay="$MAX_BACKOFF_SECONDS"
            break
        fi
        delay=$((delay * 2))
    done
    (( delay > MAX_BACKOFF_SECONDS )) && delay="$MAX_BACKOFF_SECONDS"
    printf '%d' "$delay"
}

signal_group() {
    local pid="$1"
    [[ -n "$pid" ]] || return 0
    if kill -0 "$pid" 2>/dev/null; then
        kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
        local i
        for (( i=0; i<5; i++ )); do
            kill -0 "$pid" 2>/dev/null || return 0
            sleep 1
        done
        kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
    fi
}

cleanup() {
    if [[ -n "$MONITOR_PID" ]]; then
        kill "$MONITOR_PID" 2>/dev/null || true
        wait "$MONITOR_PID" 2>/dev/null || true
        MONITOR_PID=''
    fi
    if [[ -n "$ACTIVE_PID" ]]; then
        signal_group "$ACTIVE_PID"
        wait "$ACTIVE_PID" 2>/dev/null || true
        ACTIVE_PID=''
    fi
}

on_signal() {
    local name="$1" rc="$2"
    log "Received $name during ${ACTIVE_LABEL:-session}; preserving partial Run data."
    cleanup
    exit "$rc"
}
trap 'on_signal INT 130' INT
trap 'on_signal TERM 143' TERM
trap 'on_signal HUP 129' HUP
trap 'cleanup' EXIT

record_command() {
    local log_file="$1"; shift
    printf '[%s] COMMAND:' "$(timestamp)" >> "$log_file"
    printf ' %q' "$@" >> "$log_file"
    printf '\n' >> "$log_file"
}

run_validated_command() {
    local log_file="$1" label="$2"; shift 2
    record_command "$log_file" "$@"
    ACTIVE_LABEL="$label"
    setsid "$@" >> "$log_file" 2>&1 &
    ACTIVE_PID=$!
    local rc=0
    wait "$ACTIVE_PID" || rc=$?
    ACTIVE_PID=''
    ACTIVE_LABEL=''
    return "$rc"
}

stable_file_names() {
    find . -type f \
        ! -name '*.lock' ! -name '*.tmp' ! -name '*.prf' -print0 | LC_ALL=C sort -z
}

file_list_fingerprint() {
    local data_dir="$1"
    (cd -- "$data_dir" && stable_file_names | sha256sum | awk '{print $1}')
}

has_stable_files() {
    local data_dir="$1"
    [[ -n "$(cd -- "$data_dir" && find . -type f ! -name '*.lock' ! -name '*.tmp' ! -name '*.prf' -print -quit)" ]]
}

manifest_for() { printf '%s/%s.sha256\n' "$CHECKSUM_DIR" "$1"; }
marker_for() { printf '%s/%s.ok\n' "$COMPLETE_DIR" "$1"; }
marker_field() {
    local marker="$1" key="$2"
    awk -F= -v k="$key" '$1 == k { sub(/^[^=]*=/, ""); print; exit }' "$marker"
}

verify_manifest() {
    local data_dir="$1" manifest="$2"
    [[ -s "$manifest" ]] || return 1
    (cd -- "$data_dir" && sha256sum --check --status -- "$manifest")
}

verify_marker() {
    local id="$1" data_dir="$2"
    local manifest marker current_list_hash expected_list_hash expected_hash
    manifest="$(manifest_for "$id")"
    marker="$(marker_for "$id")"
    [[ -s "$marker" && -s "$manifest" ]] || return 1
    [[ "$(marker_field "$marker" accession)" == "$id" ]] || return 1
    [[ "$(marker_field "$marker" status)" == 'VALIDATED' ]] || return 1
    [[ "$(marker_field "$marker" policy)" == "$MANIFEST_POLICY" ]] || return 1
    [[ "$(marker_field "$marker" data_dir)" == "$data_dir" ]] || return 1
    expected_hash="$(marker_field "$marker" manifest_sha256)"
    [[ "$expected_hash" =~ ^[a-f0-9]{64}$ ]] || return 1
    [[ "$(sha256sum "$manifest" | awk '{print $1}')" == "$expected_hash" ]] || return 1
    expected_list_hash="$(marker_field "$marker" file_list_sha256)"
    [[ "$expected_list_hash" =~ ^[a-f0-9]{64}$ ]] || return 1
    current_list_hash="$(file_list_fingerprint "$data_dir")" || return 1
    [[ "$current_list_hash" == "$expected_list_hash" ]] || return 1
    verify_manifest "$data_dir" "$manifest"
}

validate_sra() {
    local id="$1" data_dir="$2" log_file="$3"
    log "Validating SRA: $id"
    run_validated_command "$log_file" "vdb-validate $id" vdb-validate "$data_dir"
}

write_manifest_and_marker() {
    local id="$1" data_dir="$2" manifest marker manifest_tmp marker_tmp digest names_digest
    manifest="$(manifest_for "$id")"
    marker="$(marker_for "$id")"
    manifest_tmp="$(mktemp "$CHECKSUM_DIR/.${id}.sha256.XXXXXX")" || return 1
    if ! (cd -- "$data_dir" && stable_file_names | xargs -0 -r sha256sum) > "$manifest_tmp"; then
        error "Checksum generation failed: $id"
        rm -f -- "$manifest_tmp"
        return 1
    fi
    if [[ ! -s "$manifest_tmp" ]]; then
        error "No stable files found to checksum: $id"
        rm -f -- "$manifest_tmp"
        return 1
    fi
    digest="$(sha256sum "$manifest_tmp" | awk '{print $1}')" || return 1
    names_digest="$(file_list_fingerprint "$data_dir")" || return 1
    [[ "$digest" =~ ^[a-f0-9]{64}$ && "$names_digest" =~ ^[a-f0-9]{64}$ ]] || return 1
    marker_tmp="$(mktemp "$COMPLETE_DIR/.${id}.ok.XXXXXX")" || return 1
    {
        printf 'accession=%s\n' "$id"
        printf 'status=VALIDATED\n'
        printf 'policy=%s\n' "$MANIFEST_POLICY"
        printf 'data_dir=%s\n' "$data_dir"
        printf 'manifest_sha256=%s\n' "$digest"
        printf 'file_list_sha256=%s\n' "$names_digest"
        printf 'prefetch_version=%s\n' "$PREFETCH_VERSION"
        printf 'validator_version=%s\n' "$VALIDATOR_VERSION"
        printf 'script_version=%s\n' "$APP_VERSION"
        printf 'validated_utc=%s\n' "$(now_utc)"
    } > "$marker_tmp" || return 1
    # Marker is the commit record. Write it only after manifest reaches its final name.
    mv -f -- "$manifest_tmp" "$manifest" || return 1
    mv -f -- "$marker_tmp" "$marker" || return 1
    return 0
}

reuse_existing() {
    # Return code: 0 reused, 1 need download, 3 integrity conflict.
    local id="$1" log_file="$2" data_dir="$OUTPUT_ROOT/$id"
    local manifest marker old_policy
    manifest="$(manifest_for "$id")"
    marker="$(marker_for "$id")"
    if [[ -L "$data_dir" ]]; then
        error "Refusing symlinked accession directory: $data_dir"
        return 3
    fi
    if [[ ! -e "$data_dir" ]]; then
        if [[ -e "$marker" || -e "$manifest" ]]; then
            error "Metadata exists but SRA directory is missing: $id; inspect manually"
            return 3
        fi
        return 1
    fi
    if [[ ! -d "$data_dir" ]]; then error "Run path exists but is not a directory: $data_dir"; return 3; fi

    if [[ -e "$marker" && -e "$manifest" ]]; then
        old_policy="$(marker_field "$marker" policy)"
        if [[ "$old_policy" == "$MANIFEST_POLICY" ]]; then
            if ! verify_marker "$id" "$data_dir"; then
                error "INTEGRITY CONFLICT in existing files for $id; refusing to overwrite trusted metadata or download"
                return 3
            fi
            if [[ "$VERIFY_EXISTING" == 'full' ]] && ! validate_sra "$id" "$data_dir" "$log_file"; then
                error "vdb-validate failed for already completed Run: $id"
                return 3
            fi
            RUN_STATUS='reused'
            log "Reused validated and checksum-matched Run: $id"
            return 0
        fi
        # v1.3.1 and other older manifests: migrate only if old checksum matches.
        if ! verify_manifest "$data_dir" "$manifest"; then
            error "Legacy manifest mismatches local Run data: $id; manual review required"
            return 3
        fi
        if ! validate_sra "$id" "$data_dir" "$log_file"; then
            error "Legacy Run failed vdb-validate: $id"
            return 3
        fi
        write_manifest_and_marker "$id" "$data_dir" || return 3
        RUN_STATUS='migrated'
        log "Migrated prior validated Run metadata without download: $id"
        return 0
    fi

    if [[ -e "$marker" && ! -e "$manifest" ]]; then
        error "Completion marker exists without its manifest for $id; inspect manually"
        return 3
    fi
    if [[ -e "$manifest" && ! -e "$marker" ]]; then
        # Recover a crash between atomic manifest and marker publication.
        # Never discard or overwrite a mismatching partial manifest.
        if ! verify_manifest "$data_dir" "$manifest"; then
            error "Uncommitted manifest conflicts with data for $id; inspect manually"
            return 3
        fi
        if ! validate_sra "$id" "$data_dir" "$log_file"; then
            error "Uncommitted manifest exists but vdb-validate failed for $id"
            return 3
        fi
        write_manifest_and_marker "$id" "$data_dir" || return 3
        RUN_STATUS='recovered'
        log "Recovered interrupted metadata publication without download: $id"
        return 0
    fi

    # Possibly a complete pre-existing SRA directory without our metadata.
    if has_stable_files "$data_dir" && validate_sra "$id" "$data_dir" "$log_file"; then
        write_manifest_and_marker "$id" "$data_dir" || return 3
        RUN_STATUS='recovered'
        log "Recovered already-valid Run without downloading: $id"
        return 0
    fi
    log "Existing Run is incomplete or not yet valid; asking prefetch to resume: $id"
    return 1
}

monitor_prefetch() {
    local pid="$1" id="$2" data_dir="$3" log_file="$4"
    local current_size last_size=-1 last_growth="$(date +%s)" now free
    while kill -0 "$pid" 2>/dev/null; do
        sleep "$STALL_POLL_SECONDS" 9>&- || return 0
        kill -0 "$pid" 2>/dev/null || break
        if (( MIN_FREE_GB > 0 )); then
            if ! check_space; then
                free="$(available_space_gib 2>/dev/null || printf '?')"
                printf '[%s] GUARD: disk reserve breached (%s GiB available; minimum %s GiB). Stopping %s.\n' \
                    "$(timestamp)" "$free" "$MIN_FREE_GB" "$id" | tee -a "$log_file"
                signal_group "$pid"
                break
            fi
        fi
        if (( STALL_TIMEOUT_SECONDS > 0 )); then
            current_size="$(du -sb -- "$data_dir" 2>/dev/null | awk '{print $1}')" || current_size='0'
            [[ "$current_size" =~ ^[0-9]+$ ]] || current_size='0'
            now="$(date +%s)"
            if [[ "$last_size" != "$current_size" ]]; then
                last_size="$current_size"
                last_growth="$now"
            elif (( now - last_growth >= STALL_TIMEOUT_SECONDS )); then
                printf '[%s] WATCHDOG: no directory growth for %s seconds; stopping %s (heuristic).\n' \
                    "$(timestamp)" "$STALL_TIMEOUT_SECONDS" "$id" | tee -a "$log_file"
                signal_group "$pid"
                break
            fi
        fi
    done
}

run_prefetch() {
    local id="$1" log_file="$2" data_dir="$OUTPUT_ROOT/$id"
    local -a cmd=(prefetch --resume yes --verify yes "$PREFETCH_MAX_SIZE_FLAG" "$PREFETCH_MAX_SIZE" --output-directory "$data_dir")
    (( PREFETCH_HAS_PROGRESS == 1 )) && cmd+=(--progress)
    (( PREFETCH_HEARTBEAT_MINUTES > 0 )) && cmd+=(--heartbeat "$PREFETCH_HEARTBEAT_MINUTES")
    cmd+=("$id")
    record_command "$log_file" "${cmd[@]}"
    ACTIVE_LABEL="prefetch $id"
    # Stream native progress in the terminal and write the same data into the per-Run log.
    setsid "${cmd[@]}" 9>&- > >(tee -a "$log_file") 2>&1 &
    ACTIVE_PID=$!
    MONITOR_PID=''
    if (( MIN_FREE_GB > 0 || STALL_TIMEOUT_SECONDS > 0 )); then
        monitor_prefetch "$ACTIVE_PID" "$id" "$data_dir" "$log_file" 9>&- &
        MONITOR_PID=$!
    fi
    local rc=0
    wait "$ACTIVE_PID" || rc=$?
    if [[ -n "$MONITOR_PID" ]]; then
        kill "$MONITOR_PID" 2>/dev/null || true
        wait "$MONITOR_PID" 2>/dev/null || true
        MONITOR_PID=''
    fi
    ACTIVE_PID=''
    ACTIVE_LABEL=''
    return "$rc"
}

download_one() {
    local id="$1" log_file="$LOG_DIR/$id.log" data_dir="$OUTPUT_ROOT/$id"
    local rc=0 attempt=1 delay
    RUN_STATUS='failed'
    RUN_ATTEMPTS=0
    log "=== Run: $id ==="
    if reuse_existing "$id" "$log_file"; then return 0; else rc=$?; fi
    (( rc != 3 )) || { RUN_STATUS='integrity_conflict'; return 1; }

    while (( attempt <= MAX_ATTEMPTS )); do
        RUN_ATTEMPTS="$attempt"
        if ! check_space; then
            error "Insufficient free disk space for reserve ${MIN_FREE_GB} GiB at $OUTPUT_ROOT."
            return 1
        fi
        log "Attempt ${attempt}/${MAX_ATTEMPTS}: $id (free: $(available_space_gib) GiB)"
        if run_prefetch "$id" "$log_file"; then
            if [[ ! -d "$data_dir" || -L "$data_dir" ]]; then
                error "prefetch succeeded but exact Run directory was not created: $data_dir"
                return 1
            fi
            if ! validate_sra "$id" "$data_dir" "$log_file"; then
                error "vdb-validate failed after successful prefetch: $id; data preserved for inspection"
                RUN_STATUS='validation_failed'
                return 1
            fi
            if ! write_manifest_and_marker "$id" "$data_dir"; then
                error "Could not finalize trusted checksums/marker for $id"
                RUN_STATUS='metadata_failed'
                return 1
            fi
            RUN_STATUS='downloaded'
            log "Validated and recorded: $id"
            return 0
        fi
        log "prefetch failed/stopped; partial files preserved for resume: $id"
        if (( attempt == MAX_ATTEMPTS )); then break; fi
        delay="$(retry_delay "$attempt")"
        log "Retry after ${delay}s: $id"
        sleep "$delay" & wait "$!" || true
        attempt=$((attempt + 1))
    done
    error "Exhausted $MAX_ATTEMPTS attempts for $id"
    return 1
}

check_prefetch_compat() {
    local help_text
    PREFETCH_VERSION="$(prefetch --version 2>&1 | awk 'NF {print; exit}')" || die 'prefetch --version failed'
    VALIDATOR_VERSION="$(vdb-validate --version 2>&1 | awk 'NF {print; exit}')" || die 'vdb-validate --version failed'
    help_text="$(prefetch --help 2>&1 || true)"
    local flag
    for flag in --output-directory --resume --verify; do
        grep -Fq -- "$flag" <<< "$help_text" || die "Installed prefetch does not advertise $flag"
    done
    if grep -Fq -- '--max-size' <<< "$help_text"; then
        PREFETCH_MAX_SIZE_FLAG='--max-size'
    elif grep -Fq -- '--max_size' <<< "$help_text"; then
        PREFETCH_MAX_SIZE_FLAG='--max_size'
    else
        die 'Installed prefetch lacks --max-size or --max_size'
    fi
    grep -Fq -- '--progress' <<< "$help_text" && PREFETCH_HAS_PROGRESS=1 || true
    grep -Fq -- '--heartbeat' <<< "$help_text" && PREFETCH_HAS_HEARTBEAT=1 || true
    if (( PREFETCH_HEARTBEAT_MINUTES > 0 && PREFETCH_HAS_HEARTBEAT == 0 )); then
        die 'Installed prefetch does not support --heartbeat; set --heartbeat-minutes 0'
    fi
}

main() {
    (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4) )) || die 'Bash 4.4 or newer is required'
    parse_args "$@"
    validate_settings
    load_input

    if (( DRY_RUN == 1 )); then
        log "DRY-RUN: ${#RUNS[@]} unique Run(s); output parent: $OUTPUT_ROOT"
        log "No files will be created and no SRA download will start."
        local id
        for id in "${RUNS[@]}"; do
            printf '  prefetch --resume yes --verify yes --max-size %q -O %q %q\n' \
                "$PREFETCH_MAX_SIZE" "$OUTPUT_ROOT/$id" "$id"
        done
        return 0
    fi

    local cmd
    for cmd in prefetch vdb-validate sha256sum flock df awk find sort xargs mktemp setsid du date grep mv tee; do
        requires "$cmd"
    done
    check_prefetch_compat
    # Fail BEFORE mkdir if an external HDD/SSD mount is missing.
    if [[ -n "$REQUIRED_MOUNTPOINT" ]]; then
        requires mountpoint
        requires realpath
        [[ -d "$REQUIRED_MOUNTPOINT" ]] || die "Required mountpoint does not exist: $REQUIRED_MOUNTPOINT"
        mountpoint -q -- "$REQUIRED_MOUNTPOINT" || die "Required storage is not mounted: $REQUIRED_MOUNTPOINT"
        local resolved_mount planned_root
        resolved_mount="$(realpath -e -- "$REQUIRED_MOUNTPOINT")" || die 'Cannot resolve mountpoint'
        planned_root="$(realpath -m -- "$OUTPUT_ROOT")" || die 'Cannot resolve output path'
        [[ "$planned_root" == "$resolved_mount" || "$planned_root" == "$resolved_mount/"* ]] || \
            die "Output must be inside required mountpoint: $resolved_mount"
    fi
    # WSL2 users should prefer the Linux filesystem for heavy I/O. Warn only.
    if [[ -r /proc/sys/kernel/osrelease ]] && grep -Eqi 'microsoft|wsl' /proc/sys/kernel/osrelease; then
        if [[ "$OUTPUT_ROOT" =~ ^/mnt/[a-zA-Z](/|$) ]]; then
            log 'WARNING: Output appears to use a Windows-mounted drive (/mnt/<letter>).'
            log 'For better Linux I/O throughput, consider an output folder under $HOME.'
            log 'If using a detachable Windows disk, pass --require-mountpoint to guard startup.'
        fi
    fi
    OUTPUT_ROOT="$(absolute_dir "$OUTPUT_ROOT")" || die 'Cannot open/create output root'
    [[ "$OUTPUT_ROOT" != '/' ]] || die 'Refusing filesystem root as output'

    # Own one exclusive lock over the output parent. No data-directory deletion.
    exec 9>"$OUTPUT_ROOT/.sra-downloader.lock"
    flock -n 9 || die "Another instance is already running for: $OUTPUT_ROOT"

    LOG_DIR="$(absolute_dir "${LOG_DIR:-$OUTPUT_ROOT/.logs}")" || die 'Cannot create log directory'
    CHECKSUM_DIR="$(absolute_dir "${CHECKSUM_DIR:-$OUTPUT_ROOT/.checksums}")" || die 'Cannot create checksum directory'
    COMPLETE_DIR="$(absolute_dir "$OUTPUT_ROOT/.complete")" || die 'Cannot create completion directory'
    REPORT_DIR="$(absolute_dir "$OUTPUT_ROOT/.reports")" || die 'Cannot create report directory'
    REPORT_FILE="$REPORT_DIR/session_$(date -u '+%Y%m%dT%H%M%SZ')_$${RANDOM}.tsv"
    printf 'accession\tstatus\tattempts\tduration_seconds\tpath\n' > "$REPORT_FILE"

    log "$APP_NAME v$APP_VERSION | ${#RUNS[@]} unique Run(s) | destination $OUTPUT_ROOT"
    log "prefetch: $PREFETCH_VERSION | vdb-validate: $VALIDATOR_VERSION"
    log "Resume: enabled | per-run checksums: SHA-256 | reuse validation: $VERIFY_EXISTING"
    log "Disk reserve: ${MIN_FREE_GB} GiB | retry attempts: $MAX_ATTEMPTS | status: $REPORT_FILE"
    if (( STALL_TIMEOUT_SECONDS > 0 )); then
        log 'Warning: growth-based stall detection is heuristic; false positives are possible.'
    fi

    local id start finish count_ok=0 count_fail=0 status
    local -a failed=()
    for id in "${RUNS[@]}"; do
        start="$(date +%s)"
        if download_one "$id"; then
            count_ok=$((count_ok + 1))
        else
            count_fail=$((count_fail + 1))
            failed+=("$id")
        fi
        finish="$(date +%s)"
        status="$RUN_STATUS"
        printf '%s\t%s\t%d\t%d\t%s\n' \
            "$id" "$status" "$RUN_ATTEMPTS" "$((finish-start))" "$OUTPUT_ROOT/$id" >> "$REPORT_FILE"
        if (( count_fail > 0 && STOP_ON_FAILURE == 1 )); then
            log 'Stopping after first failed Run; use --continue-on-failure to process remaining Runs.'
            break
        fi
    done
    log "SESSION SUMMARY: succeeded=$count_ok failed=$count_fail requested=${#RUNS[@]}"
    log "Reports: $REPORT_FILE"
    if (( count_fail > 0 )); then
        log "Failed IDs: ${failed[*]}"
        log 'Re-run the same command to resume partial transfers; inspect integrity conflicts manually.'
        return 2
    fi
    (( count_ok == ${#RUNS[@]} )) || return 2
    log 'SUCCESS: every requested Run is recorded as validated.'
}

main "$@"
