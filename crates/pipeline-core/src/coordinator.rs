//! One persisted orchestration path for CLI and future desktop clients.
//! External tools run to a stage boundary. Ambiguous crash checkpoints remain
//! blocked: releasing a parent lock is not evidence that its descendants exited.
use crate::{
    Accession, AccessionLevel, CommandRunner, ConversionRequest, FasterqConversionExecutor,
    FastqFinalizationExecutor, JobId, JobRecord, JobState, NewJob, NewRun, OutputOwnership,
    OwnershipError, RunId, RunRecord, RunState, SraAcquisitionExecutor, SraToolkitPlanner,
    StateStore, StopToken, StoreError, ToolKind,
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum JobError {
    Store(StoreError),
    Ownership(OwnershipError),
    Invalid(String),
    Io(std::io::Error),
    Stage(String),
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(f),
            Self::Ownership(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::Invalid(message) | Self::Stage(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for JobError {}
impl From<StoreError> for JobError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}
impl From<OwnershipError> for JobError {
    fn from(error: OwnershipError) -> Self {
        Self::Ownership(error)
    }
}
impl From<std::io::Error> for JobError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlIntent {
    Continue,
    Pause,
    Cancel,
}

/// Durable requests do not open or write SQLite while a worker owns it.
/// Cancel takes precedence; Resume clears only Pause, never Cancel.
#[derive(Debug, Clone)]
pub struct JobControl {
    directory: PathBuf,
}

impl JobControl {
    pub fn open(job_root: impl AsRef<Path>) -> Result<Self, JobError> {
        let job_root = job_root.as_ref();
        if !fs::symlink_metadata(job_root.join("MODULE-A-JOB"))?
            .file_type()
            .is_file()
        {
            return Err(JobError::Invalid("not a Module A job destination".into()));
        }
        let directory = fs::canonicalize(job_root.join("control"))?;
        Ok(Self { directory })
    }

    pub fn request(&self, intent: ControlIntent) -> Result<(), JobError> {
        let name = match intent {
            ControlIntent::Pause => "pause.request",
            ControlIntent::Cancel => "cancel.request",
            ControlIntent::Continue => {
                return Err(JobError::Invalid("use Resume to clear Pause".into()))
            }
        };
        let _lease = OutputOwnership::acquire(&[&self.directory])?;
        let path = self.directory.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(b"requested\n")?;
                file.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if !fs::symlink_metadata(&path)?.file_type().is_file() {
                    return Err(JobError::Invalid(
                        "control request must be a regular file".into(),
                    ));
                }
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    pub fn intent(&self) -> Result<ControlIntent, JobError> {
        for (name, intent) in [
            ("cancel.request", ControlIntent::Cancel),
            ("pause.request", ControlIntent::Pause),
        ] {
            match fs::symlink_metadata(self.directory.join(name)) {
                Ok(metadata) if metadata.file_type().is_file() => return Ok(intent),
                Ok(_) => {
                    return Err(JobError::Invalid(
                        "control request must be a regular file".into(),
                    ))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(ControlIntent::Continue)
    }

    fn resume(&self) -> Result<(), JobError> {
        let _lease = OutputOwnership::acquire(&[&self.directory])?;
        match fs::remove_file(self.directory.join("pause.request")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveMode {
    Start,
    Resume,
    Retry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobEvent {
    RunSnapshot {
        run: RunRecord,
        progress: ByteProgress,
    },
    RunError {
        run_id: RunId,
        message: String,
    },
    ControlApplied(ControlIntent),
    JobFinished(JobState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteProgress {
    pub observed_bytes: i64,
    /// None until a resolver supplies a validated source-size contract.
    /// A local downloaded byte count is not such a denominator.
    pub validated_total_bytes: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobReport {
    pub state: JobState,
    pub runs: Vec<RunRecord>,
    pub errors: Vec<(RunId, String)>,
}

/// Terminal jobs can be reconciled without probing or launching external tools.
pub fn terminal_job_report(store: &StateStore, id: &JobId) -> Result<Option<JobReport>, JobError> {
    let job = store
        .get_job(id)?
        .ok_or_else(|| JobError::Invalid(format!("job not found: {id}")))?;
    let runs = store.list_job_runs(id)?;
    validate_plan(&job, &runs)?;
    if runs.iter().all(|run| run.state.is_terminal()) {
        return finish(store, id, Vec::new(), &mut |_| {}).map(Some);
    }
    Ok(None)
}

pub fn create_job(
    store: &mut StateStore,
    id: JobId,
    output_parent: impl AsRef<Path>,
    threads: u32,
    accessions: &[Accession],
) -> Result<JobRecord, JobError> {
    if id.as_str().len() > 128
        || !id
            .as_str()
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(JobError::Invalid(
            "job ID must contain 1-128 ASCII letters, digits, '-' or '_'".into(),
        ));
    }
    if threads == 0 || threads > 256 || accessions.is_empty() {
        return Err(JobError::Invalid(
            "supply at least one resolved run and 1-256 threads".into(),
        ));
    }
    let mut seen = HashSet::new();
    for accession in accessions {
        if accession.level() != AccessionLevel::Run || !seen.insert(accession.as_str()) {
            return Err(JobError::Invalid(
                "only distinct resolved SRR/ERR/DRR run accessions are supported".into(),
            ));
        }
    }
    if store.get_job(&id)?.is_some() {
        return Err(JobError::Invalid(
            "job already exists; Inspect, Resume or Retry it".into(),
        ));
    }
    fs::create_dir_all(output_parent.as_ref())?;
    let root = fs::canonicalize(output_parent.as_ref())?.join(id.as_str());
    let root_string = root
        .to_str()
        .ok_or_else(|| JobError::Invalid("job destination must be valid UTF-8".into()))?;
    // Existing user directories are never adopted, even if empty.
    fs::create_dir(&root)?;
    fs::create_dir(root.join("control"))?;
    let mut marker = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(root.join("MODULE-A-JOB"))?;
    marker.write_all(id.as_str().as_bytes())?;
    marker.sync_all()?;
    let plan = json!({"format_version": 1, "threads": threads,
        "accessions": accessions.iter().map(Accession::as_str).collect::<Vec<_>>()});
    let runs = accessions
        .iter()
        .map(|accession| NewRun {
            id: RunId::new(format!("{}:{}", id, accession)).expect("nonempty resolved identity"),
            job_id: id.clone(),
            accession_or_source: accession.to_string(),
        })
        .collect::<Vec<_>>();
    Ok(store.create_resolved_job(
        NewJob {
            id,
            input_type: "resolved-run-batch".into(),
            input_identity: json!(accessions.iter().map(Accession::as_str).collect::<Vec<_>>())
                .to_string(),
            output_root: root_string.into(),
            settings_snapshot: plan.to_string(),
            tool_versions_snapshot: "{}".into(),
        },
        &runs,
    )?)
}

pub struct JobCoordinator<'a, R: CommandRunner> {
    planner: &'a SraToolkitPlanner,
    runner: &'a R,
}

impl<'a, R: CommandRunner> JobCoordinator<'a, R> {
    pub fn new(planner: &'a SraToolkitPlanner, runner: &'a R) -> Self {
        Self { planner, runner }
    }

    pub fn drive(
        &self,
        store: &mut StateStore,
        id: &JobId,
        mode: DriveMode,
        mut emit: impl FnMut(JobEvent),
    ) -> Result<JobReport, JobError> {
        let job = store
            .get_job(id)?
            .ok_or_else(|| JobError::Invalid(format!("job not found: {id}")))?;
        let runs = store.list_job_runs(id)?;
        let threads = validate_plan(&job, &runs)?;
        if runs.iter().all(|run| run.state.is_terminal()) {
            return finish(store, id, Vec::new(), &mut emit);
        }
        let root = Path::new(&job.output_root);
        let sra = root.join("sra");
        let request = ConversionRequest::new(
            root.join("fastq"),
            root.join("temp"),
            root.join("logs"),
            threads,
        );
        // Same lease stays live between stages and across the entire batch.
        let ownership = OutputOwnership::acquire(&[
            root,
            &sra,
            &request.fastq_root,
            &request.temp_root,
            &request.log_root,
        ])?;
        let control = JobControl::open(root)?;
        if fs::read(root.join("MODULE-A-JOB"))? != id.as_str().as_bytes() {
            return Err(JobError::Invalid(
                "job destination identity does not match the database".into(),
            ));
        }
        let tools =
            json!(ToolKind::SRA_REQUIRED.iter().map(|kind| {
            let tool = self.planner.tools().tool(*kind);
            json!({"kind": kind.to_string(), "path": tool.path, "version": tool.version_output})
        }).collect::<Vec<_>>())
            .to_string();
        if job.tool_versions_snapshot != "{}" && job.tool_versions_snapshot != tools {
            return Err(JobError::Invalid(
                "tool paths/versions changed; restore the persisted toolchain before Resume/Retry"
                    .into(),
            ));
        }
        if mode != DriveMode::Start {
            control.resume()?;
        }
        store.freeze_job_tools(id, &tools)?;
        store.update_job_state(id, JobState::Active, None)?;
        let mut errors = Vec::new();
        for run in runs {
            // Terminal siblings remain byte-for-byte unchanged.
            if run.state.is_terminal() {
                snapshot(&run, &mut emit);
                continue;
            }
            if let Err(error) = self.drive_run(
                store, &run.id, mode, &sra, &request, &ownership, &control, &mut emit,
            ) {
                let message = error.to_string();
                emit(JobEvent::RunError {
                    run_id: run.id.clone(),
                    message: message.clone(),
                });
                errors.push((run.id, message));
            }
            if control.intent()? == ControlIntent::Pause {
                emit(JobEvent::ControlApplied(ControlIntent::Pause));
                store.update_job_state(id, JobState::Paused, None)?;
                let runs = store.list_job_runs(id)?;
                emit(JobEvent::JobFinished(JobState::Paused));
                return Ok(JobReport {
                    state: JobState::Paused,
                    runs,
                    errors,
                });
            }
        }
        finish(store, id, errors, &mut emit)
    }

    #[allow(clippy::too_many_arguments)]
    fn drive_run(
        &self,
        store: &mut StateStore,
        id: &RunId,
        mode: DriveMode,
        sra_root: &Path,
        request: &ConversionRequest,
        ownership: &OutputOwnership,
        control: &JobControl,
        emit: &mut impl FnMut(JobEvent),
    ) -> Result<(), JobError> {
        let mut retried = false;
        loop {
            let run = store
                .get_run(id)?
                .ok_or_else(|| JobError::Invalid(format!("run not found: {id}")))?;
            snapshot(&run, emit);
            if run.state.is_terminal() {
                return Ok(());
            }
            if matches!(
                run.state,
                RunState::Downloading | RunState::Validating | RunState::Converting
            ) {
                return Err(JobError::Stage(format!("{} is {}; automatic recovery is blocked because a previous tool/descendant may still write. Do not reset state or remove lock files. Process-lifetime reconciliation is required", id, run.state)));
            }
            match control.intent()? {
                ControlIntent::Pause => return Ok(()),
                ControlIntent::Cancel => {
                    store.transition_run(
                        id,
                        RunState::Cancelled,
                        run.last_checkpoint.as_deref(),
                        None,
                    )?;
                    emit(JobEvent::ControlApplied(ControlIntent::Cancel));
                    continue;
                }
                ControlIntent::Continue => {}
            }
            if run.state == RunState::Failed {
                if mode != DriveMode::Retry || retried {
                    return Ok(());
                }
                retried = true;
            }
            // Do not asynchronously kill external tools: existing SystemCommandRunner
            // only proves direct-child termination, not a whole descendant tree.
            let stop = StopToken::default();
            match dispatch(&run)? {
                Stage::Acquisition => {
                    SraAcquisitionExecutor::new(self.planner, self.runner)
                        .with_ownership(ownership)
                        .execute_to_sra_valid(store, id, sra_root, &request.log_root, &stop)
                        .map_err(|error| JobError::Stage(error.to_string()))?;
                }
                Stage::Conversion => {
                    FasterqConversionExecutor::new(self.planner, self.runner)
                        .with_ownership(ownership)
                        .execute_to_fastq_ready(store, id, request, &stop)
                        .map_err(|error| JobError::Stage(error.to_string()))?;
                }
                Stage::Finalization => {
                    FastqFinalizationExecutor
                        .execute_with_ownership(store, id, &stop, Some(ownership))
                        .map_err(|error| JobError::Stage(error.to_string()))?;
                }
            }
            let after = store.get_run(id)?.expect("stage retains run");
            if matches!(
                after.state,
                RunState::Failed | RunState::Paused | RunState::PausedAtBoundary
            ) {
                snapshot(&after, emit);
                return Ok(());
            }
        }
    }
}

enum Stage {
    Acquisition,
    Conversion,
    Finalization,
}

fn dispatch(run: &RunRecord) -> Result<Stage, JobError> {
    match run.state {
        RunState::Ready | RunState::Paused | RunState::WaitingForNetwork | RunState::Downloaded => {
            Ok(Stage::Acquisition)
        }
        RunState::SraValid => Ok(Stage::Conversion),
        RunState::FastqReady | RunState::Compressing | RunState::Checksumming => {
            Ok(Stage::Finalization)
        }
        RunState::Failed | RunState::PausedAtBoundary => match run.last_checkpoint.as_deref() {
            Some("prefetch" | "vdb-validate") if run.state == RunState::Failed => {
                Ok(Stage::Acquisition)
            }
            Some("fasterq-dump") => Ok(Stage::Conversion),
            Some("gzip" | "sha256") => Ok(Stage::Finalization),
            _ => Err(JobError::Invalid(
                "unknown persisted stage; no automatic reset is allowed".into(),
            )),
        },
        state => Err(JobError::Invalid(format!(
            "unsupported coordinator state: {state}"
        ))),
    }
}

fn validate_plan(job: &JobRecord, runs: &[RunRecord]) -> Result<u32, JobError> {
    let invalid = || {
        JobError::Invalid("invalid/inconsistent persisted destination plan; Inspect the job".into())
    };
    let plan: Value = serde_json::from_str(&job.settings_snapshot).map_err(|_| invalid())?;
    if plan["format_version"] != 1 || !Path::new(&job.output_root).is_absolute() {
        return Err(invalid());
    }
    let threads = plan["threads"]
        .as_u64()
        .filter(|t| (1..=256).contains(t))
        .ok_or_else(invalid)? as u32;
    let expected = plan["accessions"].as_array().ok_or_else(invalid)?;
    let expected: HashSet<&str> = expected
        .iter()
        .map(|v| v.as_str().ok_or_else(invalid))
        .collect::<Result<_, _>>()?;
    let observed: HashSet<&str> = runs
        .iter()
        .map(|r| r.accession_or_source.as_str())
        .collect();
    if expected.is_empty() || expected != observed || expected.len() != runs.len() {
        return Err(invalid());
    }
    let root = Path::new(&job.output_root);
    for run in runs {
        let accession = Accession::parse(&run.accession_or_source).map_err(|_| invalid())?;
        if accession.level() != AccessionLevel::Run
            || run.id.as_str() != format!("{}:{}", job.id, accession)
        {
            return Err(invalid());
        }
        if let Some(path) = &run.sra_path {
            if Path::new(path) != root.join("sra").join(accession.as_str()) {
                return Err(invalid());
            }
        }
        let paths: Vec<PathBuf> = serde_json::from_str(&run.fastq_paths).map_err(|_| invalid())?;
        if paths
            .iter()
            .any(|p| p.parent() != Some(root.join("fastq").join(accession.as_str()).as_path()))
        {
            return Err(invalid());
        }
    }
    Ok(threads)
}

fn snapshot(run: &RunRecord, emit: &mut impl FnMut(JobEvent)) {
    emit(JobEvent::RunSnapshot {
        run: run.clone(),
        progress: ByteProgress {
            observed_bytes: run.downloaded_bytes,
            validated_total_bytes: None,
        },
    });
}

fn finish(
    store: &StateStore,
    id: &JobId,
    errors: Vec<(RunId, String)>,
    emit: &mut impl FnMut(JobEvent),
) -> Result<JobReport, JobError> {
    let runs = store.list_job_runs(id)?;
    let state = if !errors.is_empty() || runs.iter().any(|r| r.state == RunState::Failed) {
        JobState::Failed
    } else if runs.iter().all(|r| r.state == RunState::Complete) {
        JobState::Complete
    } else if runs.iter().all(|r| r.state.is_terminal()) {
        JobState::Cancelled
    } else {
        JobState::Paused
    };
    let last_error = errors
        .first()
        .map(|(_, message)| message.as_str())
        .or_else(|| runs.iter().find_map(|r| r.last_error.as_deref()));
    store.update_job_state(id, state, last_error)?;
    emit(JobEvent::JobFinished(state));
    Ok(JobReport {
        state,
        runs,
        errors,
    })
}
