use crate::{
    ArtifactId, IdError, JobId, JobState, RetryTransitionError, RunId, RunState, StateParseError,
    TransitionError,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    fmt,
    path::Path,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewJob {
    pub id: JobId,
    pub input_type: String,
    pub input_identity: String,
    pub output_root: String,
    pub settings_snapshot: String,
    pub tool_versions_snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    pub id: JobId,
    pub schema_version: i64,
    pub input_type: String,
    pub input_identity: String,
    pub output_root: String,
    pub state: JobState,
    pub settings_snapshot: String,
    pub tool_versions_snapshot: String,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRun {
    pub id: RunId,
    pub job_id: JobId,
    pub accession_or_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub id: RunId,
    pub job_id: JobId,
    pub accession_or_source: String,
    pub state: RunState,
    pub attempt_count: i64,
    pub downloaded_bytes: i64,
    pub source_size: Option<i64>,
    pub sra_path: Option<String>,
    pub fastq_paths: String,
    pub last_checkpoint: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Sra,
    Fastq,
    CompressedFastq,
    Checksum,
    Other,
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Sra => "SRA",
            Self::Fastq => "FASTQ",
            Self::CompressedFastq => "COMPRESSED_FASTQ",
            Self::Checksum => "CHECKSUM",
            Self::Other => "OTHER",
        };
        f.write_str(value)
    }
}

impl FromStr for ArtifactKind {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "SRA" => Ok(Self::Sra),
            "FASTQ" => Ok(Self::Fastq),
            "COMPRESSED_FASTQ" => Ok(Self::CompressedFastq),
            "CHECKSUM" => Ok(Self::Checksum),
            "OTHER" => Ok(Self::Other),
            other => Err(StoreError::InvalidStoredValue {
                field: "artifact.kind",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactValidationState {
    Pending,
    Valid,
    Invalid,
}

impl fmt::Display for ArtifactValidationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Pending => "PENDING",
            Self::Valid => "VALID",
            Self::Invalid => "INVALID",
        };
        f.write_str(value)
    }
}

impl FromStr for ArtifactValidationState {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "VALID" => Ok(Self::Valid),
            "INVALID" => Ok(Self::Invalid),
            other => Err(StoreError::InvalidStoredValue {
                field: "artifact.validation_state",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewArtifact {
    pub id: ArtifactId,
    pub run_id: RunId,
    pub kind: ArtifactKind,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRecord {
    pub id: ArtifactId,
    pub run_id: RunId,
    pub kind: ArtifactKind,
    pub path: String,
    pub size_bytes: Option<i64>,
    pub sha256: Option<String>,
    pub validation_state: ArtifactValidationState,
    pub created_at: i64,
    pub finalized_at: Option<i64>,
}

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    InvalidId(IdError),
    InvalidState(StateParseError),
    InvalidTransition(TransitionError),
    InvalidRetryTransition(RetryTransitionError),
    InvalidStoredValue { field: &'static str, value: String },
    NotFound { kind: &'static str, id: String },
    UnsupportedSchema(i64),
    Serialization(serde_json::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database error: {error}"),
            Self::InvalidId(error) => error.fmt(f),
            Self::InvalidState(error) => error.fmt(f),
            Self::InvalidTransition(error) => error.fmt(f),
            Self::InvalidRetryTransition(error) => error.fmt(f),
            Self::InvalidStoredValue { field, value } => {
                write!(f, "invalid stored value for {field}: {value}")
            }
            Self::NotFound { kind, id } => write!(f, "{kind} not found: {id}"),
            Self::UnsupportedSchema(version) => {
                write!(
                    f,
                    "database schema version {version} is newer than supported"
                )
            }
            Self::Serialization(error) => write!(f, "state serialization error: {error}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidId(error) => Some(error),
            Self::InvalidState(error) => Some(error),
            Self::InvalidTransition(error) => Some(error),
            Self::InvalidRetryTransition(error) => Some(error),
            Self::Serialization(error) => Some(error),
            Self::InvalidStoredValue { .. }
            | Self::NotFound { .. }
            | Self::UnsupportedSchema(_) => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<IdError> for StoreError {
    fn from(value: IdError) -> Self {
        Self::InvalidId(value)
    }
}

impl From<StateParseError> for StoreError {
    fn from(value: StateParseError) -> Self {
        Self::InvalidState(value)
    }
}

impl From<TransitionError> for StoreError {
    fn from(value: TransitionError) -> Self {
        Self::InvalidTransition(value)
    }
}

impl From<RetryTransitionError> for StoreError {
    fn from(value: RetryTransitionError) -> Self {
        Self::InvalidRetryTransition(value)
    }
}

pub struct StateStore {
    connection: Connection,
}

impl StateStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let mut connection = Connection::open(path)?;
        configure_connection(&connection)?;
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut connection = Connection::open_in_memory()?;
        configure_connection(&connection)?;
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn create_job(&self, job: NewJob) -> Result<JobRecord, StoreError> {
        let now = unix_timestamp();
        self.connection.execute(
            "INSERT INTO jobs (
                job_id, schema_version, input_type, input_identity, output_root,
                overall_state, settings_snapshot, tool_versions_snapshot,
                last_error, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?9)",
            params![
                job.id.as_str(),
                SCHEMA_VERSION,
                job.input_type,
                job.input_identity,
                job.output_root,
                JobState::Queued.to_string(),
                job.settings_snapshot,
                job.tool_versions_snapshot,
                now,
            ],
        )?;

        self.get_job(&job.id)?.ok_or_else(|| StoreError::NotFound {
            kind: "job",
            id: job.id.to_string(),
        })
    }

    pub fn get_job(&self, id: &JobId) -> Result<Option<JobRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT job_id, schema_version, input_type, input_identity, output_root,
                    overall_state, settings_snapshot, tool_versions_snapshot,
                    last_error, created_at, updated_at
             FROM jobs
             WHERE job_id = ?1",
        )?;
        let mut rows = statement.query(params![id.as_str()])?;

        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        let id = JobId::new(row.get::<_, String>(0)?)?;
        let state = row.get::<_, String>(5)?.parse::<JobState>()?;

        Ok(Some(JobRecord {
            id,
            schema_version: row.get(1)?,
            input_type: row.get(2)?,
            input_identity: row.get(3)?,
            output_root: row.get(4)?,
            state,
            settings_snapshot: row.get(6)?,
            tool_versions_snapshot: row.get(7)?,
            last_error: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        }))
    }

    pub fn update_job_state(
        &self,
        id: &JobId,
        state: JobState,
        last_error: Option<&str>,
    ) -> Result<(), StoreError> {
        let changed = self.connection.execute(
            "UPDATE jobs
             SET overall_state = ?2, last_error = ?3, updated_at = ?4
             WHERE job_id = ?1",
            params![id.as_str(), state.to_string(), last_error, unix_timestamp()],
        )?;

        if changed == 0 {
            return Err(StoreError::NotFound {
                kind: "job",
                id: id.to_string(),
            });
        }

        Ok(())
    }

    pub fn list_recoverable_jobs(&self) -> Result<Vec<JobRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT job_id
             FROM jobs
             WHERE overall_state NOT IN ('COMPLETE', 'CANCELLED')
             ORDER BY created_at, job_id",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        ids.into_iter()
            .map(|id| {
                let id = JobId::new(id)?;
                self.get_job(&id)?.ok_or_else(|| StoreError::NotFound {
                    kind: "job",
                    id: id.to_string(),
                })
            })
            .collect()
    }

    pub fn create_run(&self, run: NewRun) -> Result<RunRecord, StoreError> {
        let now = unix_timestamp();
        self.connection.execute(
            "INSERT INTO runs (
                run_id, job_id, accession_or_source, state, attempt_count,
                downloaded_bytes, source_size, sra_path, fastq_paths,
                last_checkpoint, last_error, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 0, 0, NULL, NULL, '[]', NULL, NULL, ?5, ?5)",
            params![
                run.id.as_str(),
                run.job_id.as_str(),
                run.accession_or_source,
                RunState::Queued.to_string(),
                now,
            ],
        )?;

        self.get_run(&run.id)?.ok_or_else(|| StoreError::NotFound {
            kind: "run",
            id: run.id.to_string(),
        })
    }

    pub fn get_run(&self, id: &RunId) -> Result<Option<RunRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT run_id, job_id, accession_or_source, state, attempt_count,
                    downloaded_bytes, source_size, sra_path, fastq_paths,
                    last_checkpoint, last_error, created_at, updated_at
             FROM runs
             WHERE run_id = ?1",
        )?;
        let mut rows = statement.query(params![id.as_str()])?;

        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        let id = RunId::new(row.get::<_, String>(0)?)?;
        let job_id = JobId::new(row.get::<_, String>(1)?)?;
        let state = row.get::<_, String>(3)?.parse::<RunState>()?;

        Ok(Some(RunRecord {
            id,
            job_id,
            accession_or_source: row.get(2)?,
            state,
            attempt_count: row.get(4)?,
            downloaded_bytes: row.get(5)?,
            source_size: row.get(6)?,
            sra_path: row.get(7)?,
            fastq_paths: row.get(8)?,
            last_checkpoint: row.get(9)?,
            last_error: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        }))
    }

    pub fn transition_run(
        &mut self,
        id: &RunId,
        next: RunState,
        last_checkpoint: Option<&str>,
        last_error: Option<&str>,
    ) -> Result<RunRecord, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let current = transaction
            .query_row(
                "SELECT state FROM runs WHERE run_id = ?1",
                params![id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound {
                kind: "run",
                id: id.to_string(),
            })?
            .parse::<RunState>()?;

        current.transition_to(next)?;

        transaction.execute(
            "UPDATE runs
             SET state = ?2, last_checkpoint = ?3, last_error = ?4, updated_at = ?5
             WHERE run_id = ?1",
            params![
                id.as_str(),
                next.to_string(),
                last_checkpoint,
                last_error,
                unix_timestamp(),
            ],
        )?;
        transaction.commit()?;

        self.get_run(id)?.ok_or_else(|| StoreError::NotFound {
            kind: "run",
            id: id.to_string(),
        })
    }

    pub fn retry_run(
        &mut self,
        id: &RunId,
        next: RunState,
        last_checkpoint: Option<&str>,
    ) -> Result<RunRecord, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let current = transaction
            .query_row(
                "SELECT state FROM runs WHERE run_id = ?1",
                params![id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound {
                kind: "run",
                id: id.to_string(),
            })?
            .parse::<RunState>()?;

        current.retry_to(next)?;

        transaction.execute(
            "UPDATE runs
             SET state = ?2, last_checkpoint = ?3, updated_at = ?4
             WHERE run_id = ?1",
            params![
                id.as_str(),
                next.to_string(),
                last_checkpoint,
                unix_timestamp(),
            ],
        )?;
        transaction.commit()?;

        self.get_run(id)?.ok_or_else(|| StoreError::NotFound {
            kind: "run",
            id: id.to_string(),
        })
    }

    pub fn begin_run_attempt(&mut self, id: &RunId) -> Result<i64, StoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let changed = transaction.execute(
            "UPDATE runs
             SET attempt_count = attempt_count + 1, updated_at = ?2
             WHERE run_id = ?1",
            params![id.as_str(), unix_timestamp()],
        )?;

        if changed == 0 {
            return Err(StoreError::NotFound {
                kind: "run",
                id: id.to_string(),
            });
        }

        let attempt_count = transaction.query_row(
            "SELECT attempt_count FROM runs WHERE run_id = ?1",
            params![id.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        transaction.commit()?;

        Ok(attempt_count)
    }

    pub fn update_run_download_snapshot(
        &self,
        id: &RunId,
        downloaded_bytes: i64,
        sra_path: &str,
    ) -> Result<(), StoreError> {
        let changed = self.connection.execute(
            "UPDATE runs
             SET downloaded_bytes = ?2,
                 sra_path = ?3,
                 updated_at = ?4
             WHERE run_id = ?1",
            params![id.as_str(), downloaded_bytes, sra_path, unix_timestamp()],
        )?;

        if changed == 0 {
            return Err(StoreError::NotFound {
                kind: "run",
                id: id.to_string(),
            });
        }

        Ok(())
    }

    pub fn record_run_attempt(
        &self,
        id: &RunId,
        downloaded_bytes: i64,
        source_size: Option<i64>,
        sra_path: Option<&str>,
        last_error: Option<&str>,
    ) -> Result<(), StoreError> {
        let changed = self.connection.execute(
            "UPDATE runs
             SET attempt_count = attempt_count + 1,
                 downloaded_bytes = ?2,
                 source_size = ?3,
                 sra_path = ?4,
                 last_error = ?5,
                 updated_at = ?6
             WHERE run_id = ?1",
            params![
                id.as_str(),
                downloaded_bytes,
                source_size,
                sra_path,
                last_error,
                unix_timestamp(),
            ],
        )?;

        if changed == 0 {
            return Err(StoreError::NotFound {
                kind: "run",
                id: id.to_string(),
            });
        }

        Ok(())
    }

    pub fn record_finalized_fastq_artifacts(
        &mut self,
        run_id: &RunId,
        artifacts: &[(ArtifactId, String, i64)],
    ) -> Result<(), StoreError> {
        let paths = artifacts
            .iter()
            .map(|(_, path, _)| path.as_str())
            .collect::<Vec<_>>();
        let fastq_paths = serde_json::to_string(&paths).map_err(StoreError::Serialization)?;
        let now = unix_timestamp();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let changed = transaction.execute(
            "UPDATE runs
             SET fastq_paths = ?2, updated_at = ?3
             WHERE run_id = ?1",
            params![run_id.as_str(), fastq_paths, now],
        )?;

        if changed == 0 {
            return Err(StoreError::NotFound {
                kind: "run",
                id: run_id.to_string(),
            });
        }

        for (artifact_id, path, size_bytes) in artifacts {
            transaction.execute(
                "INSERT INTO artifacts (
                    artifact_id, run_id, kind, path, size_bytes, sha256,
                    validation_state, created_at, finalized_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?7)",
                params![
                    artifact_id.as_str(),
                    run_id.as_str(),
                    ArtifactKind::Fastq.to_string(),
                    path,
                    size_bytes,
                    ArtifactValidationState::Pending.to_string(),
                    now,
                ],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn create_artifact(&self, artifact: NewArtifact) -> Result<ArtifactRecord, StoreError> {
        let now = unix_timestamp();
        self.connection.execute(
            "INSERT INTO artifacts (
                artifact_id, run_id, kind, path, size_bytes, sha256,
                validation_state, created_at, finalized_at
             ) VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, ?6, NULL)",
            params![
                artifact.id.as_str(),
                artifact.run_id.as_str(),
                artifact.kind.to_string(),
                artifact.path,
                ArtifactValidationState::Pending.to_string(),
                now,
            ],
        )?;

        self.get_artifact(&artifact.id)?
            .ok_or_else(|| StoreError::NotFound {
                kind: "artifact",
                id: artifact.id.to_string(),
            })
    }

    pub fn get_artifact(&self, id: &ArtifactId) -> Result<Option<ArtifactRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT artifact_id, run_id, kind, path, size_bytes, sha256,
                    validation_state, created_at, finalized_at
             FROM artifacts
             WHERE artifact_id = ?1",
        )?;
        let mut rows = statement.query(params![id.as_str()])?;

        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        Ok(Some(ArtifactRecord {
            id: ArtifactId::new(row.get::<_, String>(0)?)?,
            run_id: RunId::new(row.get::<_, String>(1)?)?,
            kind: row.get::<_, String>(2)?.parse::<ArtifactKind>()?,
            path: row.get(3)?,
            size_bytes: row.get(4)?,
            sha256: row.get(5)?,
            validation_state: row
                .get::<_, String>(6)?
                .parse::<ArtifactValidationState>()?,
            created_at: row.get(7)?,
            finalized_at: row.get(8)?,
        }))
    }

    pub fn finalize_artifact(
        &self,
        id: &ArtifactId,
        size_bytes: i64,
        sha256: Option<&str>,
        validation_state: ArtifactValidationState,
    ) -> Result<(), StoreError> {
        let changed = self.connection.execute(
            "UPDATE artifacts
             SET size_bytes = ?2, sha256 = ?3, validation_state = ?4, finalized_at = ?5
             WHERE artifact_id = ?1",
            params![
                id.as_str(),
                size_bytes,
                sha256,
                validation_state.to_string(),
                unix_timestamp(),
            ],
        )?;

        if changed == 0 {
            return Err(StoreError::NotFound {
                kind: "artifact",
                id: id.to_string(),
            });
        }

        Ok(())
    }
}

fn configure_connection(connection: &Connection) -> Result<(), StoreError> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let current: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current > SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(current));
    }

    if current == SCHEMA_VERSION {
        return Ok(());
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE jobs (
            job_id TEXT PRIMARY KEY NOT NULL,
            schema_version INTEGER NOT NULL,
            input_type TEXT NOT NULL,
            input_identity TEXT NOT NULL,
            output_root TEXT NOT NULL,
            overall_state TEXT NOT NULL,
            settings_snapshot TEXT NOT NULL,
            tool_versions_snapshot TEXT NOT NULL,
            last_error TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
         );

         CREATE TABLE runs (
            run_id TEXT PRIMARY KEY NOT NULL,
            job_id TEXT NOT NULL,
            accession_or_source TEXT NOT NULL,
            state TEXT NOT NULL,
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
            downloaded_bytes INTEGER NOT NULL DEFAULT 0 CHECK (downloaded_bytes >= 0),
            source_size INTEGER CHECK (source_size IS NULL OR source_size >= 0),
            sra_path TEXT,
            fastq_paths TEXT NOT NULL DEFAULT '[]',
            last_checkpoint TEXT,
            last_error TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(job_id, accession_or_source),
            FOREIGN KEY(job_id) REFERENCES jobs(job_id) ON DELETE CASCADE
         );

         CREATE TABLE artifacts (
            artifact_id TEXT PRIMARY KEY NOT NULL,
            run_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            path TEXT NOT NULL,
            size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
            sha256 TEXT,
            validation_state TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            finalized_at INTEGER,
            FOREIGN KEY(run_id) REFERENCES runs(run_id) ON DELETE CASCADE
         );

         CREATE INDEX idx_runs_job_id ON runs(job_id);
         CREATE INDEX idx_runs_state ON runs(state);
         CREATE INDEX idx_artifacts_run_id ON artifacts(run_id);

         PRAGMA user_version = 1;",
    )?;
    transaction.commit()?;

    Ok(())
}

fn unix_timestamp() -> i64 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    i64::try_from(seconds).unwrap_or(i64::MAX)
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

    fn sample_job() -> NewJob {
        NewJob {
            id: JobId::new("job-1").expect("valid job id"),
            input_type: "single_accession".to_owned(),
            input_identity: "SRR000001".to_owned(),
            output_root: "/tmp/rnaseq".to_owned(),
            settings_snapshot: "{}".to_owned(),
            tool_versions_snapshot: "{}".to_owned(),
        }
    }

    fn sample_run(job_id: &JobId) -> NewRun {
        NewRun {
            id: RunId::new("run-1").expect("valid run id"),
            job_id: job_id.clone(),
            accession_or_source: "SRR000001".to_owned(),
        }
    }

    fn temp_database_path() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rnaseq-pipeline-state-{}-{nonce}.sqlite",
            process::id()
        ))
    }

    #[test]
    fn persists_job_run_and_artifact_across_reopen() {
        let path = temp_database_path();
        let job = sample_job();
        let run = sample_run(&job.id);
        let artifact_id = ArtifactId::new("artifact-1").expect("valid artifact id");

        {
            let mut store = StateStore::open(&path).expect("open database");
            store.create_job(job.clone()).expect("create job");
            store.create_run(run.clone()).expect("create run");
            store
                .transition_run(&run.id, RunState::Resolving, Some("resolve"), None)
                .expect("transition run");
            store
                .create_artifact(NewArtifact {
                    id: artifact_id.clone(),
                    run_id: run.id.clone(),
                    kind: ArtifactKind::Sra,
                    path: "/tmp/rnaseq/SRR000001.sra".to_owned(),
                })
                .expect("create artifact");
            store
                .finalize_artifact(
                    &artifact_id,
                    1024,
                    Some("abc123"),
                    ArtifactValidationState::Valid,
                )
                .expect("finalize artifact");
        }

        {
            let store = StateStore::open(&path).expect("reopen database");
            let persisted_job = store
                .get_job(&job.id)
                .expect("read job")
                .expect("job exists");
            assert_eq!(persisted_job.input_identity, "SRR000001");

            let persisted_run = store
                .get_run(&run.id)
                .expect("read run")
                .expect("run exists");
            assert_eq!(persisted_run.state, RunState::Resolving);
            assert_eq!(persisted_run.last_checkpoint.as_deref(), Some("resolve"));

            let artifact = store
                .get_artifact(&artifact_id)
                .expect("read artifact")
                .expect("artifact exists");
            assert_eq!(artifact.validation_state, ArtifactValidationState::Valid);
            assert_eq!(artifact.sha256.as_deref(), Some("abc123"));

            let recoverable = store.list_recoverable_jobs().expect("recoverable jobs");
            assert_eq!(recoverable.len(), 1);
            assert_eq!(recoverable[0].id, job.id);
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_invalid_transition_without_mutating_persisted_state() {
        let mut store = StateStore::open_in_memory().expect("open database");
        let job = sample_job();
        let run = sample_run(&job.id);
        store.create_job(job).expect("create job");
        store.create_run(run.clone()).expect("create run");

        let error = store
            .transition_run(&run.id, RunState::Complete, None, None)
            .expect_err("invalid transition must fail");
        assert!(matches!(error, StoreError::InvalidTransition(_)));

        let persisted = store
            .get_run(&run.id)
            .expect("read run")
            .expect("run exists");
        assert_eq!(persisted.state, RunState::Queued);
    }

    #[test]
    fn retry_transition_requires_failed_state_and_allowed_target() {
        let mut store = StateStore::open_in_memory().expect("open database");
        let job = sample_job();
        let run = sample_run(&job.id);
        store.create_job(job).expect("create job");
        store.create_run(run.clone()).expect("create run");

        store
            .transition_run(&run.id, RunState::Resolving, Some("resolve"), None)
            .expect("resolving");
        store
            .transition_run(
                &run.id,
                RunState::Failed,
                Some("resolve"),
                Some("resolver failed"),
            )
            .expect("failed");

        let retried = store
            .retry_run(&run.id, RunState::Resolving, Some("retry-resolve"))
            .expect("retry resolving");
        assert_eq!(retried.state, RunState::Resolving);
        assert_eq!(retried.last_error.as_deref(), Some("resolver failed"));

        store
            .transition_run(
                &run.id,
                RunState::Failed,
                Some("retry-resolve"),
                Some("failed again"),
            )
            .expect("failed again");

        let error = store
            .retry_run(&run.id, RunState::Complete, None)
            .expect_err("retry to COMPLETE must fail");
        assert!(matches!(error, StoreError::InvalidRetryTransition(_)));

        let persisted = store
            .get_run(&run.id)
            .expect("read run")
            .expect("run exists");
        assert_eq!(persisted.state, RunState::Failed);
    }

    #[test]
    fn enforces_run_to_job_foreign_key() {
        let store = StateStore::open_in_memory().expect("open database");
        let missing_job = JobId::new("missing-job").expect("valid job id");
        let result = store.create_run(sample_run(&missing_job));
        assert!(matches!(result, Err(StoreError::Database(_))));
    }

    #[test]
    fn completed_jobs_are_not_recoverable() {
        let store = StateStore::open_in_memory().expect("open database");
        let job = sample_job();
        store.create_job(job.clone()).expect("create job");
        store
            .update_job_state(&job.id, JobState::Complete, None)
            .expect("complete job");

        assert!(store
            .list_recoverable_jobs()
            .expect("recoverable jobs")
            .is_empty());
    }
}
