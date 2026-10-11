use super::super::{
    tests::{files, server},
    transport::Api,
};
use super::*;
fn session(port: u16) -> Session {
    Session {
        api: Some(Api::new(port, "management-secret".into()).unwrap()),
        ..Session::default()
    }
}
fn catalog() -> Value {
    json!({"data":[
        {"id":"gpt-test","owned_by":"openai"},
        {"id":"claude-test","owned_by":"anthropic"},
        {"id":"gemini-test","owned_by":"google"},
        {"id":"team/custom-alias","owned_by":"custom"}
    ]})
}
fn complete() -> Value {
    json!({"model":"returned-model","choices":[{"message":{"role":"assistant","content":"1, 2, 3","reasoning_content":"private-reasoning"},"finish_reason":"stop"}],"private":"not-projected"})
}
#[test]
fn all_provider_models_and_aliases_come_from_live_proxy_catalog() {
    let (port, task) = server(vec![(200, json!(["client-secret"])), (200, catalog())]);
    let s = session(port);
    let key = client_key(&s).unwrap();
    let rows = models(&s, "", &key).unwrap();
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().any(|m| m.id == "team/custom-alias"));
    assert_eq!(
        rows.iter().find(|m| m.id == "gemini-test").unwrap().brand,
        "gemini"
    );
    let requests = task.join().unwrap();
    assert!(requests[0].starts_with("GET /v8/management/config/access/api-keys "));
    assert!(requests[0].contains("Bearer management-secret"));
    assert!(requests[1].starts_with("GET /v1/models "));
    assert!(requests[1].contains("Bearer client-secret"));
    assert!(!requests[1].contains("management-secret"));
    assert!(!serde_json::to_string(&rows).unwrap().contains("secret"));
}
#[test]
fn sampling_uses_proxy_alias_without_oauth_type_or_provider_restrictions() {
    for provider in ["codex", "gemini", "custom"] {
        let mut rows = files(false);
        rows["files"][0]["provider"] = json!(provider);
        let (port, task) = server(vec![
            (200, rows),
            (200, json!(["client-secret"])),
            (200, catalog()),
            (200, complete()),
        ]);
        let answer = sample(&mut session(port), "", "team/custom-alias", "challenge").unwrap();
        assert_eq!(answer.text, "1, 2, 3");
        assert_eq!(answer.finish, "stop");
        let projection = serde_json::to_string(&answer).unwrap();
        assert!(!projection.contains("private") && !projection.contains("secret"));
        let requests = task.join().unwrap();
        assert!(requests[3].starts_with("POST /v1/chat/completions "));
        assert!(!requests[3].contains("management-secret"));
        let body: Value =
            serde_json::from_str(requests[3].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["model"], "team/custom-alias");
        assert_eq!(body["stream"], false);
        assert_eq!(
            body["messages"],
            json!([{"role":"user","content":"challenge"}])
        );
        assert!(body.get("tools").is_none());
    }
}
#[test]
fn disabled_scope_or_removed_model_never_posts() {
    for disabled in [true, false] {
        let mut replies = vec![(200, files(disabled)), (200, json!(["client-secret"]))];
        if !disabled {
            replies.push((200, json!({"models":[{"id":"gpt-test"}]})));
            replies.push((200, catalog()));
        }
        let (port, task) = server(replies);
        assert!(sample(&mut session(port), "mock.json:idx", "removed", "challenge").is_err());
        assert!(task.join().unwrap().iter().all(|r| r.starts_with("GET ")));
    }
    assert!(sample(&mut Session::default(), "", "model", &"x".repeat(4001)).is_err());
}
#[test]
fn legacy_scope_is_encoded_and_filters_live_catalog_without_pinning() {
    let (port, task) = server(vec![
        (200, json!({"api-keys":["client-secret"]})),
        (200, json!({"models":[{"id":"gemini-test"}]})),
        (200, catalog()),
    ]);
    let mut s = session(port);
    s.api.as_mut().unwrap().legacy = true;
    s.records = super::super::normalize(&files(false)).unwrap();
    s.records.get_mut("mock.json:idx").unwrap().name = "a&b #.json".into();
    let key = client_key(&s).unwrap();
    let rows = models(&s, "mock.json:idx", &key).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "gemini-test");
    let requests = task.join().unwrap();
    assert!(requests[0].starts_with("GET /v0/management/api-keys "));
    assert!(requests[1].starts_with("GET /v0/management/auth-files/models?name=a%26b+%23.json "));
}
#[test]
fn refresh_replaces_catalog_and_invalid_or_duplicate_ids_are_rejected() {
    let (port, task) = server(vec![
        (200, catalog()),
        (
            200,
            json!({"data":[{"id":"new-model"},{"id":"new-model"},{"id":"bad\n"},{"id":""}]}),
        ),
    ]);
    let s = session(port);
    assert_eq!(models(&s, "", "client-secret").unwrap().len(), 4);
    let next = models(&s, "", "client-secret").unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].id, "new-model");
    task.join().unwrap();
}
#[test]
fn key_configuration_is_narrow_validated_and_never_persisted_or_projected() {
    for value in [
        json!([" bad "]),
        json!(["bad\n"]),
        json!([{}]),
        json!({"unexpected":"secret"}),
    ] {
        let (port, task) = server(vec![(200, value)]);
        let error = client_key(&session(port)).unwrap_err();
        assert!(!error.contains("secret"));
        assert_eq!(task.join().unwrap().len(), 1);
    }
    let (port, task) = server(vec![(200, json!([])), (200, catalog())]);
    let s = session(port);
    let key = client_key(&s).unwrap();
    assert!(key.is_empty());
    models(&s, "", &key).unwrap();
    assert!(!task.join().unwrap()[1]
        .to_lowercase()
        .contains("authorization:"));
}
#[test]
fn proxy_errors_and_incomplete_answers_are_not_successes_and_never_retry() {
    for status in [401, 429, 500, 302] {
        let (port, task) = server(vec![
            (200, files(false)),
            (200, json!(["client-secret"])),
            (200, catalog()),
            (status, json!({"error":"SECRET"})),
        ]);
        let err = sample(&mut session(port), "", "gpt-test", "prompt").unwrap_err();
        assert!(!err.contains("SECRET"));
        let requests = task.join().unwrap();
        assert_eq!(
            requests.iter().filter(|r| r.starts_with("POST ")).count(),
            1
        );
    }
    for reason in ["length", "tool_calls", "content_filter", ""] {
        let mut value = complete();
        value["choices"][0]["finish_reason"] = json!(reason);
        assert_eq!(response::parse(&value).unwrap().finish, "incomplete");
    }
    let mut tool = complete();
    tool["choices"][0]["message"]["tool_calls"] = json!([{}]);
    assert_eq!(response::parse(&tool).unwrap().finish, "incomplete");
    for value in [
        json!({"error":"SECRET"}),
        json!({"choices":[]}),
        json!({"choices":[{"message":{"content":null}}]}),
    ] {
        assert!(!response::parse(&value).unwrap_err().contains("SECRET"));
    }
}

#[test]
fn absent_client_keys_allow_service_to_check_unauthenticated_access() {
    for (legacy, status, value) in [
        (false, 404, json!({"error":"not_found"})),
        (false, 200, Value::Null),
        (true, 200, json!({"api-keys":null})),
    ] {
        let (port, task) = server(vec![(status, value), (200, catalog())]);
        let mut s = session(port);
        s.api.as_mut().unwrap().legacy = legacy;
        let key = client_key(&s).unwrap();
        assert!(key.is_empty());
        assert_eq!(models(&s, "", &key).unwrap().len(), 4);
        assert!(!task.join().unwrap()[1]
            .to_lowercase()
            .contains("authorization:"));
    }
}
#[test]
fn concurrent_catalog_waits_for_slow_session_but_sampling_does_not_queue() {
    use std::{
        sync::{mpsc, Arc},
        thread,
        time::Duration,
    };
    let state = Arc::new(Management::default());
    let held = state.0.lock().unwrap();
    assert!(lock_session(&state, false).is_err());
    let other = state.clone();
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _session = lock_session(&other, true).unwrap();
        tx.send(()).unwrap();
    });
    assert!(rx.recv_timeout(Duration::from_millis(1100)).is_err());
    drop(held);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.join().unwrap();
}
