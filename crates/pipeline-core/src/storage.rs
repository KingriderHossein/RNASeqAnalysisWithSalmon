use std::{
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoragePlan {
    pub estimated_download_bytes: u64,
    pub estimated_persistent_bytes: u64,
    pub recommended_peak_bytes: u64,
}

impl StoragePlan {
    pub fn new(
        estimated_download_bytes: u64,
        estimated_persistent_bytes: u64,
        recommended_peak_bytes: u64,
    ) -> Self {
        Self {
            estimated_download_bytes,
            estimated_persistent_bytes,
            recommended_peak_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageWarningKind {
    BelowDownloadEstimate,
    BelowPersistentEstimate,
    BelowRecommendedPeak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageWarning {
    pub kind: StorageWarningKind,
    pub required_bytes: u64,
    pub available_bytes: u64,
    pub missing_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageInspection {
    pub destination: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub plan: StoragePlan,
    pub warnings: Vec<StorageWarning>,
}

impl StorageInspection {
    pub fn can_continue(&self) -> bool {
        true
    }

    pub fn has_space_warning(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[derive(Debug)]
pub enum StorageError {
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    Metadata {
        path: PathBuf,
        source: std::io::Error,
    },
    NotDirectory(PathBuf),
    MissingDirectory(PathBuf),
    ProbeCreate {
        path: PathBuf,
        source: std::io::Error,
    },
    ProbeWrite {
        path: PathBuf,
        source: std::io::Error,
    },
    ProbeCleanup {
        path: PathBuf,
        source: std::io::Error,
    },
    SpaceQuery {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDirectory { path, source } => {
                write!(f, "cannot create destination {}: {source}", path.display())
            }
            Self::Metadata { path, source } => {
                write!(f, "cannot inspect destination {}: {source}", path.display())
            }
            Self::NotDirectory(path) => {
                write!(f, "destination is not a directory: {}", path.display())
            }
            Self::MissingDirectory(path) => {
                write!(f, "destination does not exist: {}", path.display())
            }
            Self::ProbeCreate { path, source } => {
                write!(f, "destination is not writable at {}: {source}", path.display())
            }
            Self::ProbeWrite { path, source } => {
                write!(f, "cannot write destination probe {}: {source}", path.display())
            }
            Self::ProbeCleanup { path, source } => {
                write!(f, "cannot remove destination probe {}: {source}", path.display())
            }
            Self::SpaceQuery { path, source } => {
                write!(f, "cannot query filesystem space for {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDirectory { source, .. }
            | Self::Metadata { source, .. }
            | Self::ProbeCreate { source, .. }
            | Self::ProbeWrite { source, .. }
            | Self::ProbeCleanup { source, .. }
            | Self::SpaceQuery { source, .. } => Some(source),
            Self::NotDirectory(_) | Self::MissingDirectory(_) => None,
        }
    }
}

pub struct StorageInspector;

impl StorageInspector {
    pub fn inspect(
        destination: impl AsRef<Path>,
        plan: StoragePlan,
        create_if_missing: bool,
    ) -> Result<StorageInspection, StorageError> {
        let destination = destination.as_ref().to_path_buf();

        if !destination.exists() {
            if create_if_missing {
                fs::create_dir_all(&destination).map_err(|source| StorageError::CreateDirectory {
                    path: destination.clone(),
                    source,
                })?;
            } else {
                return Err(StorageError::MissingDirectory(destination));
            }
        }

        let metadata = fs::metadata(&destination).map_err(|source| StorageError::Metadata {
            path: destination.clone(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(StorageError::NotDirectory(destination));
        }

        verify_writable(&destination)?;

        let total_bytes =
            fs4::total_space(&destination).map_err(|source| StorageError::SpaceQuery {
                path: destination.clone(),
                source,
            })?;
        let available_bytes =
            fs4::available_space(&destination).map_err(|source| StorageError::SpaceQuery {
                path: destination.clone(),
                source,
            })?;

        Ok(StorageInspection {
            destination,
            total_bytes,
            available_bytes,
            plan,
            warnings: build_warnings(plan, available_bytes),
        })
    }
}

fn build_warnings(plan: StoragePlan, available_bytes: u64) -> Vec<StorageWarning> {
    let mut warnings = Vec::new();

    push_warning(
        &mut warnings,
        StorageWarningKind::BelowDownloadEstimate,
        plan.estimated_download_bytes,
        available_bytes,
    );
    push_warning(
        &mut warnings,
        StorageWarningKind::BelowPersistentEstimate,
        plan.estimated_persistent_bytes,
        available_bytes,
    );
    push_warning(
        &mut warnings,
        StorageWarningKind::BelowRecommendedPeak,
        plan.recommended_peak_bytes,
        available_bytes,
    );

    warnings
}

fn push_warning(
    warnings: &mut Vec<StorageWarning>,
    kind: StorageWarningKind,
    required_bytes: u64,
    available_bytes: u64,
) {
    if available_bytes < required_bytes {
        warnings.push(StorageWarning {
            kind,
            required_bytes,
            available_bytes,
            missing_bytes: required_bytes.saturating_sub(available_bytes),
        });
    }
}

fn verify_writable(destination: &Path) -> Result<(), StorageError> {
    let probe_path = destination.join(probe_file_name());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
        .map_err(|source| StorageError::ProbeCreate {
            path: probe_path.clone(),
            source,
        })?;

    let write_result = file.write_all(b"rnaseq-pipeline-write-probe");
    drop(file);

    if let Err(source) = write_result {
        let _ = fs::remove_file(&probe_path);
        return Err(StorageError::ProbeWrite {
            path: probe_path,
            source,
        });
    }

    fs::remove_file(&probe_path).map_err(|source| StorageError::ProbeCleanup {
        path: probe_path,
        source,
    })
}

fn probe_file_name() -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(".rnaseq-write-probe-{}-{nonce}", process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        path::PathBuf,
    };

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rnaseq-storage-test-{label}-{}", probe_file_name()))
    }

    #[test]
    fn low_space_is_warning_not_error() {
        let root = temp_root("warning");
        fs::create_dir_all(&root).expect("create test directory");
        let plan = StoragePlan::new(u64::MAX, u64::MAX, u64::MAX);

        let inspection =
            StorageInspector::inspect(&root, plan, false).expect("inspection should succeed");

        assert!(inspection.can_continue());
        assert!(inspection.has_space_warning());
        assert_eq!(inspection.warnings.len(), 3);

        fs::remove_dir_all(root).expect("clean test directory");
    }

    #[test]
    fn creates_missing_nested_destination() {
        let root = temp_root("create");
        let destination = root.join("nested").join("destination");
        let plan = StoragePlan::new(0, 0, 0);

        let inspection =
            StorageInspector::inspect(&destination, plan, true).expect("create destination");

        assert_eq!(inspection.destination, destination);
        assert!(inspection.destination.is_dir());
        assert!(!inspection.has_space_warning());

        fs::remove_dir_all(root).expect("clean test directory");
    }

    #[test]
    fn rejects_file_as_destination() {
        let root = temp_root("file");
        fs::create_dir_all(&root).expect("create test directory");
        let destination = root.join("not-a-directory");
        File::create(&destination).expect("create test file");

        let error = StorageInspector::inspect(&destination, StoragePlan::new(0, 0, 0), false)
            .expect_err("file destination must fail");

        assert!(matches!(error, StorageError::NotDirectory(path) if path == destination));

        fs::remove_dir_all(root).expect("clean test directory");
    }

    #[test]
    fn missing_destination_can_remain_uncreated() {
        let root = temp_root("missing");

        let error = StorageInspector::inspect(&root, StoragePlan::new(0, 0, 0), false)
            .expect_err("missing destination must fail when creation is disabled");

        assert!(matches!(error, StorageError::MissingDirectory(path) if path == root));
        assert!(!root.exists());
    }
}
