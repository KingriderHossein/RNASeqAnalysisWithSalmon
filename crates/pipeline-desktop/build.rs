fn main() {
    #[cfg(feature = "native")]
    {
        let commands = &[
            "create_download_job",
            "open_download_job",
            "list_download_jobs",
            "run_download_job",
            "request_download_control",
            "inspect_storage",
            "choose_folder",
            "choose_database",
            "read_log_tail",
            "tool_status",
        ];
        tauri_build::try_build(
            tauri_build::Attributes::new()
                .app_manifest(tauri_build::AppManifest::new().commands(commands)),
        )
        .expect("Tauri build configuration");
    }
}
