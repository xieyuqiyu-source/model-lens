//! Probe-only adapter: fixed destinations, server-side credential selection, no token projection.
mod response;
#[cfg(test)]
mod tests;
use super::{Management, Record, Session};
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::Manager;

#[derive(Serialize)]
pub struct Model {
    id: String,
    name: String,
    effort: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    text: String,
    finish: String,
    reported_model: String,
    elapsed_ms: u64,
}
fn record(session: &mut Session, id: &str) -> Result<Record, String> {
    session.reload()?;
    let row = session.records.get(id).ok_or("凭证已不存在，请重新选择")?;
    if !row.row.enabled {
        return Err("凭证已停用，未发起采样".into());
    }
    if !row.row.probe_supported {
        return Err("自动探查仅支持已确认类型的 Codex / Claude OAuth 凭证，可使用手动检测".into());
    }
    Ok(row.clone())
}
fn models(session: &Session, record: &Record) -> Result<Vec<Model>, String> {
    let api = session.api.as_ref().ok_or("请先连接管理接口")?;
    let query: String =
        reqwest::Url::parse_with_params("http://localhost/", &[("name", &record.name)])
            .map_err(|_| "凭证名称无效")?
            .query()
            .unwrap_or_default()
            .into();
    let path = if api.legacy {
        "/auth-files/models"
    } else {
        "/credentials/models"
    };
    let value = api.call(Method::GET, &format!("{path}?{query}"), None)?;
    let visible = parse_models(&value)?;
    // api-call bypasses CLIProxy alias/prefix resolution; only send original IDs.
    let path = if api.legacy {
        "/model-definitions"
    } else {
        "/routing/model-definitions"
    };
    let definitions = api.call(Method::GET, &format!("{path}/{}", record.provider), None)?;
    let originals = parse_models(&definitions)?;
    Ok(visible
        .into_iter()
        .filter_map(|mut model| {
            let original = originals.iter().find(|m| m.id == model.id)?;
            model.effort = original.effort.clone();
            Some(model)
        })
        .collect())
}
fn parse_models(value: &Value) -> Result<Vec<Model>, String> {
    let rows = value["models"].as_array().ok_or("模型列表格式异常")?;
    let mut output = Vec::new();
    for row in rows.iter().take(1000) {
        let id = row["id"].as_str().unwrap_or_default();
        if id.is_empty()
            || id.len() > 200
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
        let levels = row["thinking"]["levels"].as_array();
        let effort = ["none", "minimal", "low"]
            .into_iter()
            .find(|level| levels.is_some_and(|l| l.iter().any(|v| v.as_str() == Some(level))))
            .map(str::to_owned);
        output.push(Model {
            id: id.into(),
            name,
            effort,
        });
    }
    Ok(output)
}
fn request(record: &Record, model: &Model, prompt: &str) -> Value {
    let mut header = json!({"Authorization":"Bearer $TOKEN$", "Content-Type":"application/json"});
    let (url, mut body) = if record.provider == "codex" {
        header["Accept"] = json!("text/event-stream");
        header["User-Agent"] = json!("codex-tui/0.149.1");
        header["Originator"] = json!("codex_cli_rs");
        if !record.account_id.is_empty() {
            header["Chatgpt-Account-Id"] = json!(record.account_id);
        }
        (
            "https://chatgpt.com/backend-api/codex/responses",
            json!({
                "model":model.id,"instructions":"","store":false,"stream":true,
                "input":[{"role":"user","content":[{"type":"input_text","text":prompt}]}]
            }),
        )
    } else {
        header["anthropic-version"] = json!("2023-06-01");
        header["anthropic-beta"] = json!("oauth-2025-04-20");
        (
            "https://api.anthropic.com/v1/messages",
            json!({
                "model":model.id,"max_tokens":6000,"stream":false,
                "messages":[{"role":"user","content":prompt}]
            }),
        )
    };
    if record.provider == "codex" {
        if let Some(effort) = &model.effort {
            body["reasoning"] = json!({"effort":effort});
        }
    }
    json!({"auth_index":record.auth_index,"method":"POST","url":url,"header":header,"data":body.to_string()})
}
fn sample(session: &mut Session, id: &str, model: &str, prompt: &str) -> Result<Answer, String> {
    if id.len() > 1100
        || model.is_empty()
        || model.len() > 200
        || prompt.trim().is_empty()
        || prompt.chars().count() > 4000
    {
        return Err("探查参数无效".into());
    }
    let record = record(session, id)?;
    let selected = models(session, &record)?
        .into_iter()
        .find(|m| m.id == model)
        .ok_or("该凭证已不支持所选模型，请刷新模型列表")?;
    let started = Instant::now();
    let value = session.api.as_ref().unwrap().call_with_timeout(
        Method::POST,
        "/requests/api-call",
        Some(&request(&record, &selected, prompt)),
        Duration::from_secs(120),
    )?;
    let mut answer = response::parse(&record.provider, &value)?;
    answer.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(answer)
}
async fn run<T: Send + 'static>(
    app: tauri::AppHandle,
    f: impl FnOnce(&mut Session) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Management>();
        let mut session = state
            .0
            .try_lock()
            .map_err(|_| "管理操作正在进行，请稍后重试")?;
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
    run(app, move |s| {
        let row = record(s, &id)?;
        models(s, &row)
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
    run(app, move |s| sample(s, &id, &model, &prompt)).await
}
