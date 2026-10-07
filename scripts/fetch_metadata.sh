#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="$ROOT/metadata/source"
mkdir -p "$SOURCE_DIR"

curl -L --fail --retry 3 \
  -o "$SOURCE_DIR/GSE89223_family.soft.gz" \
  "https://ftp.ncbi.nlm.nih.gov/geo/series/GSE89nnn/GSE89223/soft/GSE89223_family.soft.gz"

gzip -t "$SOURCE_DIR/GSE89223_family.soft.gz"

curl -L --fail --retry 3 \
  -o "$SOURCE_DIR/SRP092131_runinfo.csv" \
  "https://trace.ncbi.nlm.nih.gov/Traces/sra-db-be/runinfo?acc=SRP092131"

test "$(wc -l < "$SOURCE_DIR/SRP092131_runinfo.csv")" -eq 33

echo "Fetched GEO SOFT and SRA RunInfo metadata for GSE89223 / SRP092131."
