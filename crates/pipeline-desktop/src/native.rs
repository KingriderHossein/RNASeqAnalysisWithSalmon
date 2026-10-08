use crate::bridge::DesktopBridge;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{Emitter, Manager, State};

#[tauri::command]
async fn create_download_job(
    bridge: State<'_, DesktopBridge>,
    workspace: String,
    id: String,
    runs: String,
    threads: u32,
) -> Result<Value, String> {
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        bridge.create(&PathBuf::from(workspace), &id, &runs, threads)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_download_job(
    bridge: State<'_, DesktopBridge>,
    database: String,
    id: String,
) -> Result<Value, String> {
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || bridge.open(&PathBuf::from(database), &id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_download_jobs(database: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || DesktopBridge::list_jobs(&PathBuf::from(database)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn run_download_job(
    bridge: State<'_, DesktopBridge>,
    app: tauri::AppHandle,
    mode: String,
) -> Result<Value, String> {
    let session = bridge.begin(&mode)?;
    let bridge = bridge.inner().clone();
    let accepted = bridge.snapshot()?;
    std::thread::Builder::new().name("module-a-worker".into()).spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.execute_system(|snapshot| { let _ = app.emit("download-snapshot", snapshot); })
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(message)) => { let _ = app.emit("download-error", message); }
            Err(_) => {
                let message = "Worker interrupted. Refresh the saved job; ambiguous tool checkpoints remain blocked.";
                bridge.interrupted(message);
                let _ = app.emit("download-error", message);
            }
        }
        if let Ok(snapshot) = bridge.snapshot() { let _ = app.emit("download-snapshot", snapshot); }
    }).map_err(|e| e.to_string())?;
    Ok(accepted)
}

#[tauri::command]
async fn request_download_control(
    bridge: State<'_, DesktopBridge>,
    intent: String,
) -> Result<Value, String> {
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || bridge.control(&intent))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn inspect_storage(
    destination: String,
    recommendation: Option<String>,
) -> Result<Value, String> {
    let recommendation = recommendation
        .map(|v| {
            v.parse::<u64>()
                .map_err(|_| "invalid workspace recommendation".to_owned())
        })
        .transpose()?;
    tauri::async_runtime::spawn_blocking(move || {
        DesktopBridge::inspect_storage(&PathBuf::from(destination), recommendation)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn choose_folder() -> Result<Option<String>, String> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose a download workspace")
        .pick_folder()
        .await
        .map(|file| {
            file.path()
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "destination must be valid UTF-8".into())
        })
        .transpose()
}

#[tauri::command]
async fn choose_database() -> Result<Option<String>, String> {
    rfd::AsyncFileDialog::new()
        .set_title("Open a saved job database")
        .add_filter("Job database", &["sqlite", "db"])
        .pick_file()
        .await
        .map(|file| {
            file.path()
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "database path must be valid UTF-8".into())
        })
        .transpose()
}

#[tauri::command]
async fn read_log_tail(
    bridge: State<'_, DesktopBridge>,
    accession: String,
    stream: String,
) -> Result<String, String> {
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || bridge.log_tail(&accession, &stream))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn tool_status() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let tools =
            pipeline_core::ToolRegistry::discover_sra_toolkit().map_err(|e| e.to_string())?;
        Ok(json!(pipeline_core::ToolKind::SRA_REQUIRED
            .iter()
            .map(|kind| {
                let tool = tools.tool(*kind);
                json!({"name": kind.to_string(), "path": tool.path, "version": tool.version_output})
            })
            .collect::<Vec<_>>()))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn run() {
    tauri::Builder::default()
        .manage(DesktopBridge::default())
        .invoke_handler(tauri::generate_handler![create_download_job, open_download_job,
            list_download_jobs, run_download_job, request_download_control, inspect_storage,
            choose_folder, choose_database, read_log_tail, tool_status])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<DesktopBridge>().active() {
                    api.prevent_close();
                    let _ = window.emit("download-error", "Pause or Cancel this job and wait for its current stage before closing the window.");
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Tauri application");
}
