use csv::ReaderBuilder;
use std::{
    collections::HashSet,
    fmt, fs,
    path::{Path, PathBuf},
};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessionLevel {
    Study,
    Experiment,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessionKind {
    GeoSeries,
    SraStudy,
    SraExperiment,
    SraRun,
    EnaStudy,
    EnaExperiment,
    EnaRun,
    DdbjStudy,
    DdbjExperiment,
    DdbjRun,
}

impl AccessionKind {
    pub fn level(self) -> AccessionLevel {
        match self {
            Self::GeoSeries | Self::SraStudy | Self::EnaStudy | Self::DdbjStudy => {
                AccessionLevel::Study
            }
            Self::SraExperiment | Self::EnaExperiment | Self::DdbjExperiment => {
                AccessionLevel::Experiment
            }
            Self::SraRun | Self::EnaRun | Self::DdbjRun => AccessionLevel::Run,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Accession {
    value: String,
    kind: AccessionKind,
}

impl Accession {
    pub fn parse(input: &str) -> Result<Self, InputError> {
        let value = input.trim().to_ascii_uppercase();
        let Some((prefix, digits)) = split_prefix_digits(&value) else {
            return Err(InputError::InvalidAccession(input.trim().to_owned()));
        };
        let kind = match prefix {
            "GSE" => AccessionKind::GeoSeries,
            "SRP" => AccessionKind::SraStudy,
            "SRX" => AccessionKind::SraExperiment,
            "SRR" => AccessionKind::SraRun,
            "ERP" => AccessionKind::EnaStudy,
            "ERX" => AccessionKind::EnaExperiment,
            "ERR" => AccessionKind::EnaRun,
            "DRP" => AccessionKind::DdbjStudy,
            "DRX" => AccessionKind::DdbjExperiment,
            "DRR" => AccessionKind::DdbjRun,
            _ => return Err(InputError::InvalidAccession(input.trim().to_owned())),
        };

        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InputError::InvalidAccession(input.trim().to_owned()));
        }

        Ok(Self { value, kind })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn kind(&self) -> AccessionKind {
        self.kind
    }

    pub fn level(&self) -> AccessionLevel {
        self.kind.level()
    }

    pub fn requires_run_resolution(&self) -> bool {
        self.level() != AccessionLevel::Run
    }
}

impl fmt::Display for Accession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchInput {
    pub accessions: Vec<Accession>,
    pub ignored_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectUrl {
    url: Url,
}

impl DirectUrl {
    pub fn parse(input: &str) -> Result<Self, InputError> {
        let mut url = Url::parse(input.trim()).map_err(InputError::InvalidUrl)?;
        match url.scheme() {
            "http" | "https" => {}
            other => return Err(InputError::UnsupportedUrlScheme(other.to_owned())),
        }
        if url.host_str().is_none() {
            return Err(InputError::MissingUrlHost);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(InputError::EmbeddedUrlCredentials);
        }

        url.set_fragment(None);
        Ok(Self { url })
    }

    pub fn as_url(&self) -> &Url {
        &self.url
    }

    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }
}

#[derive(Debug)]
pub enum InputError {
    InvalidAccession(String),
    UnsupportedBatchExtension(PathBuf),
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    Csv {
        path: PathBuf,
        source: csv::Error,
    },
    EmptyBatch(PathBuf),
    InvalidUrl(url::ParseError),
    UnsupportedUrlScheme(String),
    MissingUrlHost,
    EmbeddedUrlCredentials,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAccession(value) => write!(f, "unsupported or invalid accession: {value}"),
            Self::UnsupportedBatchExtension(path) => {
                write!(f, "unsupported batch file type: {}", path.display())
            }
            Self::ReadFile { path, source } => {
                write!(f, "cannot read batch file {}: {source}", path.display())
            }
            Self::Csv { path, source } => {
                write!(f, "cannot parse batch file {}: {source}", path.display())
            }
            Self::EmptyBatch(path) => {
                write!(
                    f,
                    "batch file contains no supported accessions: {}",
                    path.display()
                )
            }
            Self::InvalidUrl(source) => write!(f, "invalid direct URL: {source}"),
            Self::UnsupportedUrlScheme(scheme) => {
                write!(f, "unsupported direct URL scheme: {scheme}")
            }
            Self::MissingUrlHost => f.write_str("direct URL must contain a host"),
            Self::EmbeddedUrlCredentials => {
                f.write_str("direct URL must not contain embedded username/password credentials")
            }
        }
    }
}

impl std::error::Error for InputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::Csv { source, .. } => Some(source),
            Self::InvalidUrl(source) => Some(source),
            Self::InvalidAccession(_)
            | Self::UnsupportedBatchExtension(_)
            | Self::EmptyBatch(_)
            | Self::UnsupportedUrlScheme(_)
            | Self::MissingUrlHost
            | Self::EmbeddedUrlCredentials => None,
        }
    }
}

pub fn parse_batch_file(path: impl AsRef<Path>) -> Result<BatchInput, InputError> {
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("txt") => parse_text_batch(path),
        Some("csv") => parse_delimited_batch(path, b','),
        Some("tsv") => parse_delimited_batch(path, b'\t'),
        _ => Err(InputError::UnsupportedBatchExtension(path.to_path_buf())),
    }
}

fn parse_text_batch(path: &Path) -> Result<BatchInput, InputError> {
    let content = fs::read_to_string(path).map_err(|source| InputError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let fields = content.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';');
    collect_batch_fields(path, fields)
}

fn parse_delimited_batch(path: &Path, delimiter: u8) -> Result<BatchInput, InputError> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_path(path)
        .map_err(|source| InputError::Csv {
            path: path.to_path_buf(),
            source,
        })?;

    let mut fields = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|source| InputError::Csv {
            path: path.to_path_buf(),
            source,
        })?;
        fields.extend(record.iter().map(str::to_owned));
    }

    collect_batch_fields(path, fields.iter().map(String::as_str))
}

fn collect_batch_fields<'a>(
    path: &Path,
    fields: impl IntoIterator<Item = &'a str>,
) -> Result<BatchInput, InputError> {
    let mut seen_accessions = HashSet::new();
    let mut seen_ignored = HashSet::new();
    let mut accessions = Vec::new();
    let mut ignored_fields = Vec::new();

    for field in fields {
        let field = field.trim();
        if field.is_empty() || is_known_header(field) {
            continue;
        }

        match Accession::parse(field) {
            Ok(accession) => {
                if seen_accessions.insert(accession.as_str().to_owned()) {
                    accessions.push(accession);
                }
            }
            Err(_) => {
                if seen_ignored.insert(field.to_owned()) {
                    ignored_fields.push(field.to_owned());
                }
            }
        }
    }

    if accessions.is_empty() {
        return Err(InputError::EmptyBatch(path.to_path_buf()));
    }

    Ok(BatchInput {
        accessions,
        ignored_fields,
    })
}

fn is_known_header(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "accession"
            | "accessions"
            | "run"
            | "run_accession"
            | "study_accession"
            | "experiment_accession"
            | "sample"
            | "sample_id"
            | "id"
    )
}

fn split_prefix_digits(value: &str) -> Option<(&str, &str)> {
    if value.len() < 4 {
        return None;
    }
    let prefix = value.get(..3)?;
    let digits = value.get(3..)?;
    Some((prefix, digits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_file(extension: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rnaseq-input-test-{}-{nonce}.{extension}",
            process::id()
        ))
    }

    #[test]
    fn parses_and_normalizes_supported_accessions() {
        let run = Accession::parse(" srr4453783 ").expect("valid run accession");
        assert_eq!(run.as_str(), "SRR4453783");
        assert_eq!(run.kind(), AccessionKind::SraRun);
        assert_eq!(run.level(), AccessionLevel::Run);
        assert!(!run.requires_run_resolution());

        let study = Accession::parse("gse89223").expect("valid GEO accession");
        assert_eq!(study.kind(), AccessionKind::GeoSeries);
        assert!(study.requires_run_resolution());

        let experiment = Accession::parse("ERX12345").expect("valid ENA experiment");
        assert_eq!(experiment.level(), AccessionLevel::Experiment);
    }

    #[test]
    fn rejects_invalid_single_accession() {
        assert!(matches!(
            Accession::parse("SRRabc"),
            Err(InputError::InvalidAccession(_))
        ));
        assert!(matches!(
            Accession::parse("XYZ12345"),
            Err(InputError::InvalidAccession(_))
        ));
        assert!(matches!(
            Accession::parse("éRR12345"),
            Err(InputError::InvalidAccession(_))
        ));
    }

    #[test]
    fn parses_text_batch_and_deduplicates_in_first_seen_order() {
        let path = temp_file("txt");
        fs::write(&path, "SRR000002\nSRR000001\nsrr000002\nnot_an_accession\n")
            .expect("write batch file");

        let batch = parse_batch_file(&path).expect("parse batch");
        let values = batch
            .accessions
            .iter()
            .map(Accession::as_str)
            .collect::<Vec<_>>();

        assert_eq!(values, vec!["SRR000002", "SRR000001"]);
        assert_eq!(batch.ignored_fields, vec!["not_an_accession"]);

        fs::remove_file(path).expect("remove batch file");
    }

    #[test]
    fn parses_csv_without_turning_headers_or_metadata_into_accessions() {
        let path = temp_file("csv");
        fs::write(
            &path,
            "accession,group\nSRR000001,tumor\nSRX000002,control\nSRR000001,tumor\n",
        )
        .expect("write CSV");

        let batch = parse_batch_file(&path).expect("parse CSV");
        assert_eq!(batch.accessions.len(), 2);
        assert_eq!(batch.accessions[0].as_str(), "SRR000001");
        assert_eq!(batch.accessions[1].as_str(), "SRX000002");
        assert_eq!(batch.ignored_fields, vec!["group", "tumor", "control"]);

        fs::remove_file(path).expect("remove CSV");
    }

    #[test]
    fn parses_tsv_batch() {
        let path = temp_file("tsv");
        fs::write(
            &path,
            "run_accession\tgroup\nERR000001\ttumor\nDRR000002\tnormal\n",
        )
        .expect("write TSV");

        let batch = parse_batch_file(&path).expect("parse TSV");
        assert_eq!(batch.accessions.len(), 2);
        assert_eq!(batch.accessions[0].kind(), AccessionKind::EnaRun);
        assert_eq!(batch.accessions[1].kind(), AccessionKind::DdbjRun);

        fs::remove_file(path).expect("remove TSV");
    }

    #[test]
    fn batch_without_accessions_is_rejected() {
        let path = temp_file("txt");
        fs::write(&path, "accession\nnot-valid\n").expect("write batch file");

        let result = parse_batch_file(&path);
        assert!(matches!(result, Err(InputError::EmptyBatch(_))));

        fs::remove_file(path).expect("remove batch file");
    }

    #[test]
    fn accepts_http_and_https_direct_urls() {
        let https = DirectUrl::parse("https://example.org/data.fastq.gz#fragment")
            .expect("valid HTTPS URL");
        assert_eq!(https.as_str(), "https://example.org/data.fastq.gz");

        let http = DirectUrl::parse("http://example.org/data.sra").expect("valid HTTP URL");
        assert_eq!(http.as_url().scheme(), "http");
    }

    #[test]
    fn rejects_unsupported_url_schemes_and_embedded_credentials() {
        assert!(matches!(
            DirectUrl::parse("file:///tmp/data.fastq"),
            Err(InputError::UnsupportedUrlScheme(_))
        ));
        assert!(matches!(
            DirectUrl::parse("https://user:secret@example.org/data.fastq"),
            Err(InputError::EmbeddedUrlCredentials)
        ));
    }
}
