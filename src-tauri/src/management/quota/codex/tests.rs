use super::*;
use crate::management::{
    normalize,
    tests::{files, server},
};
#[test]
fn free_weekly_window_is_not_a_five_hour_window_and_unknown_is_not_free() {
    let mut row = Credential::default();
    crate::management::quota::parse("codex",&json!({"plan_type":"free","rate_limit":{"primary_window":{"limit_window_seconds":604800,"used_percent":40,"reset_after_seconds":3600}}}),&mut row).unwrap();
    assert_eq!(row.plan.as_deref(), Some("free"));
    assert_eq!(row.five, None);
    assert_eq!(row.week, Some(60.0));
    assert_eq!(row.quota_items.len(), 1);
    assert_eq!(row.quota_items[0].period, "1w");
    assert!(!row.reset_week.is_empty());
    crate::management::quota::parse(
        "codex",
        &json!({"planType":"free","rateLimit":{"primaryWindow":{"usedPercent":0}}}),
        &mut row,
    )
    .unwrap();
    assert_eq!(row.quota_items[0].period, "周期");
    assert_eq!(row.quota_items[0].remaining, Some(100.0));
    assert_eq!(plan(&json!({"plan_type":"private-token"})), None);
    assert_eq!(
        plan(&json!({"id_token":{"plan_type":"pro"}})),
        Some("pro".into())
    );
    let records=normalize(&json!({"files":[{"name":"a.json","provider":"codex","metadata":{"plan_type":"plus"}},{"name":"b.json","provider":"claude","plan_type":"pro"}]})).unwrap();
    assert_eq!(records["a.json:"].row.plan.as_deref(), Some("plus"));
    assert!(records["b.json:"].row.plan.is_none());
}
#[test]
fn counts_preserve_zero_and_reject_missing_fractional_or_invalid_values() {
    for (body, want) in [
        (json!({"available_count":0}), Some(0)),
        (json!({"availableCount":"3"}), Some(3)),
        (json!({}), None),
        (json!({"available_count":-1}), None),
        (json!({"available_count":1.5}), None),
        (json!({"available_count":"NaN"}), None),
        (json!({"credits":[]}), Some(0)),
        (json!({"credits":[{}]}), None),
    ] {
        assert_eq!(reset_count(&body), want);
    }
    assert_eq!(
        reset_count(
            &json!({"credits":[{"reset_type":"codex_rate_limits","status":"available"},{"reset_type":"codex_rate_limits","status":"redeemed"},{"reset_type":"other","status":"available"}]})
        ),
        Some(1)
    );
}
#[test]
fn reset_refresh_only_reads_fixed_endpoint_and_failure_keeps_previous_count() {
    let (port, task) = server(vec![
        (200, json!({"status_code":200,"body":{"available_count":2}})),
        (200, json!({"status_code":403,"body":"private"})),
    ]);
    let api = Api::new(port, "mock-key".into()).unwrap();
    let record = normalize(&files(false))
        .unwrap()
        .remove("mock.json:idx")
        .unwrap();
    let mut row = record.row.clone();
    refresh_resets(&api, &record, &json!({}), &mut row);
    assert_eq!(row.reset_available, Some(2));
    refresh_resets(&api, &record, &json!({}), &mut row);
    assert_eq!(row.reset_available, Some(2));
    assert!(!row.reset_count_error.unwrap().contains("private"));
    for r in task.join().unwrap() {
        let body: Value = serde_json::from_str(r.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["method"], "GET");
        assert_eq!(
            body["url"],
            "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits"
        );
        assert!(!r.contains("/consume"));
    }
}
