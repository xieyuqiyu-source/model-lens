use super::*;
use crate::management::{
    normalize,
    tests::{files, server},
    transport::Api,
};
use serde_json::json;

fn session(port: u16) -> Session {
    Session {
        api: Some(Api::new(port, "mock-key".into()).unwrap()),
        ..Session::default()
    }
}
fn empty() -> Value {
    json!({"files": []})
}
fn imported(name: &str) -> Value {
    json!({"files":[{"name":name,"auth_index":"new","provider":"codex","disabled":true,"source":"file"}]})
}
#[test]
fn import_validates_before_network_and_redacts_content() {
    let mut s = Session::default();
    for (name, contents) in [
        ("../outside.json", r#"{"type":"codex"}"#),
        ("bad.txt", r#"{"type":"codex"}"#),
        (".hidden.json", "{}"),
        ("a.json", "synthetic-secret-malformed"),
        ("a.json", "[]"),
        ("a.json", "{}"),
        ("a.json", r#"{"type":"bad provider"}"#),
    ] {
        let error = s.import_credential(name, contents).unwrap_err();
        assert!(!error.contains("synthetic-secret"));
        assert!(!error.contains("请先连接"));
    }
    assert!(parse_import("a.json", &"x".repeat(MAX_IMPORT_BYTES + 1)).is_err());
    assert!(parse_import("valid.JSON", "\u{feff}{\"type\":\"codex\"}").is_ok());
}

#[test]
fn pasted_import_names_are_safe_and_do_not_use_tokens() {
    let (name, body) = parse_import(
        "",
        r#"{"type":"codex","email":" a+b@example.invalid ","access_token":"synthetic-secret"}"#,
    )
    .unwrap();
    assert_eq!(name, "codex-a+b@example.invalid.json");
    assert_eq!(body["email"], " a+b@example.invalid ");
    for email in [
        json!("../outside/path:\n秘密@example.invalid"),
        json!("é".repeat(400)),
        Value::Null,
        json!(42),
        json!(" "),
    ] {
        let contents = json!({"type":"gemini-cli","email":email,"access_token":"synthetic-secret"})
            .to_string();
        let (name, _) = parse_import("", &contents).unwrap();
        assert!(valid_name(&name));
        assert!(!name.contains("synthetic-secret"));
        if !email.is_string() || email.as_str().unwrap().trim().is_empty() {
            assert!(name.starts_with("gemini-cli-import-"));
        }
    }
    assert!(parse_import("", "broken").is_err());
    assert!(parse_import("", "{}").is_err());
    assert_eq!(
        parse_import(
            "original.json",
            r#"{"type":"codex","email":"a@example.invalid"}"#
        )
        .unwrap()
        .0,
        "original.json"
    );
}

#[test]
fn pasted_import_uses_generated_name_for_write_verification_and_conflicts() {
    let name = "codex-a+b@example.invalid.json";
    let contents = r#"{"type":"codex","email":"a+b@example.invalid"}"#;
    for legacy in [false, true] {
        let (port, task) = server(vec![
            (200, empty()),
            (200, json!({"status":"ok"})),
            (200, imported(name)),
            (200, imported(&name.to_uppercase())),
        ]);
        let mut s = session(port);
        s.api.as_mut().unwrap().legacy = legacy;
        s.import_credential("", contents).unwrap();
        assert!(s
            .import_credential("", contents)
            .unwrap_err()
            .contains("同名"));
        let requests = task.join().unwrap();
        let base = if legacy {
            "/v0/management/auth-files"
        } else {
            "/v8/management/credentials"
        };
        assert!(requests[1].starts_with(&format!(
            "POST {base}?name=codex-a%2Bb%40example.invalid.json "
        )));
        assert_eq!(requests.iter().filter(|r| r.starts_with("POST")).count(), 1);
    }
}
#[test]
fn import_and_delete_use_encoded_names_on_v8_and_v0() {
    let name = "account &all=true.json";
    for (legacy, success) in [(false, 200), (true, 200), (false, 204), (true, 204)] {
        let mut replies = vec![];
        if legacy {
            replies.push((404, json!({})));
        }
        replies.extend([
            (200, empty()),
            (success, json!({"status":"ok"})),
            (200, imported(name)),
            (200, imported(name)),
            (success, json!({"status":"ok"})),
            (200, empty()),
        ]);
        let (port, task) = server(replies);
        let mut s = session(port);
        s.import_credential(
            name,
            r#"{"type":"codex","access_token":"synthetic-import-only","disabled":true}"#,
        )
        .unwrap();
        let row = s.records.values().next().unwrap().row.clone();
        assert!(row.deletable && !row.enabled);
        assert!(!serde_json::to_string(&row)
            .unwrap()
            .contains("synthetic-import-only"));
        s.delete_credential(&row.id, &row.delete_token).unwrap();
        assert!(s.records.is_empty());
        let requests = task.join().unwrap();
        let base = if legacy {
            "/v0/management/auth-files"
        } else {
            "/v8/management/credentials"
        };
        let writes: Vec<_> = requests
            .iter()
            .filter(|r| r.starts_with("POST") || r.starts_with("DELETE"))
            .collect();
        assert_eq!(writes.len(), 2);
        assert!(writes[0].starts_with(&format!("POST {base}?name=account+%26all%3Dtrue.json ")));
        assert!(writes[1].starts_with(&format!("DELETE {base}?name=account+%26all%3Dtrue.json ")));
        assert!(writes[0].contains("synthetic-import-only"));
        assert!(!writes[1].contains("synthetic-import-only"));
    }
}
#[test]
fn duplicate_import_and_changed_delete_never_write() {
    let (port, task) = server(vec![(200, files(false)), (200, files(true))]);
    let mut s = session(port);
    assert!(s
        .import_credential("MOCK.json", r#"{"type":"codex"}"#)
        .unwrap_err()
        .contains("同名"));
    let row = s.records.values().next().unwrap().row.clone();
    assert!(s
        .delete_credential(&row.id, &row.delete_token)
        .unwrap_err()
        .contains("其他界面"));
    assert!(task.join().unwrap().iter().all(|r| r.starts_with("GET")));
}
#[test]
fn delete_rejects_runtime_unsafe_and_shared_files() {
    for payload in [
        json!({"files":[{"name":"runtime.json","auth_index":"idx","runtime_only":true}]}),
        json!({"files":[{"name":"../outside.json","auth_index":"idx","source":"file"}]}),
        json!({"files":[{"name":"config.json","auth_index":"idx","source":"config"}]}),
        json!({"files":[{"name":"shared.json","auth_index":"one"},{"name":"shared.json","auth_index":"two"}]}),
    ] {
        let row = normalize(&payload)
            .unwrap()
            .values()
            .next()
            .unwrap()
            .row
            .clone();
        let (port, task) = server(vec![(200, payload)]);
        assert!(session(port)
            .delete_credential(&row.id, &row.delete_token)
            .is_err());
        assert_eq!(task.join().unwrap().len(), 1);
    }
}
#[test]
fn writes_reconcile_even_on_failure_and_do_not_retry() {
    for deleting in [false, true] {
        let before = if deleting { files(false) } else { empty() };
        let after = if deleting {
            empty()
        } else {
            imported("new.json")
        };
        let (port, task) = server(vec![
            (200, before.clone()),
            (500, json!({"error":"private-secret"})),
            (200, after),
        ]);
        let mut s = session(port);
        let error = if deleting {
            let row = normalize(&before)
                .unwrap()
                .values()
                .next()
                .unwrap()
                .row
                .clone();
            s.delete_credential(&row.id, &row.delete_token).unwrap_err()
        } else {
            s.import_credential("new.json", r#"{"type":"codex"}"#)
                .unwrap_err()
        };
        assert!(error.contains("没有自动重试") && !error.contains("private-secret"));
        assert!(!s.snapshot.stale);
        assert_eq!(s.records.is_empty(), deleting);
        let requests = task.join().unwrap();
        assert_eq!(requests.len(), 3);
    }
}
#[test]
fn acknowledged_write_must_be_verified_and_failed_reload_marks_stale() {
    for deleting in [false, true] {
        for failed_reload in [false, true] {
            let before = if deleting { files(false) } else { empty() };
            let (port, task) = server(vec![
                (200, before.clone()),
                (200, json!({"status":"ok"})),
                (if failed_reload { 500 } else { 200 }, before.clone()),
            ]);
            let mut s = session(port);
            let result = if deleting {
                let row = normalize(&before)
                    .unwrap()
                    .values()
                    .next()
                    .unwrap()
                    .row
                    .clone();
                s.delete_credential(&row.id, &row.delete_token)
            } else {
                s.import_credential("new.json", r#"{"type":"codex"}"#)
            };
            assert!(result.is_err());
            assert_eq!(s.snapshot.stale, failed_reload);
            assert_eq!(task.join().unwrap().len(), 3);
        }
    }
}
#[test]
fn stopped_credentials_can_query_quota_without_toggling() {
    let (port, task) = server(vec![
        (
            200,
            json!({"status_code":200,"body":{"rate_limit":{"primary_window":{"used_percent":30}}}}),
        ),
        (200, json!({"status_code":200,"body":{"available_count":0}})),
    ]);
    let mut s = session(port);
    s.records = normalize(&files(true)).unwrap();
    assert_eq!(s.quota_targets(None), vec!["mock.json:idx"]);
    s.quota("mock.json:idx").unwrap();
    assert!(!s.records["mock.json:idx"].row.enabled);
    assert_eq!(s.records["mock.json:idx"].row.five, Some(70.0));
    let requests = task.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("POST /v8/management/requests/api-call "));
}

#[test]
fn unicode_file_name_collisions_never_upload() {
    for (existing, requested) in [
        ("Équipe.json", "équipe.json"),
        ("équipe.json", "e\u{301}quipe.json"),
        ("straße.json", "STRASSE.json"),
        ("STRAẞE.json", "straße.json"),
        ("Ａccount.json", "account.json"),
    ] {
        let (port, task) = server(vec![(200, imported(existing))]);
        assert!(session(port)
            .import_credential(requested, r#"{"type":"codex"}"#)
            .unwrap_err()
            .contains("同名"));
        assert_eq!(task.join().unwrap().len(), 1);
    }
}
