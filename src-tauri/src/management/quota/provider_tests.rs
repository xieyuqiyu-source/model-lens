use super::*;
use crate::management::{normalize, tests::server, Session};

fn record(provider: &str) -> Record {
    normalize(&json!({"files":[{"name":"mock.json","auth_index":"idx","provider":provider,"disabled":true,"account_type":"oauth","project_id":"synthetic-project"}]}))
        .unwrap().remove("mock.json:idx").unwrap()
}
fn reply(body: Value) -> (u16, Value) {
    (200, json!({"status_code":200,"body":body}))
}
fn payload(request: &str) -> Value {
    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}
#[test]
fn provider_support_icons_and_project_projection() {
    for (provider, brand) in [
        ("xai", "grok"),
        ("grok", "grok"),
        ("gemini", "gemini"),
        ("gemini-cli", "gemini"),
        ("antigravity", "gemini"),
    ] {
        let r = record(provider);
        assert!(r.row.supported);
        assert!(!r.row.enabled);
        assert_eq!(r.row.brand, brand);
        assert!(!serde_json::to_string(&r.row)
            .unwrap()
            .contains("synthetic-project"));
        let map = normalize(&json!({"files":[{"name":"key.json","auth_index":"idx","provider":provider,"account_type":"api_key"}]})).unwrap();
        assert!(!map.values().next().unwrap().row.supported);
    }
}
#[test]
fn xai_cycles_and_products_never_become_five_hour_windows() {
    let mut row = Credential::default();
    parse("xai", &json!({"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_MONTHLY","end":"2026-11-01T00:00:00Z"},"creditUsagePercent":25,"productUsage":[{"product":"grok-test","usagePercent":100}]}}), &mut row).unwrap();
    assert_eq!(row.quota_items[0].remaining, Some(75.0));
    assert_eq!(row.quota_items[0].period, "月度");
    assert_eq!(row.quota_items[1].remaining, None);
    assert_eq!(row.quota_items[1].detail, "共享额度已用 100%");
    assert_eq!((row.five, row.week), (None, None));
    parse(
        "grok",
        &json!({"config":{"monthly_limit":{"val":"10000"},"used":{"val":"2500"}}}),
        &mut row,
    )
    .unwrap();
    assert_eq!(row.quota_items[0].remaining, Some(75.0));
    assert_eq!(row.quota_items[0].period, "月度");
    parse(
        "xai",
        &json!({"config":{"credit_usage_percent":"0"}}),
        &mut row,
    )
    .unwrap();
    assert_eq!(row.quota_items[0].period, "周期");
    assert_eq!(row.quota_items[0].remaining, Some(100.0));
    assert!(parse(
        "xai",
        &json!({"config":{"creditUsagePercent":-1}}),
        &mut row
    )
    .is_err());
}
#[test]
fn gemini_preserves_full_zero_unknown_and_amount_only_buckets() {
    let mut row = Credential::default();
    parse("gemini-cli", &json!({"buckets":[
        {"modelId":"gemini-test-pro","tokenType":"REQUESTS","remainingFraction":1,"resetTime":"2026-10-12T00:00:00Z"},
        {"modelId":"gemini-test-flash","remainingFraction":0},
        {"modelId":"unknown","remainingFraction":1.2},
        {"modelId":"amount-only","tokenType":"REQUESTS","remainingAmount":"17"}
    ]}), &mut row).unwrap();
    assert_eq!(
        row.quota_items
            .iter()
            .map(|q| q.remaining)
            .collect::<Vec<_>>(),
        vec![Some(100.0), Some(0.0), None, None]
    );
    assert_eq!(row.quota_items[3].detail, "剩余 17 请求");
    assert_eq!(row.quota_items[0].reset, "2026-10-12T00:00:00Z");
    assert!(parse(
        "gemini",
        &json!({"buckets":[{"remainingFraction":-1}]}),
        &mut row
    )
    .is_err());
    assert!(parse("gemini", &json!({"buckets":[]}), &mut row).is_err());
}
#[test]
fn antigravity_preserves_group_window_names() {
    let mut row = Credential::default();
    parse("antigravity", &json!({"groups":[{"displayName":"Gemini Pro","buckets":[{"window":"five-hour","remainingFraction":0.4},{"window":"weekly","remainingFraction":1}]}]}), &mut row).unwrap();
    assert_eq!(row.quota_items[0].label, "Gemini Pro · 5h");
    assert_eq!(row.quota_items[1].period, "1w");
    assert_eq!(row.quota_items[0].remaining, Some(40.0));
}
#[test]
fn disabled_xai_uses_only_billing_read_and_keeps_errors_redacted() {
    let r = record("xai");
    let (port, task) = server(vec![
        reply(json!({"config":{"creditUsagePercent":20}})),
        (200, json!({"status_code":403,"body":"private-upstream"})),
        (200, json!({"status_code":403,"body":"private-upstream"})),
    ]);
    let api = Api::new(port, "mock-key".into()).unwrap();
    let row = fetch(&api, &r).unwrap();
    assert_eq!(row.quota_items[0].remaining, Some(80.0));
    assert!(!row.enabled);
    let error = fetch(&api, &r).err().unwrap();
    assert!(error.contains("403") && !error.contains("private-upstream"));
    for request in task.join().unwrap() {
        let body = payload(&request);
        assert_eq!(body["auth_index"], "idx");
        assert_eq!(body["method"], "GET");
        assert!([
            "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
            "https://cli-chat-proxy.grok.com/v1/billing"
        ]
        .contains(&body["url"].as_str().unwrap()));
        assert_eq!(body["header"]["Authorization"], "Bearer $TOKEN$");
        assert_eq!(body["header"]["x-xai-token-auth"], "xai-grok-cli");
        assert!(body.get("data").is_none());
    }
}
#[test]
fn gemini_project_discovery_and_legacy_proxy_do_not_read_auth_files() {
    let mut r = record("gemini-cli");
    r.project_id.clear();
    let (port, task) = server(vec![
        reply(json!({"cloudaicompanionProject":"discovered-project"})),
        reply(json!({"buckets":[{"remainingFraction":0.5}]})),
    ]);
    let mut api = Api::new(port, "mock-key".into()).unwrap();
    api.legacy = true;
    assert_eq!(
        fetch(&api, &r).unwrap().quota_items[0].remaining,
        Some(50.0)
    );
    let requests = task.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|r| r.starts_with("POST /v0/management/api-call ")));
    assert!(payload(&requests[0])["url"]
        .as_str()
        .unwrap()
        .ends_with(":loadCodeAssist"));
    let quota = payload(&requests[1]);
    assert_eq!(
        quota["url"],
        "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota"
    );
    assert_eq!(
        serde_json::from_str::<Value>(quota["data"].as_str().unwrap()).unwrap(),
        json!({"project":"discovered-project"})
    );
}
#[test]
fn known_project_skips_discovery_and_missing_project_stops_before_quota() {
    let r = record("gemini");
    let (port, task) = server(vec![reply(json!({"buckets":[{"remainingFraction":0}]}))]);
    fetch(&Api::new(port, "mock-key".into()).unwrap(), &r).unwrap();
    assert_eq!(task.join().unwrap().len(), 1);
    let mut missing = r;
    missing.project_id.clear();
    let (port, task) = server(vec![reply(json!({"allowedTiers":[]}))]);
    assert!(fetch(&Api::new(port, "mock-key".into()).unwrap(), &missing)
        .err()
        .unwrap()
        .contains("项目标识"));
    assert_eq!(task.join().unwrap().len(), 1);
}
#[test]
fn refresh_failure_and_list_reload_keep_new_provider_last_quota() {
    let mut r = record("xai");
    parse(
        "xai",
        &json!({"config":{"creditUsagePercent":20}}),
        &mut r.row,
    )
    .unwrap();
    r.row.quota_at = Some(123);
    let (port, task) = server(vec![
        (200, json!({"status_code":429,"body":"private"})),
        (200, json!({"status_code":429,"body":"private"})),
        (
            200,
            json!({"files":[{"name":"mock.json","auth_index":"idx","provider":"xai","disabled":true}]}),
        ),
    ]);
    let mut session = Session {
        api: Some(Api::new(port, "mock-key".into()).unwrap()),
        ..Session::default()
    };
    session.records.insert(r.row.id.clone(), r);
    assert!(session.quota("mock.json:idx").is_err());
    session.reload().unwrap();
    let row = &session.records["mock.json:idx"].row;
    assert_eq!(row.quota_items[0].remaining, Some(80.0));
    assert_eq!(row.quota_at, Some(123));
    assert!(row.quota_error.as_ref().unwrap().contains("429"));
    task.join().unwrap();
}

#[test]
fn xai_product_contributions_do_not_invent_independent_balances() {
    let mut row = Credential::default();
    parse("xai", &json!({"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY"},"creditUsagePercent":16,"productUsage":[{"product":"Pro","usagePercent":14},{"product":"Flash","usagePercent":2}]}}), &mut row).unwrap();
    assert_eq!(row.quota_items[0].period, "1w");
    assert_eq!(row.quota_items[0].remaining, Some(84.0));
    assert!(row.quota_items[1..].iter().all(|q| q.remaining.is_none()));
    parse(
        "xai",
        &json!({"config":{"productUsage":[{"product":"Pro","usagePercent":14}]}}),
        &mut row,
    )
    .unwrap();
    assert!(row.quota_items.iter().all(|q| q.remaining.is_none()));
}

#[test]
fn xai_monthly_fallback_does_not_masquerade_as_weekly_or_call_models() {
    for primary in [
        json!({"status_code":404,"body":"private"}),
        json!({"status_code":200,"body":{"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY"},"productUsage":[{"product":"pro","usagePercent":14}]}}}),
    ] {
        let (port, task) = server(vec![
            (200, primary),
            reply(json!({"config":{"monthlyLimit":{"val":100},"used":{"val":25}}})),
        ]);
        let row = fetch(&Api::new(port, "mock-key".into()).unwrap(), &record("xai")).unwrap();
        assert_eq!(row.quota_items[0].remaining, Some(75.0));
        assert_eq!(row.quota_items[0].period, "月度");
        assert_eq!(row.week, None);
        let requests = task.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            payload(&requests[1])["url"],
            "https://cli-chat-proxy.grok.com/v1/billing"
        );
        assert!(requests.iter().all(|r| payload(r)["method"] == "GET"));
    }
}
