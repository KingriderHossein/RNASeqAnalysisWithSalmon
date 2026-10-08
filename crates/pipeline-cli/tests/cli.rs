use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rnaseq-pipeline"))
}

#[test]
fn batch_preview_and_create_preserve_selection_and_reject_before_writes() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("cli-batch-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let file = root.join("batch with spaces.csv");
    fs::write(
        &file,
        "run,notes\nSRR900002,ERR999999\nerr900001,tumor\nSRR900002,normal\n",
    )
    .unwrap();
    let preview = binary().arg("preview-batch").arg(&file).output().unwrap();
    assert!(preview.status.success());
    assert_eq!(
        String::from_utf8(preview.stdout).unwrap(),
        "SRR900002\nERR900001\n"
    );
    assert!(String::from_utf8(preview.stderr)
        .unwrap()
        .contains("1 duplicate"));
    let db = root.join("state.sqlite");
    let output = root.join("outputs");
    let created = binary()
        .arg("create-batch")
        .arg(&db)
        .arg("batch")
        .arg(&output)
        .arg("2")
        .arg(&file)
        .output()
        .unwrap();
    assert!(created.status.success(), "{created:?}");
    let store = pipeline_core::StateStore::open_existing(&db).unwrap();
    let runs = store
        .list_job_runs(&pipeline_core::JobId::new("batch").unwrap())
        .unwrap();
    assert_eq!(runs.len(), 2);
    assert!(runs
        .iter()
        .all(|r| r.state == pipeline_core::RunState::Ready));
    assert!(!runs.iter().any(|r| r.accession_or_source == "ERR999999"));
    drop(store);

    for content in [b"run\nSRR1\nSRX2".as_slice(), &[0xff], b"\"SRR1"] {
        fs::write(&file, content).unwrap();
        let invalid_db = root.join("untouched.sqlite");
        let invalid_output = root.join("untouched-output");
        assert!(!binary()
            .arg("create-batch")
            .arg(&invalid_db)
            .arg("invalid")
            .arg(&invalid_output)
            .arg("2")
            .arg(&file)
            .output()
            .unwrap()
            .status
            .success());
        assert!(!invalid_db.exists() && !invalid_output.exists());
    }
    fs::write(
        &file,
        vec![b' '; pipeline_core::run_batch::MAX_BATCH_BYTES + 1],
    )
    .unwrap();
    assert!(!binary()
        .arg("preview-batch")
        .arg(&file)
        .output()
        .unwrap()
        .status
        .success());
    let unsupported = root.join("runs.json");
    fs::write(&unsupported, "SRR1").unwrap();
    assert!(!binary()
        .arg("preview-batch")
        .arg(&unsupported)
        .output()
        .unwrap()
        .status
        .success());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn help_and_invalid_arguments_do_not_require_sra_toolkit() {
    let help = binary().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)
        .unwrap()
        .contains("safe stage"));
    assert!(!binary()
        .args(["create", "only-one-argument"])
        .status()
        .unwrap()
        .success());
}

#[test]
fn create_inspect_pause_cancel_and_duplicate_preservation() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("module-a-cli-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let out = root.join("destination with spaces");
    let created = binary()
        .arg("create")
        .arg(&db)
        .arg("job")
        .arg(&out)
        .args(["2", "SRR900001", "ERR900002"])
        .output()
        .unwrap();
    assert!(created.status.success(), "{:?}", created);
    let job = out.join("job");
    let original = fs::read(job.join("MODULE-A-JOB")).unwrap();
    assert!(!binary()
        .arg("create")
        .arg(&db)
        .arg("job")
        .arg(&out)
        .args(["2", "SRR900003"])
        .status()
        .unwrap()
        .success());
    assert_eq!(fs::read(job.join("MODULE-A-JOB")).unwrap(), original);
    let inspected = binary()
        .arg("inspect")
        .arg(&db)
        .arg("job")
        .output()
        .unwrap();
    assert!(inspected.status.success());
    let text = String::from_utf8(inspected.stdout).unwrap();
    assert!(text.contains("SRR900001") && text.contains("ERR900002") && text.contains("READY"));
    // Both commands work while a different client owns the database.
    let store = pipeline_core::StateStore::open(&db).unwrap();
    assert!(binary().arg("pause").arg(&job).status().unwrap().success());
    assert!(binary().arg("cancel").arg(&job).status().unwrap().success());
    assert!(job.join("control/pause.request").is_file());
    assert!(job.join("control/cancel.request").is_file());
    assert!(!binary()
        .arg("inspect")
        .arg(&db)
        .arg("job")
        .status()
        .unwrap()
        .success());
    drop(store);
    let resumed = binary()
        .arg("resume")
        .arg(&db)
        .arg("job")
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(
        resumed.status.code(),
        Some(4),
        "queued cancellation needs no tool discovery"
    );
    let store = pipeline_core::StateStore::open_existing(&db).unwrap();
    assert!(store
        .list_job_runs(&pipeline_core::JobId::new("job").unwrap())
        .unwrap()
        .iter()
        .all(|run| run.state == pipeline_core::RunState::Cancelled));
    drop(store);
    let missing = root.join("missing.sqlite");
    assert!(!binary()
        .arg("inspect")
        .arg(&missing)
        .arg("job")
        .status()
        .unwrap()
        .success());
    assert!(!missing.exists());
    fs::remove_dir_all(root).unwrap();
}
