use super::{probe::account, Management};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tauri::{Manager, ResourceId, Webview};
use tauri_plugin_updater::Update;

static REPLACING: AtomicBool = AtomicBool::new(false);

pub fn replacing() -> bool {
    REPLACING.load(Ordering::Acquire)
}

#[tauri::command]
pub async fn install_macos_update(webview: Webview, rid: ResourceId) -> Result<(), String> {
    if webview.label() != "main" {
        return Err("请在主窗口安装应用更新".into());
    }
    let mut update = webview
        .resources_table()
        .get::<Update>(rid)
        .map(|update| (*update).clone())
        .map_err(|_| "更新信息已失效，请重新检查")?;
    update.timeout = Some(Duration::from_secs(300));
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|_| "更新下载或签名验证失败，请重试。")?;

    let app = webview.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let busy = "请等待当前凭证操作、探查或 CPA 任务结束后重试。";
        let state = app.state::<Management>();
        // Keep CPA jobs locked out through bundle replacement and app exit.
        let session = state.0.try_lock().map_err(|_| busy)?;
        if session.snapshot.busy || session.snapshot.account_tests.values().any(account::active) {
            return Err(busy.into());
        }
        REPLACING.store(true, Ordering::Release);
        if update.install(bytes).is_err() {
            REPLACING.store(false, Ordering::Release);
            return Err("应用更新安装失败，请重试。".into());
        }
        app.restart();
    })
    .await
    .map_err(|_| "应用更新任务未完成".to_owned())?
}
