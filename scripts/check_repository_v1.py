#!/usr/bin/env python3
"""Repository checks v1.0.0: read-only, offline, standard library only.

Checks tracked files and the committed cohort contract. Does not import analysis
scripts, run R/Salmon, check remote URLs or accept scientific validation gates.
"""

from __future__ import annotations

import ast
import csv
import json
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
VERSION = "1.0.0"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True
    )
    return [ROOT / name.decode("utf-8") for name in result.stdout.split(b"\0") if name]


def check_manifest(path: Path) -> None:
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        columns = set(reader.fieldnames or [])
        required = {
            "gsm", "title", "run", "patient_id", "analysis_group",
            "paper_final_set", "paper_group", "paper_exclusion_reason",
            "paired_pca_sensitivity", "library_layout",
        }
        require(required <= columns, f"Missing manifest columns: {sorted(required - columns)}")
        rows = list(reader)

    require(len(rows) == 32, "Manifest must contain exactly 32 runs")
    require(all(None not in row and None not in row.values() for row in rows),
            "Malformed TSV row: inconsistent column count")
    for key in ("gsm", "title", "run"):
        values = [row[key] for row in rows]
        require(all(values) and len(set(values)) == 32, f"Empty or duplicate {key}")
    require(all(re.fullmatch(r"SRR\d+", row["run"]) for row in rows), "Invalid SRR ID")
    require(all(row["library_layout"] == "SINGLE" for row in rows),
            "All study runs must be single-end")
    for row in rows:
        for key in ("paper_final_set", "paired_pca_sensitivity"):
            require(row[key] in {"yes", "no"}, f"Invalid {key} for {row['run']}")
        if row["paper_final_set"] == "no":
            require(row["paper_group"] == "excluded" and bool(row["paper_exclusion_reason"]),
                    f"Missing exclusion provenance: {row['run']}")

    track_a = [row for row in rows if row["paper_final_set"] == "yes"]
    require(Counter(row["paper_group"] for row in track_a) == {"tumor": 10, "control": 12},
            "Track A must contain 10 tumor and 12 control samples")
    track_b = [row for row in rows if row["paired_pca_sensitivity"] == "yes"]
    require(len(track_b) == 18, "Track B must contain 18 samples")
    require(all(row["paper_final_set"] == "yes" for row in track_b),
            "Track B membership must stay within the retained paper cohort")
    pairs: dict[str, list[str]] = defaultdict(list)
    for row in track_b:
        require(bool(row["patient_id"]), f"Missing patient ID: {row['run']}")
        pairs[row["patient_id"]].append(row["analysis_group"])
    require(len(pairs) == 9, "Track B must contain nine patients")
    for patient, groups in pairs.items():
        require(Counter(groups) == {"pca_tumor": 1, "pca_adjacent_normal": 1},
                f"Invalid tumor/adjacent-normal pair: {patient}")


def markdown_targets(text: str) -> list[str]:
    # Only link destinations are checked. Heading anchors and external URLs are
    # intentionally out of scope. Fenced/inline examples are not live links.
    text = re.sub(r"(?ms)^\s*(`{3,}|~{3,})[^\n]*\n.*?^\s*\1\s*$", "", text)
    text = re.sub(r"(`+).*?\1", "", text, flags=re.S)
    inline = re.findall(r"\]\(\s*(?:<([^>]+)>|([^\s)]+))", text)
    references = re.findall(
        r"(?m)^ {0,3}\[[^\]\n]+\]:\s*(?:<([^>]+)>|(\S+))", text
    )
    return [angle or plain for angle, plain in inline + references]


def check_links(path: Path) -> int:
    count = 0
    for target in markdown_targets(path.read_text(encoding="utf-8")):
        url = urlsplit(target)
        if url.scheme or url.netloc or not url.path:
            continue
        decoded = unquote(url.path)
        destination = ((ROOT / decoded.lstrip("/")) if decoded.startswith("/")
                       else (path.parent / decoded)).resolve()
        require(destination.is_relative_to(ROOT), f"Link leaves repository: {target}")
        require(destination.exists(), f"Missing link target: {target}")
        count += 1
    return count


def main() -> int:
    files = tracked_files()
    failures: list[str] = []
    counts: Counter[str] = Counter()
    for path in files:
        try:
            if path.suffix == ".py":
                ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
                counts["Python files"] += 1
            elif path.suffix == ".json":
                json.loads(path.read_text(encoding="utf-8"))
                counts["JSON files"] += 1
            elif path.suffix == ".svg":
                tree = ET.parse(path)
                require(tree.getroot().tag == "{http://www.w3.org/2000/svg}svg",
                        "SVG namespace/root is missing")
                counts["SVG files"] += 1
            elif path.suffix == ".md":
                counts["relative Markdown links"] += check_links(path)
                counts["Markdown files"] += 1
        except (OSError, ValueError, SyntaxError, ET.ParseError) as error:
            failures.append(f"{path.relative_to(ROOT)}: {error}")
    try:
        check_manifest(ROOT / "metadata/derived/GSE89223_sample_manifest.tsv")
        counts["cohort manifests"] += 1
    except (OSError, ValueError, KeyError) as error:
        failures.append(f"Sample manifest: {error}")

    print(f"Repository checks v{VERSION}")
    for label, count in sorted(counts.items()):
        print(f"  {label}: {count}")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print("PASS: static checks and cohort structure; scientific acceptance is separate.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
