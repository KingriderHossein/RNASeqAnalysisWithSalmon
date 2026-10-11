# Stage 4 — raw SRA acquisition (Ubuntu and Windows + WSL2)

These are **project-scoped command-line acquisition helpers** for GSE89223 / SRP092131. They do not restore the separate Rust/Tauri Download Manager.

**Stage 1 is the sample/cohort lock, not raw download.** The authoritative list is [the 32-run manifest](../metadata/derived/GSE89223_sample_manifest.tsv). **Stage 4** uses that list to acquire SRA archives, then separately extracts and checks FASTQ, performs FastQC/MultiQC, and documents the preprocessing decision before G4 acceptance. Acquisition success does not imply a passing scientific gate.

## Included entry points

| File | Role |
| --- | --- |
| [manifest_runs.py](../scripts/acquisition/manifest_runs.py) | Derive 32 approved SRR IDs from the frozen manifest; reject invalid/duplicate IDs and non-single-end entries; refuse to overwrite a different list |
| [sra_downloader_v1.5.0.sh](../scripts/acquisition/sra_downloader_v1.5.0.sh) | Linux/Ubuntu CLI: prefetch + resume/retry + vdb-validate + SHA-256, logs, disk and mount guards |
| [sra-downloader-wsl.ps1](../scripts/acquisition/sra-downloader-wsl.ps1) | Windows PowerShell front end calling the **same Bash engine** in WSL2 Ubuntu; translates Windows paths through wslpath |
| [Offline Bash tests](../scripts/acquisition/test_sra_downloader_v1.5.0.sh) | Synthetic stubs; no SRA or network calls |
| [Manifest tests](../scripts/acquisition/test_manifest_runs.py) | Validate manifest list generation without downloading |

Cloning this repository and running CI never downloads sequencing data.

## Requirements

Ubuntu / Debian Linux, or Windows 10/11 with **WSL2 and Ubuntu**; Bash 4.4+, NCBI SRA Toolkit inside Linux (prefetch and vdb-validate), GNU coreutils/findutils/util-linux, and disk space for the intended Runs. Python 3.10+ is used **only for generating the run list**. For Windows use Windows PowerShell 5.1+ or PowerShell 7 and wsl.exe. These wrappers do not implement a native Windows downloader.

Install dependencies **inside Ubuntu or WSL Ubuntu**:

~~~bash
sudo apt update
sudo apt install sra-toolkit coreutils findutils util-linux gawk
prefetch --version
vdb-validate --version
~~~

The Ubuntu package may be older than NCBI's current release. The Bash CLI detects required prefetch flags and refuses an incompatible installation.

## 1. Generate only manifest-approved Run IDs

From the cloned repository root:

~~~bash
python3 scripts/acquisition/manifest_runs.py --output raw/acquisition/GSE89223_runs.txt
~~~

This lists all **32** manifest runs, including samples excluded from a particular downstream analysis track. It does **not** change Track A/Track B membership. The generated text includes a SHA-256 of the source TSV. Identical reruns reuse an existing file; different existing files are not overwritten. The raw/ directory is Git-ignored.

From Windows PowerShell (if Python is installed on Windows):

~~~powershell
py -3 .\scripts\acquisition\manifest_runs.py --output .\raw\acquisition\GSE89223_runs.txt
~~~

Alternatively, generate the list inside WSL Ubuntu from the same checkout.

## 2. Ubuntu — dry run and real acquisition

~~~bash
bash scripts/acquisition/sra_downloader_v1.5.0.sh --input raw/acquisition/GSE89223_runs.txt --output "$HOME/gse89223/sra" --dry-run
~~~

Test a single **real manifest Run** first:

~~~bash
bash scripts/acquisition/sra_downloader_v1.5.0.sh --run SRR4453804 --output "$HOME/gse89223/sra" --min-free-gb 20
~~~

After validating disk space and one real download, repeat the batch command **without** --dry-run to acquire all 32. To process later Runs after one fails, add --continue-on-failure. Processing is **sequential**, not parallel.

For an external Linux disk, verify the real mountpoint using findmnt and pass --require-mountpoint together with an output location underneath that mount. Do not use a guessed /mnt path. Mount detection is checked at startup and cannot guarantee continued physical disk availability.

## 3. Windows + WSL2 — PowerShell

Clone the repository under a normal **Windows drive-letter path**, so the PowerShell file and its neighboring Bash file can be resolved by wslpath. Run PowerShell **from the repository root**:

~~~powershell
wsl --list --verbose
.\scripts\acquisition\sra-downloader-wsl.ps1 -Check -Distribution Ubuntu
.\scripts\acquisition\sra-downloader-wsl.ps1 -Run SRR4453804 -Distribution Ubuntu -DryRun
~~~

Start one Run with outputs on the Linux filesystem (the default, generally faster for Linux I/O):

~~~powershell
.\scripts\acquisition\sra-downloader-wsl.ps1 -Run SRR4453804 -Distribution Ubuntu
~~~

To download all manifest runs after creating the file:

~~~powershell
.\scripts\acquisition\sra-downloader-wsl.ps1 -InputFile '.\raw\acquisition\GSE89223_runs.txt' -Distribution Ubuntu -ContinueOnFailure
~~~

To use a Windows data drive explicitly:

~~~powershell
.\scripts\acquisition\sra-downloader-wsl.ps1 -Run SRR4453804 -Distribution Ubuntu -OutputDir 'D:\RNA_Data\SRA' -MinFreeGiB 30
~~~

The launcher adds a WSL mountpoint guard for drive-letter output directories. Windows network shares / UNC paths are **not** supported by the launcher. No global PowerShell execution-policy change is required by the package; follow your organization's security policy.

## Resume, safety and output records

- The downloader asks prefetch to resume partial SRA transfers. It retries with exponential backoff and preserves partial data. If the process ends or Windows reboots, re-run the **same command**; there is no reboot auto-start.
- For each Run, evidence resides under the chosen SRA output root: an accession directory, .logs/, .checksums/ (local SHA-256), .complete/, .reports/ (per-session TSV), and a locking file.
- Checksum conflicts are not silently accepted. A local SHA-256 is **not** an independent NCBI-published checksum.
- The free-space reserve is **not an upfront size estimate**; prefetch and subsequent FASTQ conversion can require substantial disk space.
- Data are never deleted or moved by the downloader. Existing validated FASTQ, old raw data and Salmon results remain untouched.
- The two wrappers download **SRA archives only**. They do not extract FASTQ, run QC, run Salmon or approve G4.

## 4. Extract FASTQ separately (single-end study)

After downloading and validating a Run with the same output root:

~~~bash
mkdir -p "$HOME/gse89223/fastq" "$HOME/gse89223/tmp"
fasterq-dump "$HOME/gse89223/sra/SRR4453804" --split-files -e 4 -O "$HOME/gse89223/fastq" -t "$HOME/gse89223/tmp" -p
~~~

Inspect actual FASTQ filenames and read counts. This dataset is **single-end Ion Torrent Proton**, so do not assume paired-end files. Check sizes and SHA-256 for exact FASTQ/FASTQ.gz outputs, preserve acquisition metadata, then run FastQC/MultiQC and apply [G4 scientific validation](VALIDATION.md). These conversion/QC tasks remain separately controlled; the new downloader **does not run them automatically**.

## Offline verification

~~~bash
bash -n scripts/acquisition/sra_downloader_v1.5.0.sh
bash scripts/acquisition/test_sra_downloader_v1.5.0.sh
python3 -m unittest discover -s scripts/acquisition -p 'test_*.py'
python3 scripts/check_repository_v1.py
~~~

The Bash suite uses stub prefetch/vdb-validate and **does not access the network**. CI also parses PowerShell on Windows without starting WSL. These checks do not prove live NCBI/WSL2 end-to-end correctness or close a scientific gate.
