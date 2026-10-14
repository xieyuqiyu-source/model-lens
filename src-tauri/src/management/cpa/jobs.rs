use super::super::Management;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager};
static BUSY: AtomicBool = AtomicBool::new(false);
static JOB: Mutex<Job> = Mutex::new(Job {
    busy: false,
    message: String::new(),
    revision: 0,
});
#[derive(Clone, serde::Serialize)]
pub struct Job {
    busy: bool,
    message: String,
    revision: u64,
}
struct Guard {
    app: tauri::AppHandle,
    owns_session: bool,
    result: Option<String>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.owns_session {
            if let Ok(mut session) = self.app.state::<Management>().0.lock() {
                session.snapshot.busy = false;
                session.publish(&self.app);
            }
        }
        if self.owns_session {
            report(
                &self.app,
                false,
                self.result
                    .take()
                    .unwrap_or_else(|| "CPA 任务中断，请重新检测状态".into()),
            );
        }
        BUSY.store(false, Ordering::Release);
    }
}
fn report(app: &tauri::AppHandle, busy: bool, message: String) {
    if let Ok(mut job) = JOB.lock() {
        job.busy = busy;
        job.message = message;
        job.revision += 1;
        let _ = app.emit("cpa-job", job.clone());
    }
}
pub(super) fn phase(app: &tauri::AppHandle, message: &str) {
    report(app, true, message.into());
}
#[tauri::command]
pub fn cpa_job() -> Result<Job, String> {
    JOB.lock()
        .map(|j| j.clone())
        .map_err(|_| "任务状态不可用".into())
}
pub(super) async fn run(
    app: tauri::AppHandle,
    action: impl FnOnce(&tauri::AppHandle) -> Result<String, String> + Send + 'static,
) -> Result<String, String> {
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err("CPA 操作正在进行".into());
    }
    // Ownership is acquired before spawning so cancellation cannot leave the gate locked.
    let guard = Guard {
        app: app.clone(),
        owns_session: false,
        result: None,
    };
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut guard = guard;
        {
            let state = app.state::<Management>();
            let mut s = state.0.try_lock().map_err(|_| "管理操作正在进行")?;
            if s.snapshot.busy
                || s.snapshot
                    .account_tests
                    .values()
                    .any(super::super::probe::account::active)
            {
                return Err("请等待当前凭证操作或探查结束".into());
            }
            s.snapshot.busy = true;
            guard.owns_session = true;
            s.publish(&app);
        }
        phase(&app, "正在准备…");
        let result = action(&app);
        // Reconcile credentials after restart without changing saved connection information.
        if let Ok(mut s) = app.state::<Management>().0.lock() {
            if s.snapshot.connected {
                s.snapshot.error = s.reload().err();
            }
        }
        guard.result = Some(result.clone().unwrap_or_else(|e| e));
        drop(guard);
        let _ = app.emit("refresh-service", ());
        result
    })
    .await
    .map_err(|_| "CPA 后台任务异常结束".to_owned())?;
    result
}
#[tauri::command]
pub async fn cpa_start(
    app: tauri::AppHandle,
    executable: String,
    port: u16,
) -> Result<String, String> {
    run(app, move |app| {
        let snapshot =
            crate::discovery::scan(app.path().app_data_dir().map_err(|_| "无法定位应用目录")?)?;
        if !snapshot.status_known {
            return Err("进程状态未知，请先重新检测".into());
        }
        if !snapshot
            .services
            .iter()
            .any(|s| s.location.executable == executable)
        {
            return Err("该目录未经过服务发现，请重新检测".into());
        }
        let layout = super::runtime::Layout::existing(std::path::Path::new(&executable), port)?;
        phase(app, "正在启动 / 重启 CPA…");
        super::runtime::restart(&layout)?;
        super::install::restore_connection(app, &layout)?;
        Ok("CPA 已启动，后续由 ModelLens 独立管理".into())
    })
    .await
}
#[tauri::command]
pub async fn cpa_choose_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let result = objc2::MainThreadMarker::new().and_then(|main| {
            let panel = objc2_app_kit::NSOpenPanel::openPanel(main);
            panel.setCanChooseDirectories(true);
            panel.setCanChooseFiles(false);
            panel.setAllowsMultipleSelection(false);
            panel.setCanCreateDirectories(true);
            if panel.runModal() != 1 {
                return None;
            }
            panel
                .URL()
                .and_then(|u| u.path())
                .map(|p| format!("{}/CLIProxy", p))
        });
        let _ = tx.send(result);
    })
    .map_err(|_| "无法打开目录选择器")?;
    tauri::async_runtime::spawn_blocking(move || rx.recv().map_err(|_| "目录选择未完成".to_owned()))
        .await
        .map_err(|_| "目录选择未完成".to_owned())?
}
#[tauri::command]
pub async fn cpa_install(
    app: tauri::AppHandle,
    directory: String,
    port: u16,
) -> Result<String, String> {
    run(app, move |app| {
        super::install::install(app, &directory, port)
    })
    .await
}
