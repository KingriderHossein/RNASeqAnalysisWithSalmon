use pipeline_core::{
    coordinator::{create_job, ControlIntent, DriveMode, JobControl, JobCoordinator, JobEvent},
    Accession, ArtifactId, ArtifactKind, CommandOutcome, CommandRunner, CommandSpec, JobId,
    JobState, OutputOwnership, ProcessContext, ProcessError, RunState, SraToolkitPlanner,
    StateStore, StopToken, ToolInfo, ToolKind, ToolRegistry,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    db: PathBuf,
    job_root: PathBuf,
    id: JobId,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture(accessions: &[&str]) -> (Fixture, StateStore) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("module-a-job-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let mut store = StateStore::open(&db).unwrap();
    let id = JobId::new("batch").unwrap();
    let runs = accessions
        .iter()
        .map(|s| Accession::parse(s).unwrap())
        .collect::<Vec<_>>();
    let job = create_job(&mut store, id.clone(), root.join("out"), 2, &runs).unwrap();
    (
        Fixture {
            root,
            db,
            job_root: PathBuf::from(job.output_root),
            id,
        },
        store,
    )
}

fn planner() -> SraToolkitPlanner {
    let tool = |kind: ToolKind| ToolInfo {
        kind,
        path: PathBuf::from(kind.binary_name()),
        version_output: "synthetic-1".into(),
    };
    SraToolkitPlanner::new(ToolRegistry {
        prefetch: tool(ToolKind::Prefetch),
        vdb_validate: tool(ToolKind::VdbValidate),
        fasterq_dump: tool(ToolKind::FasterqDump),
    })
}

#[derive(Default)]
struct FakeRunner {
    seen: Mutex<Vec<String>>,
    fail_once: Mutex<Option<String>>,
}

impl CommandRunner for FakeRunner {
    fn run(
        &self,
        spec: &CommandSpec,
        context: &ProcessContext,
        stop: &StopToken,
    ) -> Result<CommandOutcome, ProcessError> {
        assert!(
            !stop.is_stop_requested(),
            "external tool is not asynchronously killed"
        );
        let program = spec.program.to_str().unwrap();
        self.seen.lock().unwrap().push(program.into());
        let position = spec.args.iter().position(|a| a == &OsString::from("-O"));
        let mut success = true;
        if program == "prefetch" {
            let accession = spec.args[0].to_str().unwrap();
            let mut fail = self.fail_once.lock().unwrap();
            if fail.as_deref() == Some(accession) {
                *fail = None;
                success = false;
            }
            if success {
                let root = PathBuf::from(&spec.args[position.unwrap() + 1]).join(accession);
                fs::create_dir_all(&root).unwrap();
                fs::write(
                    root.join(format!("{accession}.sra")),
                    b"synthetic SRA placeholder",
                )
                .unwrap();
            }
        } else if program == "fasterq-dump" {
            let source = Path::new(&spec.args[0]);
            let accession = source.file_name().unwrap().to_str().unwrap();
            let root = PathBuf::from(&spec.args[position.unwrap() + 1]);
            fs::write(
                root.join(format!("{accession}.fastq")),
                b"@synthetic\nACGT\n+\nIIII\n",
            )
            .unwrap();
        }
        Ok(CommandOutcome {
            exit_code: Some(if success { 0 } else { 7 }),
            success,
            stopped_by_request: false,
            stdout_log: context
                .log_directory
                .join(format!("{}.stdout.log", context.log_prefix)),
            stderr_log: context
                .log_directory
                .join(format!("{}.stderr.log", context.log_prefix)),
        })
    }
}

#[test]
fn batch_completes_and_terminal_resume_changes_no_run_or_artifact() {
    let (f, mut store) = fixture(&["SRR900001", "ERR900002"]);
    let planner = planner();
    let runner = FakeRunner::default();
    let coordinator = JobCoordinator::new(&planner, &runner);
    let mut snapshots = 0;
    let report = coordinator
        .drive(&mut store, &f.id, DriveMode::Start, |event| {
            if let JobEvent::RunSnapshot { progress, .. } = event {
                assert_eq!(progress.validated_total_bytes, None);
                assert!(progress.observed_bytes >= 0);
                for name in ["sra", "fastq", "temp", "logs"] {
                    assert!(OutputOwnership::acquire(&[&f.job_root.join(name)]).is_err());
                }
                snapshots += 1;
            }
        })
        .unwrap();
    assert_eq!(report.state, JobState::Complete);
    assert!(snapshots >= 8);
    let manifest = f.job_root.join("fastq/SRR900001/compressed/SHA256SUMS");
    let original = fs::read(&manifest).unwrap();
    let seen = runner.seen.lock().unwrap().len();
    drop(store);
    let mut store = StateStore::open(&f.db).unwrap();
    let resumed = coordinator
        .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
        .unwrap();
    assert_eq!(resumed.runs, report.runs);
    assert_eq!(runner.seen.lock().unwrap().len(), seen);
    assert_eq!(fs::read(manifest).unwrap(), original);
    drop(store);
}

#[test]
fn failed_run_isolated_and_only_explicit_retry_reexecutes_it() {
    let (f, mut store) = fixture(&["SRR900001", "SRR900002"]);
    let planner = planner();
    let runner = FakeRunner {
        fail_once: Mutex::new(Some("SRR900002".into())),
        ..FakeRunner::default()
    };
    let coordinator = JobCoordinator::new(&planner, &runner);
    let first = coordinator
        .drive(&mut store, &f.id, DriveMode::Start, |_| {})
        .unwrap();
    assert_eq!(first.state, JobState::Failed);
    assert_eq!(first.runs[0].state, RunState::Complete);
    assert_eq!(first.runs[1].state, RunState::Failed);
    let sibling = f
        .job_root
        .join("fastq/SRR900001/compressed/SRR900001.fastq.gz");
    let sibling_bytes = fs::read(&sibling).unwrap();
    let count = runner.seen.lock().unwrap().len();
    coordinator
        .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
        .unwrap();
    assert_eq!(runner.seen.lock().unwrap().len(), count);
    let retried = coordinator
        .drive(&mut store, &f.id, DriveMode::Retry, |_| {})
        .unwrap();
    assert_eq!(retried.state, JobState::Complete);
    assert_eq!(retried.runs[0], first.runs[0]);
    assert_eq!(fs::read(sibling).unwrap(), sibling_bytes);
    assert!(retried.runs[1].attempt_count > first.runs[1].attempt_count);
    drop(store);
}

#[test]
fn pause_at_sra_boundary_survives_reopen_and_resumes_conversion() {
    let (f, mut store) = fixture(&["SRR900001", "SRR900002"]);
    let planner = planner();
    let runner = FakeRunner::default();
    let control = JobControl::open(&f.job_root).unwrap();
    let coordinator = JobCoordinator::new(&planner, &runner);
    let report = coordinator
        .drive(&mut store, &f.id, DriveMode::Start, |event| {
            if let JobEvent::RunSnapshot { run, .. } = event {
                if run.state == RunState::SraValid {
                    control.request(ControlIntent::Pause).unwrap();
                }
            }
        })
        .unwrap();
    assert_eq!(report.state, JobState::Paused);
    assert_eq!(report.runs[0].state, RunState::SraValid);
    assert_eq!(report.runs[1].state, RunState::Ready);
    drop(store);
    let mut store = StateStore::open(&f.db).unwrap();
    let paused = coordinator
        .drive(&mut store, &f.id, DriveMode::Start, |_| {})
        .unwrap();
    assert_eq!(paused.state, JobState::Paused);
    assert_eq!(runner.seen.lock().unwrap().len(), 2);
    let completed = coordinator
        .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
        .unwrap();
    assert_eq!(completed.state, JobState::Complete);
    assert_eq!(
        runner
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.as_str() == "prefetch")
            .count(),
        2
    );
    drop(store);
}

#[test]
fn cancel_preserves_completed_sibling_and_cannot_be_resumed() {
    let (f, mut store) = fixture(&["SRR900001", "SRR900002"]);
    let planner = planner();
    let runner = FakeRunner::default();
    let control = JobControl::open(&f.job_root).unwrap();
    let coordinator = JobCoordinator::new(&planner, &runner);
    let report = coordinator
        .drive(&mut store, &f.id, DriveMode::Start, |event| {
            if let JobEvent::RunSnapshot { run, .. } = event {
                if run.state == RunState::Complete {
                    control.request(ControlIntent::Cancel).unwrap();
                }
            }
        })
        .unwrap();
    assert_eq!(report.state, JobState::Cancelled);
    assert_eq!(report.runs[0].state, RunState::Complete);
    assert_eq!(report.runs[1].state, RunState::Cancelled);
    assert!(f.job_root.join("fastq/SRR900001/compressed").is_dir());
    let second = coordinator
        .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
        .unwrap();
    assert_eq!(second.runs, report.runs);
    assert_eq!(runner.seen.lock().unwrap().len(), 3);
    drop(store);
}

#[test]
fn ambiguous_external_checkpoint_blocks_without_reset_or_spawn() {
    for state in [
        RunState::Downloading,
        RunState::Validating,
        RunState::Converting,
    ] {
        let (f, mut store) = fixture(&["SRR900001"]);
        let id = store.list_job_runs(&f.id).unwrap()[0].id.clone();
        store
            .transition_run(&id, RunState::Downloading, Some("prefetch"), None)
            .unwrap();
        if state != RunState::Downloading {
            let path = f.job_root.join("sra/SRR900001");
            fs::create_dir_all(&path).unwrap();
            store
                .update_run_download_snapshot(&id, 1, path.to_str().unwrap())
                .unwrap();
            store
                .transition_run(&id, RunState::Downloaded, Some("prefetch-complete"), None)
                .unwrap();
            store
                .transition_run(&id, RunState::Validating, Some("vdb-validate"), None)
                .unwrap();
            if state == RunState::Converting {
                store
                    .transition_run(&id, RunState::SraValid, Some("vdb-validate-complete"), None)
                    .unwrap();
                store
                    .transition_run(&id, RunState::Converting, Some("fasterq-dump"), None)
                    .unwrap();
            }
        }
        let before = store.get_run(&id).unwrap().unwrap();
        drop(store);
        let mut store = StateStore::open(&f.db).unwrap();
        let planner = planner();
        let runner = FakeRunner::default();
        let report = JobCoordinator::new(&planner, &runner)
            .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
            .unwrap();
        assert_eq!(report.state, JobState::Failed);
        assert!(report.errors[0].1.contains("descendant may still write"));
        assert_eq!(store.get_run(&id).unwrap().unwrap(), before);
        assert!(runner.seen.lock().unwrap().is_empty());
        drop(store);
    }
}

#[test]
fn existing_destination_and_duplicate_or_study_input_are_rejected() {
    let (f, mut store) = fixture(&["SRR900001"]);
    let inputs = [Accession::parse("SRR900002").unwrap()];
    let occupied = f.root.join("out/existing");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("keep"), b"original").unwrap();
    assert!(create_job(
        &mut store,
        JobId::new("existing").unwrap(),
        f.root.join("out"),
        2,
        &inputs
    )
    .is_err());
    assert_eq!(fs::read(occupied.join("keep")).unwrap(), b"original");
    assert!(store
        .get_job(&JobId::new("existing").unwrap())
        .unwrap()
        .is_none());
    for inputs in [
        vec![Accession::parse("GSE89223").unwrap()],
        vec![inputs[0].clone(), inputs[0].clone()],
    ] {
        assert!(create_job(
            &mut store,
            JobId::new("invalid").unwrap(),
            f.root.join("out"),
            2,
            &inputs
        )
        .is_err());
    }
    drop(store);
}

#[test]
fn pause_keeps_completed_sibling_and_its_bytes_unchanged() {
    let (f, mut store) = fixture(&["SRR900001", "SRR900002"]);
    let planner = planner();
    let runner = FakeRunner::default();
    let control = JobControl::open(&f.job_root).unwrap();
    let coordinator = JobCoordinator::new(&planner, &runner);
    let paused = coordinator
        .drive(&mut store, &f.id, DriveMode::Start, |event| {
            if let JobEvent::RunSnapshot { run, .. } = event {
                if run.state == RunState::Complete {
                    control.request(ControlIntent::Pause).unwrap();
                }
            }
        })
        .unwrap();
    assert_eq!(paused.state, JobState::Paused);
    assert_eq!(paused.runs[0].state, RunState::Complete);
    assert_eq!(paused.runs[1].state, RunState::Ready);
    let path = f
        .job_root
        .join("fastq/SRR900001/compressed/SRR900001.fastq.gz");
    let bytes = fs::read(&path).unwrap();
    drop(store);
    let mut store = StateStore::open_existing(&f.db).unwrap();
    let complete = coordinator
        .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
        .unwrap();
    assert_eq!(complete.state, JobState::Complete);
    assert_eq!(complete.runs[0], paused.runs[0]);
    assert_eq!(fs::read(path).unwrap(), bytes);
    drop(store);
}

#[test]
fn output_contention_does_not_change_state_or_attempts() {
    let (f, mut store) = fixture(&["SRR900001"]);
    let before = store.list_job_runs(&f.id).unwrap();
    let lease = OutputOwnership::acquire(&[&f.job_root.join("fastq")]).unwrap();
    let planner = planner();
    let runner = FakeRunner::default();
    assert!(JobCoordinator::new(&planner, &runner)
        .drive(&mut store, &f.id, DriveMode::Start, |_| {})
        .is_err());
    assert_eq!(store.list_job_runs(&f.id).unwrap(), before);
    assert!(runner.seen.lock().unwrap().is_empty());
    drop(lease);
    drop(store);
}

#[test]
fn restart_dispatches_all_safe_acquisition_checkpoints() {
    for state in [
        RunState::Ready,
        RunState::Paused,
        RunState::WaitingForNetwork,
        RunState::Downloaded,
    ] {
        let (f, mut store) = fixture(&["SRR900001"]);
        let id = store.list_job_runs(&f.id).unwrap()[0].id.clone();
        if state != RunState::Ready {
            store
                .transition_run(&id, RunState::Downloading, Some("prefetch"), None)
                .unwrap();
            if state == RunState::Downloaded {
                let path = f.job_root.join("sra/SRR900001");
                fs::create_dir_all(&path).unwrap();
                fs::write(path.join("SRR900001.sra"), b"synthetic").unwrap();
                store
                    .update_run_download_snapshot(&id, 9, path.to_str().unwrap())
                    .unwrap();
                store
                    .transition_run(&id, state, Some("prefetch-complete"), None)
                    .unwrap();
            } else {
                store
                    .transition_run(&id, state, Some("prefetch"), None)
                    .unwrap();
            }
        }
        drop(store);
        let mut store = StateStore::open_existing(&f.db).unwrap();
        let planner = planner();
        let runner = FakeRunner::default();
        let report = JobCoordinator::new(&planner, &runner)
            .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
            .unwrap();
        assert_eq!(report.state, JobState::Complete, "{state}");
        let first = runner.seen.lock().unwrap()[0].clone();
        assert_eq!(
            first,
            if state == RunState::Downloaded {
                "vdb-validate"
            } else {
                "prefetch"
            }
        );
        drop(store);
    }
}

#[test]
fn restart_dispatches_finalization_checkpoints_without_external_tools() {
    for state in [
        RunState::FastqReady,
        RunState::Compressing,
        RunState::Checksumming,
        RunState::PausedAtBoundary,
    ] {
        let (f, mut store) = fixture(&["SRR900001"]);
        let planner = planner();
        let runner = FakeRunner::default();
        let coordinator = JobCoordinator::new(&planner, &runner);
        let control = JobControl::open(&f.job_root).unwrap();
        let paused = coordinator
            .drive(&mut store, &f.id, DriveMode::Start, |event| {
                if let JobEvent::RunSnapshot { run, .. } = event {
                    if run.state == RunState::FastqReady {
                        control.request(ControlIntent::Pause).unwrap();
                    }
                }
            })
            .unwrap();
        assert_eq!(paused.runs[0].state, RunState::FastqReady);
        let id = &paused.runs[0].id;
        if state != RunState::FastqReady {
            store
                .transition_run(id, RunState::Compressing, Some("gzip"), None)
                .unwrap();
        }
        if state == RunState::Checksumming {
            use flate2::{write::GzEncoder, Compression};
            use std::io::Write;
            let directory = f.job_root.join("fastq/SRR900001/compressed");
            fs::create_dir(&directory).unwrap();
            let path = directory.join("SRR900001.fastq.gz");
            let mut encoder =
                GzEncoder::new(fs::File::create(&path).unwrap(), Compression::default());
            encoder.write_all(b"@synthetic\nACGT\n+\nIIII\n").unwrap();
            encoder.finish().unwrap();
            store
                .record_finalized_artifacts(
                    id,
                    ArtifactKind::CompressedFastq,
                    &[(
                        ArtifactId::new("synthetic-gzip").unwrap(),
                        path.to_str().unwrap().into(),
                        fs::metadata(&path).unwrap().len() as i64,
                    )],
                )
                .unwrap();
            store
                .transition_run(id, state, Some("sha256"), None)
                .unwrap();
        } else if state == RunState::PausedAtBoundary {
            store.transition_run(id, state, Some("gzip"), None).unwrap();
        }
        drop(store);
        let mut store = StateStore::open_existing(&f.db).unwrap();
        let before = runner.seen.lock().unwrap().len();
        let report = coordinator
            .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
            .unwrap();
        assert_eq!(
            report.state,
            JobState::Complete,
            "{state}: {:?}",
            report.errors
        );
        assert_eq!(runner.seen.lock().unwrap().len(), before);
        assert!(f
            .job_root
            .join("fastq/SRR900001/compressed/SHA256SUMS")
            .is_file());
        drop(store);
    }
}

#[test]
#[cfg(unix)]
fn resume_preserves_a_preexisting_control_symlink() {
    let (f, mut store) = fixture(&["SRR900001"]);
    let target = f.root.join("user-file");
    fs::write(&target, b"keep original").unwrap();
    let link = f.job_root.join("control/pause.request");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let before = store.list_job_runs(&f.id).unwrap();
    let planner = planner();
    let runner = FakeRunner::default();
    let error = JobCoordinator::new(&planner, &runner)
        .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
        .unwrap_err();
    assert!(error.to_string().contains("regular file"));
    assert_eq!(fs::read_link(link).unwrap(), target);
    assert_eq!(fs::read(target).unwrap(), b"keep original");
    assert_eq!(store.list_job_runs(&f.id).unwrap(), before);
    assert!(runner.seen.lock().unwrap().is_empty());
    drop(store);
}

#[test]
fn changed_toolchain_retains_pause_and_never_spawns() {
    let (f, mut store) = fixture(&["SRR900001"]);
    let original = planner();
    let runner = FakeRunner::default();
    let control = JobControl::open(&f.job_root).unwrap();
    control.request(ControlIntent::Pause).unwrap();
    JobCoordinator::new(&original, &runner)
        .drive(&mut store, &f.id, DriveMode::Start, |_| {})
        .unwrap();
    let mut tools = original.tools().clone();
    tools.prefetch.version_output = "changed".into();
    let changed = SraToolkitPlanner::new(tools);
    let error = JobCoordinator::new(&changed, &runner)
        .drive(&mut store, &f.id, DriveMode::Resume, |_| {})
        .unwrap_err();
    assert!(error.to_string().contains("tool paths/versions changed"));
    assert_eq!(control.intent().unwrap(), ControlIntent::Pause);
    assert!(runner.seen.lock().unwrap().is_empty());
    drop(store);
}
