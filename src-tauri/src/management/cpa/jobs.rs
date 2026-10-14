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
    logs: Vec::new(),
    update: None,
});
#[derive(Clone, serde::Serialize)]
pub struct Job {
    busy: bool,
    message: String,
    revision: u64,
    logs: Vec<LogEntry>,
    update: Option<UpdateResult>,
}
#[derive(Clone, serde::Serialize)]
pub struct UpdateResult {
    executable: String,
    commit: String,
    revision: u64,
}
pub(super) fn update_result(app: &tauri::AppHandle, result: Option<(String, String)>) {
    if let Ok(mut job) = JOB.lock() {
        job.revision += 1;
        job.update = result.map(|(executable, commit)| UpdateResult {
            executable,
            commit,
            revision: job.revision,
        });
        let _ = app.emit("cpa-job", job.clone());
    }
}
#[derive(Clone, serde::Serialize)]
struct LogEntry {
    id: u64,
    at: u64,
    message: String,
}
impl Job {
    fn record(&mut self, message: String) {
        self.revision += 1;
        self.logs.push(LogEntry {
            id: self.revision,
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            message,
        });
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }
}
fn log(app: &tauri::AppHandle, message: String) {
    if let Ok(mut job) = JOB.lock() {
        if !job.busy {
            job.message = message.clone();
        }
        job.record(message);
        let _ = app.emit("cpa-job", job.clone());
    }
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
        job.message = message.clone();
        job.record(message);
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
        log(&app, "CPA 操作正在进行，未执行重复请求".into());
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
            let mut s = state.0.try_lock().map_err(|_| {
                log(&app, "管理操作正在进行，未执行 CPA 命令".into());
                "管理操作正在进行"
            })?;
            if s.snapshot.busy
                || s.snapshot
                    .account_tests
                    .values()
                    .any(super::super::probe::account::active)
            {
                log(&app, "请等待当前凭证操作或探查结束，未执行 CPA 命令".into());
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
pub async fn cpa_set_running(
    app: tauri::AppHandle,
    executable: String,
    port: u16,
    running: bool,
    expected_pid: Option<u32>,
) -> Result<String, String> {
    run(app, move |app| {
        let snapshot =
            crate::discovery::scan(app.path().app_data_dir().map_err(|_| "无法定位应用目录")?)?;
        if !snapshot.status_known {
            return Err("进程状态未知，请先重新检测".into());
        }
        let service = snapshot
            .services
            .iter()
            .find(|s| s.location.executable == executable)
            .ok_or("该目录未经过服务发现，请重新检测")?;
        if service.pid != expected_pid {
            return Err("CPA 进程已变化，请重新检测后操作".into());
        }
        let layout = super::runtime::Layout::existing(std::path::Path::new(&executable), port)?;
        phase(
            app,
            if running {
                "$ CPA --config config.yaml · 正在启动…"
            } else {
                "$ SIGTERM CPA / dev · 正在关闭…"
            },
        );
        super::runtime::set_running(&layout, running, expected_pid)?;
        if running {
            super::install::restore_connection(app, &layout)?;
            let state = app.state::<Management>();
            let mut s = state.0.lock().map_err(|_| "管理状态不可用")?;
            if !s.snapshot.connected {
                let store = super::super::local_connection::LocalFile::for_app(app)?;
                s.restore_for_port(&store, port)
                    .map_err(|_| "CPA 已启动，但管理连接恢复失败，请检查连接设置")?;
            }
            if s.snapshot.connected && s.snapshot.port == port {
                super::super::quota_cache::restore(app, &mut s)
                    .map_err(|_| "CPA 已启动，但额度缓存恢复失败，请手动刷新额度")?;
            }
            Ok("CPA 已启动".into())
        } else {
            let state = app.state::<Management>();
            let mut s = state.0.lock().map_err(|_| "管理状态不可用")?;
            if s.snapshot.port == port {
                s.disconnect();
                s.snapshot.error = None;
            }
            Ok("CPA 已关闭".into())
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logs_are_ordered_bounded_and_do_not_replace_active_job_state() {
        let mut job = Job {
            busy: true,
            message: "正在编译".into(),
            revision: 0,
            logs: vec![],
            update: None,
        };
        for i in 0..105 {
            job.record(format!("command {i}"));
        }
        assert_eq!(job.logs.len(), 100);
        assert_eq!(job.logs[0].message, "command 5");
        assert_eq!(job.logs.last().unwrap().id, 105);
        assert!(job.busy);
        assert_eq!(job.message, "正在编译");
    }
}
