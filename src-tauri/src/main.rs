#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{ffi::OsString, path::PathBuf, sync::Arc};
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
#[derive(Debug, Default)]
struct StartupOptions {
    data_dir: Option<PathBuf>,
    automation_port: Option<u16>,
}
impl StartupOptions {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, std::io::Error> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg == "--data-dir" {
                let directory =
                    PathBuf::from(args.next().ok_or_else(|| {
                        std::io::Error::other("--data-dirには絶対パスが必要です")
                    })?);
                if !directory.is_absolute() {
                    return Err(std::io::Error::other("--data-dirには絶対パスが必要です"));
                }
                options.data_dir = Some(directory);
            } else if arg == "--automation-port" {
                let port = args
                    .next()
                    .and_then(|s| s.to_str()?.parse::<u16>().ok())
                    .filter(|p| *p >= 1024)
                    .ok_or_else(|| {
                        std::io::Error::other("--automation-portには1024〜65535のポートが必要です")
                    })?;
                options.automation_port = Some(port);
            }
        }
        if options.automation_port.is_some() && options.data_dir.is_none() {
            return Err(std::io::Error::other(
                "自動試験には専用の--data-dirが必要です",
            ));
        }
        Ok(options)
    }
}
fn main() {
    let options = StartupOptions::parse(std::env::args_os().skip(1)).expect("起動引数が不正です");
    let mut context = tauri::generate_context!();
    // Elevated WebView2 hosts ignore WEBVIEW2_* environment overrides. Only an
    // explicit automation launch enables CDP, with its own isolated profile.
    let automation_windows = if options.automation_port.is_some() {
        let windows = context.config().app.windows.clone();
        for window in &mut context.config_mut().app.windows {
            window.create = false;
        }
        windows
    } else {
        Vec::new()
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let path = match &options.data_dir {
                Some(directory) => directory.join("steward.sqlite3"),
                None => app.path().app_data_dir()?.join("steward.sqlite3"),
            };
            let service = Service::new(path).map_err(std::io::Error::other)?;
            app.manage(Arc::new(service));
            if let Some(port) = options.automation_port {
                let directory = options.data_dir.as_ref().expect("validated data directory");
                for window in &automation_windows {
                    tauri::WebviewWindowBuilder::from_config(app, window)?
                        .additional_browser_args(&format!(
                            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-address=127.0.0.1 --remote-debugging-port={port}"
                        ))
                        .data_directory(directory.join("webview").join(&window.label))
                        .build()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![steward])
        .run(context)
        .expect("Dev Stewardの起動に失敗しました");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<StartupOptions, std::io::Error> {
        StartupOptions::parse(args.iter().map(OsString::from))
    }
    #[test]
    fn normal_launch_has_no_debugging_override() {
        let options = parse(&[]).unwrap();
        assert!(options.automation_port.is_none());
        assert!(options.data_dir.is_none());
    }
    #[test]
    fn automation_requires_an_isolated_absolute_directory_and_valid_port() {
        assert!(parse(&["--automation-port", "9224"]).is_err());
        assert!(parse(&["--data-dir", "relative", "--automation-port", "9224"]).is_err());
        let directory = std::env::temp_dir().join("steward-argument-test");
        for invalid in ["0", "1023", "65536", "bad", ""] {
            assert!(
                parse(&[
                    "--data-dir",
                    directory.to_str().unwrap(),
                    "--automation-port",
                    invalid
                ])
                .is_err()
            );
        }
        let options = parse(&[
            "--data-dir",
            directory.to_str().unwrap(),
            "--automation-port",
            "9224",
        ])
        .unwrap();
        assert_eq!(options.automation_port, Some(9224));
        assert_eq!(options.data_dir, Some(directory));
    }
}
