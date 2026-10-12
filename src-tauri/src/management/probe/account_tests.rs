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
        snapshot: crate::management::Snapshot {
            connected: true,
            ..Default::default()
        },
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
            credential_id: ID.into(),
            credential: "demo@example.invalid".into(),
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
    let batch = run_test(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.is_none());
    assert_eq!(batch.samples.len(), 3);
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
    let batch = run_test(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.unwrap().contains("429"));
    assert!(batch.samples.is_empty());
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
    let batch = run_test(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.unwrap().contains("变化"));
    assert_eq!(batch.samples.len(), 1);
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
    assert!(run_test(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).is_err());
    assert!(finish(&mut s, ID, 6, "match".into(), "match".into()).is_err());
    assert_eq!(s.snapshot.account_tests[ID].status, "analyzing");
    s.snapshot.account_tests.clear();
    let batch = run_test(&mut s, ID, "custom-alias", vec!["prompt".into(); 3], |_| {}).unwrap();
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
    let batch = run_test(&mut s, ID, "gpt-5.4", vec!["prompt".into(); 3], |_| {}).unwrap();
    assert!(batch.error.is_none());
    assert_eq!(batch.samples.len(), 3);
    assert!(!s.records[ID].row.enabled);
    assert!(!task.join().unwrap().iter().any(|r| r.starts_with("PATCH")));
}

fn run_test(
    s: &mut Session,
    id: &str,
    model: &str,
    prompts: Vec<String>,
    _: impl FnMut(&mut Session),
) -> Result<Batch, String> {
    reserve(s, id, model)?;
    let challenges = prompts
        .into_iter()
        .enumerate()
        .map(|(i, prompt)| Challenge {
            id: i.to_string(),
            prompt,
            expected_count: 300 + i,
        })
        .collect();
    execute(s, id, model, challenges, |_| Ok(()))
}
#[test]
fn three_slots_are_shared_and_old_jobs_cannot_overwrite_after_disconnect() {
    let mut s = session(1);
    let record = s.records[ID].clone();
    for i in 0..4 {
        s.records.insert(format!("account-{i}"), record.clone());
    }
    for i in 0..3 {
        reserve(&mut s, &format!("account-{i}"), "gpt-5.4").unwrap();
    }
    assert!(reserve(&mut s, "account-0", "gpt-5.4").is_err());
    assert!(reserve(&mut s, "account-3", "gpt-5.4").is_err());
    assert!(!s.snapshot.busy);
    let mut job = job_session(&s, "account-0");
    job.snapshot
        .account_tests
        .get_mut("account-0")
        .unwrap()
        .status = "match".into();
    remember(&mut job, "account-0");
    sync_job(&mut s, &job, "account-0").unwrap();
    reserve(&mut s, "account-3", "gpt-5.4").unwrap();
    let old = job_session(&s, "account-3");
    s.disconnect();
    assert!(sync_job(&mut s, &old, "account-3").is_err());
    assert!(s.snapshot.account_history.iter().all(|h| !active(&h.test)));
}
#[test]
fn history_keeps_active_jobs_when_new_completions_fill_the_list() {
    let mut s = session(1);
    reserve(&mut s, ID, "gpt-5.4").unwrap();
    let running = s.snapshot.account_tests[ID].run_id;
    for i in 0..12 {
        let id = format!("other-{i}");
        s.records.insert(id.clone(), s.records[ID].clone());
        reserve(&mut s, &id, "gpt-5.4").unwrap();
        s.snapshot.account_tests.get_mut(&id).unwrap().status = "error".into();
        remember(&mut s, &id);
    }
    assert_eq!(s.snapshot.account_history.len(), 10);
    assert!(s
        .snapshot
        .account_history
        .iter()
        .any(|h| h.test.run_id == running));
}

#[test]
fn provider_requests_overlap_for_three_jobs_without_holding_management_lock() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{mpsc, Arc, Barrier, Mutex},
        thread,
        time::Duration,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let payload = json!({"files":(0..3).map(|i|json!({"name":format!("account-{i}"),"auth_index":format!("idx-{i}"),"provider":"codex","email":format!("{i}@example.invalid")})).collect::<Vec<_>>()});
    let mut session = session(port);
    session.records = normalize(&payload).unwrap();
    let shared = Arc::new(Mutex::new(session));
    let gate = Arc::new(Barrier::new(4));
    let (sent, received) = mpsc::channel();
    let server_gate = gate.clone();
    let server = thread::spawn(move || {
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut handlers = Vec::new();
        for _ in 0..27 {
            let (mut stream, _) = listener.accept().unwrap();
            let payload = payload.clone();
            let count = count.clone();
            let gate = server_gate.clone();
            let sent = sent.clone();
            handlers.push(thread::spawn(move || {
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut block = [0; 4096];
                loop {
                    let n = stream.read(&mut block).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&block[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        let length = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: ")?.parse::<usize>().ok())
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                let body = if request.starts_with("POST ") {
                    if count.fetch_add(1, Ordering::SeqCst) < 3 {
                        sent.send(()).unwrap();
                        gate.wait();
                    }
                    answer().1
                } else if request.starts_with("GET /v8/management/credentials ") {
                    payload
                } else {
                    json!({"models":[{"id":"gpt-5.4"}]})
                };
                let body = body.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }));
        }
        for h in handlers {
            h.join().unwrap();
        }
        assert_eq!(count.load(Ordering::SeqCst), 9);
    });
    let mut workers = Vec::new();
    for i in 0..3 {
        let shared = shared.clone();
        workers.push(thread::spawn(move || {
            let id = format!("account-{i}:idx-{i}");
            let mut job = {
                let mut s = shared.lock().unwrap();
                reserve(&mut s, &id, "gpt-5.4").unwrap();
                job_session(&s, &id)
            };
            let challenges = (0..3)
                .map(|j| Challenge {
                    id: j.to_string(),
                    prompt: "synthetic".into(),
                    expected_count: 300 + j,
                })
                .collect();
            let batch = execute(&mut job, &id, "gpt-5.4", challenges, |job| {
                sync_job(&mut shared.lock().unwrap(), job, &id)
            })
            .unwrap();
            assert!(batch.error.is_none());
            assert_eq!(batch.samples.len(), 3);
        }));
    }
    for _ in 0..3 {
        received
            .recv_timeout(Duration::from_secs(5))
            .expect("all three provider requests must overlap");
    }
    assert!(
        !shared
            .try_lock()
            .expect("provider requests must release management lock")
            .snapshot
            .busy
    );
    gate.wait();
    for worker in workers {
        worker.join().unwrap();
    }
    server.join().unwrap();
    let s = shared.lock().unwrap();
    assert_eq!(s.snapshot.account_history.len(), 3);
    assert!(s
        .snapshot
        .account_history
        .iter()
        .all(|h| h.samples.len() == 3));
}
#[test]
fn history_eviction_restores_a_test_entrypoint_and_final_shared_change_returns_error() {
    let mut s = session(1);
    for i in 0..11 {
        let id = format!("finished-{i}");
        s.records.insert(id.clone(), s.records[ID].clone());
        reserve(&mut s, &id, "gpt-5.4").unwrap();
        s.snapshot.account_tests.get_mut(&id).unwrap().status = "unknown".into();
        remember(&mut s, &id);
    }
    assert!(!s.snapshot.account_tests.contains_key("finished-0"));
    reserve(&mut s, ID, "gpt-5.4").unwrap();
    s.snapshot.account_tests.get_mut(ID).unwrap().status = "analyzing".into();
    remember(&mut s, ID);
    let mut job = job_session(&s, ID);
    job.snapshot.account_tests.get_mut(ID).unwrap().status = "match".into();
    remember(&mut job, ID);
    s.records.get_mut(ID).unwrap().row.name = "replacement@example.invalid".into();
    assert!(commit_finished(&mut s, &mut job, ID, Ok(())).is_err());
    assert_eq!(s.snapshot.account_tests[ID].status, "error");
    assert_eq!(s.snapshot.account_history[0].test.status, "error");
}

#[test]
fn historical_retry_rejects_replaced_account_or_disconnected_session() {
    let mut s = session(1);
    let run = reserve(&mut s, ID, "gpt-5.4").unwrap();
    assert!(validate_retry(&s, ID, Some(run)).is_ok());
    assert!(validate_retry(&s, ID, Some(run + 1)).is_err());
    s.records.get_mut(ID).unwrap().row.name = "changed@example.invalid".into();
    assert!(validate_retry(&s, ID, Some(run)).is_err());
    s.snapshot.account_tests.clear();
    assert!(validate_retry(&s, ID, Some(run)).is_err());
    assert!(validate_retry(&s, ID, None).is_ok());
}
