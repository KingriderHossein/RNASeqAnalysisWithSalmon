//! Presentation/session adapter. Workflow transitions and ownership remain in core.
use pipeline_core::{
    coordinator::{
        cancel_job, create_job, terminal_job_report, ControlIntent, DriveMode, JobCoordinator,
        JobEvent, JobStage,
    },
    Accession, ArtifactKind, CommandRunner, JobId, JobRecord, JobState, RunRecord, RunState,
    SraToolkitPlanner, StateStore, StorageInspector, StoragePlan, SystemCommandRunner,
    ToolRegistry,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Default)]
struct SessionState {
    database: Option<PathBuf>,
    job_id: Option<JobId>,
    snapshot: Value,
    active: bool,
    revision: u64,
}

#[derive(Clone, Default)]
pub struct DesktopBridge {
    state: Arc<Mutex<SessionState>>,
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl DesktopBridge {
    fn lock(&self) -> Result<MutexGuard<'_, SessionState>, String> {
        self.state.lock().map_err(|_| {
            "desktop session is unavailable; restart and inspect the persisted job".into()
        })
    }

    pub fn active(&self) -> bool {
        self.state.lock().map(|s| s.active).unwrap_or(true)
    }

    pub fn snapshot(&self) -> Result<Value, String> {
        Ok(self.lock()?.snapshot.clone())
    }

    pub fn interrupted(&self, message: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot["worker_error"] = json!(message);
            state.snapshot["stale"] = json!(true);
            publish(&mut state);
        }
    }

    pub fn create(
        &self,
        workspace: &Path,
        id: &str,
        runs: &str,
        threads: u32,
    ) -> Result<Value, String> {
        let mut state = self.lock()?;
        if state.active {
            return Err("a job is running; Pause or Cancel it before creating another".into());
        }
        let accessions = runs
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(Accession::parse)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        StorageInspector::inspect(workspace, StoragePlan::new(0, 0, 0), true).map_err(error)?;
        let workspace = fs::canonicalize(workspace).map_err(error)?;
        let database = workspace.join("module-a.sqlite");
        let mut store = StateStore::open(&database).map_err(error)?;
        let job = create_job(
            &mut store,
            JobId::new(id).map_err(error)?,
            &workspace,
            threads,
            &accessions,
        )
        .map_err(error)?;
        state.snapshot = persisted_snapshot(&store, &database, &job)?;
        state.database = Some(database);
        state.job_id = Some(job.id);
        publish(&mut state);
        Ok(state.snapshot.clone())
    }

    pub fn list_jobs(database: &Path) -> Result<Value, String> {
        let store = StateStore::open_existing(database).map_err(error)?;
        Ok(json!(store
            .list_jobs()
            .map_err(error)?
            .iter()
            .map(|job| json!({"id": job.id.as_str(), "state": job.state.to_string()}))
            .collect::<Vec<_>>()))
    }

    pub fn open(&self, database: &Path, id: &str) -> Result<Value, String> {
        let database = fs::canonicalize(database).map_err(error)?;
        let id = JobId::new(id).map_err(error)?;
        let mut state = self.lock()?;
        if state.active {
            if state.database.as_ref() == Some(&database) && state.job_id.as_ref() == Some(&id) {
                return Ok(state.snapshot.clone());
            }
            return Err(
                "a job is running in this window; Pause or Cancel it before opening another".into(),
            );
        }
        let store = StateStore::open_existing(&database).map_err(error)?;
        let job = store
            .get_job(&id)
            .map_err(error)?
            .ok_or_else(|| format!("job not found: {id}"))?;
        state.snapshot = persisted_snapshot(&store, &database, &job)?;
        state.database = Some(database);
        state.job_id = Some(id);
        publish(&mut state);
        Ok(state.snapshot.clone())
    }

    /// Reserve synchronously before spawning, so double Start cannot launch workers.
    pub fn begin(&self, mode: &str) -> Result<WorkerSession, String> {
        let mode = match mode {
            "start" => DriveMode::Start,
            "resume" => DriveMode::Resume,
            "retry" => DriveMode::Retry,
            _ => return Err("unknown job action".into()),
        };
        let mut state = self.lock()?;
        if state.active {
            return Err("this window already owns an active worker; use Pause or Cancel".into());
        }
        let database = state.database.clone().ok_or("open or create a job first")?;
        let job_id = state.job_id.clone().ok_or("open or create a job first")?;
        state.active = true;
        state.snapshot["active"] = json!(true);
        state.snapshot["worker_error"] = Value::Null;
        publish(&mut state);
        Ok(WorkerSession {
            bridge: self.clone(),
            database,
            job_id,
            mode,
        })
    }

    pub fn control(&self, intent: &str) -> Result<Value, String> {
        let intent = match intent {
            "pause" => ControlIntent::Pause,
            "cancel" => ControlIntent::Cancel,
            _ => return Err("unknown control request".into()),
        };
        let mut state = self.lock()?;
        let root = state.snapshot["job"]["output_root"]
            .as_str()
            .ok_or("open a job first")?;
        pipeline_core::coordinator::JobControl::open(root)
            .and_then(|c| c.request(intent))
            .map_err(error)?;
        state.snapshot["pending_control"] = json!(if intent == ControlIntent::Pause {
            "pause"
        } else {
            "cancel"
        });
        if !state.active && intent == ControlIntent::Cancel {
            let database = state.database.as_ref().ok_or("open a job first")?;
            let id = state.job_id.as_ref().ok_or("open a job first")?;
            let result = (|| {
                let mut store = StateStore::open_existing(database).map_err(error)?;
                cancel_job(&mut store, id, |_| {}).map_err(error)?;
                let job = store.get_job(id).map_err(error)?.ok_or("job disappeared")?;
                persisted_snapshot(&store, database, &job)
            })();
            match result {
                Ok(snapshot) => state.snapshot = snapshot,
                Err(message) => state.snapshot["worker_error"] = json!(message),
            }
        }
        publish(&mut state);
        Ok(state.snapshot.clone())
    }

    fn event(&self, event: JobEvent) -> Result<Value, String> {
        let mut state = self.lock()?;
        match event {
            JobEvent::RunSnapshot { run, .. } => {
                let rows = state.snapshot["runs"]
                    .as_array_mut()
                    .ok_or("missing job snapshot")?;
                if let Some(row) = rows.iter_mut().find(|r| r["id"] == run.id.as_str()) {
                    *row = run_value(&run);
                }
                state.snapshot["activity"] = Value::Null;
            }
            JobEvent::StageStarting { run_id, stage } => {
                let stage = match stage {
                    JobStage::Acquisition => "Acquiring reads",
                    JobStage::Conversion => "Converting FASTQ",
                    JobStage::Finalization => "Compressing and verifying",
                };
                state.snapshot["activity"] = json!({"run_id": run_id.as_str(), "label": stage});
            }
            JobEvent::RunError { message, .. } => {
                state.snapshot["worker_error"] = json!(message);
                state.snapshot["activity"] = Value::Null;
            }
            JobEvent::ControlApplied(_) => {}
            JobEvent::JobFinished(job_state) => {
                state.snapshot["job"]["state"] = json!(job_state.to_string());
                state.snapshot["activity"] = Value::Null;
            }
        }
        publish(&mut state);
        Ok(state.snapshot.clone())
    }

    pub fn inspect_storage(
        destination: &Path,
        recommendation: Option<u64>,
    ) -> Result<Value, String> {
        let result = StorageInspector::inspect(
            destination,
            StoragePlan::new(0, 0, recommendation.unwrap_or(0)),
            true,
        )
        .map_err(error)?;
        Ok(
            json!({"destination": result.destination, "available_bytes": result.available_bytes.to_string(),
            "download_estimate_bytes": null, "persistent_estimate_bytes": null,
            "recommended_peak_bytes": recommendation.map(|v| v.to_string()),
            "can_continue": result.can_continue(), "warning": result.has_space_warning()}),
        )
    }

    /// Only tails regular logs belonging to the selected job/run, never an arbitrary path.
    pub fn log_tail(&self, accession: &str, stream: &str) -> Result<String, String> {
        if !matches!(stream, "stdout" | "stderr") {
            return Err("unknown log stream".into());
        }
        let snapshot = self.snapshot()?;
        if !snapshot["runs"]
            .as_array()
            .ok_or("open a job first")?
            .iter()
            .any(|r| r["accession"] == accession)
        {
            return Err("run does not belong to this job".into());
        }
        let root = Path::new(
            snapshot["job"]["output_root"]
                .as_str()
                .ok_or("open a job first")?,
        )
        .join("logs");
        let root = fs::canonicalize(root).map_err(error)?;
        let prefix = format!("{accession}-attempt-");
        let suffix = format!(".{stream}.log");
        let mut logs = Vec::new();
        for entry in fs::read_dir(&root).map_err(error)? {
            let entry = entry.map_err(error)?;
            if !entry.file_type().map_err(error)?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(rest) = name
                .strip_prefix(&prefix)
                .and_then(|s| s.strip_suffix(&suffix))
            else {
                continue;
            };
            let Some(attempt) = rest.split('-').next().and_then(|s| s.parse::<u64>().ok()) else {
                continue;
            };
            let path = fs::canonicalize(entry.path()).map_err(error)?;
            if path.parent() == Some(root.as_path()) {
                logs.push((attempt, name.to_owned(), path));
            }
        }
        logs.sort();
        let Some((_, _, path)) = logs.last() else {
            return Ok("No log for this stream yet.".into());
        };
        let mut file = fs::File::open(path).map_err(error)?;
        let size = file.metadata().map_err(error)?.len();
        file.seek(SeekFrom::Start(size.saturating_sub(64 * 1024)))
            .map_err(error)?;
        let mut bytes = Vec::new();
        file.take(64 * 1024)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        Ok(format!(
            "{} (last 64 KiB at most)\n\n{}",
            path.display(),
            String::from_utf8_lossy(&bytes)
        ))
    }
}

pub struct WorkerSession {
    bridge: DesktopBridge,
    database: PathBuf,
    job_id: JobId,
    mode: DriveMode,
}

impl Drop for WorkerSession {
    fn drop(&mut self) {
        if let Ok(mut state) = self.bridge.state.lock() {
            state.active = false;
            state.snapshot["active"] = json!(false);
            state.snapshot["activity"] = Value::Null;
            publish(&mut state);
        }
    }
}

impl WorkerSession {
    pub fn execute_system(self, emit: impl FnMut(Value)) -> Result<(), String> {
        self.execute_with(
            || {
                ToolRegistry::discover_sra_toolkit()
                    .map(SraToolkitPlanner::new)
                    .map_err(error)
            },
            &SystemCommandRunner::default(),
            emit,
        )
    }

    pub fn execute_with<R: CommandRunner>(
        self,
        planner: impl FnOnce() -> Result<SraToolkitPlanner, String>,
        runner: &R,
        mut emit: impl FnMut(Value),
    ) -> Result<(), String> {
        let result = (|| {
            let mut store = StateStore::open_existing(&self.database).map_err(error)?;
            let root = store
                .get_job(&self.job_id)
                .map_err(error)?
                .ok_or("job disappeared")?
                .output_root;
            let control = pipeline_core::coordinator::JobControl::open(&root).map_err(error)?;
            if control.intent().map_err(error)? == ControlIntent::Cancel {
                cancel_job(&mut store, &self.job_id, |event| {
                    if let Ok(snapshot) = self.bridge.event(event) {
                        emit(snapshot);
                    }
                })
                .map_err(error)?;
            } else if terminal_job_report(&store, &self.job_id)
                .map_err(error)?
                .is_none()
            {
                let planner = planner()?;
                JobCoordinator::new(&planner, runner)
                    .drive(&mut store, &self.job_id, self.mode, |event| {
                        if let Ok(snapshot) = self.bridge.event(event) {
                            emit(snapshot);
                        }
                    })
                    .map_err(error)?;
            }
            let job = store
                .get_job(&self.job_id)
                .map_err(error)?
                .ok_or("job disappeared")?;
            let mut state = self.bridge.lock()?;
            state.snapshot = persisted_snapshot(&store, &self.database, &job)?;
            state.snapshot["active"] = json!(true);
            publish(&mut state);
            Ok(())
        })();
        if let Err(message) = &result {
            let mut state = self.bridge.lock()?;
            state.snapshot["worker_error"] = json!(message);
            state.snapshot["stale"] = json!(true);
            publish(&mut state);
        }
        // self drops after the store/leases, before the native host emits final state.
        result
    }
}

fn run_value(run: &RunRecord) -> Value {
    json!({"id": run.id.as_str(), "accession": run.accession_or_source, "state": run.state.to_string(),
        "observed_bytes": run.downloaded_bytes.to_string(), "total_bytes": null,
        "attempts": run.attempt_count, "checkpoint": run.last_checkpoint, "error": run.last_error,
        "sra_path": run.sra_path, "fastq_paths": run.fastq_paths})
}

fn publish(state: &mut SessionState) {
    state.revision = state.revision.saturating_add(1);
    state.snapshot["revision"] = json!(state.revision.to_string());
    let states = state.snapshot["runs"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row["state"].as_str()?.parse::<RunState>().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let has_work = states.iter().any(|s| !s.is_terminal());
    let ambiguous = states.iter().any(|s| {
        matches!(
            s,
            RunState::Downloading | RunState::Validating | RunState::Converting
        )
    });
    let stale = state.snapshot["stale"].as_bool().unwrap_or(false);
    let blocked = !state.active && (ambiguous || stale);
    let failed = states.contains(&RunState::Failed);
    let resumable = states
        .iter()
        .any(|s| !s.is_terminal() && *s != RunState::Failed);
    let paused = state.snapshot["job"]["state"] == JobState::Paused.to_string();
    let queued = state.snapshot["job"]["state"] == JobState::Queued.to_string();
    state.snapshot["actions"] = json!({
        "primary": !state.active && !blocked && has_work && resumable,
        "primary_mode": if queued { "start" } else { "resume" },
        "primary_label": if queued { "Start job" } else if paused { "Resume job" } else { "Continue unfinished runs" },
        "pause": state.active, "cancel": has_work && !blocked,
        "retry": !state.active && !blocked && failed, "refresh": !state.active,
    });
    state.snapshot["blocked_reason"] = if !state.active && stale {
        json!("The last displayed checkpoint may be stale. Refresh checkpoints before continuing; in-flight tools remain blocked until their lifetime is reconciled.")
    } else if blocked {
        json!("An earlier tool may still be writing. Automatic recovery is blocked until its process lifetime is reconciled. Open the logs; do not reset checkpoints or remove ownership files.")
    } else {
        Value::Null
    };
}

fn persisted_snapshot(
    store: &StateStore,
    database: &Path,
    job: &JobRecord,
) -> Result<Value, String> {
    let runs = store.list_job_runs(&job.id).map_err(error)?;
    let mut artifacts = Vec::new();
    for run in &runs {
        for kind in [
            ArtifactKind::Fastq,
            ArtifactKind::CompressedFastq,
            ArtifactKind::Checksum,
        ] {
            for artifact in store.list_artifacts_by_kind(&run.id, kind).map_err(error)? {
                artifacts.push(json!({"run_id": run.id.as_str(), "kind": kind.to_string(), "path": artifact.path,
                    "size_bytes": artifact.size_bytes.map(|s| s.to_string()), "sha256": artifact.sha256}));
            }
        }
    }
    let intent = pipeline_core::coordinator::JobControl::open(&job.output_root)
        .and_then(|c| c.intent())
        .map_err(error)?;
    let pending = match intent {
        ControlIntent::Pause if job.state != JobState::Paused => Some("pause"),
        ControlIntent::Cancel if !runs.iter().all(|r| r.state.is_terminal()) => Some("cancel"),
        _ => None,
    };
    Ok(json!({"format_version": 1, "database": database,
        "job": {"id": job.id.as_str(), "state": job.state.to_string(), "output_root": job.output_root,
            "error": job.last_error, "settings": job.settings_snapshot, "tools": job.tool_versions_snapshot},
        "runs": runs.iter().map(run_value).collect::<Vec<_>>(), "artifacts": artifacts,
        "active": false, "activity": null, "pending_control": pending, "worker_error": null, "stale": false}))
}
