use super::{operate, probe, Session, Snapshot};

fn copy_key(
    session: &Session,
    write: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    let key = probe::client_key(session)?;
    if key.is_empty() {
        return Err("CLIProxy 未配置客户端 API Key".into());
    }
    write(&key)
}

#[cfg(target_os = "macos")]
fn clipboard(_: &tauri::AppHandle, value: &str) -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new("/usr/bin/pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法写入剪贴板")?;
    let wrote = child
        .stdin
        .take()
        .ok_or("无法写入剪贴板")?
        .write_all(value.as_bytes());
    let result = child.wait();
    if wrote.is_err() || !result.is_ok_and(|s| s.success()) {
        return Err("无法写入剪贴板".into());
    }
    Ok(())
}
#[cfg(windows)]
fn clipboard(app: &tauri::AppHandle, value: &str) -> Result<(), String> {
    use tauri::Manager;
    let error = "无法写入剪贴板";
    let window = app.get_webview_window("main").ok_or(error)?;
    let hwnd = window.hwnd().map_err(|_| error)?;
    let _clipboard = clipboard_win::Clipboard::new_attempts_for(hwnd.0, 10).map_err(|_| error)?;
    use clipboard_win::{options::NoClear, raw};
    let private =
        raw::register_format("ExcludeClipboardContentFromMonitorProcessing").ok_or(error)?;
    raw::empty().map_err(|_| error)?;
    raw::set_without_clear(private.get(), &[1]).map_err(|_| error)?;
    raw::set_string_with(value, NoClear).map_err(|_| error.into())
}

#[tauri::command]
pub async fn copy_client_key(app: tauri::AppHandle) -> Result<Snapshot, String> {
    operate(app, |s, app| copy_key(s, |key| clipboard(app, key))).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::management::{tests::server, transport::Api};
    use serde_json::json;
    #[test]
    fn key_goes_only_to_clipboard_and_empty_or_failure_never_writes() {
        let (port, task) = server(vec![
            (200, json!(["synthetic-client-key"])),
            (200, json!([])),
            (403, json!({"secret":"private"})),
        ]);
        let session = Session {
            api: Some(Api::new(port, "mock-key".into()).unwrap()),
            ..Session::default()
        };
        copy_key(&session, |key| {
            assert_eq!(key, "synthetic-client-key");
            Ok(())
        })
        .unwrap();
        assert!(copy_key(&session, |_| panic!("empty key must not touch clipboard")).is_err());
        let error =
            copy_key(&session, |_| panic!("failed read must not touch clipboard")).unwrap_err();
        assert!(!error.contains("private"));
        assert!(!serde_json::to_string(&session.snapshot)
            .unwrap()
            .contains("synthetic-client-key"));
        assert!(task
            .join()
            .unwrap()
            .iter()
            .all(|r| r.starts_with("GET /v8/management/config/access/api-keys ")));
    }
}
