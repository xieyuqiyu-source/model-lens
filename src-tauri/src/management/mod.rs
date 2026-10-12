pub mod access;
pub mod batch;
mod local_connection;
mod mutations;
pub mod probe;
mod quota;
mod quota_cache;
mod quota_timer;
mod transport;
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
use transport::Api;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub provider: String,
    pub enabled: bool,
    pub editable: bool,
    pub deletable: bool,
    pub file_name: String,
    pub delete_token: String,
    pub supported: bool,
    pub five: Option<f64>,
    pub week: Option<f64>,
    pub reset_five: String,
    pub reset_week: String,
    pub quota_items: Vec<quota::QuotaItem>,
    pub plan: Option<String>,
    pub reset_available: Option<u64>,
    pub reset_count_at: Option<u64>,
    pub reset_count_error: Option<String>,
    pub quota_at: Option<u64>,
    pub quota_error: Option<String>,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub connected: bool,
    pub saved: bool,
    pub port: u16,
    pub busy: bool,
    pub stale: bool,
    pub credentials: Vec<Credential>,
    pub error: Option<String>,
    pub revision: u64,
    pub account_tests: HashMap<String, probe::account::AccountTest>,
}
#[derive(Clone)]
struct Record {
    row: Credential,
    name: String,
    auth_index: String,
    provider: String,
    account_id: String,
    project_id: String,
    revision: String,
}
#[derive(Default)]
struct Session {
    initialized: bool,
    api: Option<Api>,
    records: HashMap<String, Record>,
    snapshot: Snapshot,
}
#[derive(Default)]
pub struct Management(Mutex<Session>);
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn text(v: &Value, key: &str) -> String {
    v[key]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(512)
        .collect()
}
fn normalize(payload: &Value) -> Result<HashMap<String, Record>, String> {
    let files = payload["files"].as_array().ok_or("凭证列表格式异常")?;
    let mut records = HashMap::new();
    for item in files {
        let name = text(item, "name");
        if name.is_empty() {
            continue;
        }
        let auth_index = text(item, "auth_index");
        let id = format!("{}:{}", name, auth_index);
        let provider = {
            let p = text(item, "provider");
            if p.is_empty() {
                text(item, "type")
            } else {
                p
            }
        }
        .to_lowercase();
        let brand = match provider.as_str() {
            "codex" | "openai" | "openai-compatibility" => "openai",
            "claude" => "claude",
            "gemini" | "gemini-cli" | "antigravity" => "gemini",
            "xai" | "grok" => "grok",
            "deepseek" => "deepseek",
            _ => "unknown",
        }
        .to_owned();
        let email = text(item, "email");
        let supported = !auth_index.is_empty()
            && matches!(
                provider.as_str(),
                "codex" | "claude" | "xai" | "grok" | "gemini" | "gemini-cli" | "antigravity"
            )
            && !matches!(
                text(item, "account_type").as_str(),
                "api_key" | "api-key" | "apikey"
            );
        // Never expose `account`, raw tokens, paths or arbitrary server fields to the webview.
        let deletable = mutations::file_backed(item, &name);
        let row = Credential {
            id: id.clone(),
            name: if email.is_empty() {
                name.clone()
            } else {
                email
            },
            brand,
            provider: provider.clone(),
            enabled: item["disabled"].as_bool() == Some(false),
            editable: item["disabled"].is_boolean()
                && !item["runtime_only"].as_bool().unwrap_or(false),
            supported,
            deletable,
            file_name: if deletable {
                name.clone()
            } else {
                String::new()
            },
            delete_token: mutations::delete_token(item),
            plan: if provider == "codex" {
                quota::codex::plan(item)
            } else {
                None
            },
            ..Credential::default()
        };
        let account_id = text(&item["id_token"], "chatgpt_account_id");
        let project_id = [item, &item["metadata"], &item["attributes"]]
            .iter()
            .flat_map(|v| [text(v, "project_id"), text(v, "projectId")])
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        if records
            .insert(
                id,
                Record {
                    row,
                    name,
                    auth_index,
                    provider,
                    account_id,
                    project_id,
                    revision: [text(item, "modtime"), text(item, "updated_at")].join(""),
                },
            )
            .is_some()
        {
            return Err("凭证身份重复，已停止更新列表".into());
        }
    }
    Ok(records)
}
impl Session {
    fn publish(&mut self, app: &tauri::AppHandle) {
        self.snapshot.account_tests.retain(|id, test| {
            // Keep an active lease until its task finishes; discard stale completed borders.
            matches!(test.status.as_str(), "running" | "analyzing")
                || self
                    .records
                    .get(id)
                    .is_some_and(|r| probe::account::identity(r) == test.identity)
        });
        self.snapshot.credentials = self.records.values().map(|r| r.row.clone()).collect();
        self.snapshot
            .credentials
            .sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        self.snapshot.revision += 1;
        let _ = app.emit("credentials-changed", &self.snapshot);
    }
    fn reload(&mut self) -> Result<(), String> {
        let result = self
            .api
            .as_mut()
            .ok_or("请先连接本机管理接口")?
            .list()
            .and_then(|v| normalize(&v));
        let mut records = match result {
            Ok(r) => r,
            Err(e) => {
                self.snapshot.stale = true;
                return Err(e);
            }
        };
        for (id, record) in &mut records {
            if let Some(old) = self.records.get(id) {
                if quota_cache::identity(record)
                    .is_some_and(|key| Some(key) == quota_cache::identity(old))
                {
                    quota_cache::QuotaData::from(&old.row).apply(&mut record.row);
                }
            }
        }
        self.records = records;
        self.snapshot.stale = false;
        Ok(())
    }
    fn quota_targets(&self, id: Option<String>) -> Vec<String> {
        match id {
            Some(id) => vec![id],
            None => self
                .records
                .iter()
                .filter(|(_, r)| r.row.supported)
                .map(|(id, _)| id.clone())
                .collect(),
        }
    }
    fn quota(&mut self, id: &str) -> Result<(), String> {
        let record = self
            .records
            .get(id)
            .ok_or("凭证已不存在，请刷新列表")?
            .clone();
        match quota::fetch(self.api.as_ref().ok_or("请先连接管理接口")?, &record) {
            Ok(row) => {
                self.records.get_mut(id).unwrap().row = row;
                Ok(())
            }
            Err(e) => {
                self.records.get_mut(id).unwrap().row.quota_error = Some(e.clone());
                Err(e)
            }
        }
    }
    fn toggle(
        &mut self,
        id: &str,
        expected: bool,
        enabled: bool,
        token: Option<&str>,
    ) -> Result<(), String> {
        self.reload()?;
        let record = self.records.get(id).ok_or("凭证已不存在")?;
        if !record.row.editable {
            return Err("该凭证不能独立切换".into());
        }
        if record.row.enabled != expected || token.is_some_and(|t| t != record.row.delete_token) {
            return Err("账号状态已被其他界面修改，已刷新，请重新确认".into());
        }
        let body = json!({"name":record.name,"auth_index":record.auth_index,"disabled":!enabled});
        let write =
            self.api
                .as_ref()
                .unwrap()
                .call(Method::PATCH, "/credentials/status", Some(&body));
        // Always reconcile after an attempted write; never retry a possibly applied mutation.
        if self.reload().is_err() {
            return Err("开关结果未确认，请刷新列表核实；暂时禁用开关".into());
        }
        if write.is_err() {
            return Err("开关请求未获确认，已重新读取实际状态；没有自动重试".into());
        }
        if self
            .records
            .get(id)
            .is_none_or(|r| r.row.enabled != enabled)
        {
            return Err("服务返回状态与请求不一致，已显示最新状态".into());
        }
        Ok(())
    }
}
async fn operate(
    app: tauri::AppHandle,
    f: impl FnOnce(&mut Session, &tauri::AppHandle) -> Result<(), String> + Send + 'static,
) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Management>();
        let mut session = state
            .0
            .try_lock()
            .map_err(|_| "另一项操作正在进行，请稍后重试")?;
        session.snapshot.busy = true;
        session.snapshot.error = None;
        session.publish(&app);
        if let Err(e) = f(&mut session, &app) {
            session.snapshot.error = Some(e);
        }
        session.snapshot.busy = false;
        session.publish(&app);
        Ok(session.snapshot.clone())
    })
    .await
    .map_err(|_| "本机管理任务失败".to_owned())?
}
#[tauri::command]
pub async fn management_snapshot(app: tauri::AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Management>()
            .0
            .lock()
            .map(|mut s| {
                if !s.initialized {
                    s.snapshot.busy = true;
                    s.publish(&app);
                    s.initialized = true;
                    s.snapshot.error = local_connection::LocalFile::for_app(&app)
                        .and_then(|store| s.restore(&store))
                        .and_then(|_| quota_cache::restore(&app, &mut s))
                        .err();
                    s.snapshot.busy = false;
                    s.publish(&app);
                }
                s.snapshot.clone()
            })
            .map_err(|_| "管理状态不可用".into())
    })
    .await
    .map_err(|_| "管理状态不可用".to_owned())?
}
#[tauri::command]
pub async fn management_connect(
    app: tauri::AppHandle,
    port: u16,
    key: String,
) -> Result<Snapshot, String> {
    operate(app, move |s, app| {
        s.connect_and_save(port, key, &local_connection::LocalFile::for_app(app)?)?;
        quota_cache::restore(app, s)
    })
    .await
}
#[tauri::command]
pub async fn management_disconnect(app: tauri::AppHandle) -> Result<Snapshot, String> {
    operate(app, |s, _| {
        s.disconnect();
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn management_restore(app: tauri::AppHandle) -> Result<Snapshot, String> {
    operate(app, |s, app| {
        s.restore(&local_connection::LocalFile::for_app(app)?)?;
        quota_cache::restore(app, s)?;
        if !s.snapshot.saved {
            return Err("尚无本地连接记录，请输入管理密钥".into());
        }
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn management_forget(app: tauri::AppHandle) -> Result<Snapshot, String> {
    operate(app, |s, app| {
        s.forget(&local_connection::LocalFile::for_app(app)?)
    })
    .await
}
#[tauri::command]
pub async fn credentials_list(app: tauri::AppHandle) -> Result<Snapshot, String> {
    operate(app, |s, _| s.reload()).await
}
#[tauri::command]
pub async fn credential_toggle(
    app: tauri::AppHandle,
    id: String,
    expected: bool,
    enabled: bool,
) -> Result<Snapshot, String> {
    operate(app, move |s, _| s.toggle(&id, expected, enabled, None)).await
}
#[tauri::command]
pub async fn quota_refresh(app: tauri::AppHandle, id: Option<String>) -> Result<Snapshot, String> {
    operate(app, move |s, app| s.refresh_quotas(app, id)).await
}
#[cfg(test)]
mod tests;

#[tauri::command]
pub async fn credential_import(
    app: tauri::AppHandle,
    name: String,
    contents: String,
) -> Result<Snapshot, String> {
    operate(app, move |s, _| s.import_credential(&name, &contents)).await
}
#[tauri::command]
pub async fn credential_delete(
    app: tauri::AppHandle,
    id: String,
    expected: String,
) -> Result<Snapshot, String> {
    operate(app, move |s, _| s.delete_credential(&id, &expected)).await
}

impl Session {
    fn refresh_quotas(&mut self, app: &tauri::AppHandle, id: Option<String>) -> Result<(), String> {
        self.reload()?;
        let mut failed = 0;
        for id in self.quota_targets(id) {
            if self.quota(&id).is_err() {
                failed += 1;
            }
            self.publish(app);
        }
        quota_cache::persist(app, self)?;
        if failed > 0 {
            Err(format!("{failed} 个凭证额度刷新失败，请查看卡片提示"))
        } else {
            Ok(())
        }
    }
}
pub use quota_timer::start as start_quota_timer;
