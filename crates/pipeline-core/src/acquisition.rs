use crate::ownership::{OutputOwnership, OwnershipError};
use crate::{
    Accession, CommandOutcome, CommandRunner, InputError, ProcessContext, ProcessError, RunId,
    RunRecord, RunState, SraPlanError, SraToolkitPlanner, StateStore, StopToken, StoreError,
};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

const PREFETCH_CHECKPOINT: &str = "prefetch";
const PREFETCH_COMPLETE_CHECKPOINT: &str = "prefetch-complete";
const VALIDATION_CHECKPOINT: &str = "vdb-validate";
const VALIDATION_COMPLETE_CHECKPOINT: &str = "vdb-validate-complete";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionStage {
    Prefetch,
    Validation,
}

impl fmt::Display for AcquisitionStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prefetch => f.write_str("prefetch"),
            Self::Validation => f.write_str("vdb-validate"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquisitionDisposition {
    SraValid,
    Paused {
        stage: AcquisitionStage,
    },
    Failed {
        stage: AcquisitionStage,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionResult {
    pub run: RunRecord,
    pub attempt: Option<i64>,
    pub disposition: AcquisitionDisposition,
    pub prefetch: Option<CommandOutcome>,
    pub validation: Option<CommandOutcome>,
}

#[derive(Debug)]
pub enum AcquisitionError {
    Store(StoreError),
    Ownership(OwnershipError),
    Input(InputError),
    Plan(SraPlanError),
    Process {
        stage: AcquisitionStage,
        source: ProcessError,
    },
    MissingRun(RunId),
    UnsupportedState(RunState),
    UnknownFailedCheckpoint(Option<String>),
    MissingPersistedSraPath,
    InspectDownloadedData {
        path: PathBuf,
        source: std::io::Error,
    },
    DownloadSizeOverflow {
        path: PathBuf,
        bytes: u64,
    },
}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(f),
            Self::Ownership(error) => error.fmt(f),
            Self::Input(error) => error.fmt(f),
            Self::Plan(error) => error.fmt(f),
            Self::Process { stage, source } => {
                write!(f, "{stage} process execution failed: {source}")
            }
            Self::MissingRun(id) => write!(f, "run not found: {id}"),
            Self::UnsupportedState(state) => {
                write!(f, "cannot execute SRA acquisition from state {state}")
            }
            Self::UnknownFailedCheckpoint(checkpoint) => {
                write!(
                    f,
                    "cannot determine retry stage from failed checkpoint {:?}",
                    checkpoint
                )
            }
            Self::MissingPersistedSraPath => {
                f.write_str("downloaded run is missing its persisted SRA accession-directory path")
            }
            Self::InspectDownloadedData { path, source } => {
                write!(
                    f,
                    "cannot inspect downloaded accession directory {}: {source}",
                    path.display()
                )
            }
            Self::DownloadSizeOverflow { path, bytes } => {
                write!(
                    f,
                    "downloaded accession directory {} is too large to persist as i64 bytes: {bytes}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for AcquisitionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Ownership(error) => Some(error),
            Self::Input(error) => Some(error),
            Self::Plan(error) => Some(error),
            Self::Process { source, .. } => Some(source),
            Self::InspectDownloadedData { source, .. } => Some(source),
            Self::MissingRun(_)
            | Self::UnsupportedState(_)
            | Self::UnknownFailedCheckpoint(_)
            | Self::MissingPersistedSraPath
            | Self::DownloadSizeOverflow { .. } => None,
        }
    }
}

impl From<StoreError> for AcquisitionError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

impl From<InputError> for AcquisitionError {
    fn from(value: InputError) -> Self {
        Self::Input(value)
    }
}

impl From<SraPlanError> for AcquisitionError {
    fn from(value: SraPlanError) -> Self {
        Self::Plan(value)
    }
}

#[derive(Clone, Copy)]
struct AttemptContext<'a> {
    attempt: i64,
    log_root: &'a Path,
    stop: &'a StopToken,
}

pub struct SraAcquisitionExecutor<'a, R: CommandRunner> {
    planner: &'a SraToolkitPlanner,
    runner: &'a R,
    ownership: Option<&'a OutputOwnership>,
}

impl<'a, R: CommandRunner> SraAcquisitionExecutor<'a, R> {
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

    pub fn execute_to_sra_valid(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        output_root: impl AsRef<Path>,
        log_root: impl AsRef<Path>,
        stop: &StopToken,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        let current = store
            .get_run(run_id)?
            .ok_or_else(|| AcquisitionError::MissingRun(run_id.clone()))?;

        if current.state == RunState::SraValid {
            return Ok(AcquisitionResult {
                run: current,
                attempt: None,
                disposition: AcquisitionDisposition::SraValid,
                prefetch: None,
                validation: None,
            });
        }

        let mut roots = vec![output_root.as_ref(), log_root.as_ref()];
        if let Some(parent) = current
            .sra_path
            .as_deref()
            .and_then(|p| Path::new(p).parent())
        {
            roots.push(parent);
        }
        let _ownership = OutputOwnership::ensure_or_acquire(&roots, self.ownership)
            .map_err(AcquisitionError::Ownership)?;

        let accession = Accession::parse(&current.accession_or_source)?;

        match current.state {
            RunState::Ready | RunState::Paused | RunState::WaitingForNetwork => {
                let plan = self.planner.prefetch(&accession, output_root.as_ref())?;
                let attempt = store.begin_run_attempt(run_id)?;
                store.transition_run(
                    run_id,
                    RunState::Downloading,
                    Some(PREFETCH_CHECKPOINT),
                    None,
                )?;
                self.run_prefetch_then_validate(
                    store,
                    run_id,
                    &accession,
                    plan,
                    AttemptContext {
                        attempt,
                        log_root: log_root.as_ref(),
                        stop,
                    },
                )
            }
            RunState::Downloaded => {
                let attempt = store.begin_run_attempt(run_id)?;
                self.run_validation(
                    store,
                    run_id,
                    AttemptContext {
                        attempt,
                        log_root: log_root.as_ref(),
                        stop,
                    },
                    None,
                )
            }
            RunState::Failed => match current.last_checkpoint.as_deref() {
                Some(PREFETCH_CHECKPOINT) => {
                    let plan = self.planner.prefetch(&accession, output_root.as_ref())?;
                    let attempt = store.begin_run_attempt(run_id)?;
                    store.retry_run(run_id, RunState::Downloading, Some(PREFETCH_CHECKPOINT))?;
                    self.run_prefetch_then_validate(
                        store,
                        run_id,
                        &accession,
                        plan,
                        AttemptContext {
                            attempt,
                            log_root: log_root.as_ref(),
                            stop,
                        },
                    )
                }
                Some(VALIDATION_CHECKPOINT) => {
                    let attempt = store.begin_run_attempt(run_id)?;
                    store.retry_run(run_id, RunState::Validating, Some(VALIDATION_CHECKPOINT))?;
                    self.run_validation(
                        store,
                        run_id,
                        AttemptContext {
                            attempt,
                            log_root: log_root.as_ref(),
                            stop,
                        },
                        None,
                    )
                }
                other => Err(AcquisitionError::UnknownFailedCheckpoint(
                    other.map(str::to_owned),
                )),
            },
            state => Err(AcquisitionError::UnsupportedState(state)),
        }
    }

    fn run_prefetch_then_validate(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        accession: &Accession,
        plan: crate::PrefetchPlan,
        attempt_context: AttemptContext<'_>,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        let context = self.process_context(
            store,
            run_id,
            accession,
            attempt_context.attempt,
            AcquisitionStage::Prefetch,
            attempt_context.log_root,
        )?;
        let outcome = match self
            .runner
            .run(&plan.command, &context, attempt_context.stop)
        {
            Ok(outcome) => outcome,
            Err(source) => {
                self.persist_process_error(
                    store,
                    run_id,
                    AcquisitionStage::Prefetch,
                    PREFETCH_CHECKPOINT,
                    &source,
                )?;
                return Err(AcquisitionError::Process {
                    stage: AcquisitionStage::Prefetch,
                    source,
                });
            }
        };

        if outcome.stopped_by_request {
            let run =
                store.transition_run(run_id, RunState::Paused, Some(PREFETCH_CHECKPOINT), None)?;
            return Ok(AcquisitionResult {
                run,
                attempt: Some(attempt_context.attempt),
                disposition: AcquisitionDisposition::Paused {
                    stage: AcquisitionStage::Prefetch,
                },
                prefetch: Some(outcome),
                validation: None,
            });
        }

        if !outcome.success {
            let reason = format!(
                "prefetch exited unsuccessfully with exit code {:?}",
                outcome.exit_code
            );
            let run = store.transition_run(
                run_id,
                RunState::Failed,
                Some(PREFETCH_CHECKPOINT),
                Some(&reason),
            )?;
            return Ok(AcquisitionResult {
                run,
                attempt: Some(attempt_context.attempt),
                disposition: AcquisitionDisposition::Failed {
                    stage: AcquisitionStage::Prefetch,
                    reason,
                },
                prefetch: Some(outcome),
                validation: None,
            });
        }

        if !plan.accession_directory.is_dir() {
            let reason = format!(
                "prefetch reported success but expected accession directory {} was not created",
                plan.accession_directory.display()
            );
            let run = store.transition_run(
                run_id,
                RunState::Failed,
                Some(PREFETCH_CHECKPOINT),
                Some(&reason),
            )?;
            return Ok(AcquisitionResult {
                run,
                attempt: Some(attempt_context.attempt),
                disposition: AcquisitionDisposition::Failed {
                    stage: AcquisitionStage::Prefetch,
                    reason,
                },
                prefetch: Some(outcome),
                validation: None,
            });
        }

        let downloaded_bytes = directory_size(&plan.accession_directory)?;
        let downloaded_bytes_i64 = i64::try_from(downloaded_bytes).map_err(|_| {
            AcquisitionError::DownloadSizeOverflow {
                path: plan.accession_directory.clone(),
                bytes: downloaded_bytes,
            }
        })?;
        let sra_path = plan.accession_directory.to_string_lossy().into_owned();
        store.update_run_download_snapshot(run_id, downloaded_bytes_i64, &sra_path)?;
        store.transition_run(
            run_id,
            RunState::Downloaded,
            Some(PREFETCH_COMPLETE_CHECKPOINT),
            None,
        )?;

        self.run_validation(store, run_id, attempt_context, Some(outcome))
    }

    fn run_validation(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        attempt_context: AttemptContext<'_>,
        prefetch_outcome: Option<CommandOutcome>,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        let current = store
            .get_run(run_id)?
            .ok_or_else(|| AcquisitionError::MissingRun(run_id.clone()))?;

        if current.state == RunState::Downloaded {
            store.transition_run(
                run_id,
                RunState::Validating,
                Some(VALIDATION_CHECKPOINT),
                None,
            )?;
        } else if current.state != RunState::Validating {
            return Err(AcquisitionError::UnsupportedState(current.state));
        }

        let sra_path = match current.sra_path.as_deref() {
            Some(path) => path,
            None => {
                let reason = "downloaded run is missing its persisted SRA accession-directory path";
                store.transition_run(
                    run_id,
                    RunState::Failed,
                    Some(VALIDATION_CHECKPOINT),
                    Some(reason),
                )?;
                return Err(AcquisitionError::MissingPersistedSraPath);
            }
        };
        let plan = match self.planner.validate(sra_path) {
            Ok(plan) => plan,
            Err(error) => {
                let reason = format!("cannot plan vdb-validate: {error}");
                store.transition_run(
                    run_id,
                    RunState::Failed,
                    Some(VALIDATION_CHECKPOINT),
                    Some(&reason),
                )?;
                return Err(AcquisitionError::Plan(error));
            }
        };
        let accession = match Accession::parse(&current.accession_or_source) {
            Ok(accession) => accession,
            Err(error) => {
                let reason = format!("cannot parse persisted run accession: {error}");
                store.transition_run(
                    run_id,
                    RunState::Failed,
                    Some(VALIDATION_CHECKPOINT),
                    Some(&reason),
                )?;
                return Err(AcquisitionError::Input(error));
            }
        };
        let context = self.process_context(
            store,
            run_id,
            &accession,
            attempt_context.attempt,
            AcquisitionStage::Validation,
            attempt_context.log_root,
        )?;
        let outcome = match self
            .runner
            .run(&plan.command, &context, attempt_context.stop)
        {
            Ok(outcome) => outcome,
            Err(source) => {
                self.persist_process_error(
                    store,
                    run_id,
                    AcquisitionStage::Validation,
                    VALIDATION_CHECKPOINT,
                    &source,
                )?;
                return Err(AcquisitionError::Process {
                    stage: AcquisitionStage::Validation,
                    source,
                });
            }
        };

        if !outcome.success || outcome.stopped_by_request {
            let reason = if outcome.stopped_by_request {
                "vdb-validate was stopped; validation can be retried from the downloaded SRA"
                    .to_owned()
            } else {
                format!(
                    "vdb-validate exited unsuccessfully with exit code {:?}",
                    outcome.exit_code
                )
            };
            let run = store.transition_run(
                run_id,
                RunState::Failed,
                Some(VALIDATION_CHECKPOINT),
                Some(&reason),
            )?;
            return Ok(AcquisitionResult {
                run,
                attempt: Some(attempt_context.attempt),
                disposition: AcquisitionDisposition::Failed {
                    stage: AcquisitionStage::Validation,
                    reason,
                },
                prefetch: prefetch_outcome,
                validation: Some(outcome),
            });
        }

        let run = store.transition_run(
            run_id,
            RunState::SraValid,
            Some(VALIDATION_COMPLETE_CHECKPOINT),
            None,
        )?;
        Ok(AcquisitionResult {
            run,
            attempt: Some(attempt_context.attempt),
            disposition: AcquisitionDisposition::SraValid,
            prefetch: prefetch_outcome,
            validation: Some(outcome),
        })
    }

    fn process_context(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        accession: &Accession,
        attempt: i64,
        stage: AcquisitionStage,
        log_root: &Path,
    ) -> Result<ProcessContext, AcquisitionError> {
        let prefix = format!("{}-attempt-{attempt}-{stage}", accession.as_str());
        match ProcessContext::new(log_root, prefix) {
            Ok(context) => Ok(context),
            Err(source) => {
                let checkpoint = match stage {
                    AcquisitionStage::Prefetch => PREFETCH_CHECKPOINT,
                    AcquisitionStage::Validation => VALIDATION_CHECKPOINT,
                };
                self.persist_process_error(store, run_id, stage, checkpoint, &source)?;
                Err(AcquisitionError::Process { stage, source })
            }
        }
    }

    fn persist_process_error(
        &self,
        store: &mut StateStore,
        run_id: &RunId,
        stage: AcquisitionStage,
        checkpoint: &'static str,
        source: &ProcessError,
    ) -> Result<(), AcquisitionError> {
        let reason = format!("{stage} process execution failed: {source}");
        store.transition_run(run_id, RunState::Failed, Some(checkpoint), Some(&reason))?;
        Ok(())
    }
}

fn directory_size(path: &Path) -> Result<u64, AcquisitionError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| AcquisitionError::InspectDownloadedData {
            path: path.to_path_buf(),
            source,
        })?;

    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut total = 0_u64;
    let entries = fs::read_dir(path).map_err(|source| AcquisitionError::InspectDownloadedData {
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| AcquisitionError::InspectDownloadedData {
            path: path.to_path_buf(),
            source,
        })?;
        total = total.saturating_add(directory_size(&entry.path())?);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandSpec, JobId, NewJob, NewRun, ToolInfo, ToolKind, ToolRegistry};
    use std::{
        collections::VecDeque,
        ffi::OsString,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug)]
    struct FakeRunner {
        outcomes: Mutex<VecDeque<CommandOutcome>>,
        seen: Mutex<Vec<(CommandSpec, ProcessContext)>>,
    }

    impl FakeRunner {
        fn new(outcomes: Vec<CommandOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into()),
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
            Ok(self
                .outcomes
                .lock()
                .expect("outcomes lock")
                .pop_front()
                .expect("fake outcome"))
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

    fn outcome(label: &str, success: bool, stopped_by_request: bool) -> CommandOutcome {
        CommandOutcome {
            exit_code: if success { Some(0) } else { Some(1) },
            success,
            stopped_by_request,
            stdout_log: PathBuf::from(format!("{label}.stdout.log")),
            stderr_log: PathBuf::from(format!("{label}.stderr.log")),
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rnaseq-acquisition-test-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn ready_store() -> (StateStore, RunId) {
        let mut store = StateStore::open_in_memory().expect("store");
        let job_id = JobId::new("job-1").expect("job id");
        let run_id = RunId::new("run-1").expect("run id");
        store
            .create_job(NewJob {
                id: job_id.clone(),
                input_type: "single_accession".to_owned(),
                input_identity: "SRR000001".to_owned(),
                output_root: "/tmp/rnaseq".to_owned(),
                settings_snapshot: "{}".to_owned(),
                tool_versions_snapshot: "{}".to_owned(),
            })
            .expect("create job");
        store
            .create_run(NewRun {
                id: run_id.clone(),
                job_id,
                accession_or_source: "SRR000001".to_owned(),
            })
            .expect("create run");
        store
            .transition_run(&run_id, RunState::Resolving, Some("resolve"), None)
            .expect("resolving");
        store
            .transition_run(&run_id, RunState::Ready, Some("resolve-complete"), None)
            .expect("ready");
        (store, run_id)
    }

    #[test]
    fn successful_prefetch_and_validation_reaches_sra_valid() {
        let root = temp_root("success");
        let sra_root = root.join("sra");
        let accession_dir = sra_root.join("SRR000001");
        fs::create_dir_all(&accession_dir).expect("accession directory");
        fs::write(accession_dir.join("data.bin"), b"12345").expect("fake SRA data");

        let (mut store, run_id) = ready_store();
        let runner = FakeRunner::new(vec![
            outcome("prefetch", true, false),
            outcome("validate", true, false),
        ]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = SraAcquisitionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("acquisition");

        assert_eq!(result.disposition, AcquisitionDisposition::SraValid);
        assert_eq!(result.attempt, Some(1));
        assert_eq!(result.run.state, RunState::SraValid);
        assert_eq!(result.run.downloaded_bytes, 5);
        assert_eq!(
            result.run.sra_path.as_deref(),
            Some(accession_dir.to_string_lossy().as_ref())
        );

        let seen = runner.seen();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0.program, PathBuf::from("/tools/prefetch"));
        assert_eq!(seen[1].0.program, PathBuf::from("/tools/vdb-validate"));
        assert_eq!(seen[0].1.log_prefix, "SRR000001-attempt-1-prefetch");
        assert_eq!(seen[1].1.log_prefix, "SRR000001-attempt-1-vdb-validate");

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn stopped_prefetch_becomes_paused_and_preserves_partial_directory() {
        let root = temp_root("pause");
        let sra_root = root.join("sra");
        let accession_dir = sra_root.join("SRR000001");
        fs::create_dir_all(&accession_dir).expect("accession directory");
        fs::write(accession_dir.join("partial.bin"), b"partial").expect("partial data");

        let (mut store, run_id) = ready_store();
        let runner = FakeRunner::new(vec![outcome("prefetch", false, true)]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = SraAcquisitionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("pause outcome");

        assert_eq!(
            result.disposition,
            AcquisitionDisposition::Paused {
                stage: AcquisitionStage::Prefetch
            }
        );
        assert_eq!(result.run.state, RunState::Paused);
        assert!(accession_dir.is_dir());
        assert!(accession_dir.join("partial.bin").is_file());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn paused_prefetch_resume_reuses_command_identity_with_new_attempt_logs() {
        let root = temp_root("resume");
        let sra_root = root.join("sra");
        let accession_dir = sra_root.join("SRR000001");
        fs::create_dir_all(&accession_dir).expect("accession directory");
        fs::write(accession_dir.join("data.bin"), b"123").expect("fake data");

        let (mut store, run_id) = ready_store();
        let runner = FakeRunner::new(vec![
            outcome("prefetch-stop", false, true),
            outcome("prefetch-resume", true, false),
            outcome("validate", true, false),
        ]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = SraAcquisitionExecutor::new(&planner, &runner);

        let first = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("first attempt");
        assert_eq!(first.run.state, RunState::Paused);
        assert_eq!(first.attempt, Some(1));

        let second = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("second attempt");
        assert_eq!(second.run.state, RunState::SraValid);
        assert_eq!(second.attempt, Some(2));

        let seen = runner.seen();
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].0, seen[1].0);
        assert_eq!(seen[0].1.log_prefix, "SRR000001-attempt-1-prefetch");
        assert_eq!(seen[1].1.log_prefix, "SRR000001-attempt-2-prefetch");

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn validation_failure_retries_without_prefetch() {
        let root = temp_root("validation-retry");
        let sra_root = root.join("sra");
        let accession_dir = sra_root.join("SRR000001");
        fs::create_dir_all(&accession_dir).expect("accession directory");
        fs::write(accession_dir.join("data.bin"), b"123").expect("fake data");

        let (mut store, run_id) = ready_store();
        let runner = FakeRunner::new(vec![
            outcome("prefetch", true, false),
            outcome("validate-fail", false, false),
            outcome("validate-retry", true, false),
        ]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = SraAcquisitionExecutor::new(&planner, &runner);

        let first = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("first attempt");
        assert_eq!(first.run.state, RunState::Failed);
        assert!(matches!(
            first.disposition,
            AcquisitionDisposition::Failed {
                stage: AcquisitionStage::Validation,
                ..
            }
        ));

        let second = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("validation retry");
        assert_eq!(second.run.state, RunState::SraValid);
        assert_eq!(second.attempt, Some(2));

        let seen = runner.seen();
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].0.program, PathBuf::from("/tools/prefetch"));
        assert_eq!(seen[1].0.program, PathBuf::from("/tools/vdb-validate"));
        assert_eq!(seen[2].0.program, PathBuf::from("/tools/vdb-validate"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn missing_accession_directory_never_becomes_downloaded() {
        let root = temp_root("missing-output");
        let sra_root = root.join("sra");
        fs::create_dir_all(&sra_root).expect("SRA root");

        let (mut store, run_id) = ready_store();
        let runner = FakeRunner::new(vec![outcome("prefetch", true, false)]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = SraAcquisitionExecutor::new(&planner, &runner);

        let result = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("missing output is explicit failed outcome");

        assert_eq!(result.run.state, RunState::Failed);
        assert!(matches!(
            result.disposition,
            AcquisitionDisposition::Failed {
                stage: AcquisitionStage::Prefetch,
                ..
            }
        ));
        assert!(result.validation.is_none());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn failed_prefetch_retries_same_command_with_new_attempt_identity() {
        let root = temp_root("prefetch-retry");
        let sra_root = root.join("sra");
        let accession_dir = sra_root.join("SRR000001");
        fs::create_dir_all(&accession_dir).expect("accession directory");
        fs::write(accession_dir.join("data.bin"), b"123").expect("fake data");

        let (mut store, run_id) = ready_store();
        let runner = FakeRunner::new(vec![
            outcome("prefetch-fail", false, false),
            outcome("prefetch-retry", true, false),
            outcome("validate", true, false),
        ]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = SraAcquisitionExecutor::new(&planner, &runner);

        let first = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("failed prefetch outcome");
        assert_eq!(first.run.state, RunState::Failed);

        let second = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("prefetch retry");
        assert_eq!(second.run.state, RunState::SraValid);

        let seen = runner.seen();
        assert_eq!(seen[0].0, seen[1].0);
        assert_eq!(seen[0].1.log_prefix, "SRR000001-attempt-1-prefetch");
        assert_eq!(seen[1].1.log_prefix, "SRR000001-attempt-2-prefetch");

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn sra_valid_is_idempotent_and_does_not_start_new_attempt() {
        let root = temp_root("idempotent");
        let sra_root = root.join("sra");
        let accession_dir = sra_root.join("SRR000001");
        fs::create_dir_all(&accession_dir).expect("accession directory");
        fs::write(accession_dir.join("data.bin"), b"1").expect("fake data");

        let (mut store, run_id) = ready_store();
        let runner = FakeRunner::new(vec![
            outcome("prefetch", true, false),
            outcome("validate", true, false),
        ]);
        let planner = SraToolkitPlanner::new(registry());
        let executor = SraAcquisitionExecutor::new(&planner, &runner);

        let first = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("first execution");
        assert_eq!(first.run.state, RunState::SraValid);

        let second = executor
            .execute_to_sra_valid(
                &mut store,
                &run_id,
                &sra_root,
                root.join("logs"),
                &StopToken::default(),
            )
            .expect("idempotent execution");
        assert_eq!(second.attempt, None);
        assert_eq!(second.run.attempt_count, 1);
        assert_eq!(runner.seen().len(), 2);

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn command_args_remain_os_strings() {
        let spec = CommandSpec::new("/tool", vec![OsString::from("arg")]);
        assert_eq!(spec.args, vec![OsString::from("arg")]);
    }
}
