//! Third-party OpenAI-compatible endpoint (relay or vendor API) for model probing.
//! The key is stored only in this app's private 0600 file and never returned to the webview.
use super::{parse_models, response, Answer, Model};
use crate::management::local_connection::LocalFile;
use reqwest::{blocking::Client, redirect::Policy, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const FILE: &str = "external-api.json";
const LIMIT: usize = 50;
#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    base_url: String,
    key: String,
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
/// Validates, verifies by listing models, then upserts. Empty key keeps the edited entry's key.
fn save(
    store: &LocalFile,
    id: Option<String>,
    name: &str,
    base_url: &str,
    key: String,
    list: impl Fn(&Saved) -> Result<Vec<Model>, String>,
) -> Result<SaveResult, String> {
    let mut all = read(store)?;
    let existing = id.as_ref().map(|id| {
        all.entries
            .iter()
            .position(|e| &e.id == id)
            .ok_or_else(|| "所选第三方 API 不存在，请重新选择".to_owned())
    });
    let existing = existing.transpose()?;
    let key = match (key.is_empty(), existing) {
        (true, Some(i)) => all.entries[i].key.clone(),
        (true, None) => return Err("请输入 API Key".into()),
        (false, _) => key,
    };
    if !valid_key(&key) {
        return Err("API Key 无效，不能含首尾空白或控制字符".into());
    }
    let name = name.trim();
    if name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err("名称最多 60 个字符".into());
    }
    let base_url: String = base(base_url)?.as_str().trim_end_matches('/').into();
    let entry = Saved {
        id: existing.map(|i| all.entries[i].id.clone()).unwrap_or_else(new_id),
        name: if name.is_empty() { host(&base_url) } else { name.into() },
        base_url,
        key,
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
                name: if e.name.is_empty() { host(&e.base_url) } else { e.name },
                base_url: e.base_url,
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
) -> Result<SaveResult, String> {
    blocking(move || save(&LocalFile::named(&app, FILE)?, id, &name, &base_url, key, models)).await
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
    model: String,
    prompt: String,
) -> Result<Answer, String> {
    blocking(move || sample(&load(&LocalFile::named(&app, FILE)?, &id)?, &model, &prompt)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::management::tests::server;
    fn saved(port: u16) -> Saved {
        Saved {
            id: String::new(),
            name: String::new(),
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
    fn ok(_: &Saved) -> Result<Vec<Model>, String> {
        Ok(vec![])
    }
    #[test]
    fn multiple_entries_upsert_reuse_key_and_forget_privately() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let store = LocalFile::at(path.clone());
        let a = save(&store, None, "", "https://a.example.com/v1/", "key-a".into(), ok).unwrap().id;
        let b = save(&store, None, "Relay B", "https://b.example.com/v1", "key-b".into(), ok).unwrap().id;
        assert_ne!(a, b);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let all = read(&store).unwrap().entries;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "a.example.com");
        assert_eq!(all[0].base_url, "https://a.example.com/v1");
        // Editing with an empty key keeps that entry's own key.
        save(&store, Some(a.clone()), "A2", "https://a2.example.com/v1", String::new(), ok).unwrap();
        let edited = load(&store, &a).unwrap();
        assert!(edited.key == "key-a" && edited.name == "A2" && edited.base_url.contains("a2."));
        assert_eq!(load(&store, &b).unwrap().key, "key-b");
        assert!(save(&store, None, "", "https://c.example.com", String::new(), ok).is_err());
        assert!(save(&store, Some("missing".into()), "", "https://c.example.com", "k".into(), ok).is_err());
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
        assert!(save(&store, None, "", "https://a.example.com/v1", "k".into(), fail).is_err());
        assert!(read(&store).unwrap().entries.is_empty());
    }
    #[test]
    fn single_record_from_0_2_24_is_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        std::fs::write(&path, r#"{"base_url":"https://old.example.com/v1","key":"old-key"}"#).unwrap();
        let store = LocalFile::at(path);
        let all = read(&store).unwrap().entries;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "legacy");
        save(&store, None, "", "https://new.example.com/v1", "new-key".into(), ok).unwrap();
        assert_eq!(load(&store, "legacy").unwrap().key, "old-key");
        assert_eq!(read(&store).unwrap().entries.len(), 2);
    }
}
