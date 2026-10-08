use crate::ownership::{publish_noreplace, OutputOwnership, OwnershipError};
use crate::{
    Accession, ArtifactId, CommandOutcome, CommandRunner, IdError, ProcessContext, ProcessError,
    RunId, RunRecord, RunState, SraPlanError, SraToolkitPlanner, StateStore, StopToken, StoreError,
};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

const CONVERSION_CHECKPOINT: &str = "fasterq-dump";
const CONVERSION_COMPLETE_CHECKPOINT: &str = "fasterq-complete";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionDisposition {
    FastqReady,
    PausedAtBoundary,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionResult {
    pub run: RunRecord,
    pub attempt: Option<i64>,
    pub disposition: ConversionDisposition,
    pub fastq_paths: Vec<PathBuf>,
    pub fasterq: Option<CommandOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionRequest {
    pub fastq_root: PathBuf,
    pub temp_root: PathBuf,
    pub log_root: PathBuf,
    pub threads: u32,
}

impl ConversionRequest {
    pub fn new(
        fastq_root: impl Into<PathBuf>,
        temp_root: impl Into<PathBuf>,
        log_root: impl Into<PathBuf>,
        threads: u32,
    ) -> Self {
        Self {
            fastq_root: fastq_root.into(),
            temp_root: temp_root.into(),
            log_root: log_root.into(),
            threads,
        }
    }
}

#[derive(Debug)]
pub enum ConversionError {
    Store(StoreError),
    Ownership(OwnershipError),
    Plan(SraPlanError),
    Identity(IdError),
    Process(ProcessError),
    MissingRun(RunId),
    UnsupportedState(RunState),
    UnknownFailedCheckpoint(Option<String>),
    UnknownPausedCheckpoint(Option<String>),
    MissingPersistedSraPath,
    FinalOutputExists(PathBuf),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidFastqOutput(String),
    SizeOverflow {
        path: PathBuf,
        bytes: u64,
    },
    PersistedPaths(serde_json::Error),
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(f),
            Self::Ownership(error) => error.fmt(f),
            Self::Plan(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Process(error) => write!(f, "fasterq-dump process execution failed: {error}"),
            Self::MissingRun(id) => write!(f, "run not found: {id}"),
            Self::UnsupportedState(state) => {
                write!(f, "cannot execute FASTQ conversion from state {state}")
            }
            Self::UnknownFailedCheckpoint(checkpoint) => write!(
                f,
                "cannot determine conversion retry from failed checkpoint {:?}",
                checkpoint
            ),
            Self::UnknownPausedCheckpoint(checkpoint) => write!(
                f,
                "cannot determine conversion resume from paused checkpoint {:?}",
                checkpoint
            ),
            Self::MissingPersistedSraPath => {
                f.write_str("validated run is missing its persisted SRA accession-directory path")
            }
            Self::FinalOutputExists(path) => write!(
                f,
                "final FASTQ output already exists and will not be overwritten: {}",
                path.display()
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation} {} failed: {source}", path.display()),
            Self::InvalidFastqOutput(reason) => write!(f, "invalid fasterq-dump output: {reason}"),
            Self::SizeOverflow { path, bytes } => write!(
                f,
                "FASTQ file {} is too large to persist as i64 bytes: {bytes}",
                path.display()
            ),
            Self::PersistedPaths(error) => {
                write!(f, "cannot decode persisted FASTQ paths: {error}")
            }
        }
    }
}

impl std::error::Error for ConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Ownership(error) => Some(error),
            Self::Plan(error) => Some(error),
            Self::Identity(error) => Some(error),
            Self::Process(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::PersistedPaths(error) => Some(error),
            Self::MissingRun(_)
            | Self::UnsupportedState(_)
            | Self::UnknownFailedCheckpoint(_)
            | Self::UnknownPausedCheckpoint(_)
            | Self::MissingPersistedSraPath
            | Self::FinalOutputExists(_)
            | Self::InvalidFastqOutput(_)
            | Self::SizeOverflow { .. } => None,
        }
    }
}

impl From<StoreError> for ConversionError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

impl From<SraPlanError> for ConversionError {
    fn from(value: SraPlanError) -> Self {
        Self::Plan(value)
    }
}

impl From<IdError> for ConversionError {
    fn from(value: IdError) -> Self {
        Self::Identity(value)
    }
}

pub struct FasterqConversionExecutor<'a, R: CommandRunner> {
    planner: &'a SraToolkitPlanner,
    runner: &'a R,
    ownership: Option<&'a OutputOwnership>,
}

impl<'a, R: CommandRunner> FasterqConversionExecutor<'a, R> {
    pub fn new(planner: &'a SraToolkitPlanner, runner: &'a R) -> Self {
        Self {
            planner,
            runner,
            ownership: None,
        }
    }

    pub(crate) fn with_ownership(mut self, ownership: &'a OutputOwnership) -> Self {
        self.ownership = Some(ownership);
        self
    }

    pub fn execute_to_fastq_ready(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        request: &ConversionRequest,
        stop: &StopToken,
    ) -> Result<ConversionResult, ConversionError> {
        let current = store
            .get_run(run_id)?
            .ok_or_else(|| ConversionError::MissingRun(run_id.clone()))?;

        if current.state == RunState::FastqReady {
            let paths = decode_fastq_paths(&current.fastq_paths)?;
            return Ok(ConversionResult {
                run: current,
                attempt: None,
                disposition: ConversionDisposition::FastqReady,
                fastq_paths: paths,
                fasterq: None,
            });
        }

        if request.threads == 0 {
            return Err(ConversionError::Plan(SraPlanError::ZeroThreads));
        }

        let accession = Accession::parse(&current.accession_or_source)
            .map_err(|error| ConversionError::InvalidFastqOutput(error.to_string()))?;
        let sra_path = current
            .sra_path
            .as_deref()
            .ok_or(ConversionError::MissingPersistedSraPath)?;
        let fastq_root = request.fastq_root.as_path();
        let temp_root = request.temp_root.as_path();
        let log_root = request.log_root.as_path();
        let mut roots = vec![fastq_root, temp_root, log_root];
        if let Some(parent) = Path::new(sra_path).parent() {
            roots.push(parent);
        }
        let _ownership = OutputOwnership::ensure_or_acquire(&roots, self.ownership)
            .map_err(ConversionError::Ownership)?;
        let final_directory = fastq_root.join(accession.as_str());

        if fs::symlink_metadata(&final_directory).is_ok() {
            return Err(ConversionError::FinalOutputExists(final_directory));
        }

        match current.state {
            RunState::SraValid => {}
            RunState::PausedAtBoundary
                if current.last_checkpoint.as_deref() == Some(CONVERSION_CHECKPOINT) => {}
            RunState::PausedAtBoundary => {
                return Err(ConversionError::UnknownPausedCheckpoint(
                    current.last_checkpoint.clone(),
                ));
            }
            RunState::Failed
                if current.last_checkpoint.as_deref() == Some(CONVERSION_CHECKPOINT) => {}
            RunState::Failed => {
                return Err(ConversionError::UnknownFailedCheckpoint(
                    current.last_checkpoint.clone(),
                ));
            }
            state => return Err(ConversionError::UnsupportedState(state)),
        }

        let attempt = store.begin_run_attempt(run_id)?;
        match current.state {
            RunState::SraValid | RunState::PausedAtBoundary => {
                store.transition_run(
                    run_id,
                    RunState::Converting,
                    Some(CONVERSION_CHECKPOINT),
                    None,
                )?;
            }
            RunState::Failed => {
                store.retry_run(run_id, RunState::Converting, Some(CONVERSION_CHECKPOINT))?;
            }
            _ => unreachable!("state prevalidated above"),
        }

        let staging_parent = fastq_root.join(".partial");
        let staging_directory =
            staging_parent.join(format!("{}-attempt-{attempt}", accession.as_str()));
        let temp_directory = temp_root.join(format!("{}-attempt-{attempt}", accession.as_str()));

        if let Err(error) = prepare_attempt_directories(
            fastq_root,
            &staging_parent,
            &staging_directory,
            temp_root,
            &temp_directory,
        ) {
            self.persist_internal_failure(store, run_id, &error)?;
            return Err(error);
        }

        let plan = match self.planner.fasterq(
            sra_path,
            &staging_directory,
            &temp_directory,
            request.threads,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let conversion_error = ConversionError::Plan(error);
                let cleanup = cleanup_attempt_directories(&staging_directory, &temp_directory);
                let reason = append_cleanup_warning(conversion_error.to_string(), cleanup);
                store.transition_run(
                    run_id,
                    RunState::Failed,
                    Some(CONVERSION_CHECKPOINT),
                    Some(&reason),
                )?;
                return Err(conversion_error);
            }
        };

        let context = match ProcessContext::new(
            log_root,
            format!("{}-attempt-{attempt}-fasterq-dump", accession.as_str()),
        ) {
            Ok(context) => context,
            Err(error) => {
                let conversion_error = ConversionError::Process(error);
                let cleanup = cleanup_attempt_directories(&staging_directory, &temp_directory);
                let reason = append_cleanup_warning(conversion_error.to_string(), cleanup);
                store.transition_run(
                    run_id,
                    RunState::Failed,
                    Some(CONVERSION_CHECKPOINT),
                    Some(&reason),
                )?;
                return Err(conversion_error);
            }
        };

        let outcome = match self.runner.run(&plan.command, &context, stop) {
            Ok(outcome) => outcome,
            Err(error) => {
                let conversion_error = ConversionError::Process(error);
                let cleanup = cleanup_attempt_directories(&staging_directory, &temp_directory);
                let reason = append_cleanup_warning(conversion_error.to_string(), cleanup);
                store.transition_run(
                    run_id,
                    RunState::Failed,
                    Some(CONVERSION_CHECKPOINT),
                    Some(&reason),
                )?;
                return Err(conversion_error);
            }
        };

        if outcome.stopped_by_request {
            let cleanup = cleanup_attempt_directories(&staging_directory, &temp_directory);
            let warning = cleanup_warning(cleanup);
            let run = store.transition_run(
                run_id,
                RunState::PausedAtBoundary,
                Some(CONVERSION_CHECKPOINT),
                warning.as_deref(),
            )?;
            return Ok(ConversionResult {
                run,
                attempt: Some(attempt),
                disposition: ConversionDisposition::PausedAtBoundary,
                fastq_paths: Vec::new(),
                fasterq: Some(outcome),
            });
        }

        if !outcome.success {
            let cleanup = cleanup_attempt_directories(&staging_directory, &temp_directory);
            let reason = append_cleanup_warning(
                format!(
                    "fasterq-dump exited unsuccessfully with exit code {:?}",
                    outcome.exit_code
                ),
                cleanup,
            );
            let run = store.transition_run(
                run_id,
                RunState::Failed,
                Some(CONVERSION_CHECKPOINT),
                Some(&reason),
            )?;
            return Ok(ConversionResult {
                run,
                attempt: Some(attempt),
                disposition: ConversionDisposition::Failed {
                    reason: reason.clone(),
                },
                fastq_paths: Vec::new(),
                fasterq: Some(outcome),
            });
        }

        let staged_fastqs = match discover_fastq_outputs(&staging_directory, accession.as_str()) {
            Ok(paths) => paths,
            Err(error) => {
                let cleanup = cleanup_attempt_directories(&staging_directory, &temp_directory);
                let reason = append_cleanup_warning(error.to_string(), cleanup);
                let run = store.transition_run(
                    run_id,
                    RunState::Failed,
                    Some(CONVERSION_CHECKPOINT),
                    Some(&reason),
                )?;
                return Ok(ConversionResult {
                    run,
                    attempt: Some(attempt),
                    disposition: ConversionDisposition::Failed {
                        reason: reason.clone(),
                    },
                    fastq_paths: Vec::new(),
                    fasterq: Some(outcome),
                });
            }
        };

        if let Err(error) = remove_owned_directory_if_present(&temp_directory) {
            self.persist_internal_failure(store, run_id, &error)?;
            return Err(error);
        }

        if let Err(source) = publish_noreplace(&staging_directory, &final_directory) {
            let error = ConversionError::Io {
                operation: "finalize FASTQ directory",
                path: final_directory.clone(),
                source,
            };
            self.persist_internal_failure(store, run_id, &error)?;
            return Err(error);
        }

        let mut finalized = Vec::with_capacity(staged_fastqs.len());
        for (index, staged_path) in staged_fastqs.iter().enumerate() {
            let file_name = staged_path.file_name().ok_or_else(|| {
                ConversionError::InvalidFastqOutput(format!(
                    "FASTQ path has no filename: {}",
                    staged_path.display()
                ))
            })?;
            let final_path = final_directory.join(file_name);
            let bytes = fs::metadata(&final_path)
                .map_err(|source| ConversionError::Io {
                    operation: "inspect finalized FASTQ",
                    path: final_path.clone(),
                    source,
                })?
                .len();
            let size_bytes = i64::try_from(bytes).map_err(|_| ConversionError::SizeOverflow {
                path: final_path.clone(),
                bytes,
            })?;
            let artifact_id = ArtifactId::new(format!("{}-fastq-{}", run_id.as_str(), index + 1))?;
            finalized.push((
                artifact_id,
                final_path.to_string_lossy().into_owned(),
                size_bytes,
            ));
        }

        store.record_finalized_fastq_artifacts(run_id, &finalized)?;
        let run = store.transition_run(
            run_id,
            RunState::FastqReady,
            Some(CONVERSION_COMPLETE_CHECKPOINT),
            None,
        )?;
        let fastq_paths = finalized
            .iter()
            .map(|(_, path, _)| PathBuf::from(path))
            .collect();

        Ok(ConversionResult {
            run,
            attempt: Some(attempt),
            disposition: ConversionDisposition::FastqReady,
            fastq_paths,
            fasterq: Some(outcome),
        })
    }

    fn persist_internal_failure(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        error: &ConversionError,
    ) -> Result<(), ConversionError> {
        store.transition_run(
            run_id,
            RunState::Failed,
            Some(CONVERSION_CHECKPOINT),
            Some(&error.to_string()),
        )?;
        Ok(())
    }
}

fn decode_fastq_paths(value: &str) -> Result<Vec<PathBuf>, ConversionError> {
    let paths: Vec<String> =
        serde_json::from_str(value).map_err(ConversionError::PersistedPaths)?;
    Ok(paths.into_iter().map(PathBuf::from).collect())
}

fn prepare_attempt_directories(
    fastq_root: &Path,
    staging_parent: &Path,
    staging_directory: &Path,
    temp_root: &Path,
    temp_directory: &Path,
) -> Result<(), ConversionError> {
    create_directory_all(fastq_root, "create FASTQ root")?;
    create_directory_all(staging_parent, "create FASTQ staging root")?;
    create_directory_all(temp_root, "create fasterq temporary root")?;
    create_new_directory(staging_directory, "create attempt staging directory")?;
    if let Err(error) = create_new_directory(temp_directory, "create attempt temporary directory") {
        let _ = remove_owned_directory_if_present(staging_directory);
        return Err(error);
    }
    Ok(())
}

fn create_directory_all(path: &Path, operation: &'static str) -> Result<(), ConversionError> {
    fs::create_dir_all(path).map_err(|source| ConversionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn create_new_directory(path: &Path, operation: &'static str) -> Result<(), ConversionError> {
    fs::create_dir(path).map_err(|source| ConversionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn discover_fastq_outputs(
    staging_directory: &Path,
    accession: &str,
) -> Result<Vec<PathBuf>, ConversionError> {
    let entries = fs::read_dir(staging_directory).map_err(|source| ConversionError::Io {
        operation: "inspect fasterq staging directory",
        path: staging_directory.to_path_buf(),
        source,
    })?;
    let mut fastqs = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|source| ConversionError::Io {
            operation: "read fasterq staging entry",
            path: staging_directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| ConversionError::Io {
            operation: "inspect fasterq output",
            path: path.clone(),
            source,
        })?;
        if !metadata.file_type().is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_accession_fastq_name(name, accession) {
            continue;
        }
        if metadata.len() == 0 {
            return Err(ConversionError::InvalidFastqOutput(format!(
                "FASTQ output is empty: {}",
                path.display()
            )));
        }
        fastqs.push(path);
    }

    fastqs.sort();
    if fastqs.is_empty() {
        return Err(ConversionError::InvalidFastqOutput(format!(
            "no non-empty FASTQ files for accession {accession} were created"
        )));
    }
    Ok(fastqs)
}

fn is_accession_fastq_name(name: &str, accession: &str) -> bool {
    name == format!("{accession}.fastq")
        || (name.starts_with(&format!("{accession}_")) && name.ends_with(".fastq"))
}

fn cleanup_attempt_directories(staging_directory: &Path, temp_directory: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    for path in [staging_directory, temp_directory] {
        if let Err(error) = remove_owned_directory_if_present(path) {
            warnings.push(error.to_string());
        }
    }
    warnings
}

fn remove_owned_directory_if_present(path: &Path) -> Result<(), ConversionError> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_dir_all(path).map_err(|source| ConversionError::Io {
        operation: "remove engine-owned partial directory",
        path: path.to_path_buf(),
        source,
    })
}

fn cleanup_warning(warnings: Vec<String>) -> Option<String> {
    if warnings.is_empty() {
        None
    } else {
        Some(format!(
            "partial conversion cleanup needs attention: {}",
            warnings.join("; ")
        ))
    }
}

fn append_cleanup_warning(reason: String, warnings: Vec<String>) -> String {
    match cleanup_warning(warnings) {
        Some(warning) => format!("{reason}; {warning}"),
        None => reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactKind, ArtifactValidationState, CommandSpec, JobId, NewJob, NewRun, ToolInfo,
        ToolKind, ToolRegistry,
    };
    use std::{
        collections::VecDeque,
        ffi::OsString,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug, Clone)]
    struct FakeStep {
        outcome: CommandOutcome,
        outputs: Vec<(String, Vec<u8>)>,
    }

    #[derive(Debug)]
    struct FakeRunner {
        steps: Mutex<VecDeque<FakeStep>>,
        seen: Mutex<Vec<(CommandSpec, ProcessContext)>>,
    }

    impl FakeRunner {
        fn new(steps: Vec<FakeStep>) -> Self {
            Self {
                steps: Mutex::new(steps.into()),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn seen(&self) -> Vec<(CommandSpec, ProcessContext)> {
            self.seen.lock().expect("seen lock").clone()
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(
            &self,
            spec: &CommandSpec,
            context: &ProcessContext,
            _stop: &StopToken,
        ) -> Result<CommandOutcome, ProcessError> {
            self.seen
                .lock()
                .expect("seen lock")
                .push((spec.clone(), context.clone()));
            let step = self
                .steps
                .lock()
                .expect("steps lock")
                .pop_front()
                .expect("fake step");
            if let Some(output_dir) = option_path(spec, "-O") {
                for (name, bytes) in &step.outputs {
                    fs::write(output_dir.join(name), bytes).expect("write fake FASTQ output");
                }
            }
            Ok(step.outcome)
        }
    }

    fn option_path(spec: &CommandSpec, option: &str) -> Option<PathBuf> {
        let position = spec
            .args
            .iter()
            .position(|arg| arg == &OsString::from(option))?;
        spec.args.get(position + 1).map(PathBuf::from)
    }

    fn outcome(label: &str, success: bool, stopped_by_request: bool) -> CommandOutcome {
        CommandOutcome {
            exit_code: if success { Some(0) } else { Some(1) },
            success,
            stopped_by_request,
            stdout_log: PathBuf::from(format!("{label}.stdout.log")),
            stderr_log: PathBuf::from(format!("{label}.stderr.log")),
        }
    }

    fn step(
        label: &str,
        success: bool,
        stopped_by_request: bool,
        outputs: Vec<(&str, &[u8])>,
    ) -> FakeStep {
        FakeStep {
            outcome: outcome(label, success, stopped_by_request),
            outputs: outputs
                .into_iter()
                .map(|(name, bytes)| (name.to_owned(), bytes.to_vec()))
                .collect(),
        }
    }

    fn registry() -> ToolRegistry {
        ToolRegistry {
            prefetch: ToolInfo {
                kind: ToolKind::Prefetch,
                path: PathBuf::from("/tools/prefetch"),
                version_output: "prefetch : 3.4.1".to_owned(),
            },
            vdb_validate: ToolInfo {
                kind: ToolKind::VdbValidate,
                path: PathBuf::from("/tools/vdb-validate"),
                version_output: "vdb-validate : 3.4.1".to_owned(),
            },
            fasterq_dump: ToolInfo {
                kind: ToolKind::FasterqDump,
                path: PathBuf::from("/tools/fasterq-dump"),
                version_output: "fasterq-dump : 3.4.1".to_owned(),
            },
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rnaseq-fasterq-test-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn request(root: &Path, threads: u32) -> ConversionRequest {
        ConversionRequest::new(
            root.join("fastq"),
            root.join("tmp"),
            root.join("logs"),
            threads,
        )
    }

    fn sra_valid_store(root: &Path) -> (StateStore, RunId, PathBuf) {
        let mut store = StateStore::open_in_memory().expect("store");
        let job_id = JobId::new("job-1").expect("job id");
        let run_id = RunId::new("run-1").expect("run id");
        let sra_path = root.join("sra").join("SRR000001");
        fs::create_dir_all(&sra_path).expect("SRA input");
        fs::write(sra_path.join("data.sra"), b"validated-input").expect("fake SRA");

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
            .update_run_download_snapshot(&run_id, 15, sra_path.to_string_lossy().as_ref())
            .expect("SRA snapshot");
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
            .expect("SRA valid");

        (store, run_id, sra_path)
    }

    #[test]
    fn single_fastq_is_finalized_and_persisted() {
        let root = temp_root("single");
        let (mut store, run_id, sra_path) = sra_valid_store(&root);
        let runner = FakeRunner::new(vec![step(
            "fasterq",
            true,
            false,
            vec![("SRR000001.fastq", b"@r\nAC\n+\nII\n")],
        )]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 4),
                &StopToken::default(),
            )
            .expect("conversion");

        assert_eq!(result.disposition, ConversionDisposition::FastqReady);
        assert_eq!(result.run.state, RunState::FastqReady);
        assert_eq!(result.attempt, Some(1));
        assert_eq!(result.fastq_paths.len(), 1);
        assert!(result.fastq_paths[0].is_file());
        assert!(sra_path.is_dir());

        let persisted: Vec<String> =
            serde_json::from_str(&result.run.fastq_paths).expect("persisted paths");
        assert_eq!(persisted.len(), 1);
        assert_eq!(
            persisted[0],
            result.fastq_paths[0].to_string_lossy().as_ref()
        );

        let artifact_id = ArtifactId::new("run-1-fastq-1").expect("artifact id");
        let artifact = store
            .get_artifact(&artifact_id)
            .expect("artifact query")
            .expect("artifact");
        assert_eq!(artifact.kind, ArtifactKind::Fastq);
        assert_eq!(artifact.validation_state, ArtifactValidationState::Pending);

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn split_outputs_are_discovered_without_layout_assumption() {
        let root = temp_root("paired");
        let (mut store, run_id, _) = sra_valid_store(&root);
        let runner = FakeRunner::new(vec![step(
            "fasterq",
            true,
            false,
            vec![
                ("SRR000001_1.fastq", b"@r/1\nAC\n+\nII\n"),
                ("SRR000001_2.fastq", b"@r/2\nGT\n+\nII\n"),
            ],
        )]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("conversion");

        assert_eq!(result.fastq_paths.len(), 2);
        assert!(result.fastq_paths[0].ends_with("SRR000001_1.fastq"));
        assert!(result.fastq_paths[1].ends_with("SRR000001_2.fastq"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn stopped_conversion_pauses_at_boundary_and_cleans_only_partial_outputs() {
        let root = temp_root("stop");
        let (mut store, run_id, sra_path) = sra_valid_store(&root);
        let runner = FakeRunner::new(vec![step(
            "stopped",
            false,
            true,
            vec![("SRR000001.fastq", b"partial")],
        )]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("paused conversion");

        assert_eq!(result.disposition, ConversionDisposition::PausedAtBoundary);
        assert_eq!(result.run.state, RunState::PausedAtBoundary);
        assert!(sra_path.is_dir());
        assert!(!root
            .join("fastq")
            .join(".partial")
            .join("SRR000001-attempt-1")
            .exists());
        assert!(!root.join("tmp").join("SRR000001-attempt-1").exists());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn failed_conversion_retries_with_new_staging_identity() {
        let root = temp_root("retry");
        let (mut store, run_id, _) = sra_valid_store(&root);
        let runner = FakeRunner::new(vec![
            step(
                "failed",
                false,
                false,
                vec![("SRR000001.fastq", b"partial")],
            ),
            step(
                "success",
                true,
                false,
                vec![("SRR000001.fastq", b"@r\nAC\n+\nII\n")],
            ),
        ]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let first = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("failed outcome");
        assert_eq!(first.run.state, RunState::Failed);

        let second = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("retry");
        assert_eq!(second.run.state, RunState::FastqReady);
        assert_eq!(second.attempt, Some(2));

        let seen = runner.seen();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0.args[0], sra_path_os(&root));
        assert_eq!(seen[1].0.args[0], sra_path_os(&root));
        assert_ne!(option_path(&seen[0].0, "-O"), option_path(&seen[1].0, "-O"));
        assert_ne!(option_path(&seen[0].0, "-t"), option_path(&seen[1].0, "-t"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    fn sra_path_os(root: &Path) -> OsString {
        root.join("sra").join("SRR000001").as_os_str().to_owned()
    }

    #[test]
    fn existing_final_directory_blocks_overwrite_before_attempt() {
        let root = temp_root("overwrite");
        let (mut store, run_id, _) = sra_valid_store(&root);
        let final_dir = root.join("fastq").join("SRR000001");
        fs::create_dir_all(&final_dir).expect("existing final dir");
        let runner = FakeRunner::new(Vec::new());
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let error = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect_err("must refuse overwrite");

        assert!(matches!(error, ConversionError::FinalOutputExists(path) if path == final_dir));
        assert_eq!(
            store
                .get_run(&run_id)
                .expect("run")
                .expect("exists")
                .attempt_count,
            0
        );
        assert!(runner.seen().is_empty());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn missing_fastq_output_never_reaches_fastq_ready() {
        let root = temp_root("missing");
        let (mut store, run_id, _) = sra_valid_store(&root);
        let runner = FakeRunner::new(vec![step("success-missing", true, false, Vec::new())]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("missing output is explicit failed outcome");

        assert_eq!(result.run.state, RunState::Failed);
        assert!(matches!(
            result.disposition,
            ConversionDisposition::Failed { .. }
        ));
        assert!(!root.join("fastq").join("SRR000001").exists());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn paused_conversion_resumes_from_validated_sra_with_new_attempt() {
        let root = temp_root("pause-resume");
        let (mut store, run_id, sra_path) = sra_valid_store(&root);
        let runner = FakeRunner::new(vec![
            step(
                "stopped",
                false,
                true,
                vec![("SRR000001.fastq", b"partial")],
            ),
            step(
                "success",
                true,
                false,
                vec![("SRR000001.fastq", b"@r\nAC\n+\nII\n")],
            ),
        ]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let first = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("pause");
        assert_eq!(first.run.state, RunState::PausedAtBoundary);
        assert_eq!(first.attempt, Some(1));

        let second = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("resume");
        assert_eq!(second.run.state, RunState::FastqReady);
        assert_eq!(second.attempt, Some(2));
        assert!(sra_path.is_dir());

        let seen = runner.seen();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0.args[0], sra_path.as_os_str());
        assert_eq!(seen[1].0.args[0], sra_path.as_os_str());
        assert_ne!(option_path(&seen[0].0, "-O"), option_path(&seen[1].0, "-O"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn empty_fastq_output_never_reaches_fastq_ready() {
        let root = temp_root("empty");
        let (mut store, run_id, _) = sra_valid_store(&root);
        let runner = FakeRunner::new(vec![step(
            "success-empty",
            true,
            false,
            vec![("SRR000001.fastq", b"")],
        )]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = FasterqConversionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_fastq_ready(
                &mut store,
                &run_id,
                &request(&root, 2),
                &StopToken::default(),
            )
            .expect("explicit failed outcome");

        assert_eq!(result.run.state, RunState::Failed);
        assert!(matches!(
            result.disposition,
            ConversionDisposition::Failed { .. }
        ));
        assert!(!root.join("fastq").join("SRR000001").exists());

        fs::remove_dir_all(root).expect("cleanup");
    }
}
