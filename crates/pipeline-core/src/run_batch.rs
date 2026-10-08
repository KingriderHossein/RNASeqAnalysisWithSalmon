//! Strict, bounded resolved-run input shared by both application clients.
use crate::{Accession, AccessionLevel};
use csv::ReaderBuilder;
use std::{collections::HashSet, fs::File, io::Read, path::Path};

pub const MAX_BATCH_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_BATCH_RUNS: usize = 10_000;

#[derive(Debug, Clone)]
pub struct RunBatch {
    pub accessions: Vec<Accession>,
    pub duplicate_count: usize,
    pub metadata_columns: Vec<String>,
}

/// Read at most the limit plus one byte, including if the file grows while read.
pub fn read_run_batch(path: impl AsRef<Path>) -> Result<RunBatch, String> {
    let path = path.as_ref();
    let format = batch_format(path)?;
    if !std::fs::metadata(path)
        .map_err(|e| format!("cannot inspect batch file: {e}"))?
        .is_file()
    {
        return Err("batch input must be a regular file".into());
    }
    let file = File::open(path).map_err(|e| format!("cannot open batch file: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("batch input must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BATCH_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read batch file: {e}"))?;
    if bytes.len() > MAX_BATCH_BYTES {
        return Err("batch file exceeds the 2 MiB limit; split it into smaller files".into());
    }
    let content =
        std::str::from_utf8(&bytes).map_err(|_| "batch file must use UTF-8 text".to_owned())?;
    parse_run_batch(content, format)
}

fn batch_format(path: &Path) -> Result<&str, String> {
    match path.extension().and_then(|value| value.to_str()) {
        Some(value) if value.eq_ignore_ascii_case("txt") => Ok("txt"),
        Some(value) if value.eq_ignore_ascii_case("csv") => Ok("csv"),
        Some(value) if value.eq_ignore_ascii_case("tsv") => Ok("tsv"),
        _ => Err("choose a TXT, CSV or TSV batch file".into()),
    }
}

pub fn parse_run_batch(content: &str, format: &str) -> Result<RunBatch, String> {
    if content.len() > MAX_BATCH_BYTES {
        return Err("batch file exceeds the 2 MiB limit; split it into smaller files".into());
    }
    let content = content.trim_start_matches('\u{feff}');
    let mut batch = RunBatch {
        accessions: Vec::new(),
        duplicate_count: 0,
        metadata_columns: Vec::new(),
    };
    let mut seen = HashSet::new();
    match format {
        "txt" => {
            for (index, line) in content.lines().enumerate() {
                for field in line.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
                    if field.is_empty() || (index == 0 && run_header(field)) {
                        continue;
                    }
                    add_run(&mut batch, &mut seen, field, index + 1)?;
                }
            }
        }
        "csv" | "tsv" => {
            validate_quotes(content, if format == "csv" { b',' } else { b'\t' })?;
            let mut reader = ReaderBuilder::new()
                .has_headers(false)
                .flexible(false)
                .delimiter(if format == "csv" { b',' } else { b'\t' })
                .from_reader(content.as_bytes());
            let mut run_column = None;
            for (index, record) in reader.records().enumerate() {
                let record = record.map_err(|_| {
                    format!(
                        "invalid delimited record near row {}; check column counts and UTF-8",
                        index + 1
                    )
                })?;
                if index == 0 {
                    let columns = record
                        .iter()
                        .enumerate()
                        .filter(|(_, name)| run_header(name))
                        .map(|(column, _)| column)
                        .collect::<Vec<_>>();
                    if columns.len() > 1 {
                        return Err(
                            "batch header has multiple run columns; keep exactly one".into()
                        );
                    }
                    if let Some(column) = columns.first() {
                        run_column = Some(*column);
                        batch.metadata_columns = record
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| i != column)
                            .map(|(_, name)| name.trim().to_owned())
                            .collect();
                        continue;
                    }
                }
                if let Some(column) = run_column {
                    add_run(&mut batch, &mut seen, &record[column], index + 1)?;
                } else {
                    for field in &record {
                        add_run(&mut batch, &mut seen, field, index + 1)?;
                    }
                }
            }
        }
        _ => return Err("choose a TXT, CSV or TSV batch file".into()),
    }
    if batch.accessions.is_empty() {
        return Err("batch file has no run accessions; supply SRR, ERR or DRR runs".into());
    }
    Ok(batch)
}

// The csv reader accepts an unterminated quoted field at EOF. Reject it here,
// and reject bare/trailing quotes, before accepting any accession selection.
fn validate_quotes(content: &str, delimiter: u8) -> Result<(), String> {
    let bytes = content.as_bytes();
    let mut index = 0;
    let mut quoted = false;
    let mut closed = false;
    let mut field_start = true;
    while index < bytes.len() {
        let byte = bytes[index];
        if quoted {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 1;
                } else {
                    quoted = false;
                    closed = true;
                }
            }
        } else if byte == delimiter || byte == b'\n' || byte == b'\r' {
            closed = false;
            field_start = true;
        } else if byte == b'"' && field_start && !closed {
            quoted = true;
            field_start = false;
        } else {
            if closed || byte == b'"' {
                return Err("invalid CSV/TSV quoting; check quotes and delimiters".into());
            }
            field_start = false;
        }
        index += 1;
    }
    if quoted {
        return Err("unterminated CSV/TSV quote; close the quoted field".into());
    }
    Ok(())
}

fn run_header(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "run" | "run_accession" | "sra_run" | "srr" | "accession" | "accessions"
    )
}

fn add_run(
    batch: &mut RunBatch,
    seen: &mut HashSet<String>,
    value: &str,
    row: usize,
) -> Result<(), String> {
    let accession = Accession::parse(value).map_err(|_| {
        format!("row {row} has an empty or invalid run; supply SRR, ERR or DRR accessions")
    })?;
    if accession.level() != AccessionLevel::Run {
        return Err(format!(
            "row {row} needs study/experiment resolution; supply its run accessions first"
        ));
    }
    if seen.insert(accession.as_str().to_owned()) {
        if batch.accessions.len() == MAX_BATCH_RUNS {
            return Err("batch exceeds 10,000 unique runs; split it into smaller files".into());
        }
        batch.accessions.push(accession);
    } else {
        batch.duplicate_count += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(batch: &RunBatch) -> Vec<&str> {
        batch.accessions.iter().map(Accession::as_str).collect()
    }

    #[test]
    fn text_normalizes_bom_crlf_and_deduplicates_in_order() {
        let batch = parse_run_batch("\u{feff}run\r\n srr2;ERR1\r\nSRR2,DRR3\r\n", "txt").unwrap();
        assert_eq!(values(&batch), ["SRR2", "ERR1", "DRR3"]);
        assert_eq!(batch.duplicate_count, 1);
    }

    #[test]
    fn selected_column_never_imports_accession_like_metadata() {
        let batch = parse_run_batch(
            "sra_run,experiment,notes\nSRR2,SRX9,ERR999\nERR1,SRX8,hello\n",
            "csv",
        )
        .unwrap();
        assert_eq!(values(&batch), ["SRR2", "ERR1"]);
        assert_eq!(batch.metadata_columns, ["experiment", "notes"]);
        let tsv = parse_run_batch("Run\tgroup\nDRR1\ttumor\n", "tsv").unwrap();
        assert_eq!(values(&tsv), ["DRR1"]);
    }

    #[test]
    fn headerless_records_require_every_cell_to_be_a_run() {
        assert_eq!(
            values(&parse_run_batch("SRR1,ERR2\nDRR3,SRR1", "csv").unwrap()),
            ["SRR1", "ERR2", "DRR3"]
        );
        assert!(parse_run_batch("SRR1,tumor", "csv").is_err());
    }

    #[test]
    fn invalid_or_ambiguous_input_never_returns_a_partial_batch() {
        for (content, format) in [
            ("SRR1\nSRRbroken", "txt"),
            ("SRR1\nSRX2", "txt"),
            ("GSE89223", "txt"),
            ("run\n", "txt"),
            ("\n", "txt"),
            ("run\nSRR1\n\"\"", "csv"),
            ("run,notes\nSRR1", "csv"),
            ("run,srr\nSRR1,SRR2", "csv"),
            ("sample,group\nSRR1,tumor", "csv"),
            ("\"SRR1", "csv"),
            ("SRR1\"", "csv"),
            ("\"SRR1\"extra", "csv"),
        ] {
            assert!(parse_run_batch(content, format).is_err(), "{content:?}");
        }
    }

    #[test]
    fn quoted_metadata_can_contain_delimiters_newlines_and_escaped_quotes() {
        let batch = parse_run_batch("run,notes\n\"SRR1\",\"a,b\n\"\"quoted\"\"\"", "csv").unwrap();
        assert_eq!(values(&batch), ["SRR1"]);
    }

    #[test]
    fn resource_limits_apply_to_direct_content_calls_too() {
        assert!(parse_run_batch(&" ".repeat(MAX_BATCH_BYTES + 1), "txt").is_err());
        let too_many = (0..=MAX_BATCH_RUNS)
            .map(|i| format!("SRR{i}\n"))
            .collect::<String>();
        assert!(parse_run_batch(&too_many, "txt").is_err());
    }

    #[test]
    fn project_manifest_imports_all_runs_without_changing_cohort_membership() {
        let manifest = include_str!("../../../metadata/derived/GSE89223_sample_manifest.tsv");
        let batch = parse_run_batch(manifest, "tsv").unwrap();
        assert_eq!(batch.accessions.len(), 32);
        assert_eq!(batch.duplicate_count, 0);
        assert!(batch
            .metadata_columns
            .iter()
            .any(|name| name == "paper_final_set"));
    }
}
