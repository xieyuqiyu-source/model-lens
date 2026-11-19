//! Third-party relay or vendor API for model probing.
//! The key is stored only in this app's private 0600 file and never returned to the webview.
use super::{external_protocol::Protocol, parse_models, Answer, Model};
use crate::management::{local_connection::LocalFile, transport};
use reqwest::{
    blocking::{Client, Response},
    header::CONTENT_TYPE,
    redirect::Policy,
    Url,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

const FILE: &str = "external-api.json";
const LIMIT: usize = 50;
static WRITES: Mutex<()> = Mutex::new(());
#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    base_url: String,
    key: String,
    #[serde(default)]
    protocol: Protocol,
    #[serde(default)]
    model_id: String,
    #[serde(default)]
    revision: String,
}
#[derive(Default, Serialize, Deserialize)]
struct Store {
    entries: Vec<Saved>,
}
// 0.2.24 saved a single `{base_url,key}` object; read it as a one-entry list.
#[derive(Deserialize)]
#[serde(untagged)]
enum OnDisk {
    List(Store),
    Single(Saved),
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    id: String,
    name: String,
    base_url: String,
    protocol: Protocol,
    model_id: String,
    revision: String,
}
#[derive(Serialize)]
pub struct SaveResult {
    id: String,
    models: Vec<Model>,
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
    key.len() <= 8192 && key.trim() == key && !key.chars().any(char::is_control)
}
fn api_base(raw: &str, protocol: Protocol) -> Result<String, String> {
    let url = base(raw)?;
    let full = url.as_str().trim_end_matches('/');
    for candidate in [Protocol::Chat, Protocol::Responses, Protocol::Messages] {
        if url.path().trim_end_matches('/').ends_with(candidate.path()) {
            if candidate != protocol {
                return Err("API 地址中的接口路径与所选协议不一致".into());
            }
            return Ok(full[..full.len() - candidate.path().len()].to_owned());
        }
    }
    if url.path() == "/" && protocol != Protocol::Chat {
        return Ok(format!("{full}/v1"));
    }
    Ok(full.into())
}
fn valid_model(model: &str) -> bool {
    !model.is_empty() && model.len() <= 512 && !model.chars().any(char::is_control)
}
fn request(saved: &Saved, body: Option<&Value>) -> Result<Response, String> {
    let url = base(&saved.base_url)?;
    let (path, timeout) = if body.is_some() {
        (saved.protocol.path(), 120)
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
    let response = saved
        .protocol
        .authorize(request, &saved.key)
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
    Ok(response)
}
fn models(saved: &Saved) -> Result<Vec<Model>, String> {
    if !saved.model_id.is_empty() {
        return Ok(vec![Model {
            id: saved.model_id.clone(),
            name: saved.model_id.clone(),
            brand: "unknown".into(),
        }]);
    }
    parse_models(&transport::read_json(request(saved, None)?)?, "data")
}
fn sample(saved: &Saved, model: &str, prompt: &str) -> Result<Answer, String> {
    if !valid_model(model)
        || (!saved.model_id.is_empty() && saved.model_id != model)
        || prompt.trim().is_empty()
        || prompt.chars().count() > 4000
    {
        return Err("探查参数无效".into());
    }
    let started = Instant::now();
    let payload = saved.protocol.payload(model, prompt);
    let reply = request(saved, Some(&payload))?;
    let is_stream = reply
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"));
    let mut answer = if is_stream {
        saved.protocol.parse_stream(&transport::read_body(reply)?)?
    } else {
        saved.protocol.parse(&transport::read_json(reply)?)?
    };
    answer.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(answer)
}
fn read(store: &LocalFile) -> Result<Store, String> {
    Ok(match store.read::<OnDisk>()? {
        None => Store::default(),
        Some(OnDisk::List(list)) => list,
        Some(OnDisk::Single(mut one)) => {
            one.id = "legacy".into();
            Store { entries: vec![one] }
        }
    })
}
fn load(store: &LocalFile, id: &str) -> Result<Saved, String> {
    read(store)?
        .entries
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| "所选第三方 API 不存在，请重新选择".to_owned())
}
fn host(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default()
}
fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    format!("{nanos:x}{:x}", NEXT.fetch_add(1, Ordering::Relaxed))
}
struct Profile<'a> {
    name: &'a str,
    base_url: &'a str,
    key: String,
    protocol: Protocol,
    model_id: &'a str,
}

/// Verify the catalog when used. An explicit model ID defers authentication to sampling.
fn save(
    store: &LocalFile,
    id: Option<String>,
    profile: Profile<'_>,
    list: impl Fn(&Saved) -> Result<Vec<Model>, String>,
) -> Result<SaveResult, String> {
    let _guard = WRITES.lock().map_err(|_| "第三方 API 配置暂时不可用")?;
    let Profile {
        name,
        base_url,
        key,
        protocol,
        model_id,
    } = profile;
    let mut all = read(store)?;
    let existing = id.as_ref().map(|id| {
        all.entries
            .iter()
            .position(|e| &e.id == id)
            .ok_or_else(|| "所选第三方 API 不存在，请重新选择".to_owned())
    });
    let existing = existing.transpose()?;
    let base_url = api_base(base_url, protocol)?;
    let key = match (key.is_empty(), existing) {
        (true, Some(i)) => {
            let saved = &all.entries[i];
            if !saved.key.is_empty() && base(&saved.base_url)?.origin() != base(&base_url)?.origin()
            {
                return Err(
                    "更换 API 域名或端口时，请重新填写该地址的 Key；免 Key 接口可新建配置".into(),
                );
            }
            saved.key.clone()
        }
        _ => key,
    };
    if !valid_key(&key) {
        return Err("API Key 无效，不能含首尾空白或控制字符".into());
    }
    let name = name.trim();
    if name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err("名称最多 60 个字符".into());
    }
    let model_id = model_id.trim();
    if !model_id.is_empty() && !valid_model(model_id) {
        return Err("模型 ID 不合法".into());
    }
    let entry = Saved {
        id: existing
            .map(|i| all.entries[i].id.clone())
            .unwrap_or_else(new_id),
        name: if name.is_empty() {
            host(&base_url)
        } else {
            name.into()
        },
        base_url,
        key,
        protocol,
        model_id: model_id.into(),
        revision: new_id(),
    };
    if existing.is_none() && all.entries.len() >= LIMIT {
        return Err(format!("最多保存 {LIMIT} 个第三方 API"));
    }
    let models = list(&entry)?;
    let id = entry.id.clone();
    match existing {
        Some(i) => all.entries[i] = entry,
        None => all.entries.push(entry),
    }
    store
        .write(&all)
        .map_err(|_| "已读取模型，但本机文件保存失败".to_owned())?;
    Ok(SaveResult { id, models })
}
fn forget(store: &LocalFile, id: &str) -> Result<(), String> {
    let _guard = WRITES.lock().map_err(|_| "第三方 API 配置暂时不可用")?;
    let mut all = read(store)?;
    let before = all.entries.len();
    all.entries.retain(|e| e.id != id);
    if all.entries.len() == before {
        return Ok(());
    }
    if all.entries.is_empty() {
        store.remove()
    } else {
        store
            .write(&all)
            .map_err(|_| "无法更新本机文件，记录未删除".to_owned())
    }
}
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| "第三方 API 任务未完成".to_owned())?
}
#[tauri::command]
pub async fn external_list(app: tauri::AppHandle) -> Result<Vec<Entry>, String> {
    blocking(move || {
        Ok(read(&LocalFile::named(&app, FILE)?)?
            .entries
            .into_iter()
            .map(|e| Entry {
                id: e.id,
                name: if e.name.is_empty() {
                    host(&e.base_url)
                } else {
                    e.name
                },
                base_url: e.base_url,
                protocol: e.protocol,
                model_id: e.model_id,
                revision: e.revision,
            })
            .collect())
    })
    .await
}
#[tauri::command]
pub async fn external_save(
    app: tauri::AppHandle,
    id: Option<String>,
    name: String,
    base_url: String,
    key: String,
    protocol: Protocol,
    model_id: String,
) -> Result<SaveResult, String> {
    blocking(move || {
        save(
            &LocalFile::named(&app, FILE)?,
            id,
            Profile {
                name: &name,
                base_url: &base_url,
                key,
                protocol,
                model_id: &model_id,
            },
            models,
        )
    })
    .await
}
#[tauri::command]
pub async fn external_forget(app: tauri::AppHandle, id: String) -> Result<(), String> {
    blocking(move || forget(&LocalFile::named(&app, FILE)?, &id)).await
}
#[tauri::command]
pub async fn external_models(app: tauri::AppHandle, id: String) -> Result<Vec<Model>, String> {
    blocking(move || models(&load(&LocalFile::named(&app, FILE)?, &id)?)).await
}
#[tauri::command]
pub async fn external_sample(
    app: tauri::AppHandle,
    id: String,
    revision: String,
    model: String,
    prompt: String,
) -> Result<Answer, String> {
    blocking(move || {
        let saved = load(&LocalFile::named(&app, FILE)?, &id)?;
        if saved.revision != revision {
            return Err("第三方 API 配置已更新，请重新开始检测".into());
        }
        sample(&saved, &model, &prompt)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn save_chat(
        store: &LocalFile,
        id: Option<String>,
        name: &str,
        base_url: &str,
        key: String,
        list: impl Fn(&Saved) -> Result<Vec<Model>, String>,
    ) -> Result<SaveResult, String> {
        save(
            store,
            id,
            Profile {
                name,
                base_url,
                key,
                protocol: Protocol::Chat,
                model_id: "",
            },
            list,
        )
    }
    use crate::management::tests::{server, server_with_bodies};
    fn saved(port: u16) -> Saved {
        Saved {
            id: String::new(),
            name: String::new(),
            base_url: format!("http://127.0.0.1:{port}/v1"),
            key: "third-party-secret".into(),
            protocol: Protocol::Chat,
            model_id: String::new(),
            revision: String::new(),
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
            (
                200,
                json!({"data":[{"id":"relay/gpt","owned_by":"openai"},{"id":"other"}]}),
            ),
            (
                200,
                json!({"model":"relay-returned","choices":[{"message":{"content":"1, 2"},"finish_reason":"stop"}]}),
            ),
        ]);
        let s = saved(port);
        assert_eq!(models(&s).unwrap().len(), 2);
        let answer = sample(&s, "relay/gpt", "prompt").unwrap();
        assert_eq!(answer.reported_model, "relay-returned");
        let requests = task.join().unwrap();
        assert!(requests[0].starts_with("GET /v1/models "));
        assert!(requests[1].starts_with("POST /v1/chat/completions "));
        assert!(requests
            .iter()
            .all(|r| r.contains("Bearer third-party-secret")));
        assert!(requests[1].contains("\"model\":\"relay/gpt\""));
    }
    #[test]
    fn responses_uses_full_endpoint_with_explicit_model_and_no_catalog_request() {
        let (port, task) = server(vec![(
            200,
            json!({"model":"observed","status":"completed","output":[
                {"type":"reasoning","summary":[]},
                {"type":"message","role":"assistant","content":[{"type":"output_text","text":"1, 2"}]}
            ]}),
        )]);
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFile::at(dir.path().join(FILE));
        let url = format!("http://127.0.0.1:{port}/custom/v1/responses");
        assert!(api_base(&url, Protocol::Chat).is_err());
        let result = save(
            &store,
            None,
            Profile {
                name: "Responses relay",
                base_url: &url,
                key: "test-key".into(),
                protocol: Protocol::Responses,
                model_id: "explicit/model",
            },
            models,
        )
        .unwrap();
        assert_eq!(result.models[0].id, "explicit/model");
        let saved = load(&store, &result.id).unwrap();
        assert!(!saved.revision.is_empty());
        assert!(sample(&saved, "other-model", "probe").is_err());
        let answer = sample(&saved, "explicit/model", "probe").unwrap();
        assert_eq!(
            (
                answer.text.as_str(),
                answer.finish.as_str(),
                answer.reported_model.as_str()
            ),
            ("1, 2", "stop", "observed")
        );
        let requests = task.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /custom/v1/responses "));
        assert!(requests[0].contains("Bearer test-key"));
        assert!(requests[0].contains("\"input\":\"probe\""));
        assert!(requests[0].contains("\"store\":false"));
    }
    #[test]
    fn messages_lists_and_samples_with_anthropic_headers() {
        let (port, task) = server(vec![
            (
                200,
                json!({"data":[{"id":"claude-relay","display_name":"Relay Claude"}]}),
            ),
            (
                200,
                json!({"type":"message","model":"observed","stop_reason":"end_turn","content":[
                    {"type":"thinking","thinking":"99"},{"type":"text","text":"1, 2"}
                ]}),
            ),
        ]);
        let mut saved = saved(port);
        saved.protocol = Protocol::Messages;
        assert_eq!(models(&saved).unwrap()[0].name, "Relay Claude");
        let answer = sample(&saved, "claude-relay", "probe").unwrap();
        assert_eq!(
            (answer.text.as_str(), answer.finish.as_str()),
            ("1, 2", "stop")
        );
        let requests = task.join().unwrap();
        assert!(requests[0].starts_with("GET /v1/models "));
        assert!(requests[1].starts_with("POST /v1/messages "));
        assert!(requests.iter().all(|r| {
            let lower = r.to_ascii_lowercase();
            lower.contains("anthropic-version: 2023-06-01")
                && lower.contains("x-api-key: third-party-secret")
                && !lower.contains("authorization:")
        }));
        assert!(requests[1].contains("\"max_tokens\":8192"));
    }
    #[test]
    fn saves_and_probes_without_authorization() {
        let (port, task) = server(vec![
            (200, json!({"data":[{"id":"local-model"}]})),
            (
                200,
                json!({"model":"local-model","choices":[{"message":{"content":"1, 2"},"finish_reason":"stop"}]}),
            ),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFile::at(dir.path().join(FILE));
        let url = format!("http://127.0.0.1:{port}/v1");
        let result = save_chat(&store, None, "", &url, String::new(), models).unwrap();
        assert_eq!(result.models.len(), 1);
        let entry = load(&store, &result.id).unwrap();
        assert!(entry.key.is_empty());
        let answer = sample(&entry, "local-model", "prompt").unwrap();
        assert_eq!(answer.reported_model, "local-model");
        let requests = task.join().unwrap();
        assert!(requests[0].starts_with("GET /v1/models "));
        assert!(requests[1].starts_with("POST /v1/chat/completions "));
        assert!(requests
            .iter()
            .all(|request| !request.to_ascii_lowercase().contains("\r\nauthorization:")));
    }
    #[test]
    fn probes_api_returning_sse_despite_non_stream_request() {
        let stream = concat!(
            ": heartbeat\r\n\r\n",
            "data: {\"model\":\"relay-model\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"1, \"}}]}\r\n\r\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"2\"},\"finish_reason\":\"stop\"}]}\r\n\r\n",
            "data: {\"choices\":[],\"usage\":{}}\r\n\r\n",
            "data: [DONE]\r\n\r\n",
        );
        let (port, task) = server_with_bodies(vec![(
            200,
            "text/event-stream; charset=utf-8",
            stream.into(),
        )]);
        let answer = sample(&saved(port), "relay-model", "prompt").unwrap();
        assert_eq!(answer.text, "1, 2");
        assert_eq!(answer.finish, "stop");
        assert_eq!(answer.reported_model, "relay-model");
        assert!(task.join().unwrap()[0].contains("\"stream\":false"));
    }
    #[test]
    fn errors_are_redacted_and_redirects_not_followed() {
        let (port, task) = server(vec![
            (401, json!({"error":"secret detail"})),
            (302, json!({})),
        ]);
        let s = saved(port);
        assert_eq!(
            models(&s).err().unwrap(),
            "第三方 API 认证失败，请检查 API Key"
        );
        assert!(models(&s).err().unwrap().contains("重定向"));
        assert_eq!(task.join().unwrap().len(), 2);
    }
    fn ok(_: &Saved) -> Result<Vec<Model>, String> {
        Ok(vec![])
    }
    #[test]
    fn multiple_entries_upsert_reuse_key_and_forget_privately() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let store = LocalFile::at(path.clone());
        let a = save_chat(
            &store,
            None,
            "",
            "https://a.example.com/v1/",
            "key-a".into(),
            ok,
        )
        .unwrap()
        .id;
        let b = save_chat(
            &store,
            None,
            "Relay B",
            "https://b.example.com/v1",
            "key-b".into(),
            ok,
        )
        .unwrap()
        .id;
        assert_ne!(a, b);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let all = read(&store).unwrap().entries;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "a.example.com");
        assert_eq!(all[0].base_url, "https://a.example.com/v1");
        assert!(save_chat(
            &store,
            Some(a.clone()),
            "A2",
            "https://a2.example.com/v1",
            String::new(),
            |_| { panic!("old key must not be sent to a new origin") }
        )
        .is_err());
        // Editing with an empty key keeps that entry's own key.
        save_chat(
            &store,
            Some(a.clone()),
            "A2",
            "https://a.example.com:443/custom/v1",
            String::new(),
            ok,
        )
        .unwrap();
        let edited = load(&store, &a).unwrap();
        assert_ne!(edited.revision, all[0].revision);
        assert!(
            edited.key == "key-a" && edited.name == "A2" && edited.base_url.ends_with("/custom/v1")
        );
        assert_eq!(load(&store, &b).unwrap().key, "key-b");
        assert!(save_chat(&store, None, "", "https://c.example.com", " ".into(), ok).is_err());
        assert!(save_chat(
            &store,
            Some("missing".into()),
            "",
            "https://c.example.com",
            "k".into(),
            ok
        )
        .is_err());
        forget(&store, &a).unwrap();
        assert!(load(&store, &a).is_err());
        assert_eq!(read(&store).unwrap().entries.len(), 1);
        forget(&store, &b).unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn failed_verification_does_not_save() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFile::at(dir.path().join(FILE));
        let fail = |_: &Saved| -> Result<Vec<Model>, String> { Err("auth".into()) };
        assert!(save_chat(
            &store,
            None,
            "",
            "https://a.example.com/v1",
            "k".into(),
            fail
        )
        .is_err());
        assert!(read(&store).unwrap().entries.is_empty());
    }
    #[test]
    fn single_record_from_0_2_24_is_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        std::fs::write(
            &path,
            r#"{"base_url":"https://old.example.com/v1","key":"old-key"}"#,
        )
        .unwrap();
        let store = LocalFile::at(path);
        let all = read(&store).unwrap().entries;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "legacy");
        assert!(all[0].protocol == Protocol::Chat && all[0].revision.is_empty());
        save_chat(
            &store,
            None,
            "",
            "https://new.example.com/v1",
            "new-key".into(),
            ok,
        )
        .unwrap();
        assert_eq!(load(&store, "legacy").unwrap().key, "old-key");
        assert_eq!(read(&store).unwrap().entries.len(), 2);
    }
}
