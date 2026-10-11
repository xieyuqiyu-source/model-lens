use super::{providers, remaining, reset, QuotaItem};
use crate::management::{transport::Api, Credential, Record};
use serde_json::{json, Value};

pub(crate) fn plan(body: &Value) -> Option<String> {
    [
        body,
        &body["metadata"],
        &body["attributes"],
        &body["id_token"],
        &body["metadata"]["id_token"],
    ]
    .iter()
    .flat_map(|v| ["plan_type", "planType", "chatgpt_plan_type"].map(|k| &v[k]))
    .filter_map(Value::as_str)
    .map(|v| v.trim().to_ascii_lowercase())
    .find(|v| {
        matches!(
            v.as_str(),
            "free" | "plus" | "pro" | "team" | "business" | "enterprise" | "edu" | "go"
        )
    })
}
fn field<'a>(v: &'a Value, snake: &str, camel: &str) -> &'a Value {
    v.get(snake).filter(|v| !v.is_null()).unwrap_or(&v[camel])
}
pub(super) fn parse_usage(body: &Value, row: &mut Credential) -> Result<(), String> {
    if let Some(value) = plan(body) {
        row.plan = Some(value);
    }
    let rate = field(body, "rate_limit", "rateLimit");
    if !rate.is_object() {
        return Err("上游未提供额度窗口".into());
    }
    for (key, camel, fallback) in [
        ("primary_window", "primaryWindow", 18000),
        ("secondary_window", "secondaryWindow", 604800),
    ] {
        let window = field(rate, key, camel);
        if !window.is_object() {
            continue;
        }
        let seconds = field(window, "limit_window_seconds", "limitWindowSeconds")
            .as_u64()
            .unwrap_or(if row.plan.as_deref() == Some("free") {
                0
            } else {
                fallback
            });
        let value = remaining(field(window, "used_percent", "usedPercent"));
        let mut at = reset(field(window, "reset_at", "resetAt"));
        if at.is_empty() {
            if let Some(offset) = field(window, "reset_after_seconds", "resetAfterSeconds").as_u64()
            {
                if let Some(time) = crate::management::now().checked_add(offset) {
                    at = time.to_string();
                }
            }
        }
        let period = match seconds {
            18000 => "5h".into(),
            604800 => "1w".into(),
            2419200..=2678400 => "月度".into(),
            0 => "周期".into(),
            s if s % 3600 == 0 => format!("{}h", s / 3600),
            s => format!("{s}s"),
        };
        if seconds == 18000 {
            row.five = value;
            row.reset_five = at.clone();
        }
        if seconds == 604800 {
            row.week = value;
            row.reset_week = at.clone();
        }
        row.quota_items.push(QuotaItem {
            label: period.clone(),
            period,
            remaining: value,
            reset: at,
            detail: String::new(),
        });
    }
    if row.quota_items.iter().all(|q| q.remaining.is_none()) {
        return Err("上游未提供可识别的额度".into());
    }
    Ok(())
}
fn count(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|n| *n <= u32::MAX as u64)
}
fn reset_count(body: &Value) -> Option<u64> {
    if let Some(n) = count(field(body, "available_count", "availableCount")) {
        return Some(n);
    }
    // An explicit malformed count must not be replaced by a guessed array count.
    if body.get("available_count").is_some() || body.get("availableCount").is_some() {
        return None;
    }
    let items = body["credits"].as_array()?;
    if items.iter().any(|v| {
        !v.is_object()
            || !v["status"].is_string()
            || !field(v, "reset_type", "resetType").is_string()
    }) {
        return None;
    }
    Some(
        items
            .iter()
            .filter(|v| {
                v["status"] == "available"
                    && field(v, "reset_type", "resetType") == "codex_rate_limits"
            })
            .count() as u64,
    )
}
pub(super) fn refresh_resets(api: &Api, record: &Record, usage: &Value, row: &mut Credential) {
    let inline = reset_count(field(
        usage,
        "rate_limit_reset_credits",
        "rateLimitResetCredits",
    ));
    let result = if let Some(value) = inline {
        Ok(value)
    } else {
        let mut headers = json!({"Authorization":"Bearer $TOKEN$","User-Agent":"codex-tui/0.149.1","Accept":"application/json","OpenAI-Beta":"codex-1","Originator":"Codex Desktop"});
        if !record.account_id.is_empty() {
            headers["Chatgpt-Account-Id"] = json!(record.account_id);
        }
        providers::request(
            api,
            record,
            "GET",
            "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits",
            &headers,
            None,
        )
        .and_then(|v| reset_count(&v).ok_or("服务未提供剩余重置次数".into()))
    };
    match result {
        Ok(n) => {
            row.reset_available = Some(n);
            row.reset_count_at = Some(crate::management::now());
            row.reset_count_error = None;
        }
        Err(e) => row.reset_count_error = Some(e),
    }
}
#[cfg(test)]
mod tests;
