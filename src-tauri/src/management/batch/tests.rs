use super::*;
use crate::management::{normalize, tests::server, transport::Api};
use serde_json::{json, Value};
fn files(a: bool, b: bool) -> Value {
    json!({"files":[{"name":"a.json","auth_index":"a","provider":"codex","disabled":!a},{"name":"b.json","auth_index":"b","provider":"codex","disabled":!b}]})
}
fn setup(
    before: &Value,
    replies: Vec<(u16, Value)>,
) -> (Session, Vec<Target>, std::thread::JoinHandle<Vec<String>>) {
    let (port, task) = server(replies);
    let s = Session {
        api: Some(Api::new(port, "mock-key".into()).unwrap()),
        records: normalize(before).unwrap(),
        ..Session::default()
    };
    let targets = ["a.json:a", "b.json:b"]
        .iter()
        .map(|id| {
            let row = &s.records[*id].row;
            Target {
                id: (*id).into(),
                delete_token: row.delete_token.clone(),
                enabled: row.enabled,
            }
        })
        .collect();
    (s, targets, task)
}
#[test]
fn batch_toggle_preserves_expectations_and_publishes_each_success() {
    let before = files(false, false);
    let (mut s, targets, task) = setup(
        &before,
        vec![
            (200, before.clone()),
            (200, before.clone()),
            (200, before.clone()),
            (200, json!({})),
            (200, files(true, false)),
            (200, files(true, false)),
            (200, files(true, false)),
            (200, json!({})),
            (200, files(true, true)),
        ],
    );
    let mut progress = 0;
    s.batch(&targets, Some(true), |_| progress += 1).unwrap();
    assert_eq!(progress, 2);
    assert!(s.records.values().all(|r| r.row.enabled));
    assert_eq!(
        task.join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PATCH"))
            .count(),
        2
    );
}
#[test]
fn batch_preflight_conflict_and_duplicates_do_not_write() {
    let before = files(false, false);
    let (mut s, targets, task) = setup(&before, vec![(200, files(false, true))]);
    assert!(s
        .batch(&targets, Some(true), |_| {})
        .unwrap_err()
        .contains("修改"));
    assert_eq!(task.join().unwrap().len(), 1);
    assert!(Session::default()
        .batch(
            &[targets[0].clone(), targets[0].clone()],
            Some(true),
            |_| {}
        )
        .is_err());
    assert!(Session::default().batch(&[], None, |_| {}).is_err());
}
#[test]
fn batch_stops_after_first_uncertain_write_and_keeps_observed_state() {
    let before = files(false, false);
    let (mut s, targets, task) = setup(
        &before,
        vec![
            (200, before.clone()),
            (200, before.clone()),
            (200, before.clone()),
            (500, json!({"error":"private"})),
            (200, files(true, false)),
        ],
    );
    let error = s.batch(&targets, Some(true), |_| {}).unwrap_err();
    assert!(error.contains("0/2") && !error.contains("private"));
    assert!(s.records["a.json:a"].row.enabled);
    assert!(!s.records["b.json:b"].row.enabled);
    assert_eq!(
        task.join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PATCH"))
            .count(),
        1
    );
}
#[test]
fn batch_delete_stops_on_later_external_change_with_completed_count() {
    let before = files(false, false);
    let remaining = json!({"files":[before["files"][1].clone()]});
    let changed =
        json!({"files":[{"name":"b.json","auth_index":"b","provider":"codex","disabled":false}]});
    let (mut s, targets, task) = setup(
        &before,
        vec![
            (200, before.clone()),
            (200, before.clone()),
            (200, before.clone()),
            (200, json!({})),
            (200, remaining),
            (200, changed),
        ],
    );
    let error = s.batch(&targets, None, |_| {}).unwrap_err();
    assert!(error.contains("1/2"));
    assert!(!s.records.contains_key("a.json:a"));
    assert_eq!(
        task.join()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("DELETE"))
            .count(),
        1
    );
}

#[test]
fn final_toggle_reload_rechecks_identity_not_only_enabled_state() {
    let before = files(false, false);
    let mut changed = before.clone();
    changed["files"][0]["email"] = json!("changed@example.invalid");
    let (mut s, targets, task) = setup(
        &before,
        vec![(200, before.clone()), (200, before.clone()), (200, changed)],
    );
    assert!(s.batch(&targets, Some(true), |_| {}).is_err());
    assert!(task.join().unwrap().iter().all(|r| r.starts_with("GET")));
}
