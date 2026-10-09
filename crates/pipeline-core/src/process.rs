use std::{
    ffi::OsString,
    fmt,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl CommandSpec {
    pub fn new(program: impl Into<PathBuf>, args: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessContext {
    pub log_directory: PathBuf,
    pub log_prefix: String,
}

impl ProcessContext {
    pub fn new(
        log_directory: impl Into<PathBuf>,
        log_prefix: impl Into<String>,
    ) -> Result<Self, ProcessError> {
        let log_prefix = log_prefix.into();
        if log_prefix.is_empty()
            || !log_prefix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(ProcessError::InvalidLogPrefix(log_prefix));
        }

        Ok(Self {
            log_directory: log_directory.into(),
            log_prefix,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stopped_by_request: bool,
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
}

type StopProbe = dyn Fn() -> Result<bool, String> + Send + Sync;

#[derive(Clone, Default)]
pub struct StopToken {
    requested: Arc<AtomicBool>,
    probe: Option<Arc<StopProbe>>,
    probe_error: Arc<Mutex<Option<String>>>,
}

impl fmt::Debug for StopToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StopToken")
            .field("requested", &self.requested.load(Ordering::SeqCst))
            .field("has_probe", &self.probe.is_some())
            .finish_non_exhaustive()
    }
}

impl StopToken {
    /// Only in-process work may use a durable probe until tool-tree termination
    /// is proven. Once observed, a request or read error stays latched.
    pub(crate) fn with_probe(
        probe: impl Fn() -> Result<bool, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            probe: Some(Arc::new(probe)),
            ..Self::default()
        }
    }

    pub fn request_stop(&self) {
        self.requested.store(true, Ordering::SeqCst);
    }

    pub fn is_stop_requested(&self) -> bool {
        if self.requested.load(Ordering::SeqCst) {
            return true;
        }
        if let Some(probe) = &self.probe {
            match probe() {
                Ok(false) => {}
                Ok(true) => self.request_stop(),
                Err(error) => {
                    let mut first = self.probe_error.lock().unwrap_or_else(|e| e.into_inner());
                    if first.is_none() {
                        *first = Some(error);
                    }
                    self.request_stop();
                }
            }
        }
        self.requested.load(Ordering::SeqCst)
    }

    pub(crate) fn probe_error(&self) -> Option<String> {
        self.probe_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

pub trait CommandRunner {
    fn run(
        &self,
        spec: &CommandSpec,
        context: &ProcessContext,
        stop: &StopToken,
    ) -> Result<CommandOutcome, ProcessError>;
}

#[derive(Debug, Clone)]
pub struct SystemCommandRunner {
    poll_interval: Duration,
}

impl Default for SystemCommandRunner {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
        }
    }
}

impl SystemCommandRunner {
    pub fn with_poll_interval(poll_interval: Duration) -> Self {
        Self { poll_interval }
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(
        &self,
        spec: &CommandSpec,
        context: &ProcessContext,
        stop: &StopToken,
    ) -> Result<CommandOutcome, ProcessError> {
        fs::create_dir_all(&context.log_directory).map_err(|source| {
            ProcessError::CreateLogDirectory {
                path: context.log_directory.clone(),
                source,
            }
        })?;

        let stdout_log = context
            .log_directory
            .join(format!("{}.stdout.log", context.log_prefix));
        let stderr_log = context
            .log_directory
            .join(format!("{}.stderr.log", context.log_prefix));

        let stdout_file = create_log_file(&stdout_log)?;
        let stderr_file = create_log_file(&stderr_log)?;

        let mut child = Command::new(&spec.program)
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .map_err(|source| ProcessError::Spawn {
                program: spec.program.clone(),
                source,
            })?;

        loop {
            if let Some(status) = child.try_wait().map_err(|source| ProcessError::Wait {
                program: spec.program.clone(),
                source,
            })? {
                return Ok(CommandOutcome {
                    exit_code: status.code(),
                    success: status.success(),
                    stopped_by_request: false,
                    stdout_log,
                    stderr_log,
                });
            }

            if stop.is_stop_requested() {
                child.kill().map_err(|source| ProcessError::Kill {
                    program: spec.program.clone(),
                    source,
                })?;
                let status = child.wait().map_err(|source| ProcessError::Wait {
                    program: spec.program.clone(),
                    source,
                })?;

                return Ok(CommandOutcome {
                    exit_code: status.code(),
                    success: false,
                    stopped_by_request: true,
                    stdout_log,
                    stderr_log,
                });
            }

            thread::sleep(self.poll_interval);
        }
    }
}

#[derive(Debug)]
pub enum ProcessError {
    InvalidLogPrefix(String),
    CreateLogDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    CreateLogFile {
        path: PathBuf,
        source: std::io::Error,
    },
    Spawn {
        program: PathBuf,
        source: std::io::Error,
    },
    Wait {
        program: PathBuf,
        source: std::io::Error,
    },
    Kill {
        program: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLogPrefix(prefix) => write!(f, "invalid process log prefix: {prefix}"),
            Self::CreateLogDirectory { path, source } => {
                write!(
                    f,
                    "cannot create log directory {}: {source}",
                    path.display()
                )
            }
            Self::CreateLogFile { path, source } => {
                write!(f, "cannot create new log file {}: {source}", path.display())
            }
            Self::Spawn { program, source } => {
                write!(f, "cannot start process {}: {source}", program.display())
            }
            Self::Wait { program, source } => {
                write!(
                    f,
                    "cannot read process status for {}: {source}",
                    program.display()
                )
            }
            Self::Kill { program, source } => {
                write!(f, "cannot stop process {}: {source}", program.display())
            }
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidLogPrefix(_) => None,
            Self::CreateLogDirectory { source, .. }
            | Self::CreateLogFile { source, .. }
            | Self::Spawn { source, .. }
            | Self::Wait { source, .. }
            | Self::Kill { source, .. } => Some(source),
        }
    }
}

impl ProcessError {
    /// A wait/kill error does not prove the previously spawned child exited.
    pub fn may_have_live_child(&self) -> bool {
        matches!(self, Self::Wait { .. } | Self::Kill { .. })
    }
}

fn create_log_file(path: &Path) -> Result<std::fs::File, ProcessError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ProcessError::CreateLogFile {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rnaseq-process-test-{label}-{}-{nonce}",
            process::id()
        ))
    }

    #[test]
    fn successful_command_captures_logs() {
        let root = temp_root("success");
        let rustc = which::which("rustc").expect("rustc must exist while testing Rust");
        let spec = CommandSpec::new(rustc, vec![OsString::from("--version")]);
        let context = ProcessContext::new(&root, "rustc-version").expect("context");

        let outcome = SystemCommandRunner::default()
            .run(&spec, &context, &StopToken::default())
            .expect("run rustc");

        assert!(outcome.success);
        assert!(!outcome.stopped_by_request);
        assert!(outcome.stdout_log.is_file());
        assert!(outcome.stderr_log.is_file());
        assert!(!fs::read_to_string(&outcome.stdout_log)
            .expect("read stdout")
            .trim()
            .is_empty());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn nonzero_exit_is_an_explicit_outcome() {
        let root = temp_root("failure");
        let rustc = which::which("rustc").expect("rustc must exist while testing Rust");
        let spec = CommandSpec::new(
            rustc,
            vec![OsString::from("--definitely-invalid-rnaseq-pipeline-flag")],
        );
        let context = ProcessContext::new(&root, "rustc-failure").expect("context");

        let outcome = SystemCommandRunner::default()
            .run(&spec, &context, &StopToken::default())
            .expect("runner should return process outcome");

        assert!(!outcome.success);
        assert!(!outcome.stopped_by_request);
        assert_ne!(outcome.exit_code, Some(0));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn existing_log_file_is_not_silently_overwritten() {
        let root = temp_root("existing-log");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("duplicate.stdout.log"), "keep me").expect("seed log");

        let rustc = which::which("rustc").expect("rustc");
        let spec = CommandSpec::new(rustc, vec![OsString::from("--version")]);
        let context = ProcessContext::new(&root, "duplicate").expect("context");

        let error = SystemCommandRunner::default()
            .run(&spec, &context, &StopToken::default())
            .expect_err("existing log must block overwrite");

        assert!(matches!(error, ProcessError::CreateLogFile { .. }));
        assert_eq!(
            fs::read_to_string(root.join("duplicate.stdout.log")).expect("read log"),
            "keep me"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn stop_token_round_trip() {
        let token = StopToken::default();
        assert!(!token.is_stop_requested());
        token.request_stop();
        assert!(token.is_stop_requested());
    }

    #[test]
    fn probed_stop_is_latched_and_shared_without_repolling() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = calls.clone();
        let token = StopToken::with_probe(move || Ok(seen.fetch_add(1, Ordering::SeqCst) == 1));
        let clone = token.clone();
        assert!(!token.is_stop_requested());
        assert!(clone.is_stop_requested());
        assert!(token.is_stop_requested());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(token.probe_error(), None);
    }

    #[test]
    fn probe_failure_stops_and_retains_the_first_error() {
        let token = StopToken::with_probe(|| Err("control unreadable".into()));
        assert!(token.is_stop_requested());
        assert!(token.clone().is_stop_requested());
        assert_eq!(token.probe_error().as_deref(), Some("control unreadable"));
    }

    #[test]
    #[ignore = "helper process for stop-request integration test"]
    fn child_sleep_helper() {
        thread::sleep(Duration::from_secs(10));
    }

    #[test]
    fn stop_request_kills_owned_child() {
        let root = temp_root("stop");
        let executable = std::env::current_exe().expect("test executable");
        let spec = CommandSpec::new(
            executable,
            vec![
                OsString::from("--ignored"),
                OsString::from("--exact"),
                OsString::from("process::tests::child_sleep_helper"),
                OsString::from("--nocapture"),
            ],
        );
        let context = ProcessContext::new(&root, "child-stop").expect("context");
        let token = StopToken::default();
        let child_token = token.clone();

        let handle = thread::spawn(move || {
            SystemCommandRunner::with_poll_interval(Duration::from_millis(20)).run(
                &spec,
                &context,
                &child_token,
            )
        });

        thread::sleep(Duration::from_millis(150));
        token.request_stop();

        let outcome = handle
            .join()
            .expect("runner thread")
            .expect("runner outcome");
        assert!(!outcome.success);
        assert!(outcome.stopped_by_request);

        fs::remove_dir_all(root).expect("cleanup");
    }
}
