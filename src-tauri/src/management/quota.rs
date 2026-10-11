use super::{transport::Api, Credential, Record};
use reqwest::Method;
use serde_json::{json, Value};

fn remaining(value: &Value) -> Option<f64> {
    let used = value.as_f64().or_else(|| value.as_str()?.parse().ok())?;
    (used.is_finite() && used >= 0.0)
        .then(|| ((100.0 - used).clamp(0.0, 100.0) * 10.0).round() / 10.0)
}
fn reset(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|v| v.to_string()))
        .unwrap_or_default()
}
pub fn parse(provider: &str, body: &Value, row: &mut Credential) -> Result<(), String> {
    row.five = None;
    row.week = None;
    row.reset_five.clear();
    row.reset_week.clear();
    if provider == "codex" {
        let rate = body.get("rate_limit").ok_or("上游未提供额度窗口")?;
        for (key, fallback) in [("primary_window", 18000), ("secondary_window", 604800)] {
            let window = &rate[key];
            if !window.is_object() {
                continue;
            }
            let seconds = window["limit_window_seconds"].as_u64().unwrap_or(fallback);
            let used = remaining(&window["used_percent"]);
            let at = reset(&window["reset_at"]);
            match seconds {
                18000 => {
                    row.five = used;
                    row.reset_five = at;
                }
                604800 => {
                    row.week = used;
                    row.reset_week = at;
                }
                _ => {}
            }
        }
    } else if provider == "claude" {
        row.five = remaining(&body["five_hour"]["utilization"]);
        row.week = remaining(&body["seven_day"]["utilization"]);
        row.reset_five = reset(&body["five_hour"]["resets_at"]);
        row.reset_week = reset(&body["seven_day"]["resets_at"]);
    }
    if row.five.is_none() && row.week.is_none() {
        return Err("上游未提供可识别的 5h／1w 额度".into());
    }
    Ok(())
}
pub fn fetch(api: &Api, record: &Record) -> Result<Credential, String> {
    let mut header = json!({"Authorization":"Bearer $TOKEN$", "Content-Type":"application/json"});
    let url = match record.provider.as_str() {
        "codex" => {
            header["User-Agent"] = json!("codex-tui/0.149.1");
            if !record.account_id.is_empty() {
                header["Chatgpt-Account-Id"] = json!(record.account_id);
            }
            "https://chatgpt.com/backend-api/wham/usage"
        }
        "claude" => {
            header["anthropic-beta"] = json!("oauth-2025-04-20");
            "https://api.anthropic.com/api/oauth/usage"
        }
        _ => return Err("该厂商的额度查询暂未适配".into()),
    };
    if record.auth_index.is_empty() {
        return Err("该凭证缺少 auth_index，不能安全选择查询账号".into());
    }
    let result = api.call(
        Method::POST,
        "/requests/api-call",
        Some(&json!({
            "auth_index": record.auth_index, "method":"GET", "url":url, "header":header,
        })),
    )?;
    let status = result["status_code"].as_u64().unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("额度查询返回 HTTP {status}，请检查账号状态"));
    }
    let body = match &result["body"] {
        Value::String(text) => serde_json::from_str(text).map_err(|_| "上游额度格式异常")?,
        v if v.is_object() => v.clone(),
        _ => return Err("上游未返回额度数据".into()),
    };
    let mut row = record.row.clone();
    parse(&record.provider, &body, &mut row)?;
    row.quota_at = Some(super::now());
    row.quota_error = None;
    Ok(row)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_are_remaining_and_missing_is_not_zero() {
        let mut row = Credential::default();
        parse("codex", &json!({"rate_limit":{"primary_window":{"used_percent":25,"limit_window_seconds":18000},"secondary_window":{"used_percent":80,"limit_window_seconds":2592000}}}), &mut row).unwrap();
        assert_eq!(row.five, Some(75.0));
        assert_eq!(row.week, None);
        parse(
            "claude",
            &json!({"seven_day":{"utilization":105,"resets_at":"2026-10-15T00:00:00Z"}}),
            &mut row,
        )
        .unwrap();
        assert_eq!(row.five, None);
        assert_eq!(row.week, Some(0.0));
        assert!(parse("claude", &json!({"five_hour":{"utilization":-5}}), &mut row).is_err());
    }
}
