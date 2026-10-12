use super::*;
use crate::management::{
    normalize,
    tests::{files, server},
    transport::Api,
    Session,
};
use serde_json::{json, Value};
const ID: &str = "mock.json:idx";
fn session(port: u16) -> Session {
    Session {
        api: Some(Api::new(port, "synthetic-key".into()).unwrap()),
        records: normalize(&files(false)).unwrap(),
        ..Session::default()
    }
}
fn prepare() -> Vec<(u16, Value)> {
    vec![
        (200, files(false)),
        (200, json!({"models":[{"id":"gpt-5.4"}]})),
        (200, json!({"models":[{"id":"gpt-5.4"}]})),
    ]
}
fn answer() -> (u16, Value) {
    (
        200,
        json!({"status_code":200,"body":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"synthetic answer"}]}]}}),
    )
}
fn analyzing(s: &mut Session) {
    s.snapshot.account_tests.insert(
        ID.into(),
        AccountTest {
            run_id: 7,
            model: "gpt-5.4".into(),
            status: "analyzing".into(),
            message: "".into(),
            at: now(),
            identity: identity(&s.records[ID]),
        },
    );
}
#[test]
fn three_samples_remain_pinned_through_oauth_metadata_updates() {
    let mut replies = prepare();
    for i in 0..3 {
        let mut updated = files(false);
        updated["files"][0]["updated_at"] = json!(format!("synthetic-{i}"));
        replies.extend([(200, updated), answer()]);
    }
    replies.push((200, files(false)));
    let (port, task) = server(replies);
    let mut s = session(port);
    let batch = execute(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.is_none());
    assert_eq!(batch.answers.len(), 3);
    assert_eq!(s.snapshot.account_tests[ID].status, "analyzing");
    finish(
        &mut s,
        ID,
        batch.run_id,
        "match".into(),
        "synthetic match".into(),
    )
    .unwrap();
    assert_eq!(s.snapshot.account_tests[ID].status, "match");
    let requests = task.join().unwrap();
    assert_eq!(
        requests.iter().filter(|r| r.starts_with("POST ")).count(),
        3
    );
    for r in requests.iter().filter(|r| r.starts_with("POST ")) {
        assert!(r.starts_with("POST /v8/management/requests/api-call "));
        let body: Value = serde_json::from_str(r.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["auth_index"], "idx");
    }
    assert!(!requests
        .iter()
        .any(|r| r.starts_with("PATCH") || r.contains("/v1/chat/completions")));
}
#[test]
fn failed_sample_stops_without_retry_or_colored_result() {
    let mut replies = prepare();
    replies.extend([
        (200, files(false)),
        (200, json!({"status_code":429,"body":"private-error"})),
    ]);
    let (port, task) = server(replies);
    let mut s = session(port);
    let batch = execute(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.unwrap().contains("429"));
    assert!(batch.answers.is_empty());
    assert_eq!(s.snapshot.account_tests[ID].status, "error");
    assert_eq!(
        task.join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("POST"))
            .count(),
        1
    );
}
#[test]
fn account_replacement_stops_remaining_paid_samples() {
    let mut replies = prepare();
    let mut replaced = files(false);
    replaced["files"][0]["email"] = json!("other@example.invalid");
    replies.extend([(200, files(false)), answer(), (200, replaced)]);
    let (port, task) = server(replies);
    let mut s = session(port);
    let batch = execute(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.unwrap().contains("变化"));
    assert_eq!(batch.answers.len(), 1);
    assert_eq!(
        task.join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("POST"))
            .count(),
        1
    );
}
#[test]
fn final_identity_check_rejects_replaced_account_or_unavailable_list() {
    let mut replaced = files(false);
    replaced["files"][0]["email"] = json!("other@example.invalid");
    for reply in [(200, replaced), (503, json!({}))] {
        let (port, task) = server(vec![reply]);
        let mut s = session(port);
        analyzing(&mut s);
        assert!(finish(&mut s, ID, 7, "match".into(), "match".into()).is_err());
        assert_eq!(s.snapshot.account_tests[ID].status, "error");
        assert_eq!(task.join().unwrap().len(), 1);
    }
}
#[test]
fn duplicates_old_results_and_custom_aliases_do_not_issue_paid_requests() {
    let (port, task) = server(prepare());
    let mut s = session(port);
    analyzing(&mut s);
    assert!(execute(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).is_err());
    assert!(finish(&mut s, ID, 6, "match".into(), "match".into()).is_err());
    assert_eq!(s.snapshot.account_tests[ID].status, "analyzing");
    s.snapshot.account_tests.clear();
    let batch = execute(&mut s, ID, "custom-alias", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.unwrap().contains("别名"));
    assert!(task.join().unwrap().iter().all(|r| r.starts_with("GET")));
}

#[test]
fn disabled_account_with_empty_live_catalog_can_be_tested_without_enabling() {
    let mut replies = vec![
        (200, files(true)),
        (200, json!({"models":[]})),
        (200, json!({"models":[{"id":"gpt-5.4"}]})),
    ];
    for _ in 0..3 {
        replies.extend([(200, files(true)), answer()]);
    }
    let (port, task) = server(replies);
    let mut s = session(port);
    s.records = normalize(&files(true)).unwrap();
    let batch = execute(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.is_none());
    assert_eq!(batch.answers.len(), 3);
    assert!(!s.records[ID].row.enabled);
    assert!(!task.join().unwrap().iter().any(|r| r.starts_with("PATCH")));
}
