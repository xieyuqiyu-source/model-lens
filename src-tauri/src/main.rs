mod discovery;
mod management;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    window::{Effect, EffectState, EffectsBuilder},
    Emitter, Manager,
};

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum AppearanceMode {
    Light,
    Dark,
    System,
}
#[tauri::command]
fn set_appearance(app: tauri::AppHandle, mode: AppearanceMode) {
    app.set_theme(match mode {
        AppearanceMode::Light => Some(tauri::Theme::Light),
        AppearanceMode::Dark => Some(tauri::Theme::Dark),
        AppearanceMode::System => None,
    });
}
struct DiscoveryLock(Mutex<()>);
#[tauri::command]
async fn discover_services(app: tauri::AppHandle) -> Result<discovery::Snapshot, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|_| "无法定位应用数据目录")?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DiscoveryLock>();
        let _lock = state.0.lock().map_err(|_| "进程检查暂时不可用")?;
        discovery::scan(data)
    })
    .await
    .map_err(|_| "进程检查未完成".to_owned())?
}
fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
#[tauri::command]
fn open_main(app: tauri::AppHandle) {
    show_main(&app);
    if let Some(w) = app.get_webview_window("tray") {
        let _ = w.hide();
    }
}
#[tauri::command]
fn close_tray(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        let _ = window.hide();
    }
}
#[tauri::command]
fn open_tray(app: tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("tray").ok_or("快捷面板尚未就绪")?;
    if window.is_visible().unwrap_or(false) {
        return window.hide().map_err(|_| "无法关闭快捷面板".into());
    }
    let rect = app
        .tray_by_id("main-tray")
        .and_then(|t| t.rect().ok().flatten());
    if let Some(rect) = rect {
        let scale = window.scale_factor().unwrap_or(1.0);
        let position = rect.position.to_logical::<f64>(scale);
        let size = rect.size.to_logical::<f64>(scale);
        window
            .set_position(tauri::LogicalPosition::new(
                (position.x + size.width - 400.0).max(0.0),
                position.y + size.height + 6.0,
            ))
            .map_err(|_| "无法定位快捷面板")?;
    } else {
        window.center().map_err(|_| "无法定位快捷面板")?;
    }
    window.show().map_err(|_| "无法打开快捷面板")?;
    window.set_focus().map_err(|_| "无法聚焦快捷面板")?;
    let _ = window.emit("refresh-service", ());
    Ok(())
}
fn main() {
    if std::env::args().any(|s| s == "--discovery-check") {
        let dir = std::env::temp_dir().join("modellens-discovery-check");
        match discovery::scan(dir) {
            Ok(s) => println!("{}", serde_json::to_string_pretty(&s).unwrap()),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app)
        }))
        .manage(DiscoveryLock(Mutex::new(())))
        .manage(management::Management::default())
        .invoke_handler(tauri::generate_handler![
            discover_services,
            set_appearance,
            open_main,
            open_tray,
            close_tray,
            management::management_snapshot,
            management::management_connect,
            management::management_disconnect,
            management::management_restore,
            management::management_forget,
            management::credentials_list,
            management::credential_toggle,
            management::batch::credentials_toggle,
            management::batch::credentials_delete,
            management::access::copy_client_key,
            management::credential_import,
            management::credential_delete,
            management::quota_refresh,
            management::groups::quota_refresh_group,
            management::cpa::cpa_open_management,
            management::cpa::cpa_pull,
            management::cpa::versions::cpa_versions,
            management::cpa::jobs::cpa_job,
            management::cpa::jobs::cpa_set_running,
            management::cpa::jobs::cpa_install,
            management::cpa::jobs::cpa_choose_directory,
            management::probe::probe_models,
            management::probe::probe_sample,
            management::probe::external::external_list,
            management::probe::external::external_save,
            management::probe::external::external_forget,
            management::probe::external::external_models,
            management::probe::external::external_sample,
            management::probe::account::commands::probe_account_samples,
            management::probe::account::commands::probe_account_finish
        ])
        .setup(|app| {
            management::start_quota_timer(app.handle().clone());
            let open = MenuItem::with_id(app, "open", "打开 ModelLens", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 ModelLens", true, Some("CmdOrCtrl+Q"))?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let tray_window = tauri::WebviewWindowBuilder::new(
                app,
                "tray",
                tauri::WebviewUrl::App("index.html?view=tray".into()),
            )
            .title("ModelLens 快捷面板")
            .inner_size(400.0, 270.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(true)
            .effects(
                EffectsBuilder::new()
                    .effect(Effect::Menu)
                    .state(EffectState::Active)
                    .radius(13.0)
                    .build(),
            )
            .visible(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .build()?;
            tray_window.on_window_event({
                let window = tray_window.clone();
                move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = window.hide();
                    }
                }
            });
            TrayIconBuilder::with_id("main-tray")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .icon_as_template(true)
                .tooltip("ModelLens · 模型透镜")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => (),
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = open_tray(tray.app_handle().clone());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("ModelLens 启动失败")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                show_main(app);
            }
        });
}
