#!/usr/bin/env python3
"""Generate an SRA Run list from the frozen GSE89223 sample manifest.

Read-only toward the manifest. Refuses to overwrite a different prior run list.
Does not download data, modify cohorts, or accept a scientific validation gate.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MANIFEST = ROOT / "metadata/derived/GSE89223_sample_manifest.tsv"


def planned_runs(manifest: Path) -> tuple[list[str], str]:
    raw = manifest.read_bytes()
    manifest_sha256 = hashlib.sha256(raw).hexdigest()
    with manifest.open("r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        required = {"gsm", "run", "library_layout"}
        if not reader.fieldnames or not required <= set(reader.fieldnames):
            raise ValueError(f"Missing manifest columns: {sorted(required - set(reader.fieldnames or []))}")
        rows = list(reader)
    if len(rows) != 32:
        raise ValueError(f"Expected exactly 32 approved GSE89223 Runs; received {len(rows)}")
    ids: list[str] = []
    gsms: set[str] = set()
    for n, row in enumerate(rows, 2):
        if None in row or None in row.values():
            raise ValueError(f"Malformed TSV row {n}")
        run = row["run"]
        gsm = row["gsm"]
        if not re.fullmatch(r"SRR[0-9]+", run or ""):
            raise ValueError(f"Invalid SRR identifier at line {n}: {run!r}")
        if not re.fullmatch(r"GSM[0-9]+", gsm or ""):
            raise ValueError(f"Invalid GSM identifier at line {n}: {gsm!r}")
        if row["library_layout"] != "SINGLE":
            raise ValueError(f"Non-single-end Run at line {n}: {run}")
        if run in ids or gsm in gsms:
            raise ValueError(f"Duplicate Run/GSM at line {n}: {run}")
        ids.append(run)
        gsms.add(gsm)
    return ids, manifest_sha256


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    p.add_argument("--output", required=True, type=Path, help="Write an immutable run list (.txt); refuse conflicting existing content")
    args = p.parse_args(argv)
    try:
        runs, sha = planned_runs(args.manifest)
        content = ("# GSE89223 / SRP092131: 32 Run accessions from frozen manifest\n"
                   f"# manifest_sha256={sha}\n" + "\n".join(runs) + "\n")
        destination = args.output
        if destination.exists():
            if not destination.is_file() or destination.read_text(encoding="utf-8") != content:
                raise ValueError(f"Refusing to overwrite different prior run list: {destination}")
            print(f"REUSED: {destination} | runs={len(runs)} | manifest_sha256={sha}")
            return 0
        destination.parent.mkdir(parents=True, exist_ok=True)
        try:
            with destination.open("x", encoding="utf-8", newline="\n") as handle:
                handle.write(content)
        except FileExistsError as exc:
            raise ValueError(f"Run list was concurrently created: {destination}; verify its contents") from exc
        print(f"CREATED: {destination} | runs={len(runs)} | manifest_sha256={sha}")
        return 0
    except (OSError, UnicodeError, ValueError, csv.Error) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
