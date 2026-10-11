use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
pub(super) fn server(replies: Vec<(u16, Value)>) -> (u16, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut block = [0; 4096];
            loop {
                let n = stream.read(&mut block).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&block[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                    let length = header
                        .lines()
                        .find_map(|s| s.strip_prefix("content-length: ")?.parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            requests.push(String::from_utf8(request).unwrap());
            let body = if status == 204 {
                String::new()
            } else {
                body.to_string()
            };
            write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        requests
    });
    (port, task)
}
pub(super) fn files(disabled: bool) -> Value {
    json!({"files":[{"name":"mock.json","auth_index":"idx","provider":"codex","email":"demo@example.invalid","disabled":disabled}]})
}
#[test]
fn projection_never_exposes_credentials() {
    let p = json!({"files":[{"name":"mock.json","auth_index":"idx","provider":"codex","disabled":false,"account":"SECRET_ACCOUNT","access_token":"SECRET_TOKEN","path":"SECRET_PATH","id_token":{"chatgpt_account_id":"private-id"}}]});
    let map = normalize(&p).unwrap();
    let out = serde_json::to_string(&map.values().next().unwrap().row).unwrap();
    for secret in [
        "SECRET_ACCOUNT",
        "SECRET_TOKEN",
        "SECRET_PATH",
        "private-id",
    ] {
        assert!(!out.contains(secret));
    }
    assert!(normalize(&json!({"error":"no"})).is_err());
    assert!(normalize(&json!({"files":[p["files"][0],p["files"][0]]})).is_err());
}
#[test]
fn legacy_fallback_and_http_error_redaction() {
    let (port, task) = server(vec![
        (404, json!({})),
        (200, files(false)),
        (401, json!({"error":"secret-must-not-escape"})),
    ]);
    let mut api = Api::new(port, "mock-key".into()).unwrap();
    assert!(api.list().is_ok());
    assert!(api.legacy);
    let error = api.list().unwrap_err();
    assert!(!error.contains("secret-must-not-escape"));
    let requests = task.join().unwrap();
    assert!(requests[0].starts_with("GET /v8/management/credentials "));
    assert!(requests[1].starts_with("GET /v0/management/auth-files "));
}
#[test]
fn write_failure_reconciles_without_retry() {
    let (port, task) = server(vec![
        (200, files(false)),
        (500, json!({"error":"private"})),
        (200, files(true)),
    ]);
    let mut session = Session {
        api: Some(Api::new(port, "mock-key".into()).unwrap()),
        ..Session::default()
    };
    assert!(session.toggle("mock.json:idx", true, false).is_err());
    assert!(!session.records["mock.json:idx"].row.enabled);
    assert!(!session.snapshot.stale);
    let requests = task.join().unwrap();
    assert_eq!(
        requests.iter().filter(|r| r.starts_with("PATCH")).count(),
        1
    );
    assert!(requests[1].contains("\"auth_index\":\"idx\""));
}
#[test]
fn external_change_prevents_write() {
    let (port, task) = server(vec![(200, files(true))]);
    let mut session = Session {
        api: Some(Api::new(port, "mock-key".into()).unwrap()),
        ..Session::default()
    };
    assert!(session
        .toggle("mock.json:idx", true, false)
        .unwrap_err()
        .contains("其他界面"));
    assert_eq!(task.join().unwrap().len(), 1);
}
#[test]
fn quota_uses_fixed_url_and_token_placeholder() {
    let (port, task) = server(vec![(
        200,
        json!({"status_code":200,"body":"{\"rate_limit\":{\"primary_window\":{\"used_percent\":20}}}"}),
    )]);
    let api = Api::new(port, "mock-key".into()).unwrap();
    let map = normalize(&files(false)).unwrap();
    let row = quota::fetch(&api, &map["mock.json:idx"]).unwrap();
    assert_eq!(row.five, Some(80.0));
    let requests = task.join().unwrap();
    assert!(requests[0].contains("https://chatgpt.com/backend-api/wham/usage"));
    assert!(requests[0].contains("Bearer $TOKEN$"));
}
