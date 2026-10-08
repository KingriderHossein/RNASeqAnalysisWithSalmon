use pipeline_core::{
    coordinator::{
        create_job, terminal_job_report, ControlIntent, DriveMode, JobControl, JobCoordinator,
    },
    Accession, JobId, JobState, SraToolkitPlanner, StateStore, SystemCommandRunner, ToolRegistry,
};
use std::{ffi::OsString, path::Path};

pub const USAGE: &str = "Module A CLI (resolved run accessions only)
Usage:
  rnaseq-pipeline create DB JOB OUTPUT_PARENT THREADS SRR... [ERR... DRR...]
  rnaseq-pipeline inspect DB JOB
  rnaseq-pipeline start DB JOB
  rnaseq-pipeline resume DB JOB
  rnaseq-pipeline retry DB JOB
  rnaseq-pipeline pause JOB_ROOT
  rnaseq-pipeline cancel JOB_ROOT
  rnaseq-pipeline tools

Pause/Cancel persist a request; active tools finish at a safe stage boundary.
Crash checkpoints with ambiguous tool lifetime stay blocked. See docs/MODULE-A-CLI.md.
";

fn utf8(value: &OsString) -> Result<&str, String> {
    value
        .to_str()
        .ok_or_else(|| "identifier/command must be valid UTF-8".into())
}

pub fn execute(args: &[OsString]) -> Result<i32, String> {
    let Some(command) = args.first() else {
        print!("{USAGE}");
        return Ok(0);
    };
    match utf8(command)? {
        "help" | "--help" | "-h" if args.len() == 1 => {
            print!("{USAGE}");
            Ok(0)
        }
        "--version" if args.len() == 1 => {
            println!("rnaseq-pipeline {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        "create" if args.len() >= 6 => {
            let id = JobId::new(utf8(&args[2])?).map_err(|e| e.to_string())?;
            let threads = utf8(&args[4])?
                .parse::<u32>()
                .map_err(|_| "THREADS must be an integer".to_owned())?;
            let accessions = args[5..]
                .iter()
                .map(|value| Accession::parse(utf8(value)?).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let mut store = StateStore::open(Path::new(&args[1])).map_err(|e| e.to_string())?;
            let job = create_job(&mut store, id, Path::new(&args[3]), threads, &accessions)
                .map_err(|e| e.to_string())?;
            println!("created {} at {}", job.id, job.output_root);
            Ok(0)
        }
        "inspect" if args.len() == 3 => {
            let id = JobId::new(utf8(&args[2])?).map_err(|e| e.to_string())?;
            let store =
                StateStore::open_existing(Path::new(&args[1])).map_err(|e| e.to_string())?;
            let job = store
                .get_job(&id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("job not found: {id}"))?;
            println!(
                "job={} state={} root={} settings={} tools={} error={:?}",
                job.id,
                job.state,
                job.output_root,
                job.settings_snapshot,
                job.tool_versions_snapshot,
                job.last_error
            );
            for run in store.list_job_runs(&id).map_err(|e| e.to_string())? {
                println!(
                    "run={} state={} bytes={} checkpoint={:?} error={:?}",
                    run.id, run.state, run.downloaded_bytes, run.last_checkpoint, run.last_error
                );
            }
            Ok(0)
        }
        "pause" | "cancel" if args.len() == 2 => {
            let intent = if utf8(command)? == "pause" {
                ControlIntent::Pause
            } else {
                ControlIntent::Cancel
            };
            JobControl::open(Path::new(&args[1]))
                .and_then(|control| control.request(intent))
                .map_err(|e| e.to_string())?;
            println!("{intent:?} requested; a worker applies it at a safe stage boundary. Completed outputs are retained.");
            Ok(0)
        }
        "tools" if args.len() == 1 => {
            let tools = ToolRegistry::discover_sra_toolkit().map_err(|e| e.to_string())?;
            println!("{tools:#?}");
            Ok(0)
        }
        "start" | "resume" | "retry" if args.len() == 3 => {
            let mode = match utf8(command)? {
                "start" => DriveMode::Start,
                "resume" => DriveMode::Resume,
                _ => DriveMode::Retry,
            };
            let id = JobId::new(utf8(&args[2])?).map_err(|e| e.to_string())?;
            let mut store =
                StateStore::open_existing(Path::new(&args[1])).map_err(|e| e.to_string())?;
            if let Some(report) = terminal_job_report(&store, &id).map_err(|e| e.to_string())? {
                println!("job={id} state={}", report.state);
                return Ok(if report.state == JobState::Complete {
                    0
                } else {
                    4
                });
            }
            let planner = SraToolkitPlanner::new(
                ToolRegistry::discover_sra_toolkit().map_err(|e| e.to_string())?,
            );
            let runner = SystemCommandRunner::default();
            let report = JobCoordinator::new(&planner, &runner)
                .drive(&mut store, &id, mode, |event| println!("{event:?}"))
                .map_err(|e| e.to_string())?;
            Ok(match report.state {
                JobState::Complete => 0,
                JobState::Cancelled => 4,
                JobState::Paused => 3,
                _ => 1,
            })
        }
        _ => Err(format!("invalid command or argument count\n{USAGE}")),
    }
}
