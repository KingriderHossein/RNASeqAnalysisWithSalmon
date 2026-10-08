use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn binary() -> Command { Command::new(env!("CARGO_BIN_EXE_rnaseq-pipeline")) }

#[test]
fn help_and_invalid_arguments_do_not_require_sra_toolkit() {
    let help = binary().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout).unwrap().contains("safe stage"));
    assert!(!binary().args(["create", "only-one-argument"]).status().unwrap().success());
}

#[test]
fn create_inspect_pause_cancel_and_duplicate_preservation() {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("module-a-cli-{}-{nonce}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let out = root.join("destination with spaces");
    let created = binary().arg("create").arg(&db).arg("job").arg(&out).args(["2", "SRR900001", "ERR900002"]).output().unwrap();
    assert!(created.status.success(), "{:?}", created);
    let job = out.join("job");
    let original = fs::read(job.join("MODULE-A-JOB")).unwrap();
    assert!(!binary().arg("create").arg(&db).arg("job").arg(&out).args(["2", "SRR900003"]).status().unwrap().success());
    assert_eq!(fs::read(job.join("MODULE-A-JOB")).unwrap(), original);
    let inspected = binary().arg("inspect").arg(&db).arg("job").output().unwrap();
    assert!(inspected.status.success());
    let text = String::from_utf8(inspected.stdout).unwrap();
    assert!(text.contains("SRR900001") && text.contains("ERR900002") && text.contains("READY"));
    // Both commands work while a different client owns the database.
    let store = pipeline_core::StateStore::open(&db).unwrap();
    assert!(binary().arg("pause").arg(&job).status().unwrap().success());
    assert!(binary().arg("cancel").arg(&job).status().unwrap().success());
    assert!(job.join("control/pause.request").is_file());
    assert!(job.join("control/cancel.request").is_file());
    assert!(!binary().arg("inspect").arg(&db).arg("job").status().unwrap().success());
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
