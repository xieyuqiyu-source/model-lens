use super::{remaining, reset, QuotaItem};
use crate::management::{text, transport::Api, Record};
use reqwest::Method;
use serde_json::{json, Value};

fn number(v: &Value) -> Option<f64> {
    let n = v.as_f64().or_else(|| v.as_str()?.parse().ok())?;
    (n.is_finite() && n >= 0.0).then_some(n)
}
fn field<'a>(v: &'a Value, camel: &str, snake: &str) -> &'a Value {
    v.get(camel).filter(|v| !v.is_null()).unwrap_or(&v[snake])
}
fn fraction(v: &Value) -> Option<f64> {
    number(v)
        .filter(|n| *n <= 1.0)
        .map(|n| (n * 1000.0).round() / 10.0)
}
fn period(v: &str) -> String {
    match v {
        "weekly" | "week" | "1w" | "USAGE_PERIOD_TYPE_WEEKLY" => "1w",
        "monthly" | "month" | "USAGE_PERIOD_TYPE_MONTHLY" => "月度",
        "5h" | "five-hour" | "five_hour" => "5h",
        _ => "周期",
    }
    .into()
}
fn label(v: &Value, key: &str, fallback: &str) -> String {
    let s = text(v, key);
    if s.is_empty() {
        fallback.into()
    } else {
        s
    }
}
pub(super) fn parse(provider: &str, body: &Value) -> Result<Vec<QuotaItem>, String> {
    let mut items = Vec::new();
    match provider {
        "xai" | "grok" => {
            let c = &body["config"];
            let cycle = field(c, "currentPeriod", "current_period");
            let p = period(cycle["type"].as_str().unwrap_or_default());
            if let Some(value) = remaining(field(c, "creditUsagePercent", "credit_usage_percent")) {
                items.push(QuotaItem {
                    label: format!("Grok · {p}额度"),
                    remaining: Some(value),
                    reset: reset(&cycle["end"]),
                    period: p.clone(),
                    ..QuotaItem::default()
                });
            }
            if let Some(products) = field(c, "productUsage", "product_usage").as_array() {
                for product in products {
                    if let Some(value) = number(field(product, "usagePercent", "usage_percent")) {
                        items.push(QuotaItem {
                            label: label(product, "product", "产品使用量"),
                            remaining: None,
                            detail: format!("共享额度已用 {value}%"),
                            reset: reset(&cycle["end"]),
                            period: p.clone(),
                            ..QuotaItem::default()
                        });
                    }
                }
            }
            let cents = |v: &Value| number(v).or_else(|| number(&v["val"]));
            if let (Some(limit), Some(used)) = (
                cents(field(c, "monthlyLimit", "monthly_limit")),
                cents(&c["used"]),
            ) {
                if limit > 0.0 {
                    items.push(QuotaItem {
                        label: "Grok · 月度额度".into(),
                        remaining: remaining(&json!(used / limit * 100.0)),
                        reset: reset(field(c, "billingPeriodEnd", "billing_period_end")),
                        period: "月度".into(),
                        ..QuotaItem::default()
                    });
                }
            }
        }
        "gemini" | "gemini-cli" => {
            if let Some(buckets) = body["buckets"].as_array() {
                for bucket in buckets {
                    let value = fraction(field(bucket, "remainingFraction", "remaining_fraction"));
                    let amount = number(field(bucket, "remainingAmount", "remaining_amount"));
                    let token = field(bucket, "tokenType", "token_type")
                        .as_str()
                        .unwrap_or_default();
                    let unit = match token {
                        "REQUESTS" => "请求",
                        "INPUT_TOKENS" => "输入 token",
                        "OUTPUT_TOKENS" => "输出 token",
                        "" => "额度",
                        other => other,
                    };
                    items.push(QuotaItem {
                        label: format!("{} · {unit}", label(bucket, "modelId", "Gemini")),
                        remaining: value,
                        detail: amount
                            .map(|a| format!("剩余 {a} {unit}"))
                            .unwrap_or_default(),
                        reset: reset(field(bucket, "resetTime", "reset_time")),
                        period: "模型".into(),
                    });
                }
            }
        }
        "antigravity" => {
            if let Some(groups) = body["groups"].as_array() {
                for group in groups {
                    if let Some(buckets) = group["buckets"].as_array() {
                        for bucket in buckets {
                            let p = period(bucket["window"].as_str().unwrap_or_default());
                            items.push(QuotaItem {
                                label: format!(
                                    "{} · {p}",
                                    label(group, "displayName", "Gemini 分组")
                                ),
                                remaining: fraction(field(
                                    bucket,
                                    "remainingFraction",
                                    "remaining_fraction",
                                )),
                                reset: reset(field(bucket, "resetTime", "reset_time")),
                                period: p,
                                ..QuotaItem::default()
                            });
                        }
                    }
                }
            }
        }
        _ => {}
    }
    if !items
        .iter()
        .any(|q| q.remaining.is_some() || !q.detail.is_empty())
    {
        return Err("上游未提供可识别的额度；该账号可能不支持此查询".into());
    }
    Ok(items)
}

fn request(
    api: &Api,
    record: &Record,
    method: &str,
    url: &str,
    header: &Value,
    data: Option<Value>,
) -> Result<Value, String> {
    let mut payload =
        json!({"auth_index":record.auth_index,"method":method,"url":url,"header":header});
    if let Some(data) = data {
        payload["data"] = json!(data.to_string());
    }
    let response = api.call(Method::POST, "/requests/api-call", Some(&payload))?;
    let status = response["status_code"].as_u64().unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("额度查询返回 HTTP {status}，请检查账号状态"));
    }
    match &response["body"] {
        Value::String(s) => serde_json::from_str(s).map_err(|_| "上游额度格式异常".into()),
        v if v.is_object() => Ok(v.clone()),
        _ => Err("上游未返回额度数据".into()),
    }
}
pub(super) fn fetch(api: &Api, record: &Record) -> Result<Value, String> {
    let mut header = json!({"Authorization":"Bearer $TOKEN$","Content-Type":"application/json"});
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
        "xai" | "grok" => {
            header["x-xai-token-auth"] = json!("xai-grok-cli");
            header["x-grok-client-version"] = json!("1.0.46");
            header["accept"] = json!("*/*");
            header["User-Agent"] = json!("grok-pager/1.0.46 grok-shell/1.0.46 (macos; aarch64)");
            "https://cli-chat-proxy.grok.com/v1/billing?format=credits"
        }
        "gemini" | "gemini-cli" | "antigravity" => {
            let antigravity = record.provider == "antigravity";
            let base = if antigravity {
                "https://daily-cloudcode-pa.googleapis.com"
            } else {
                "https://cloudcode-pa.googleapis.com"
            };
            if antigravity {
                header["User-Agent"] =
                    json!("antigravity/cli/1.0.13 (aidev_client; os_type=darwin; arch=arm64)");
            }
            let mut project = record.project_id.clone();
            if project.is_empty() {
                // Read-only project discovery. Never download auth files or call onboardUser.
                let info = request(
                    api,
                    record,
                    "POST",
                    &format!("{base}/v1internal:loadCodeAssist"),
                    &header,
                    Some(
                        json!({"metadata":{"ideType":"IDE_UNSPECIFIED","platform":"PLATFORM_UNSPECIFIED","pluginType":"GEMINI"}}),
                    ),
                )?;
                project = text(&info, "cloudaicompanionProject");
                if project.is_empty() {
                    project = text(&info["cloudaicompanionProject"], "id");
                }
                if project.is_empty() {
                    return Err(
                        "Gemini 未提供项目标识，请先在对应客户端完成账号登录／项目配置".into(),
                    );
                }
            }
            let action = if antigravity {
                "retrieveUserQuotaSummary"
            } else {
                "retrieveUserQuota"
            };
            return request(
                api,
                record,
                "POST",
                &format!("{base}/v1internal:{action}"),
                &header,
                Some(json!({"project":project})),
            );
        }
        _ => return Err("该凭证类型的额度查询暂未适配".into()),
    };
    let primary = request(api, record, "GET", url, &header, None);
    if matches!(record.provider.as_str(), "xai" | "grok") {
        // A separate billing endpoint serves monthly-only accounts. Never issue
        // the upstream management UI's paid model health-check fallback.
        let has_total = primary
            .as_ref()
            .ok()
            .and_then(|body| parse(&record.provider, body).ok())
            .is_some_and(|items| items.iter().any(|q| q.remaining.is_some()));
        if !has_total {
            return request(
                api,
                record,
                "GET",
                "https://cli-chat-proxy.grok.com/v1/billing",
                &header,
                None,
            );
        }
    }
    primary
}
