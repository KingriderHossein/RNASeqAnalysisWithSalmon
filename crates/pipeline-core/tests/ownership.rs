//! Synthetic multi-process regression tests. Never invoke SRA Toolkit or use raw reads.
use pipeline_core::{OutputOwnership, OwnershipError, StateStore, StoreError};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rnaseq-ownership-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn child(root: &Path, mode: &str, id: &str) -> ChildGuard {
    ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "child_entry", "--nocapture"])
            .env("PIPELINE_OWNERSHIP_TEST_ROOT", root)
            .env("PIPELINE_OWNERSHIP_TEST_MODE", mode)
            .env("PIPELINE_OWNERSHIP_TEST_ID", id)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

fn wait_file(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(start.elapsed() < Duration::from_secs(15), "timeout: {}", path.display());
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn child_entry() {
    let Some(root) = std::env::var_os("PIPELINE_OWNERSHIP_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("PIPELINE_OWNERSHIP_TEST_MODE").unwrap();
    let id = std::env::var("PIPELINE_OWNERSHIP_TEST_ID").unwrap();
    match mode.as_str() {
        "database" => {
            let _store = StateStore::open(root.join("state.sqlite")).unwrap();
            fs::write(root.join(format!("ready-{id}")), b"ready").unwrap();
            std::io::stdin().read_exact(&mut [0]).unwrap();
        }
        "output" => {
            let _ownership = OutputOwnership::acquire(&[&root]).unwrap();
            fs::write(root.join(format!("ready-{id}")), b"ready").unwrap();
            std::io::stdin().read_exact(&mut [0]).unwrap();
        }
        "publish" => {
            fs::write(root.join(format!("ready-{id}")), b"ready").unwrap();
            wait_file(&root.join("go"));
            let result = pipeline_core::ownership::publish_noreplace(
                &root.join(format!("stage-{id}")),
                &root.join("final"),
            );
            let bytes: &[u8] = if result.is_ok() { b"won" } else { b"blocked" };
            fs::write(root.join(format!("result-{id}")), bytes).unwrap();
        }
        _ => panic!("unknown child test mode"),
    }
}

#[test]
fn second_process_cannot_open_state_and_crash_releases_ownership() {
    let fixture = Fixture::new("database");
    let mut holder = child(&fixture.0, "database", "one");
    wait_file(&fixture.0.join("ready-one"));
    let bytes = fs::read(fixture.0.join("state.sqlite")).unwrap();
    assert!(matches!(
        StateStore::open(fixture.0.join("state.sqlite")),
        Err(StoreError::Ownership(OwnershipError::Busy { .. }))
    ));
    assert_eq!(fs::read(fixture.0.join("state.sqlite")).unwrap(), bytes);
    holder.0.kill().unwrap();
    holder.0.wait().unwrap();
    let _reopened = StateStore::open(fixture.0.join("state.sqlite")).unwrap();
    assert!(fixture.0.join("state.sqlite.pipeline.lock").is_file());
}

#[test]
fn separate_databases_cannot_write_the_same_output_root() {
    let fixture = Fixture::new("output");
    fs::write(fixture.0.join("original"), b"keep").unwrap();
    let _first_db = StateStore::open(fixture.0.join("first.sqlite")).unwrap();
    let _second_db = StateStore::open(fixture.0.join("second.sqlite")).unwrap();
    let mut holder = child(&fixture.0, "output", "one");
    wait_file(&fixture.0.join("ready-one"));
    assert!(matches!(
        OutputOwnership::acquire(&[&fixture.0]),
        Err(OwnershipError::Busy { .. })
    ));
    assert_eq!(fs::read(fixture.0.join("original")).unwrap(), b"keep");
    holder.0.kill().unwrap();
    holder.0.wait().unwrap();
    let _recovered = OutputOwnership::acquire(&[&fixture.0]).unwrap();
    assert!(fixture.0.join(".pipeline-owner.lock").is_file());
}

#[test]
fn duplicate_roots_and_directory_aliases_share_ownership() {
    let fixture = Fixture::new("aliases");
    let alias = fixture.0.join(".");
    let guard = OutputOwnership::acquire(&[&fixture.0, &alias]).unwrap();
    assert!(matches!(OutputOwnership::acquire(&[&alias]), Err(OwnershipError::Busy { .. })));
    drop(guard);
    OutputOwnership::acquire(&[&alias]).unwrap();
}

#[test]
fn existing_empty_destination_cannot_be_replaced_after_an_earlier_check() {
    let fixture = Fixture::new("empty-destination");
    let source = fixture.0.join("stage");
    let destination = fixture.0.join("final");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("synthetic"), b"candidate").unwrap();
    assert!(!destination.exists());
    // Simulate a non-cooperating writer in the old check-to-rename window.
    fs::create_dir(&destination).unwrap();
    assert!(pipeline_core::ownership::publish_noreplace(&source, &destination).is_err());
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    assert_eq!(fs::read(source.join("synthetic")).unwrap(), b"candidate");
}

#[test]
fn manifest_publication_preserves_existing_bytes() {
    let fixture = Fixture::new("manifest");
    let source = fixture.0.join("candidate");
    let destination = fixture.0.join("SHA256SUMS");
    fs::write(&source, b"new hashes").unwrap();
    fs::write(&destination, b"existing hashes").unwrap();
    assert!(pipeline_core::ownership::publish_noreplace(&source, &destination).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"new hashes");
    assert_eq!(fs::read(&destination).unwrap(), b"existing hashes");
}

#[test]
fn two_uncoordinated_processes_publish_exactly_one_whole_directory() {
    let fixture = Fixture::new("race");
    for id in ["one", "two"] {
        let stage = fixture.0.join(format!("stage-{id}"));
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("synthetic"), id).unwrap();
    }
    let mut first = child(&fixture.0, "publish", "one");
    let mut second = child(&fixture.0, "publish", "two");
    wait_file(&fixture.0.join("ready-one"));
    wait_file(&fixture.0.join("ready-two"));
    fs::write(fixture.0.join("go"), b"go").unwrap();
    wait_file(&fixture.0.join("result-one"));
    wait_file(&fixture.0.join("result-two"));
    assert!(first.0.wait().unwrap().success());
    assert!(second.0.wait().unwrap().success());
    let results = [
        fs::read_to_string(fixture.0.join("result-one")).unwrap(),
        fs::read_to_string(fixture.0.join("result-two")).unwrap(),
    ];
    assert_eq!(results.iter().filter(|r| r.as_str() == "won").count(), 1);
    let winner = if results[0] == "won" { "one" } else { "two" };
    let loser = if winner == "one" { "two" } else { "one" };
    assert_eq!(fs::read_to_string(fixture.0.join("final/synthetic")).unwrap(), winner);
    assert_eq!(fs::read_to_string(fixture.0.join(format!("stage-{loser}/synthetic"))).unwrap(), loser);
}

#[cfg(unix)]
#[test]
fn dangling_destination_and_lock_symlinks_are_preserved() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new("symlinks");
    let source = fixture.0.join("stage");
    let destination = fixture.0.join("final");
    fs::create_dir(&source).unwrap();
    symlink(fixture.0.join("missing"), &destination).unwrap();
    assert!(pipeline_core::ownership::publish_noreplace(&source, &destination).is_err());
    assert!(fs::symlink_metadata(&destination).unwrap().file_type().is_symlink());
    symlink(fixture.0.join("other-missing"), fixture.0.join(".pipeline-owner.lock")).unwrap();
    assert!(matches!(OutputOwnership::acquire(&[&fixture.0]), Err(OwnershipError::Io { .. })));
    assert!(!fixture.0.join("other-missing").exists());
}
