#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::Arc;
use steward_core::service::Service;
use tauri::Manager;

#[tauri::command]
async fn steward(
    method: String,
    payload: serde_json::Value,
    state: tauri::State<'_, Arc<Service>>,
) -> Result<serde_json::Value, String> {
    let service = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.call(&method, payload))
        .await
        .map_err(|e| e.to_string())?
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("steward.sqlite3");
            let args: Vec<_> = std::env::args_os().collect();
            let path = if let Some(index) = args.iter().position(|s| s == "--data-dir") {
                let directory =
                    std::path::PathBuf::from(args.get(index + 1).ok_or_else(|| {
                        std::io::Error::other("--data-dirには絶対パスが必要です")
                    })?);
                if !directory.is_absolute() {
                    return Err(std::io::Error::other("--data-dirには絶対パスが必要です").into());
                }
                directory.join("steward.sqlite3")
            } else {
                path
            };
            let service = Service::new(path).map_err(std::io::Error::other)?;
            app.manage(Arc::new(service));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![steward])
        .run(tauri::generate_context!())
        .expect("Dev Stewardの起動に失敗しました");
}
