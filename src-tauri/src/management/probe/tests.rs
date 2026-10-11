use super::super::{tests::server, transport::Api};
use super::*;
fn files(disabled: bool) -> Value {
    let mut value = super::super::tests::files(disabled);
    value["files"][0]["account_type"] = json!("oauth");
    value
}
fn catalog() -> Value {
    json!({"models":[{"id":"gpt-test","display_name":"Test","thinking":{"levels":["high","low","none"]}}]})
}
fn complete() -> Value {
    json!({"status":"completed","model":"gpt-test-returned","output":[{"type":"message","content":[{"type":"output_text","text":"1, 2, 3"}]}]})
}
#[test]
fn selected_credential_catalog_and_sampling_use_fixed_destination() {
    let event = format!("data: {{\"type\":\"response.output_text.delta\",\"delta\":\"ignored\"}}\r\n\r\ndata: {}\r\n\r\n",json!({"type":"response.completed","response":complete()}));
    let (port, task) = server(vec![
        (200, files(false)),
        (200, catalog()),
        (200, catalog()),
        (
            200,
            json!({"status_code":200,"body":event,"header":{"secret":"not-forwarded"}}),
        ),
    ]);
    let mut s = Session {
        api: Some(Api::new(port, "mock-key".into()).unwrap()),
        ..Session::default()
    };
    let answer = sample(&mut s, "mock.json:idx", "gpt-test", "test prompt").unwrap();
    assert_eq!(answer.text, "1, 2, 3");
    assert_eq!(answer.finish, "stop");
    let requests = task.join().unwrap();
    assert!(requests[1].starts_with("GET /v8/management/credentials/models?name=mock.json "));
    let body: Value = serde_json::from_str(requests[3].split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["auth_index"], "idx");
    assert_eq!(
        body["url"],
        "https://chatgpt.com/backend-api/codex/responses"
    );
    assert_eq!(body["header"]["Authorization"], "Bearer $TOKEN$");
    let payload: Value = serde_json::from_str(body["data"].as_str().unwrap()).unwrap();
    assert_eq!(payload["input"].as_array().unwrap().len(), 1);
    assert!(payload.get("tools").is_none());
    assert_eq!(payload["instructions"], "");
    assert_eq!(payload["reasoning"]["effort"], "none");
    assert!(!serde_json::to_string(&answer)
        .unwrap()
        .contains("not-forwarded"));
}
#[test]
fn disabled_or_removed_model_never_sends_upstream_request() {
    for disabled in [true, false] {
        let replies = if disabled {
            vec![(200, files(true))]
        } else {
            vec![(200, files(false)), (200, catalog()), (200, catalog())]
        };
        let (port, task) = server(replies);
        let mut s = Session {
            api: Some(Api::new(port, "mock-key".into()).unwrap()),
            ..Session::default()
        };
        assert!(sample(&mut s, "mock.json:idx", "not-present", "prompt").is_err());
        assert!(task.join().unwrap().iter().all(|r| r.starts_with("GET ")));
    }
    assert!(sample(&mut Session::default(), "id", "model", &"x".repeat(4001)).is_err());
}
#[test]
fn legacy_model_catalog_and_escaped_names() {
    let (port, task) = server(vec![(200, catalog()), (200, catalog())]);
    let mut api = Api::new(port, "mock-key".into()).unwrap();
    api.legacy = true;
    let s = Session {
        api: Some(api),
        ..Session::default()
    };
    let record = Record {
        name: "a&b #.json".into(),
        ..super::super::normalize(&files(false))
            .unwrap()
            .remove("mock.json:idx")
            .unwrap()
    };
    assert_eq!(models(&s, &record).unwrap().len(), 1);
    let request = &task.join().unwrap()[0];
    assert!(request.starts_with("GET /v0/management/auth-files/models?name=a%26b+%23.json "));
}
#[test]
fn claude_payload_is_single_turn_and_response_requires_natural_finish() {
    let mut record = super::super::normalize(&files(false))
        .unwrap()
        .remove("mock.json:idx")
        .unwrap();
    record.provider = "claude".into();
    let model = Model {
        id: "claude-test".into(),
        name: "Test".into(),
        effort: None,
    };
    let body = request(&record, &model, "challenge");
    assert_eq!(body["url"], "https://api.anthropic.com/v1/messages");
    let payload: Value = serde_json::from_str(body["data"].as_str().unwrap()).unwrap();
    assert!(payload.get("system").is_none());
    assert!(payload.get("tools").is_none());
    assert_eq!(payload["messages"].as_array().unwrap().len(), 1);
    for (reason, finish) in [
        ("end_turn", "stop"),
        ("max_tokens", "incomplete"),
        ("tool_use", "incomplete"),
    ] {
        let r=response::parse("claude",&json!({"status_code":200,"body":{"model":"claude-test","stop_reason":reason,"content":[{"type":"thinking","thinking":"hidden"},{"type":"text","text":"1,2,3"}]}})).unwrap();
        assert_eq!(r.finish, finish);
        assert_eq!(r.text, "1,2,3");
    }
}
#[test]
fn error_bodies_and_broken_streams_are_not_results_or_secrets() {
    for body in [
        json!({"status_code":401,"body":"SECRET"}),
        json!({"status_code":200,"body":"data: {broken SECRET}\n\n"}),
        json!({"status_code":200,"body":"data: {\"type\":\"response.failed\",\"error\":\"SECRET\"}\n\n"}),
        json!({"status_code":200,"body":"data: {\"type\":\"response.output_text.delta\",\"delta\":\"SECRET\"}\n\n"}),
    ] {
        let error = response::parse("codex", &body).err().unwrap();
        assert!(!error.contains("SECRET"));
    }
    let mut incomplete = complete();
    incomplete["status"] = json!("incomplete");
    assert_eq!(
        response::parse("codex", &json!({"status_code":200,"body":incomplete}))
            .unwrap()
            .finish,
        "incomplete"
    );
}

#[test]
fn api_key_or_unknown_account_type_is_never_sent_to_oauth_endpoint() {
    for account_type in ["api_key", ""] {
        let mut payload = files(false);
        payload["files"][0]["account_type"] = json!(account_type);
        let (port, task) = server(vec![(200, payload)]);
        let mut s = Session {
            api: Some(Api::new(port, "mock-key".into()).unwrap()),
            ..Session::default()
        };
        assert!(sample(&mut s, "mock.json:idx", "gpt-test", "prompt")
            .err()
            .unwrap()
            .contains("OAuth"));
        let requests = task.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET "));
    }
}

#[test]
fn aliases_and_prefixes_are_not_treated_as_raw_upstream_ids() {
    let aliases = json!({"models":[{"id":"fast"},{"id":"team/gpt-test"},{"id":"gpt-test"}]});
    let (port, task) = server(vec![(200, files(false)), (200, aliases), (200, catalog())]);
    let mut s = Session {
        api: Some(Api::new(port, "mock-key".into()).unwrap()),
        ..Session::default()
    };
    assert!(sample(&mut s, "mock.json:idx", "fast", "prompt").is_err());
    let requests = task.join().unwrap();
    assert!(requests.iter().all(|r| r.starts_with("GET ")));
    assert!(requests[2].starts_with("GET /v8/management/routing/model-definitions/codex "));
}
