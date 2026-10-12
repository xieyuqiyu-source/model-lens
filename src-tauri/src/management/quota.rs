pub(super) mod codex;
#[cfg(test)]
mod provider_tests;
pub(super) mod providers;

use super::{transport::Api, Credential, Record};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaItem {
    pub label: String,
    pub remaining: Option<f64>,
    pub detail: String,
    pub reset: String,
    pub period: String,
}

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
    row.quota_items.clear();
    row.five = None;
    row.week = None;
    row.reset_five.clear();
    row.reset_week.clear();
    if matches!(
        provider,
        "xai" | "grok" | "gemini" | "gemini-cli" | "antigravity"
    ) {
        row.quota_items = providers::parse(provider, body)?;
        return Ok(());
    }
    if provider == "codex" {
        codex::parse_usage(body, row)?;
    } else if provider == "claude" {
        row.five = remaining(&body["five_hour"]["utilization"]);
        row.week = remaining(&body["seven_day"]["utilization"]);
        row.reset_five = reset(&body["five_hour"]["resets_at"]);
        row.reset_week = reset(&body["seven_day"]["resets_at"]);
    }
    if row.five.is_none()
        && row.week.is_none()
        && row.quota_items.iter().all(|q| q.remaining.is_none())
    {
        return Err("上游未提供可识别的 5h／1w 额度".into());
    }
    Ok(())
}
pub fn fetch(api: &Api, record: &Record) -> Result<Credential, String> {
    if !record.row.supported || record.auth_index.is_empty() {
        return Err("该凭证不支持额度查询或缺少 auth_index".into());
    }
    let body = providers::fetch(api, record)?;
    let mut row = record.row.clone();
    parse(&record.provider, &body, &mut row)?;
    if record.provider == "codex" {
        codex::refresh_resets(api, record, &body, &mut row);
    }
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
