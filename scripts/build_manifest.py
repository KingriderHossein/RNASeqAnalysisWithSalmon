#!/usr/bin/env python3
import csv
import gzip
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOFT = ROOT / "metadata/source/GSE89223_family.soft.gz"
RUNINFO = ROOT / "metadata/source/SRP092131_runinfo.csv"
OUT = ROOT / "metadata/derived/GSE89223_sample_manifest.tsv"

# Final differential-expression cohort reported by Nikitina et al. 2017.
PAPER_TUMOR = {"BP3","CP1","CP2","CP3","CP8","CP11","CP12","CP13","CP14","CP15"}
PAPER_CONTROL = {"BN3","CN1","CN3","CN8","CN11","CN12","CN13","CN14","CN15","BN1","BP1","BP2"}

def parse_soft(path):
    records = []
    current = None
    with gzip.open(path, "rt", errors="replace") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            if line.startswith("^SAMPLE = "):
                if current:
                    records.append(current)
                current = {"gsm": line.split("=", 1)[1].strip(), "characteristics": {}}
            elif current is not None:
                if line.startswith("!Sample_title = "):
                    current["title"] = line.split("=", 1)[1].strip()
                elif line.startswith("!Sample_source_name_ch1 = "):
                    current["source_name"] = line.split("=", 1)[1].strip()
                elif line.startswith("!Sample_characteristics_ch1 = "):
                    value = line.split("=", 1)[1].strip()
                    if ":" in value:
                        key, val = value.split(":", 1)
                        current["characteristics"][key.strip().lower()] = val.strip().replace("\xa0", " ")
                elif line.startswith("!Sample_relation = BioSample: "):
                    current["biosample"] = line.rsplit("/", 1)[-1].strip()
                elif line.startswith("!Sample_relation = SRA: "):
                    current["srx"] = line.rsplit("=", 1)[-1].strip()
    if current:
        records.append(current)
    return records

def classify(diagnosis, tissue):
    if diagnosis == "prostate cancer" and tissue == "tumorous prostate tissue":
        return "pca_tumor"
    if diagnosis == "prostate cancer" and tissue == "normal prostate tissue":
        return "pca_adjacent_normal"
    if diagnosis == "benign prostatic hyperplasia" and tissue == "adenomatous prostate tissue":
        return "bph_adenoma"
    if diagnosis == "benign prostatic hyperplasia" and tissue == "normal prostate tissue":
        return "bph_normal"
    return "unclassified"

def main():
    records = parse_soft(SOFT)
    with RUNINFO.open(newline="") as handle:
        runs = {row["Experiment"]: row for row in csv.DictReader(handle)}

    rows = []
    for record in records:
        ch = record["characteristics"]
        diagnosis = ch.get("diagnosis", "")
        tissue = ch.get("tissue", "")
        title = record.get("title", "")
        run = runs.get(record.get("srx", ""), {})
        flags = []
        if title.startswith("B") and diagnosis == "prostate cancer":
            flags.append("B-prefix but GEO diagnosis=prostate cancer; paper confirms record correction")
        if not run:
            flags.append("no SRA RunInfo match")
        paper_group = "tumor" if title in PAPER_TUMOR else "control" if title in PAPER_CONTROL else "excluded"
        rows.append({
            "gsm": record.get("gsm", ""),
            "title": title,
            "patient_id": ch.get("patient identifier", ""),
            "diagnosis": diagnosis,
            "tissue": tissue,
            "analysis_group": classify(diagnosis, tissue),
            "pair_status": "pending",
            "paper_final_set": "yes" if paper_group != "excluded" else "no",
            "paper_group": paper_group,
            "paired_pca_sensitivity": "pending",
            "srx": record.get("srx", ""),
            "run": run.get("Run", ""),
            "biosample": record.get("biosample", ""),
            "library_strategy": run.get("LibraryStrategy", ""),
            "library_selection": run.get("LibrarySelection", ""),
            "library_source": run.get("LibrarySource", ""),
            "library_layout": run.get("LibraryLayout", ""),
            "platform": run.get("Platform", ""),
            "model": run.get("Model", ""),
            "spots": run.get("spots", ""),
            "bases": run.get("bases", ""),
            "avg_length": run.get("avgLength", ""),
            "size_mb": run.get("size_MB", ""),
            "source_name": record.get("source_name", ""),
            "age": ch.get("age at operation", ""),
            "psa": ch.get("psa level (ng/ml)", ""),
            "gleason_sum": ch.get("gleason sum", ""),
            "primary_gleason": ch.get("primary gleason score", ""),
            "secondary_gleason": ch.get("secondary gleason score", ""),
            "extraprostatic_invasion": ch.get("extraprostatic invasion", ""),
            "population": ch.get("population", ""),
            "review_flag": "; ".join(flags),
        })

    by_patient = defaultdict(set)
    for row in rows:
        by_patient[row["patient_id"]].add(row["analysis_group"])

    for row in rows:
        groups = by_patient[row["patient_id"]]
        if row["analysis_group"] in {"pca_tumor", "pca_adjacent_normal"}:
            paired = {"pca_tumor", "pca_adjacent_normal"} <= groups
            row["pair_status"] = "paired" if paired else "unpaired"
            row["paired_pca_sensitivity"] = "yes" if paired and row["paper_final_set"] == "yes" else "no"
        elif row["analysis_group"] in {"bph_adenoma", "bph_normal"}:
            row["pair_status"] = "paired" if {"bph_adenoma", "bph_normal"} <= groups else "unpaired"
            row["paired_pca_sensitivity"] = "no"
        else:
            row["pair_status"] = "unclassified"
            row["paired_pca_sensitivity"] = "no"

    fields = list(rows[0])
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, delimiter="\t")
        writer.writeheader()
        writer.writerows(rows)

    print("samples:", len(rows))
    print("biological groups:", dict(Counter(r["analysis_group"] for r in rows)))
    print("paper groups:", dict(Counter(r["paper_group"] for r in rows)))
    print("paper cohort n:", sum(r["paper_final_set"] == "yes" for r in rows))
    print("paired PCa sensitivity n:", sum(r["paired_pca_sensitivity"] == "yes" for r in rows))
    print("paired PCa patients:", sorted({r["patient_id"] for r in rows if r["paired_pca_sensitivity"] == "yes"}))
    print("layouts:", dict(Counter(r["library_layout"] for r in rows)))
    print("archived MB:", sum(int(r["size_mb"] or 0) for r in rows))

if __name__ == "__main__":
    main()