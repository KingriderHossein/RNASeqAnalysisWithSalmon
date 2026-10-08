use pipeline_desktop::bridge::DesktopBridge;
use pipeline_core::{CommandRunner, CommandSpec, CommandOutcome, ProcessContext, ProcessError, StopToken, SraToolkitPlanner, ToolRegistry, ToolInfo, ToolKind, StateStore};
use std::{fs, path::{Path, PathBuf}, sync::{Mutex, atomic::{AtomicU64, Ordering}}};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("desktop-bridge-{}-{}", std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Workspace { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn planner() -> SraToolkitPlanner {
    let tool = |kind: ToolKind| ToolInfo { kind, path: kind.binary_name().into(), version_output: "synthetic".into() };
    SraToolkitPlanner::new(ToolRegistry { prefetch: tool(ToolKind::Prefetch), vdb_validate: tool(ToolKind::VdbValidate), fasterq_dump: tool(ToolKind::FasterqDump) })
}
#[derive(Default)]
struct Runner { calls: Mutex<Vec<String>> }
impl CommandRunner for Runner {
    fn run(&self, spec: &CommandSpec, context: &ProcessContext, _: &StopToken) -> Result<CommandOutcome, ProcessError> {
        let program = spec.program.to_str().unwrap(); self.calls.lock().unwrap().push(program.into());
        if program == "prefetch" {
            let accession = spec.args[0].to_str().unwrap();
            let position = spec.args.iter().position(|v| v == "-O").unwrap();
            let root = PathBuf::from(&spec.args[position + 1]).join(accession);
            fs::create_dir_all(&root).unwrap(); fs::write(root.join(format!("{accession}.sra")), b"synthetic").unwrap();
        } else if program == "fasterq-dump" {
            let accession = Path::new(&spec.args[0]).file_name().unwrap().to_str().unwrap();
            let position = spec.args.iter().position(|v| v == "-O").unwrap();
            fs::write(PathBuf::from(&spec.args[position + 1]).join(format!("{accession}.fastq")), b"@synthetic\nACGT\n+\nIIII\n").unwrap();
        }
        Ok(CommandOutcome { success: true, exit_code: Some(0), stopped_by_request: false, stdout_log: context.log_directory.join("synthetic.stdout.log"), stderr_log: context.log_directory.join("synthetic.stderr.log") })
    }
}

#[test]
fn live_worker_reservation_cache_and_pause_survive_restart() {
    let workspace = Workspace::new(); let bridge = DesktopBridge::default();
    let created = bridge.create(&workspace.0, "batch", "SRR900001\nERR900002", 2).unwrap();
    let database = PathBuf::from(created["database"].as_str().unwrap());
    let runner = Runner::default(); let session = bridge.begin("start").unwrap();
    assert!(bridge.begin("start").is_err());
    assert!(bridge.create(&workspace.0, "other", "SRR900003", 2).is_err());
    let mut requested = false; let mut previous = 0;
    session.execute_with(|| Ok(planner()), &runner, |snapshot| {
        let revision = snapshot["revision"].as_str().unwrap().parse::<u64>().unwrap();
        assert!(revision > previous); previous = revision;
        assert!(StateStore::open_existing(&database).is_err());
        assert_eq!(bridge.open(&database, "batch").unwrap()["revision"], snapshot["revision"]);
        if !requested && snapshot["activity"]["label"] == "Acquiring reads" {
            let control = bridge.control("pause").unwrap();
            assert_eq!(control["pending_control"], "pause"); requested = true;
        }
    }).unwrap();
    assert!(requested); assert!(!bridge.active());
    let paused = bridge.snapshot().unwrap();
    assert_eq!(paused["job"]["state"], "PAUSED");
    assert_eq!(paused["runs"][0]["state"], "SRA_VALID");
    assert_eq!(paused["runs"][1]["state"], "READY");
    let reopened = DesktopBridge::default();
    assert!(reopened.open(&database, "batch").unwrap()["actions"]["primary"].as_bool().unwrap());
    reopened.begin("resume").unwrap().execute_with(|| Ok(planner()), &runner, |_| {}).unwrap();
    let complete = reopened.snapshot().unwrap();
    assert_eq!(complete["job"]["state"], "COMPLETE");
    assert!(!complete["actions"]["primary"].as_bool().unwrap());
    assert!(complete["artifacts"].as_array().unwrap().iter().any(|artifact| artifact["kind"] == "CHECKSUM"));
    let count = runner.calls.lock().unwrap().len();
    reopened.begin("resume").unwrap().execute_with(|| panic!("terminal jobs must not discover tools"), &runner, |_| {}).unwrap();
    assert_eq!(runner.calls.lock().unwrap().len(), count);
    assert_eq!(DesktopBridge::list_jobs(&database).unwrap()[0]["state"], "COMPLETE");
}

#[test]
fn idle_cancel_needs_no_toolkit_and_keeps_user_files() {
    let workspace = Workspace::new(); let bridge = DesktopBridge::default();
    let snapshot = bridge.create(&workspace.0, "idle", "SRR900001", 2).unwrap();
    let path = Path::new(snapshot["job"]["output_root"].as_str().unwrap()).join("keep-me");
    fs::write(&path, b"user evidence").unwrap();
    let cancelled = bridge.control("cancel").unwrap();
    assert_eq!(cancelled["job"]["state"], "CANCELLED");
    assert_eq!(cancelled["runs"][0]["state"], "CANCELLED");
    assert_eq!(fs::read(path).unwrap(), b"user evidence");
    assert!(!bridge.active());
}

#[test]
fn low_space_guidance_does_not_prevent_creation_and_logs_are_bounded() {
    let workspace = Workspace::new(); let bridge = DesktopBridge::default();
    let space = DesktopBridge::inspect_storage(&workspace.0, Some(u64::MAX)).unwrap();
    assert_eq!(space["warning"], true); assert_eq!(space["can_continue"], true);
    assert!(space["download_estimate_bytes"].is_null());
    let snapshot = bridge.create(&workspace.0, "logs", "SRR900001", 2).unwrap();
    let logs = Path::new(snapshot["job"]["output_root"].as_str().unwrap()).join("logs");
    fs::create_dir(&logs).unwrap();
    fs::write(logs.join("SRR900001-attempt-2-prefetch.stderr.log"), b"old").unwrap();
    let mut content = vec![b'x'; 100_000]; content.extend_from_slice(b"latest sentinel");
    fs::write(logs.join("SRR900001-attempt-10-prefetch.stderr.log"), content).unwrap();
    let tail = bridge.log_tail("SRR900001", "stderr").unwrap();
    assert!(tail.ends_with("latest sentinel")); assert!(tail.len() < 66_000);
    assert!(tail.contains("attempt-10"));
    assert!(bridge.log_tail("../../other", "stderr").is_err());
    assert!(bridge.log_tail("SRR900001", "../../other").is_err());
}

#[test]
fn discovery_failure_requires_refresh_and_never_claims_running_after_drop() {
    let workspace = Workspace::new(); let bridge = DesktopBridge::default();
    let snapshot = bridge.create(&workspace.0, "tools", "SRR900001", 2).unwrap();
    let runner = Runner::default();
    assert!(bridge.begin("start").unwrap().execute_with(|| Err("missing synthetic toolkit".into()), &runner, |_| {}).is_err());
    let failed = bridge.snapshot().unwrap();
    assert_eq!(failed["active"], false); assert_eq!(failed["actions"]["primary"], false);
    assert!(failed["blocked_reason"].as_str().unwrap().contains("Refresh"));
    let refreshed = bridge.open(Path::new(snapshot["database"].as_str().unwrap()), "tools").unwrap();
    assert_eq!(refreshed["runs"][0]["state"], "READY");
    assert_eq!(refreshed["actions"]["primary"], true);
    assert!(runner.calls.lock().unwrap().is_empty());
}
