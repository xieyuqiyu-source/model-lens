use super::{pinned_response, Answer};
use crate::management::{transport::Api, Record, Session};
use reqwest::Method;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) fn prepare(s: &mut Session, id: &str, model: &str) -> Result<Record, String> {
    s.reload()?;
    let mut r = s.records.get(id).ok_or("凭证已不存在")?.clone();
    if !r.row.supported
        || r.auth_index.is_empty()
        || !matches!(
            r.provider.as_str(),
            "codex" | "claude" | "xai" | "grok" | "gemini" | "gemini-cli" | "antigravity"
        )
    {
        return Err("此凭证暂不支持指定账号测试，可使用主界面自动路由探查".into());
    }
    let api = s.api.as_ref().ok_or("管理连接不可用")?;
    let path = if api.legacy {
        "/auth-files/models"
    } else {
        "/credentials/models"
    };
    let visible = api.call_with_query(Method::GET, path, &[("name", &r.name)], None)?;
    let path = if api.legacy {
        "/model-definitions"
    } else {
        "/routing/model-definitions"
    };
    let channel = if r.provider == "gemini-cli" {
        "gemini"
    } else {
        &r.provider
    };
    let originals = api.call(Method::GET, &format!("{path}/{channel}"), None)?;
    let contains = |v: &Value| {
        v["models"]
            .as_array()
            .is_some_and(|xs| xs.iter().any(|m| m["id"] == model))
    };
    // Disabled accounts are removed from the live registry; validate their original provider model instead.
    if (r.row.enabled && !contains(&visible)) || !contains(&originals) {
        return Err("当前账号不支持此原始型号，或所选项是自定义别名；请先选择原始模型".into());
    }
    if matches!(r.provider.as_str(), "gemini" | "gemini-cli" | "antigravity")
        && r.project_id.is_empty()
    {
        let base = if r.provider == "antigravity" {
            "https://daily-cloudcode-pa.googleapis.com"
        } else {
            "https://cloudcode-pa.googleapis.com"
        };
        r.project_id = crate::management::quota::providers::project(
            api,
            &r,
            base,
            &json!({"Authorization":"Bearer $TOKEN$","Content-Type":"application/json"}),
        )?;
    }
    Ok(r)
}
fn request(r: &Record, model: &str, prompt: &str) -> Result<Value, String> {
    let mut header = json!({"Authorization":"Bearer $TOKEN$","Content-Type":"application/json"});
    let (url, body) = match r.provider.as_str() {
        "codex" => {
            header["Accept"] = json!("text/event-stream");
            header["User-Agent"] = json!("codex-tui/0.149.1");
            header["Originator"] = json!("codex_cli_rs");
            if !r.account_id.is_empty() {
                header["Chatgpt-Account-Id"] = json!(r.account_id);
            }
            (
                "https://chatgpt.com/backend-api/codex/responses",
                json!({"model":model,"instructions":"","store":false,"stream":true,
                "input":[{"role":"user","content":[{"type":"input_text","text":prompt}]}]}),
            )
        }
        "claude" => {
            header["anthropic-version"] = json!("2023-06-01");
            header["anthropic-beta"] = json!("oauth-2025-04-20");
            (
                "https://api.anthropic.com/v1/messages",
                json!({"model":model,"max_tokens":6000,"stream":false,"messages":[{"role":"user","content":prompt}]}),
            )
        }
        "xai" | "grok" => {
            header["x-xai-token-auth"] = json!("xai-grok-cli");
            header["x-grok-client-version"] = json!("1.0.46");
            header["User-Agent"] = json!("grok-pager/1.0.46 grok-shell/1.0.46 (macos; aarch64)");
            (
                "https://cli-chat-proxy.grok.com/v1/responses",
                json!({"model":model,"stream":false,"store":false,
            "input":[{"role":"user","content":prompt}],"max_output_tokens":6000}),
            )
        }
        "gemini" | "gemini-cli" | "antigravity" => {
            if r.project_id.is_empty() {
                return Err("该账号未提供项目标识，无法进行指定账号测试".into());
            }
            header["User-Agent"] = json!(if r.provider == "antigravity" {
                "antigravity"
            } else {
                "google-api-nodejs-client/9.15.1"
            });
            (
                if r.provider == "antigravity" {
                    "https://daily-cloudcode-pa.googleapis.com/v1internal:generateContent"
                } else {
                    "https://cloudcode-pa.googleapis.com/v1internal:generateContent"
                },
                json!({"model":model,"project":r.project_id,
                "request":{"contents":[{"role":"user","parts":[{"text":prompt}]}],"generationConfig":{"maxOutputTokens":6000}}}),
            )
        }
        _ => return Err("此凭证暂不支持指定账号测试".into()),
    };
    Ok(
        json!({"auth_index":r.auth_index,"method":"POST","url":url,"header":header,"data":body.to_string()}),
    )
}
pub(super) fn sample(api: &Api, r: &Record, model: &str, prompt: &str) -> Result<Answer, String> {
    let started = Instant::now();
    let value = api.call_with_timeout(
        Method::POST,
        "/requests/api-call",
        Some(&request(r, model, prompt)?),
        Duration::from_secs(120),
    )?;
    let mut answer = pinned_response::parse(&r.provider, &value)?;
    answer.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(answer)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_account_requests_never_toggle_or_include_real_tokens() {
        for provider in ["codex", "claude", "xai", "gemini-cli", "antigravity"] {
            let rows = crate::management::normalize(&json!({"files":[{"name":"mock","provider":provider,"auth_index":"chosen","disabled":true,"project_id":"mock-project"}]})).unwrap();
            let value =
                request(rows.values().next().unwrap(), "model", "synthetic prompt").unwrap();
            assert_eq!(value["auth_index"], "chosen");
            assert_eq!(value["method"], "POST");
            assert_eq!(value["header"]["Authorization"], "Bearer $TOKEN$");
            assert!(!value.to_string().contains("disabled"));
        }
    }
    #[test]
    fn gemini_cli_checks_original_gemini_catalog_and_keeps_known_project() {
        use crate::management::tests::server;
        let (port, task) = server(vec![
            (
                200,
                json!({"files":[{"name":"mock","provider":"gemini-cli","auth_index":"chosen","email":"mock@example.invalid","project_id":"mock-project"}]}),
            ),
            (200, json!({"models":[{"id":"gemini-test"}]})),
            (200, json!({"models":[{"id":"gemini-test"}]})),
        ]);
        let mut s = Session {
            api: Some(Api::new(port, "synthetic-key".into()).unwrap()),
            ..Session::default()
        };
        let r = prepare(&mut s, "mock:chosen", "gemini-test").unwrap();
        assert_eq!(r.project_id, "mock-project");
        let requests = task.join().unwrap();
        assert!(requests[2].starts_with("GET /v8/management/routing/model-definitions/gemini "));
        assert!(requests.iter().all(|r| r.starts_with("GET")));
    }
}
