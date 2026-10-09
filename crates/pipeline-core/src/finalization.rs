use crate::ownership::{publish_noreplace, OutputOwnership, OwnershipError};
use crate::{
    ArtifactId, ArtifactKind, ArtifactRecord, RunId, RunRecord, RunState, StateStore, StopToken,
    StoreError,
};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

const COMPRESSION_CHECKPOINT: &str = "gzip";
const CHECKSUM_CHECKPOINT: &str = "sha256";
const COMPLETE_CHECKPOINT: &str = "finalization-complete";
const BUFFER_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizationStage {
    Compression,
    Checksum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizationDisposition {
    Complete,
    PausedAtBoundary {
        stage: FinalizationStage,
    },
    Failed {
        stage: FinalizationStage,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizationResult {
    pub run: RunRecord,
    pub attempt: Option<i64>,
    pub disposition: FinalizationDisposition,
    pub compressed_paths: Vec<PathBuf>,
    pub checksum_manifest: Option<PathBuf>,
}

#[derive(Debug)]
pub enum FinalizationError {
    Store(StoreError),
    Ownership(OwnershipError),
    MissingRun(RunId),
    UnsupportedState(RunState),
    UnknownPausedCheckpoint(Option<String>),
    UnknownFailedCheckpoint(Option<String>),
    PersistedPaths(serde_json::Error),
    MissingFastqPaths,
    InvalidFastqPath(PathBuf),
    MixedFastqParents,
    InvalidCompressedOutput(PathBuf),
    RecoveryStopped,
    FinalCompressedDirectoryExists(PathBuf),
    ChecksumManifestExists(PathBuf),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    SizeOverflow {
        path: PathBuf,
        bytes: u64,
    },
    Identity(crate::IdError),
}

impl fmt::Display for FinalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(f),
            Self::Ownership(error) => error.fmt(f),
            Self::MissingRun(id) => write!(f, "run not found: {id}"),
            Self::UnsupportedState(state) => {
                write!(f, "cannot finalize FASTQ artifacts from state {state}")
            }
            Self::UnknownPausedCheckpoint(checkpoint) => write!(
                f,
                "cannot determine finalization resume stage from checkpoint {:?}",
                checkpoint
            ),
            Self::UnknownFailedCheckpoint(checkpoint) => write!(
                f,
                "cannot determine finalization retry stage from checkpoint {:?}",
                checkpoint
            ),
            Self::PersistedPaths(error) => {
                write!(f, "cannot decode persisted FASTQ paths: {error}")
            }
            Self::MissingFastqPaths => f.write_str("run has no finalized FASTQ paths to compress"),
            Self::InvalidFastqPath(path) => write!(
                f,
                "finalized FASTQ input is missing, empty, or not a regular file: {}",
                path.display()
            ),
            Self::MixedFastqParents => {
                f.write_str("all FASTQ inputs for one run must share the same parent directory")
            }
            Self::InvalidCompressedOutput(path) => write!(
                f,
                "compressed FASTQ does not match source or is invalid: {}",
                path.display()
            ),
            Self::RecoveryStopped => {
                f.write_str("compression recovery stopped before reconciliation")
            }
            Self::FinalCompressedDirectoryExists(path) => write!(
                f,
                "compressed output directory already exists and will not be overwritten: {}",
                path.display()
            ),
            Self::ChecksumManifestExists(path) => write!(
                f,
                "checksum manifest already exists and will not be overwritten: {}",
                path.display()
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation} {} failed: {source}", path.display()),
            Self::SizeOverflow { path, bytes } => write!(
                f,
                "artifact {} is too large to persist as i64 bytes: {bytes}",
                path.display()
            ),
            Self::Identity(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FinalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Ownership(error) => Some(error),
            Self::PersistedPaths(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Identity(error) => Some(error),
            Self::MissingRun(_)
            | Self::UnsupportedState(_)
            | Self::UnknownPausedCheckpoint(_)
            | Self::UnknownFailedCheckpoint(_)
            | Self::MissingFastqPaths
            | Self::InvalidFastqPath(_)
            | Self::MixedFastqParents
            | Self::InvalidCompressedOutput(_)
            | Self::RecoveryStopped
            | Self::FinalCompressedDirectoryExists(_)
            | Self::ChecksumManifestExists(_)
            | Self::SizeOverflow { .. } => None,
        }
    }
}

impl From<StoreError> for FinalizationError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

impl From<crate::IdError> for FinalizationError {
    fn from(value: crate::IdError) -> Self {
        Self::Identity(value)
    }
}

pub struct FastqFinalizationExecutor;

impl FastqFinalizationExecutor {
    pub fn execute_to_complete(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        stop: &StopToken,
    ) -> Result<FinalizationResult, FinalizationError> {
        self.execute_with_ownership(store, run_id, stop, None)
    }

    pub(crate) fn execute_with_ownership(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        stop: &StopToken,
        ownership: Option<&OutputOwnership>,
    ) -> Result<FinalizationResult, FinalizationError> {
        let current = store
            .get_run(run_id)?
            .ok_or_else(|| FinalizationError::MissingRun(run_id.clone()))?;

        if current.state == RunState::Complete {
            return self.completed_result(store, current);
        }

        let paths = decoded_fastq_paths(&current.fastq_paths)?;
        let parent = validate_fastq_inputs(&paths)?;
        // Conversion owns the FASTQ root; finalization must use the same marker.
        let root = parent.parent().unwrap_or(&parent);
        let _ownership = OutputOwnership::ensure_or_acquire(&[root], ownership)
            .map_err(FinalizationError::Ownership)?;

        match current.state {
            RunState::FastqReady => self.run_compression(store, current, stop, false),
            RunState::Compressing => self.recover_compression(store, current, stop),
            RunState::Checksumming => self.run_checksum_resume(store, current, stop, false),
            RunState::PausedAtBoundary => match current.last_checkpoint.as_deref() {
                Some(COMPRESSION_CHECKPOINT) => self.run_compression(store, current, stop, false),
                Some(CHECKSUM_CHECKPOINT) => self.run_checksum_resume(store, current, stop, false),
                _ => Err(FinalizationError::UnknownPausedCheckpoint(
                    current.last_checkpoint.clone(),
                )),
            },
            RunState::Failed => match current.last_checkpoint.as_deref() {
                Some(COMPRESSION_CHECKPOINT) => self.run_compression(store, current, stop, true),
                Some(CHECKSUM_CHECKPOINT) => self.run_checksum_resume(store, current, stop, true),
                _ => Err(FinalizationError::UnknownFailedCheckpoint(
                    current.last_checkpoint.clone(),
                )),
            },
            state => Err(FinalizationError::UnsupportedState(state)),
        }
    }

    fn run_compression(
        &self,
        store: &mut StateStore,
        current: RunRecord,
        stop: &StopToken,
        retry: bool,
    ) -> Result<FinalizationResult, FinalizationError> {
        let fastq_paths = decoded_fastq_paths(&current.fastq_paths)?;
        let parent = validate_fastq_inputs(&fastq_paths)?;
        let compressed_directory = parent.join("compressed");

        ensure_new_destination(&compressed_directory)?;

        let attempt = store.begin_run_attempt(&current.id)?;
        if retry {
            store.retry_run(
                &current.id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
            )?;
        } else {
            store.transition_run(
                &current.id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )?;
        }

        let staging_directory = parent.join(format!(".compressed-attempt-{attempt}"));
        if let Err(error) = create_new_directory(&staging_directory) {
            self.persist_failure(store, &current.id, COMPRESSION_CHECKPOINT, &error)?;
            return Err(error);
        }

        let mut staged = Vec::with_capacity(fastq_paths.len());
        for source in &fastq_paths {
            let file_name = source
                .file_name()
                .ok_or_else(|| FinalizationError::InvalidFastqPath(source.clone()))?;
            let mut target_name = file_name.to_os_string();
            target_name.push(".gz");
            let staged_target = staging_directory.join(target_name);

            match gzip_file(source, &staged_target, stop).and_then(|result| match result {
                StreamOutcome::Complete => verify_gzip_pair(source, &staged_target, stop),
                StreamOutcome::Stopped => Ok(StreamOutcome::Stopped),
            }) {
                Ok(StreamOutcome::Complete) => staged.push(staged_target),
                Ok(StreamOutcome::Stopped) => {
                    return self.pause_compression(store, &current.id, attempt, &staging_directory);
                }
                Err(error) => {
                    let reason = append_cleanup_warning(
                        error.to_string(),
                        remove_owned_directory(&staging_directory),
                    );
                    let run = store.transition_run(
                        &current.id,
                        RunState::Failed,
                        Some(COMPRESSION_CHECKPOINT),
                        Some(&reason),
                    )?;
                    return Ok(FinalizationResult {
                        run,
                        attempt: Some(attempt),
                        disposition: FinalizationDisposition::Failed {
                            stage: FinalizationStage::Compression,
                            reason,
                        },
                        compressed_paths: Vec::new(),
                        checksum_manifest: None,
                    });
                }
            }
        }

        if stop.is_stop_requested() {
            return self.pause_compression(store, &current.id, attempt, &staging_directory);
        }
        // Recheck immediately before publication: gzip work may take a long time.
        // Never knowingly replace a user-created directory, even an empty one.
        if let Err(error) = ensure_new_destination(&compressed_directory) {
            self.persist_failure(store, &current.id, COMPRESSION_CHECKPOINT, &error)?;
            return Err(error);
        }
        if let Err(source) = publish_noreplace(&staging_directory, &compressed_directory) {
            let error = FinalizationError::Io {
                operation: "finalize compressed FASTQ directory",
                path: compressed_directory.clone(),
                source,
            };
            self.persist_failure(store, &current.id, COMPRESSION_CHECKPOINT, &error)?;
            return Err(error);
        }

        let mut finalized = Vec::with_capacity(staged.len());
        let mut compressed_paths = Vec::with_capacity(staged.len());
        for (index, staged_path) in staged.iter().enumerate() {
            let file_name = staged_path
                .file_name()
                .ok_or_else(|| FinalizationError::InvalidFastqPath(staged_path.clone()))?;
            let final_path = compressed_directory.join(file_name);
            let bytes = metadata_len(&final_path, "inspect compressed FASTQ")?;
            let size = i64::try_from(bytes).map_err(|_| FinalizationError::SizeOverflow {
                path: final_path.clone(),
                bytes,
            })?;
            finalized.push((
                ArtifactId::new(format!("{}-gzip-{}", current.id.as_str(), index + 1))?,
                final_path.to_string_lossy().into_owned(),
                size,
            ));
            compressed_paths.push(final_path);
        }

        if let Err(error) =
            store.record_finalized_artifacts(&current.id, ArtifactKind::CompressedFastq, &finalized)
        {
            // Retain COMPRESSING and the published set for verified adoption on resume.
            // FAILED gzip retry starts new compression and must not replace this set.
            return Err(FinalizationError::Store(error));
        }
        store.transition_run(
            &current.id,
            RunState::Checksumming,
            Some(CHECKSUM_CHECKPOINT),
            None,
        )?;

        self.run_checksums(
            store,
            &current.id,
            attempt,
            compressed_paths,
            &compressed_directory,
            stop,
        )
    }

    fn pause_compression(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        attempt: i64,
        staging_directory: &Path,
    ) -> Result<FinalizationResult, FinalizationError> {
        let warning = cleanup_warning(remove_owned_directory(staging_directory));
        let run = store.transition_run(
            run_id,
            RunState::PausedAtBoundary,
            Some(COMPRESSION_CHECKPOINT),
            warning.as_deref(),
        )?;
        Ok(FinalizationResult {
            run,
            attempt: Some(attempt),
            disposition: FinalizationDisposition::PausedAtBoundary {
                stage: FinalizationStage::Compression,
            },
            compressed_paths: Vec::new(),
            checksum_manifest: None,
        })
    }

    fn recover_compression(
        &self,
        store: &mut StateStore,
        current: RunRecord,
        stop: &StopToken,
    ) -> Result<FinalizationResult, FinalizationError> {
        let fastq_paths = decoded_fastq_paths(&current.fastq_paths)?;
        let parent = validate_fastq_inputs(&fastq_paths)?;
        let compressed_directory = parent.join("compressed");

        let finalized_exists = match fs::symlink_metadata(&compressed_directory) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(source) => {
                return Err(FinalizationError::Io {
                    operation: "inspect compression recovery destination",
                    path: compressed_directory,
                    source,
                });
            }
        };
        if !finalized_exists {
            // Do not delete an unproven staging directory after a process crash.
            // The new attempt has a different persisted identity and filename.
            let paused = store.transition_run(
                &current.id,
                RunState::PausedAtBoundary,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )?;
            return self.run_compression(store, paused, stop, false);
        }

        let metadata = fs::symlink_metadata(&compressed_directory).map_err(|source| {
            FinalizationError::Io {
                operation: "inspect compressed recovery directory",
                path: compressed_directory.clone(),
                source,
            }
        })?;
        if !metadata.file_type().is_dir() {
            return Err(FinalizationError::InvalidCompressedOutput(
                compressed_directory,
            ));
        }

        // An atomic directory rename may complete before SQLite persistence.
        // Adopt only exact expected regular files and source-equivalent gzip streams.
        let mut finalized = Vec::with_capacity(fastq_paths.len());
        let mut compressed_paths = Vec::with_capacity(fastq_paths.len());
        for (index, source) in fastq_paths.iter().enumerate() {
            let mut target_name = source
                .file_name()
                .ok_or_else(|| FinalizationError::InvalidFastqPath(source.clone()))?
                .to_os_string();
            target_name.push(".gz");
            let target = compressed_directory.join(target_name);
            let target_metadata = fs::symlink_metadata(&target)
                .map_err(|_| FinalizationError::InvalidCompressedOutput(target.clone()))?;
            if !target_metadata.file_type().is_file() || target_metadata.len() == 0 {
                return Err(FinalizationError::InvalidCompressedOutput(target));
            }
            match verify_gzip_pair(source, &target, stop)? {
                StreamOutcome::Stopped => {
                    // Keep COMPRESSING: this published set still needs adoption.
                    // PAUSED_AT_BOUNDARY/gzip would wrongly start new compression
                    // against an already published destination on Resume.
                    return Ok(FinalizationResult {
                        run: current,
                        attempt: None,
                        disposition: FinalizationDisposition::PausedAtBoundary {
                            stage: FinalizationStage::Compression,
                        },
                        compressed_paths: Vec::new(),
                        checksum_manifest: None,
                    });
                }
                StreamOutcome::Complete => {}
            }
            let size_bytes = i64::try_from(target_metadata.len()).map_err(|_| {
                FinalizationError::SizeOverflow {
                    path: target.clone(),
                    bytes: target_metadata.len(),
                }
            })?;
            finalized.push((
                ArtifactId::new(format!("{}-gzip-{}", current.id.as_str(), index + 1))?,
                target.to_string_lossy().into_owned(),
                size_bytes,
            ));
            compressed_paths.push(target);
        }

        let entries =
            fs::read_dir(&compressed_directory).map_err(|source| FinalizationError::Io {
                operation: "list recovered gzip directory",
                path: compressed_directory.clone(),
                source,
            })?;
        let entry_count = entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| FinalizationError::Io {
                operation: "inspect entries in recovered gzip directory",
                path: compressed_directory.clone(),
                source,
            })?
            .len();
        if entry_count != compressed_paths.len() {
            return Err(FinalizationError::InvalidCompressedOutput(
                compressed_directory,
            ));
        }

        let existing = store.list_artifacts_by_kind(&current.id, ArtifactKind::CompressedFastq)?;
        if existing.is_empty() {
            store.record_finalized_artifacts(
                &current.id,
                ArtifactKind::CompressedFastq,
                &finalized,
            )?;
        } else if existing.len() != finalized.len()
            || !existing.iter().all(|artifact| {
                finalized.iter().any(|(id, path, size)| {
                    &artifact.id == id
                        && &artifact.path == path
                        && artifact.size_bytes == Some(*size)
                })
            })
        {
            return Err(FinalizationError::InvalidCompressedOutput(
                compressed_directory,
            ));
        }

        store.transition_run(
            &current.id,
            RunState::Checksumming,
            Some(CHECKSUM_CHECKPOINT),
            None,
        )?;
        self.run_checksums(
            store,
            &current.id,
            current.attempt_count,
            compressed_paths,
            &compressed_directory,
            stop,
        )
    }

    fn run_checksum_resume(
        &self,
        store: &mut StateStore,
        current: RunRecord,
        stop: &StopToken,
        retry: bool,
    ) -> Result<FinalizationResult, FinalizationError> {
        let artifacts = store.list_artifacts_by_kind(&current.id, ArtifactKind::CompressedFastq)?;
        let compressed_paths = compressed_paths_from_artifacts(&artifacts)?;
        let compressed_directory = common_parent(&compressed_paths)?;
        let attempt = store.begin_run_attempt(&current.id)?;

        if retry {
            store.retry_run(
                &current.id,
                RunState::Checksumming,
                Some(CHECKSUM_CHECKPOINT),
            )?;
        } else if current.state != RunState::Checksumming {
            store.transition_run(
                &current.id,
                RunState::Checksumming,
                Some(CHECKSUM_CHECKPOINT),
                None,
            )?;
        }

        self.run_checksums(
            store,
            &current.id,
            attempt,
            compressed_paths,
            &compressed_directory,
            stop,
        )
    }

    fn run_checksums(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        attempt: i64,
        compressed_paths: Vec<PathBuf>,
        compressed_directory: &Path,
        stop: &StopToken,
    ) -> Result<FinalizationResult, FinalizationError> {
        let result = self.run_checksums_inner(
            store,
            run_id,
            attempt,
            compressed_paths,
            compressed_directory,
            stop,
        );
        if let Err(error) = &result {
            if matches!(store.get_run(run_id), Ok(Some(run)) if run.state == RunState::Checksumming)
            {
                let _ = self.persist_failure(store, run_id, CHECKSUM_CHECKPOINT, error);
            }
        }
        result
    }

    fn run_checksums_inner(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        attempt: i64,
        compressed_paths: Vec<PathBuf>,
        compressed_directory: &Path,
        stop: &StopToken,
    ) -> Result<FinalizationResult, FinalizationError> {
        let manifest_path = compressed_directory.join("SHA256SUMS");
        let artifacts = store.list_artifacts_by_kind(run_id, ArtifactKind::CompressedFastq)?;
        if artifacts.len() != compressed_paths.len() || artifacts.is_empty() {
            return Err(FinalizationError::MissingFastqPaths);
        }

        let persisted_run = store
            .get_run(run_id)?
            .ok_or_else(|| FinalizationError::MissingRun(run_id.clone()))?;
        let originals = decoded_fastq_paths(&persisted_run.fastq_paths)?;
        let original_parent = validate_fastq_inputs(&originals)?;
        if originals.len() != artifacts.len()
            || original_parent.join("compressed") != compressed_directory
        {
            return Err(FinalizationError::InvalidCompressedOutput(
                compressed_directory.to_path_buf(),
            ));
        }

        let mut lines = String::new();
        let mut compressed_hashes = Vec::with_capacity(artifacts.len());

        for artifact in &artifacts {
            let path = PathBuf::from(&artifact.path);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| FinalizationError::InvalidCompressedOutput(path.clone()))?;
            if !metadata.file_type().is_file()
                || metadata.len() == 0
                || artifact.size_bytes != i64::try_from(metadata.len()).ok()
            {
                return Err(FinalizationError::InvalidCompressedOutput(path));
            }
            let original = originals
                .iter()
                .find(|source| {
                    let Some(name) = source.file_name() else {
                        return false;
                    };
                    let mut expected_name = name.to_os_string();
                    expected_name.push(".gz");
                    source
                        .parent()
                        .map(|parent| parent.join("compressed").join(expected_name) == path)
                        .unwrap_or(false)
                })
                .ok_or_else(|| FinalizationError::InvalidCompressedOutput(path.clone()))?;
            let is_paused = match verify_gzip_pair(original, &path, stop)? {
                StreamOutcome::Complete => false,
                StreamOutcome::Stopped => true,
            };
            if is_paused {
                return self.pause_checksum(store, run_id, attempt, compressed_paths);
            }
            match sha256_file(&path, stop)? {
                HashOutcome::Stopped => {
                    return self.pause_checksum(store, run_id, attempt, compressed_paths);
                }
                HashOutcome::Complete(hash) => {
                    let file_name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| FinalizationError::InvalidFastqPath(path.clone()))?;
                    lines.push_str(&hash);
                    lines.push_str("  ");
                    lines.push_str(file_name);
                    lines.push('\n');
                    compressed_hashes.push((artifact.id.clone(), hash));
                }
            }
        }

        if stop.is_stop_requested() {
            return self.pause_checksum(store, run_id, attempt, compressed_paths);
        }
        // Recovery: the checksum file may have been renamed before SQLite was committed.
        // Never replace it. Accept only exact bytes recomputed from current gzip inputs.
        let manifest_preexisted = match fs::symlink_metadata(&manifest_path) {
            Ok(metadata) if metadata.file_type().is_file() => true,
            Ok(_) => return Err(FinalizationError::InvalidCompressedOutput(manifest_path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(source) => {
                return Err(FinalizationError::Io {
                    operation: "inspect checksum manifest",
                    path: manifest_path,
                    source,
                });
            }
        };
        if manifest_preexisted {
            let bytes = fs::read(&manifest_path).map_err(|source| FinalizationError::Io {
                operation: "read existing checksum manifest",
                path: manifest_path.clone(),
                source,
            })?;
            if bytes != lines.as_bytes() {
                return Err(FinalizationError::InvalidCompressedOutput(manifest_path));
            }
        } else {
            let temp_manifest =
                compressed_directory.join(format!("SHA256SUMS.tmp-attempt-{attempt}"));
            write_atomic_candidate(&temp_manifest, lines.as_bytes())?;
            if stop.is_stop_requested() {
                let _ = fs::remove_file(&temp_manifest);
                return self.pause_checksum(store, run_id, attempt, compressed_paths);
            }
            if let Err(source) = publish_noreplace(&temp_manifest, &manifest_path) {
                let _ = fs::remove_file(&temp_manifest);
                return Err(FinalizationError::Io {
                    operation: "finalize checksum manifest",
                    path: manifest_path,
                    source,
                });
            }
        }

        let manifest_bytes = metadata_len(&manifest_path, "inspect checksum manifest")?;
        let manifest_size =
            i64::try_from(manifest_bytes).map_err(|_| FinalizationError::SizeOverflow {
                path: manifest_path.clone(),
                bytes: manifest_bytes,
            })?;
        let manifest_hash = sha256_bytes(lines.as_bytes());
        let checksum_id = ArtifactId::new(format!("{}-sha256sums", run_id.as_str()))?;

        let existing_checksum = store.list_artifacts_by_kind(run_id, ArtifactKind::Checksum)?;
        if existing_checksum.is_empty() {
            if let Err(error) = store.record_checksum_results(
                &compressed_hashes,
                &(
                    checksum_id,
                    run_id.clone(),
                    manifest_path.to_string_lossy().into_owned(),
                    manifest_size,
                    manifest_hash,
                ),
            ) {
                return Err(FinalizationError::Store(error));
            }
        } else {
            // A crash may have occurred after the SQLite transaction but before COMPLETE.
            let valid_checksum = existing_checksum.len() == 1
                && existing_checksum[0].path == manifest_path.to_string_lossy()
                && existing_checksum[0].size_bytes == Some(manifest_size)
                && existing_checksum[0].sha256.as_deref() == Some(manifest_hash.as_str())
                && existing_checksum[0].validation_state == crate::ArtifactValidationState::Valid;
            let valid_hashes = compressed_hashes.iter().all(|(id, hash)| {
                artifacts.iter().any(|artifact| {
                    &artifact.id == id
                        && artifact.sha256.as_deref() == Some(hash.as_str())
                        && artifact.validation_state == crate::ArtifactValidationState::Valid
                })
            });
            if !valid_checksum || !valid_hashes {
                return Err(FinalizationError::InvalidCompressedOutput(manifest_path));
            }
        }

        if stop.is_stop_requested() {
            return self.pause_checksum(store, run_id, attempt, compressed_paths);
        }
        let run =
            store.transition_run(run_id, RunState::Complete, Some(COMPLETE_CHECKPOINT), None)?;

        Ok(FinalizationResult {
            run,
            attempt: Some(attempt),
            disposition: FinalizationDisposition::Complete,
            compressed_paths,
            checksum_manifest: Some(manifest_path),
        })
    }

    fn pause_checksum(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        attempt: i64,
        compressed_paths: Vec<PathBuf>,
    ) -> Result<FinalizationResult, FinalizationError> {
        let run = store.transition_run(
            run_id,
            RunState::PausedAtBoundary,
            Some(CHECKSUM_CHECKPOINT),
            None,
        )?;
        Ok(FinalizationResult {
            run,
            attempt: Some(attempt),
            disposition: FinalizationDisposition::PausedAtBoundary {
                stage: FinalizationStage::Checksum,
            },
            compressed_paths,
            checksum_manifest: None,
        })
    }

    fn completed_result(
        &self,
        store: &StateStore,
        run: RunRecord,
    ) -> Result<FinalizationResult, FinalizationError> {
        let compressed = store.list_artifacts_by_kind(&run.id, ArtifactKind::CompressedFastq)?;
        let checksum = store.list_artifacts_by_kind(&run.id, ArtifactKind::Checksum)?;
        let compressed_paths = compressed
            .into_iter()
            .map(|artifact| PathBuf::from(artifact.path))
            .collect();
        let checksum_manifest = checksum
            .first()
            .map(|artifact| PathBuf::from(&artifact.path));

        Ok(FinalizationResult {
            run,
            attempt: None,
            disposition: FinalizationDisposition::Complete,
            compressed_paths,
            checksum_manifest,
        })
    }

    fn persist_failure(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        checkpoint: &'static str,
        error: &FinalizationError,
    ) -> Result<(), FinalizationError> {
        store.transition_run(
            run_id,
            RunState::Failed,
            Some(checkpoint),
            Some(&error.to_string()),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamOutcome {
    Complete,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HashOutcome {
    Complete(String),
    Stopped,
}

fn decoded_fastq_paths(value: &str) -> Result<Vec<PathBuf>, FinalizationError> {
    let paths: Vec<String> =
        serde_json::from_str(value).map_err(FinalizationError::PersistedPaths)?;
    if paths.is_empty() {
        return Err(FinalizationError::MissingFastqPaths);
    }
    Ok(paths.into_iter().map(PathBuf::from).collect())
}

fn validate_fastq_inputs(paths: &[PathBuf]) -> Result<PathBuf, FinalizationError> {
    if paths.is_empty() {
        return Err(FinalizationError::MissingFastqPaths);
    }
    let parent = common_parent(paths)?;
    for path in paths {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| FinalizationError::InvalidFastqPath(path.clone()))?;
        if !metadata.file_type().is_file() || metadata.len() == 0 {
            return Err(FinalizationError::InvalidFastqPath(path.clone()));
        }
    }
    Ok(parent)
}

fn common_parent(paths: &[PathBuf]) -> Result<PathBuf, FinalizationError> {
    let first = paths.first().ok_or(FinalizationError::MissingFastqPaths)?;
    let parent = first
        .parent()
        .ok_or_else(|| FinalizationError::InvalidFastqPath(first.clone()))?
        .to_path_buf();
    if paths
        .iter()
        .any(|path| path.parent().map(Path::to_path_buf) != Some(parent.clone()))
    {
        return Err(FinalizationError::MixedFastqParents);
    }
    Ok(parent)
}

fn ensure_new_destination(path: &Path) -> Result<(), FinalizationError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(FinalizationError::FinalCompressedDirectoryExists(
            path.to_path_buf(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(FinalizationError::Io {
            operation: "inspect compressed output destination",
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn create_new_directory(path: &Path) -> Result<(), FinalizationError> {
    fs::create_dir(path).map_err(|source| FinalizationError::Io {
        operation: "create compression staging directory",
        path: path.to_path_buf(),
        source,
    })
}

fn gzip_file(
    source: &Path,
    target: &Path,
    stop: &StopToken,
) -> Result<StreamOutcome, FinalizationError> {
    let input = fs::File::open(source).map_err(|source_error| FinalizationError::Io {
        operation: "open FASTQ for compression",
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|source_error| FinalizationError::Io {
            operation: "create staged gzip",
            path: target.to_path_buf(),
            source: source_error,
        })?;
    let mut reader = BufReader::new(input);
    let mut encoder = GzEncoder::new(output, Compression::default());
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        if stop.is_stop_requested() {
            return Ok(StreamOutcome::Stopped);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|source_error| FinalizationError::Io {
                operation: "read FASTQ during compression",
                path: source.to_path_buf(),
                source: source_error,
            })?;
        if read == 0 {
            break;
        }
        encoder
            .write_all(&buffer[..read])
            .map_err(|source_error| FinalizationError::Io {
                operation: "write staged gzip",
                path: target.to_path_buf(),
                source: source_error,
            })?;
    }

    let output = encoder
        .finish()
        .map_err(|source_error| FinalizationError::Io {
            operation: "finish staged gzip",
            path: target.to_path_buf(),
            source: source_error,
        })?;
    output
        .sync_all()
        .map_err(|source_error| FinalizationError::Io {
            operation: "sync staged gzip",
            path: target.to_path_buf(),
            source: source_error,
        })?;
    Ok(StreamOutcome::Complete)
}

fn verify_gzip_pair(
    source: &Path,
    compressed: &Path,
    stop: &StopToken,
) -> Result<StreamOutcome, FinalizationError> {
    let original = fs::File::open(source).map_err(|source_error| FinalizationError::Io {
        operation: "open source FASTQ for gzip verification",
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let gzip = fs::File::open(compressed).map_err(|source_error| FinalizationError::Io {
        operation: "open gzip for verification",
        path: compressed.to_path_buf(),
        source: source_error,
    })?;
    let mut original = BufReader::new(original);
    let mut decoded = GzDecoder::new(BufReader::new(gzip));
    let mut original_buffer = vec![0_u8; BUFFER_SIZE];
    let mut decoded_buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        if stop.is_stop_requested() {
            return Ok(StreamOutcome::Stopped);
        }
        let count = original
            .read(&mut original_buffer)
            .map_err(|source_error| FinalizationError::Io {
                operation: "read source FASTQ for gzip verification",
                path: source.to_path_buf(),
                source: source_error,
            })?;
        if count == 0 {
            let mut excess = [0_u8; 1];
            let extra =
                decoded
                    .read(&mut excess)
                    .map_err(|source_error| FinalizationError::Io {
                        operation: "finish gzip verification",
                        path: compressed.to_path_buf(),
                        source: source_error,
                    })?;
            if extra != 0 {
                return Err(FinalizationError::InvalidCompressedOutput(
                    compressed.to_path_buf(),
                ));
            }
            return Ok(StreamOutcome::Complete);
        }
        decoded
            .read_exact(&mut decoded_buffer[..count])
            .map_err(|source_error| FinalizationError::Io {
                operation: "decode gzip for verification",
                path: compressed.to_path_buf(),
                source: source_error,
            })?;
        if original_buffer[..count] != decoded_buffer[..count] {
            return Err(FinalizationError::InvalidCompressedOutput(
                compressed.to_path_buf(),
            ));
        }
    }
}

fn sha256_file(path: &Path, stop: &StopToken) -> Result<HashOutcome, FinalizationError> {
    let input = fs::File::open(path).map_err(|source| FinalizationError::Io {
        operation: "open compressed FASTQ for hashing",
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(input);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        if stop.is_stop_requested() {
            return Ok(HashOutcome::Stopped);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|source| FinalizationError::Io {
                operation: "read compressed FASTQ for hashing",
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(HashOutcome::Complete(bytes_to_hex(&hasher.finalize())))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    bytes_to_hex(&Sha256::digest(bytes))
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn write_atomic_candidate(path: &Path, bytes: &[u8]) -> Result<(), FinalizationError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| FinalizationError::Io {
            operation: "create checksum manifest candidate",
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes)
        .map_err(|source| FinalizationError::Io {
            operation: "write checksum manifest candidate",
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_all().map_err(|source| FinalizationError::Io {
        operation: "sync checksum manifest candidate",
        path: path.to_path_buf(),
        source,
    })
}

fn metadata_len(path: &Path, operation: &'static str) -> Result<u64, FinalizationError> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|source| FinalizationError::Io {
            operation,
            path: path.to_path_buf(),
            source,
        })
}

fn compressed_paths_from_artifacts(
    artifacts: &[ArtifactRecord],
) -> Result<Vec<PathBuf>, FinalizationError> {
    if artifacts.is_empty() {
        return Err(FinalizationError::MissingFastqPaths);
    }
    let paths = artifacts
        .iter()
        .map(|artifact| PathBuf::from(&artifact.path))
        .collect::<Vec<_>>();
    for path in &paths {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| FinalizationError::InvalidFastqPath(path.clone()))?;
        if !metadata.file_type().is_file() || metadata.len() == 0 {
            return Err(FinalizationError::InvalidFastqPath(path.clone()));
        }
    }
    Ok(paths)
}

fn remove_owned_directory(path: &Path) -> Result<(), FinalizationError> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_dir_all(path).map_err(|source| FinalizationError::Io {
        operation: "remove engine-owned compression staging",
        path: path.to_path_buf(),
        source,
    })
}

fn cleanup_warning(result: Result<(), FinalizationError>) -> Option<String> {
    result
        .err()
        .map(|error| format!("compression staging cleanup needs attention: {}", error))
}

fn append_cleanup_warning(reason: String, cleanup: Result<(), FinalizationError>) -> String {
    match cleanup_warning(cleanup) {
        Some(warning) => format!("{reason}; {warning}"),
        None => reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArtifactValidationState, JobId, NewJob, NewRun};
    use flate2::read::GzDecoder;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rnaseq-finalization-test-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn fastq_ready_store(root: &Path, files: &[(&str, &[u8])]) -> (StateStore, RunId, PathBuf) {
        fs::create_dir_all(root).expect("test fixture root");
        let mut store = StateStore::open(root.join("state.sqlite")).expect("persistent store");
        let job_id = JobId::new("job-1").expect("job id");
        let run_id = RunId::new("run-1").expect("run id");
        let sra_path = root.join("sra").join("SRR000001");
        let fastq_dir = root.join("fastq").join("SRR000001");
        fs::create_dir_all(&sra_path).expect("SRA");
        fs::create_dir_all(&fastq_dir).expect("FASTQ dir");
        fs::write(sra_path.join("data.sra"), b"validated-sra").expect("SRA data");

        store
            .create_job(NewJob {
                id: job_id.clone(),
                input_type: "single_accession".to_owned(),
                input_identity: "SRR000001".to_owned(),
                output_root: root.to_string_lossy().into_owned(),
                settings_snapshot: "{}".to_owned(),
                tool_versions_snapshot: "{}".to_owned(),
            })
            .expect("job");
        store
            .create_run(NewRun {
                id: run_id.clone(),
                job_id,
                accession_or_source: "SRR000001".to_owned(),
            })
            .expect("run");
        store
            .transition_run(&run_id, RunState::Resolving, Some("resolve"), None)
            .expect("resolving");
        store
            .transition_run(&run_id, RunState::Ready, Some("resolve-complete"), None)
            .expect("ready");
        store
            .transition_run(&run_id, RunState::Downloading, Some("prefetch"), None)
            .expect("downloading");
        store
            .update_run_download_snapshot(&run_id, 13, sra_path.to_string_lossy().as_ref())
            .expect("snapshot");
        store
            .transition_run(
                &run_id,
                RunState::Downloaded,
                Some("prefetch-complete"),
                None,
            )
            .expect("downloaded");
        store
            .transition_run(&run_id, RunState::Validating, Some("vdb-validate"), None)
            .expect("validating");
        store
            .transition_run(
                &run_id,
                RunState::SraValid,
                Some("vdb-validate-complete"),
                None,
            )
            .expect("valid");
        store
            .transition_run(&run_id, RunState::Converting, Some("fasterq-dump"), None)
            .expect("converting");

        let mut artifacts = Vec::new();
        for (index, (name, bytes)) in files.iter().enumerate() {
            let path = fastq_dir.join(name);
            fs::write(&path, bytes).expect("FASTQ");
            artifacts.push((
                ArtifactId::new(format!("run-1-fastq-{}", index + 1)).expect("artifact"),
                path.to_string_lossy().into_owned(),
                i64::try_from(bytes.len()).expect("size"),
            ));
        }
        store
            .record_finalized_fastq_artifacts(&run_id, &artifacts)
            .expect("FASTQ artifacts");
        store
            .transition_run(
                &run_id,
                RunState::FastqReady,
                Some("fasterq-complete"),
                None,
            )
            .expect("FASTQ ready");

        (store, run_id, sra_path)
    }

    #[test]
    fn compresses_and_checksums_single_fastq_to_complete() {
        let root = temp_root("single");
        let original = b"@r\nACGT\n+\nIIII\n";
        let (mut store, run_id, sra_path) =
            fastq_ready_store(&root, &[("SRR000001.fastq", original)]);
        let executor = FastqFinalizationExecutor;

        let result = executor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect("finalize");

        assert_eq!(result.run.state, RunState::Complete);
        assert_eq!(result.disposition, FinalizationDisposition::Complete);
        assert_eq!(result.compressed_paths.len(), 1);
        assert!(sra_path.is_dir());
        assert!(root
            .join("fastq")
            .join("SRR000001")
            .join("SRR000001.fastq")
            .is_file());

        let mut decoder =
            GzDecoder::new(fs::File::open(&result.compressed_paths[0]).expect("gzip"));
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).expect("decode");
        assert_eq!(decoded, original);

        let manifest = result.checksum_manifest.expect("manifest");
        let manifest_text = fs::read_to_string(&manifest).expect("manifest text");
        assert!(manifest_text.contains("SRR000001.fastq.gz"));

        let compressed = store
            .list_artifacts_by_kind(&run_id, ArtifactKind::CompressedFastq)
            .expect("artifacts");
        assert_eq!(compressed.len(), 1);
        assert_eq!(
            compressed[0].validation_state,
            ArtifactValidationState::Valid
        );
        let persisted_hash = compressed[0].sha256.as_deref().expect("compressed SHA-256");
        assert_eq!(persisted_hash.len(), 64);

        let manifest_line = manifest_text.lines().next().expect("manifest line");
        let (manifest_hash, manifest_name) = manifest_line
            .split_once("  ")
            .expect("standard SHA256SUMS line");
        assert_eq!(manifest_hash, persisted_hash);
        assert_eq!(manifest_name, "SRR000001.fastq.gz");

        let actual_hash = match sha256_file(&result.compressed_paths[0], &StopToken::default())
            .expect("rehash compressed FASTQ")
        {
            HashOutcome::Complete(hash) => hash,
            HashOutcome::Stopped => panic!("unexpected stop"),
        };
        assert_eq!(actual_hash, persisted_hash);

        let checksum_artifacts = store
            .list_artifacts_by_kind(&run_id, ArtifactKind::Checksum)
            .expect("checksum");
        assert_eq!(checksum_artifacts.len(), 1);
        assert_eq!(
            checksum_artifacts[0].validation_state,
            ArtifactValidationState::Valid
        );
        assert_eq!(checksum_artifacts[0].path, manifest.to_string_lossy());
        assert_eq!(
            checksum_artifacts[0].sha256.as_ref().map(String::len),
            Some(64)
        );

        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn multi_fastq_outputs_all_round_trip() {
        let root = temp_root("multi");
        let one = b"@r/1\nAC\n+\nII\n";
        let two = b"@r/2\nGT\n+\nII\n";
        let (mut store, run_id, _) = fastq_ready_store(
            &root,
            &[("SRR000001_1.fastq", one), ("SRR000001_2.fastq", two)],
        );
        let executor = FastqFinalizationExecutor;

        let result = executor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect("finalize");

        assert_eq!(result.compressed_paths.len(), 2);
        for (path, expected) in result
            .compressed_paths
            .iter()
            .zip([one.as_slice(), two.as_slice()])
        {
            let mut decoder = GzDecoder::new(fs::File::open(path).expect("gzip"));
            let mut decoded = Vec::new();
            decoder.read_to_end(&mut decoded).expect("decode");
            assert_eq!(decoded, expected);
        }

        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn stop_before_compression_pauses_and_preserves_inputs() {
        let root = temp_root("compression-stop");
        let original = b"@r\nACGT\n+\nIIII\n";
        let (mut store, run_id, sra_path) =
            fastq_ready_store(&root, &[("SRR000001.fastq", original)]);
        let stop = StopToken::default();
        stop.request_stop();
        let executor = FastqFinalizationExecutor;

        let result = executor
            .execute_to_complete(&mut store, &run_id, &stop)
            .expect("pause");

        assert_eq!(result.run.state, RunState::PausedAtBoundary);
        assert_eq!(
            result.disposition,
            FinalizationDisposition::PausedAtBoundary {
                stage: FinalizationStage::Compression
            }
        );
        assert!(sra_path.is_dir());
        assert!(root
            .join("fastq")
            .join("SRR000001")
            .join("SRR000001.fastq")
            .is_file());
        assert!(!root
            .join("fastq")
            .join("SRR000001")
            .join(".compressed-attempt-1")
            .exists());

        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn stream_probes_stop_after_work_has_started() {
        let root = temp_root("stream-probes");
        fs::create_dir_all(&root).unwrap();
        let original = root.join("source.fastq");
        let compressed = root.join("source.fastq.gz");
        fs::write(&original, vec![b'A'; BUFFER_SIZE * 4]).unwrap();
        let after_one_buffer = || {
            let calls = std::sync::atomic::AtomicUsize::new(0);
            StopToken::with_probe(move || {
                Ok(calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0)
            })
        };
        assert!(matches!(
            gzip_file(&original, &compressed, &after_one_buffer()).unwrap(),
            StreamOutcome::Stopped
        ));
        assert!(fs::metadata(&compressed).unwrap().len() > 0);
        fs::remove_file(&compressed).unwrap();
        gzip_file(&original, &compressed, &StopToken::default()).unwrap();
        assert!(matches!(
            verify_gzip_pair(&original, &compressed, &after_one_buffer()).unwrap(),
            StreamOutcome::Stopped
        ));
        assert!(matches!(
            sha256_file(&original, &after_one_buffer()).unwrap(),
            HashOutcome::Stopped
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stopped_published_compression_recovery_preserves_adoption_checkpoint() {
        let root = temp_root("recovery-stop");
        let original = b"@r\nACGT\n+\nIIII\n";
        let (mut store, run_id, _) = fastq_ready_store(&root, &[("SRR000001.fastq", original)]);
        let parent = root.join("fastq/SRR000001");
        let directory = parent.join("compressed");
        fs::create_dir(&directory).unwrap();
        let compressed = directory.join("SRR000001.fastq.gz");
        gzip_file(
            &parent.join("SRR000001.fastq"),
            &compressed,
            &StopToken::default(),
        )
        .unwrap();
        let published = fs::read(&compressed).unwrap();
        let before = store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .unwrap();
        let stop = StopToken::with_probe(|| Ok(true));
        let paused = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &stop)
            .unwrap();
        assert_eq!(paused.run, before);
        assert!(matches!(
            paused.disposition,
            FinalizationDisposition::PausedAtBoundary { .. }
        ));
        assert_eq!(fs::read(&compressed).unwrap(), published);
        assert!(!directory.join("SHA256SUMS").exists());
        assert!(store
            .list_artifacts_by_kind(&run_id, ArtifactKind::CompressedFastq)
            .unwrap()
            .is_empty());
        let resumed = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .unwrap();
        assert_eq!(resumed.run.state, RunState::Complete);
        assert_eq!(fs::read(&compressed).unwrap(), published);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn control_error_before_manifest_publication_leaves_recoverable_output() {
        let root = temp_root("manifest-control-error");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAC\n+\nII\n")]);
        let directory = root.join("fastq/SRR000001/compressed");
        let observed = directory.clone();
        let stop = StopToken::with_probe(move || {
            if observed.is_dir()
                && fs::read_dir(&observed).unwrap().any(|entry| {
                    entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with("SHA256SUMS.tmp-")
                })
            {
                Err("synthetic control read failure at publication".into())
            } else {
                Ok(false)
            }
        });
        let paused = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &stop)
            .unwrap();
        assert_eq!(paused.run.state, RunState::PausedAtBoundary);
        assert_eq!(
            paused.run.last_checkpoint.as_deref(),
            Some(CHECKSUM_CHECKPOINT)
        );
        assert!(stop.probe_error().unwrap().contains("at publication"));
        assert!(!directory.join("SHA256SUMS").exists());
        let compressed = directory.join("SRR000001.fastq.gz");
        let bytes = fs::read(&compressed).unwrap();
        let resumed = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .unwrap();
        assert_eq!(resumed.run.state, RunState::Complete);
        assert_eq!(fs::read(compressed).unwrap(), bytes);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pause_after_manifest_publication_resumes_without_replacing_valid_bytes() {
        let root = temp_root("published-manifest-stop");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAC\n+\nII\n")]);
        let manifest = root.join("fastq/SRR000001/compressed/SHA256SUMS");
        let observed = manifest.clone();
        let stop = StopToken::with_probe(move || Ok(observed.is_file()));
        let paused = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &stop)
            .unwrap();
        assert_eq!(paused.run.state, RunState::PausedAtBoundary);
        assert_eq!(
            paused.run.last_checkpoint.as_deref(),
            Some(CHECKSUM_CHECKPOINT)
        );
        let bytes = fs::read(&manifest).unwrap();
        assert!(!bytes.is_empty());
        let checksum = store
            .list_artifacts_by_kind(&run_id, ArtifactKind::Checksum)
            .unwrap();
        assert_eq!(checksum.len(), 1);
        let resumed = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .unwrap();
        assert_eq!(resumed.run.state, RunState::Complete);
        assert_eq!(fs::read(manifest).unwrap(), bytes);
        assert_eq!(
            store
                .list_artifacts_by_kind(&run_id, ArtifactKind::Checksum)
                .unwrap(),
            checksum
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compression_pause_retries_with_new_attempt() {
        let root = temp_root("compression-retry");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAC\n+\nII\n")]);
        let stop = StopToken::default();
        stop.request_stop();
        let executor = FastqFinalizationExecutor;

        let first = executor
            .execute_to_complete(&mut store, &run_id, &stop)
            .expect("pause");
        assert_eq!(first.attempt, Some(1));

        let second = executor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect("resume");
        assert_eq!(second.run.state, RunState::Complete);
        assert_eq!(second.attempt, Some(2));

        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn checksum_pause_resumes_without_recompression() {
        let root = temp_root("checksum-resume");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAC\n+\nII\n")]);
        let executor = FastqFinalizationExecutor;

        let fastq_paths = decoded_fastq_paths(
            &store
                .get_run(&run_id)
                .expect("run")
                .expect("exists")
                .fastq_paths,
        )
        .expect("paths");
        let parent = validate_fastq_inputs(&fastq_paths).expect("parent");
        let compressed_dir = parent.join("compressed");
        fs::create_dir(&compressed_dir).expect("compressed dir");
        let compressed_path = compressed_dir.join("SRR000001.fastq.gz");
        gzip_file(&fastq_paths[0], &compressed_path, &StopToken::default()).expect("gzip");
        let bytes = metadata_len(&compressed_path, "compressed size").expect("size");
        store
            .record_finalized_artifacts(
                &run_id,
                ArtifactKind::CompressedFastq,
                &[(
                    ArtifactId::new("run-1-gzip-1").expect("id"),
                    compressed_path.to_string_lossy().into_owned(),
                    i64::try_from(bytes).expect("size"),
                )],
            )
            .expect("compressed artifact");
        store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .expect("compressing");
        store
            .transition_run(
                &run_id,
                RunState::Checksumming,
                Some(CHECKSUM_CHECKPOINT),
                None,
            )
            .expect("checksumming");

        let stop = StopToken::default();
        stop.request_stop();
        let paused = executor
            .run_checksums(
                &mut store,
                &run_id,
                1,
                vec![compressed_path.clone()],
                &compressed_dir,
                &stop,
            )
            .expect("checksum pause");
        assert_eq!(paused.run.state, RunState::PausedAtBoundary);

        let resumed = executor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect("checksum resume");
        assert_eq!(resumed.run.state, RunState::Complete);
        assert_eq!(resumed.attempt, Some(1));
        assert!(compressed_path.is_file());

        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn staged_gzip_verification_rejects_corruption_and_mismatched_source() {
        let root = temp_root("gzip-integrity");
        fs::create_dir_all(&root).expect("root");
        let source = root.join("sample.fastq");
        let compressed = root.join("sample.fastq.gz");
        fs::write(&source, b"@r\nAAAA\n+\nIIII\n").expect("source");
        gzip_file(&source, &compressed, &StopToken::default()).expect("gzip");
        assert_eq!(
            verify_gzip_pair(&source, &compressed, &StopToken::default()).expect("verify"),
            StreamOutcome::Complete
        );

        // The gzip is valid but no longer represents the source FASTQ.
        fs::write(&source, b"@r\nCCCC\n+\nIIII\n").expect("mutated source");
        assert!(matches!(
            verify_gzip_pair(&source, &compressed, &StopToken::default()),
            Err(FinalizationError::InvalidCompressedOutput(_))
        ));
        fs::write(&compressed, b"not-a-gzip").expect("corrupt gzip");
        assert!(verify_gzip_pair(&source, &compressed, &StopToken::default()).is_err());
        assert_eq!(
            fs::read(&source).expect("source preserved"),
            b"@r\nCCCC\n+\nIIII\n"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn recovery_restarts_abandoned_compression_without_deleting_partials() {
        let root = temp_root("compression-crash-staging");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAC\n+\nII\n")]);
        let old_attempt = store.begin_run_attempt(&run_id).expect("old attempt");
        store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .expect("compressing");
        let old_staging = root
            .join("fastq")
            .join("SRR000001")
            .join(format!(".compressed-attempt-{old_attempt}"));
        fs::create_dir(&old_staging).expect("old staging");
        fs::write(old_staging.join("keep.txt"), b"preserve-unproven-partial").expect("old data");

        let result = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect("resume");
        assert_eq!(result.run.state, RunState::Complete);
        assert_eq!(result.attempt, Some(old_attempt + 1));
        assert_eq!(
            fs::read(old_staging.join("keep.txt")).expect("untouched"),
            b"preserve-unproven-partial"
        );
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn recovery_adopts_valid_gzip_renamed_before_sqlite_commit() {
        let root = temp_root("compression-crash-renamed");
        let source_bytes = b"@r\nACGT\n+\nIIII\n";
        let (mut store, run_id, _) = fastq_ready_store(&root, &[("SRR000001.fastq", source_bytes)]);
        let source = root.join("fastq").join("SRR000001").join("SRR000001.fastq");
        let compressed_dir = source.parent().expect("parent").join("compressed");
        fs::create_dir(&compressed_dir).expect("final dir");
        gzip_file(
            &source,
            &compressed_dir.join("SRR000001.fastq.gz"),
            &StopToken::default(),
        )
        .expect("gzip");
        store.begin_run_attempt(&run_id).expect("attempt");
        store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .expect("compressing");

        // Recreate the actual application restart boundary using a new SQLite connection.
        drop(store);
        let mut store =
            StateStore::open(root.join("state.sqlite")).expect("reopen persisted state");
        assert_eq!(
            store.get_run(&run_id).expect("run").expect("found").state,
            RunState::Compressing
        );
        let result = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect("recovered");
        assert_eq!(result.run.state, RunState::Complete);
        assert_eq!(result.attempt, Some(1));
        assert_eq!(
            store
                .list_artifacts_by_kind(&run_id, ArtifactKind::CompressedFastq)
                .expect("compressed artifacts")
                .len(),
            1
        );
        assert_eq!(fs::read(&source).expect("source"), source_bytes);
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn recovery_rejects_corrupt_gzip_without_deleting_anything() {
        let root = temp_root("compression-corruption");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAC\n+\nII\n")]);
        let dir = root.join("fastq").join("SRR000001");
        fs::create_dir(dir.join("compressed")).expect("final dir");
        let invalid = dir.join("compressed").join("SRR000001.fastq.gz");
        fs::write(&invalid, b"not-a-gzip-stream").expect("bad gzip");
        store.begin_run_attempt(&run_id).expect("attempt");
        store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .expect("compressing");

        assert!(FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .is_err());
        assert_eq!(
            store.get_run(&run_id).expect("run").expect("exists").state,
            RunState::Compressing
        );
        assert!(invalid.exists());
        assert!(dir.join("SRR000001.fastq").exists());
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn recovery_adopts_manifest_renamed_before_sqlite_commit() {
        let root = temp_root("checksum-crash-renamed");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nGT\n+\nII\n")]);
        let source = root.join("fastq").join("SRR000001").join("SRR000001.fastq");
        let compressed_dir = source.parent().expect("parent").join("compressed");
        fs::create_dir(&compressed_dir).expect("compressed");
        let compressed = compressed_dir.join("SRR000001.fastq.gz");
        gzip_file(&source, &compressed, &StopToken::default()).expect("gzip");
        let bytes = metadata_len(&compressed, "compressed bytes").expect("size");
        store
            .record_finalized_artifacts(
                &run_id,
                ArtifactKind::CompressedFastq,
                &[(
                    ArtifactId::new("run-1-gzip-1").expect("id"),
                    compressed.to_string_lossy().into_owned(),
                    i64::try_from(bytes).expect("size"),
                )],
            )
            .expect("artifact");
        store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .expect("compressing");
        store
            .transition_run(
                &run_id,
                RunState::Checksumming,
                Some(CHECKSUM_CHECKPOINT),
                None,
            )
            .expect("checksumming");
        let hash = match sha256_file(&compressed, &StopToken::default()).expect("hash") {
            HashOutcome::Complete(hash) => hash,
            HashOutcome::Stopped => panic!("no stop"),
        };
        fs::write(
            compressed_dir.join("SHA256SUMS"),
            format!("{hash}  SRR000001.fastq.gz\n"),
        )
        .expect("orphan manifest");

        let result = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect("recovered manifest");
        assert_eq!(result.run.state, RunState::Complete);
        assert_eq!(
            store
                .list_artifacts_by_kind(&run_id, ArtifactKind::Checksum)
                .expect("checksum artifacts")
                .len(),
            1
        );
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn checksum_recovery_refuses_mismatching_manifest_without_deleting_it() {
        let root = temp_root("checksum-corruption");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAA\n+\nII\n")]);
        let source = root.join("fastq").join("SRR000001").join("SRR000001.fastq");
        let compressed_dir = source.parent().expect("parent").join("compressed");
        fs::create_dir(&compressed_dir).expect("compressed");
        let compressed = compressed_dir.join("SRR000001.fastq.gz");
        gzip_file(&source, &compressed, &StopToken::default()).expect("gzip");
        let bytes = metadata_len(&compressed, "bytes").expect("size");
        store
            .record_finalized_artifacts(
                &run_id,
                ArtifactKind::CompressedFastq,
                &[(
                    ArtifactId::new("run-1-gzip-1").expect("id"),
                    compressed.to_string_lossy().into_owned(),
                    i64::try_from(bytes).expect("size"),
                )],
            )
            .expect("artifact");
        store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .expect("compressing");
        store
            .transition_run(
                &run_id,
                RunState::Checksumming,
                Some(CHECKSUM_CHECKPOINT),
                None,
            )
            .expect("checksumming");
        let manifest = compressed_dir.join("SHA256SUMS");
        fs::write(&manifest, b"invalid checksum\n").expect("manifest");

        assert!(FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .is_err());
        assert_eq!(
            store.get_run(&run_id).expect("run").expect("exists").state,
            RunState::Failed
        );
        assert_eq!(
            fs::read(&manifest).expect("manifest"),
            b"invalid checksum\n"
        );
        assert!(source.exists());
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn checksum_recovery_rejects_modified_gzip_before_manifest_creation() {
        let root = temp_root("checksum-recompressed-different-data");
        let original = b"@r\nAAAA\n+\nIIII\n";
        let (mut store, run_id, _) = fastq_ready_store(&root, &[("SRR000001.fastq", original)]);
        let source = root.join("fastq").join("SRR000001").join("SRR000001.fastq");
        let compressed_dir = source.parent().expect("parent").join("compressed");
        fs::create_dir(&compressed_dir).expect("compressed");
        let compressed = compressed_dir.join("SRR000001.fastq.gz");
        gzip_file(&source, &compressed, &StopToken::default()).expect("gzip original");
        let original_compressed_size = fs::metadata(&compressed).expect("gzip metadata").len();
        store
            .record_finalized_artifacts(
                &run_id,
                ArtifactKind::CompressedFastq,
                &[(
                    ArtifactId::new("run-1-gzip-1").expect("id"),
                    compressed.to_string_lossy().into_owned(),
                    i64::try_from(original_compressed_size).expect("size"),
                )],
            )
            .expect("artifact");
        store
            .transition_run(
                &run_id,
                RunState::Compressing,
                Some(COMPRESSION_CHECKPOINT),
                None,
            )
            .expect("compressing");
        store
            .transition_run(
                &run_id,
                RunState::Checksumming,
                Some(CHECKSUM_CHECKPOINT),
                None,
            )
            .expect("checksumming");

        let altered_source = source.parent().expect("dir").join("altered.fastq");
        fs::write(&altered_source, b"@r\nCCCC\n+\nIIII\n").expect("altered");
        let altered_gzip = compressed_dir.join("altered.fastq.gz");
        gzip_file(&altered_source, &altered_gzip, &StopToken::default()).expect("altered gzip");
        assert_eq!(
            fs::metadata(&altered_gzip)
                .expect("alternate metadata")
                .len(),
            original_compressed_size
        );
        fs::rename(&altered_gzip, &compressed).expect("replace test gzip");

        let result = FastqFinalizationExecutor.execute_to_complete(
            &mut store,
            &run_id,
            &StopToken::default(),
        );
        assert!(matches!(
            result,
            Err(FinalizationError::InvalidCompressedOutput(_))
        ));
        assert_eq!(
            store.get_run(&run_id).expect("state").expect("run").state,
            RunState::Failed
        );
        assert_eq!(fs::read(&source).expect("raw FASTQ"), original);
        assert!(!compressed_dir.join("SHA256SUMS").exists());
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn prepublication_destination_check_preserves_existing_empty_directory() {
        let root = temp_root("late-existing-empty-dir");
        fs::create_dir_all(&root).expect("root");
        let staged = root.join(".compressed-attempt-1");
        let user_directory = root.join("compressed");
        fs::create_dir(&staged).expect("stage");
        fs::write(staged.join("created.fastq.gz"), b"partial-test-data").expect("stage data");
        fs::create_dir(&user_directory).expect("preexisting empty dir");

        assert!(matches!(
            ensure_new_destination(&user_directory),
            Err(FinalizationError::FinalCompressedDirectoryExists(_))
        ));
        assert!(user_directory.is_dir());
        assert_eq!(
            fs::read(staged.join("created.fastq.gz")).expect("stage retained"),
            b"partial-test-data"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_destination_is_not_treated_as_absent() {
        use std::os::unix::fs::symlink;
        let root = temp_root("dangling-destination");
        fs::create_dir_all(&root).expect("root");
        let target = root.join("nonexistent");
        let link = root.join("compressed");
        symlink(&target, &link).expect("symlink");
        assert!(matches!(
            ensure_new_destination(&link),
            Err(FinalizationError::FinalCompressedDirectoryExists(_))
        ));
        assert!(fs::symlink_metadata(&link).is_ok());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn preexisting_compressed_directory_blocks_overwrite() {
        let root = temp_root("overwrite");
        let (mut store, run_id, _) =
            fastq_ready_store(&root, &[("SRR000001.fastq", b"@r\nAC\n+\nII\n")]);
        let compressed = root.join("fastq").join("SRR000001").join("compressed");
        fs::create_dir(&compressed).expect("preexisting");
        let executor = FastqFinalizationExecutor;

        let error = executor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .expect_err("overwrite blocked");

        assert!(matches!(
            error,
            FinalizationError::FinalCompressedDirectoryExists(path) if path == compressed
        ));
        assert_eq!(
            store
                .get_run(&run_id)
                .expect("run")
                .expect("exists")
                .attempt_count,
            0
        );

        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn ownership_contention_does_not_mutate_checkpoint_or_originals() {
        let root = temp_root("ownership-contention");
        let original = b"@r\nAC\n+\nII\n";
        let (mut store, run_id, _) = fastq_ready_store(&root, &[("SRR000001.fastq", original)]);
        let before = store.get_run(&run_id).unwrap().unwrap();
        let fastq_root = root.join("fastq");
        let holder = OutputOwnership::acquire(&[&fastq_root]).unwrap();
        assert!(matches!(
            FastqFinalizationExecutor.execute_to_complete(
                &mut store,
                &run_id,
                &StopToken::default()
            ),
            Err(FinalizationError::Ownership(OwnershipError::Busy { .. }))
        ));
        assert_eq!(store.get_run(&run_id).unwrap().unwrap(), before);
        assert_eq!(
            fs::read(fastq_root.join("SRR000001/SRR000001.fastq")).unwrap(),
            original
        );
        drop(holder);
        let result = FastqFinalizationExecutor
            .execute_to_complete(&mut store, &run_id, &StopToken::default())
            .unwrap();
        assert_eq!(result.run.state, RunState::Complete);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn published_data_survive_database_failure_and_can_be_adopted_on_resume() {
        for kind in ["COMPRESSED_FASTQ", "CHECKSUM"] {
            let root = temp_root(kind);
            let original = b"@r\nAC\n+\nII\n";
            let (mut store, run_id, _) = fastq_ready_store(&root, &[("SRR000001.fastq", original)]);
            // Isolated test fault injection deliberately bypasses the engine lock.
            let injector = rusqlite::Connection::open(root.join("state.sqlite")).unwrap();
            injector
                .execute_batch(&format!(
                    "CREATE TRIGGER forced_failure BEFORE INSERT ON artifacts \
                     WHEN NEW.kind = '{kind}' BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;"
                ))
                .unwrap();
            let failed = FastqFinalizationExecutor.execute_to_complete(
                &mut store,
                &run_id,
                &StopToken::default(),
            );
            if let Ok(result) = failed {
                assert!(matches!(
                    result.disposition,
                    FinalizationDisposition::Failed { .. }
                ));
            }
            let dir = root.join("fastq/SRR000001");
            assert!(dir.join("compressed/SRR000001.fastq.gz").is_file());
            assert_eq!(fs::read(dir.join("SRR000001.fastq")).unwrap(), original);
            if kind == "CHECKSUM" {
                assert!(dir.join("compressed/SHA256SUMS").is_file());
            } else {
                assert_eq!(
                    store.get_run(&run_id).unwrap().unwrap().state,
                    RunState::Compressing
                );
            }
            injector
                .execute_batch("DROP TRIGGER forced_failure;")
                .unwrap();
            drop(injector);
            drop(store);
            let mut resumed = StateStore::open(root.join("state.sqlite")).unwrap();
            let result = FastqFinalizationExecutor
                .execute_to_complete(&mut resumed, &run_id, &StopToken::default())
                .unwrap();
            assert_eq!(result.run.state, RunState::Complete);
            drop(resumed);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
