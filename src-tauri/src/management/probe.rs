//! CLIProxy owns provider selection, aliases and credential routing. Keys stay in Rust.
pub mod account;
mod pinned;
mod pinned_response;
mod response;
#[cfg(test)]
mod tests;
use super::{Management, Session};
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Instant;
use tauri::Manager;

#[derive(Serialize)]
pub struct Model {
    id: String,
    name: String,
    brand: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    text: String,
    finish: String,
    reported_model: String,
    elapsed_ms: u64,
}
pub(super) fn client_key(session: &Session) -> Result<String, String> {
    let api = session.api.as_ref().ok_or("请先连接管理接口")?;
    // User-authorized access to this one configuration field; never fetch the full config.
    let value = api.call(
        Method::GET,
        if api.legacy {
            "/api-keys"
        } else {
            "/config/access/api-keys"
        },
        None,
    );
    let value = match value {
        Err(e) if e == "HTTP 404" => return Ok(String::new()),
        result => result?,
    };
    let keys = if api.legacy {
        &value["api-keys"]
    } else {
        &value
    };
    if keys.is_null() {
        return Ok(String::new());
    }
    let keys = keys.as_array().ok_or("客户端 API Key 配置格式异常")?;
    if keys.is_empty() {
        return Ok(String::new());
    }
    keys.iter()
        .filter_map(Value::as_str)
        .find(|key| {
            !key.is_empty()
                && key.len() <= 8192
                && key.trim() == *key
                && !key.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .ok_or("客户端 API Key 配置无效".into())
}
fn models(session: &Session, scope: &str, key: &str) -> Result<Vec<Model>, String> {
    let api = session.api.as_ref().ok_or("请先连接管理接口")?;
    if scope.len() > 1100 {
        return Err("模型范围无效".into());
    }
    let allowed = if scope.is_empty() {
        None
    } else {
        let record = session
            .records
            .get(scope)
            .filter(|r| r.row.enabled)
            .ok_or("凭证已停用或不存在，请选择其他模型范围")?;
        let path = if api.legacy {
            "/auth-files/models"
        } else {
            "/credentials/models"
        };
        let value = api.call_with_query(Method::GET, path, &[("name", &record.name)], None)?;
        Some(parse_models(&value, "models")?)
    };
    let value = api.inference(key, None)?;
    let rows = parse_models(&value, "data")?;
    Ok(rows
        .into_iter()
        .filter(|m| {
            allowed
                .as_ref()
                .is_none_or(|list| list.iter().any(|a| a.id == m.id))
        })
        .collect())
}
fn parse_models(value: &Value, field: &str) -> Result<Vec<Model>, String> {
    let rows = value[field].as_array().ok_or("模型列表格式异常")?;
    let mut output = Vec::new();
    for row in rows {
        let id = row["id"].as_str().unwrap_or_default();
        if id.is_empty()
            || id.len() > 512
            || id.chars().any(char::is_control)
            || output.iter().any(|m: &Model| m.id == id)
        {
            continue;
        }
        let name = row["display_name"]
            .as_str()
            .filter(|n| !n.is_empty())
            .unwrap_or(id)
            .chars()
            .take(200)
            .collect();
        let brand = match row["owned_by"].as_str().unwrap_or_default() {
            "openai" | "codex" => "openai",
            "anthropic" | "claude" => "claude",
            "google" | "gemini" | "antigravity" => "gemini",
            "deepseek" => "deepseek",
            _ => "unknown",
        }
        .into();
        output.push(Model {
            id: id.into(),
            name,
            brand,
        });
    }
    output.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(output)
}
fn sample(session: &mut Session, scope: &str, model: &str, prompt: &str) -> Result<Answer, String> {
    if scope.len() > 1100
        || model.is_empty()
        || model.len() > 512
        || model.chars().any(char::is_control)
        || prompt.trim().is_empty()
        || prompt.chars().count() > 4000
    {
        return Err("探查参数无效".into());
    }
    session.reload()?;
    let key = client_key(session)?;
    if !models(session, scope, &key)?.iter().any(|m| m.id == model) {
        return Err("所选模型已不可用，请刷新模型列表".into());
    }
    let started = Instant::now();
    let payload = json!({"model":model,"stream":false,"max_tokens":6000,
        "messages":[{"role":"user","content":prompt}]});
    let value = session
        .api
        .as_ref()
        .unwrap()
        .inference(&key, Some(&payload))?;
    let mut answer = response::parse(&value)?;
    answer.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(answer)
}
fn lock_session(
    state: &Management,
    wait: bool,
) -> Result<std::sync::MutexGuard<'_, Session>, String> {
    if wait {
        state.0.lock().map_err(|_| "管理会话不可用".into())
    } else {
        state
            .0
            .try_lock()
            .map_err(|_| "管理操作正在进行，请稍后重试".into())
    }
}
async fn run<T: Send + 'static>(
    app: tauri::AppHandle,
    wait: bool,
    f: impl FnOnce(&mut Session) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Management>();
        // Catalog reads from both windows queue; paid sampling never retries or queues.
        let mut session = lock_session(&state, wait)?;
        session.snapshot.busy = true;
        session.publish(&app);
        let result = f(&mut session);
        session.snapshot.busy = false;
        session.publish(&app);
        result
    })
    .await
    .map_err(|_| "探查任务未完成".to_owned())?
}
#[tauri::command]
pub async fn probe_models(app: tauri::AppHandle, id: String) -> Result<Vec<Model>, String> {
    run(app, true, move |s| {
        s.reload()?;
        let key = client_key(s)?;
        models(s, &id, &key)
    })
    .await
}
#[tauri::command]
pub async fn probe_sample(
    app: tauri::AppHandle,
    id: String,
    model: String,
    prompt: String,
) -> Result<Answer, String> {
    run(app, false, move |s| sample(s, &id, &model, &prompt)).await
}
