//! Third-party OpenAI-compatible endpoint (relay or vendor API) for model probing.
//! The key is stored only in this app's private 0600 file and never returned to the webview.
use super::{parse_models, response, Answer, Model};
use crate::management::local_connection::LocalFile;
use reqwest::{blocking::Client, redirect::Policy, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const FILE: &str = "external-api.json";
#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    base_url: String,
    key: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    saved: bool,
    base_url: String,
}
fn base(raw: &str) -> Result<Url, String> {
    let raw = raw.trim().trim_end_matches('/');
    let url = Url::parse(raw).map_err(|_| "Base URL 格式无效")?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    // Plain HTTP would expose the key on the network; only allow it for this Mac.
    if raw.len() > 2048
        || !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Base URL 需为 https 地址（本机可用 http），且不能包含账号、查询参数或片段".into(),
        );
    }
    Ok(url)
}
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 8192 && key.trim() == key && !key.chars().any(char::is_control)
}
fn request(saved: &Saved, body: Option<&Value>) -> Result<Value, String> {
    let url = base(&saved.base_url)?;
    let (path, timeout) = if body.is_some() {
        ("/chat/completions", 120)
    } else {
        ("/models", 25)
    };
    let url = Url::parse(&format!("{}{path}", url.as_str().trim_end_matches('/')))
        .map_err(|_| "Base URL 格式无效")?;
    // No redirects: a redirect could forward the bearer key to another host.
    let client = Client::builder()
        .redirect(Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(timeout))
        .build()
        .map_err(|_| "无法初始化网络连接")?;
    let request = match body {
        Some(body) => client.post(url).json(body),
        None => client.get(url),
    };
    let response = request
        .bearer_auth(&saved.key)
        .send()
        .map_err(|_| "无法连接第三方 API，或请求已超时".to_owned())?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(match status {
            401 | 403 => "第三方 API 认证失败，请检查 API Key".into(),
            404 => "接口不存在，请确认 Base URL（通常以 /v1 结尾）".into(),
            429 => "第三方 API 请求过于频繁或额度不足，请稍后重试".into(),
            300..=399 => "第三方 API 返回重定向，已停止；请填写最终地址".into(),
            _ => format!("第三方 API 返回 HTTP {status}"),
        });
    }
    crate::management::transport::read_json(response)
}
fn models(saved: &Saved) -> Result<Vec<Model>, String> {
    parse_models(&request(saved, None)?, "data")
}
fn sample(saved: &Saved, model: &str, prompt: &str) -> Result<Answer, String> {
    if model.is_empty()
        || model.len() > 512
        || model.chars().any(char::is_control)
        || prompt.trim().is_empty()
        || prompt.chars().count() > 4000
    {
        return Err("探查参数无效".into());
    }
    let started = Instant::now();
    let payload = json!({"model":model,"stream":false,"max_tokens":6000,
        "messages":[{"role":"user","content":prompt}]});
    let mut answer = response::parse(&request(saved, Some(&payload))?)?;
    answer.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(answer)
}
fn load(store: &LocalFile) -> Result<Saved, String> {
    store
        .read()?
        .ok_or_else(|| "请先填写第三方 API".to_owned())
}
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| "第三方 API 任务未完成".to_owned())?
}
#[tauri::command]
pub async fn external_status(app: tauri::AppHandle) -> Result<Status, String> {
    blocking(move || {
        let saved: Option<Saved> = LocalFile::named(&app, FILE)?.read()?;
        Ok(Status {
            saved: saved.is_some(),
            base_url: saved.map(|s| s.base_url).unwrap_or_default(),
        })
    })
    .await
}
/// Verifies the endpoint by listing models before saving; an empty key reuses the saved one.
#[tauri::command]
pub async fn external_save(
    app: tauri::AppHandle,
    base_url: String,
    key: String,
) -> Result<Vec<Model>, String> {
    blocking(move || {
        let store = LocalFile::named(&app, FILE)?;
        let key = if key.is_empty() {
            load(&store).map_err(|_| "请输入 API Key".to_owned())?.key
        } else {
            key
        };
        if !valid_key(&key) {
            return Err("API Key 无效，不能含首尾空白或控制字符".into());
        }
        let saved = Saved {
            base_url: base(&base_url)?.as_str().trim_end_matches('/').into(),
            key,
        };
        let rows = models(&saved)?;
        store
            .write(&saved)
            .map_err(|_| "已读取模型，但本机文件保存失败".to_owned())?;
        Ok(rows)
    })
    .await
}
#[tauri::command]
pub async fn external_forget(app: tauri::AppHandle) -> Result<(), String> {
    blocking(move || LocalFile::named(&app, FILE)?.remove()).await
}
#[tauri::command]
pub async fn external_models(app: tauri::AppHandle) -> Result<Vec<Model>, String> {
    blocking(move || models(&load(&LocalFile::named(&app, FILE)?)?)).await
}
#[tauri::command]
pub async fn external_sample(
    app: tauri::AppHandle,
    model: String,
    prompt: String,
) -> Result<Answer, String> {
    blocking(move || sample(&load(&LocalFile::named(&app, FILE)?)?, &model, &prompt)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::management::tests::server;
    fn saved(port: u16) -> Saved {
        Saved {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            key: "third-party-secret".into(),
        }
    }
    #[test]
    fn base_url_rejects_plain_remote_http_credentials_and_query() {
        assert!(base("https://relay.example.com/v1/").is_ok());
        assert!(base("http://localhost:3000/v1").is_ok());
        for bad in [
            "http://relay.example.com/v1",
            "https://user:pass@relay.example.com/v1",
            "https://relay.example.com/v1?k=1",
            "ftp://relay.example.com",
            "relay.example.com/v1",
        ] {
            assert!(base(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn lists_and_samples_with_key_under_base_path() {
        let (port, task) = server(vec![
            (200, json!({"data":[{"id":"relay/gpt","owned_by":"openai"},{"id":"other"}]})),
            (200, json!({"model":"relay-returned","choices":[{"message":{"content":"1, 2"},"finish_reason":"stop"}]})),
        ]);
        let s = saved(port);
        assert_eq!(models(&s).unwrap().len(), 2);
        let answer = sample(&s, "relay/gpt", "prompt").unwrap();
        assert_eq!(answer.reported_model, "relay-returned");
        let requests = task.join().unwrap();
        assert!(requests[0].starts_with("GET /v1/models "));
        assert!(requests[1].starts_with("POST /v1/chat/completions "));
        assert!(requests.iter().all(|r| r.contains("Bearer third-party-secret")));
        assert!(requests[1].contains("\"model\":\"relay/gpt\""));
    }
    #[test]
    fn errors_are_redacted_and_redirects_not_followed() {
        let (port, task) = server(vec![(401, json!({"error":"secret detail"})), (302, json!({}))]);
        let s = saved(port);
        assert_eq!(models(&s).err().unwrap(), "第三方 API 认证失败，请检查 API Key");
        assert!(models(&s).err().unwrap().contains("重定向"));
        assert_eq!(task.join().unwrap().len(), 2);
    }
    #[test]
    fn saved_record_roundtrips_privately() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFile::at(dir.path().join(FILE));
        store.write(&saved(1)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert_eq!(load(&store).unwrap().key, "third-party-secret");
        store.remove().unwrap();
        assert!(load(&store).is_err());
    }
}
