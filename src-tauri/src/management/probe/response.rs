use super::Answer;
use serde_json::Value;

pub(super) fn parse(provider: &str, envelope: &Value) -> Result<Answer, String> {
    let status = envelope["status_code"]
        .as_u64()
        .ok_or("采样响应缺少状态码")?;
    if !(200..300).contains(&status) {
        return Err(format!("模型采样返回 HTTP {status}，已停止后续请求"));
    }
    let body = &envelope["body"];
    let value = if provider == "codex" {
        if body.is_object() {
            body.clone()
        } else {
            let text = body.as_str().ok_or("采样数据格式异常")?;
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                value
            } else {
                completed_response(text)?
            }
        }
    } else {
        match body {
            Value::String(text) => serde_json::from_str(text).map_err(|_| "采样数据格式异常")?,
            Value::Object(_) => body.clone(),
            _ => return Err("采样数据格式异常".into()),
        }
    };
    if value.get("error").is_some_and(|e| !e.is_null()) {
        return Err("模型采样失败，未使用错误响应评分".into());
    }
    let (text, finish) = if provider == "codex" {
        let output = value["output"].as_array().ok_or("采样响应缺少回答")?;
        let text = output
            .iter()
            .filter(|o| o["type"] == "message")
            .filter_map(|o| o["content"].as_array())
            .flatten()
            .filter(|c| c["type"] == "output_text")
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        (
            text,
            if value["status"] == "completed" {
                "stop"
            } else {
                "incomplete"
            },
        )
    } else {
        let text = value["content"]
            .as_array()
            .ok_or("采样响应缺少回答")?
            .iter()
            .filter(|c| c["type"] == "text")
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        (
            text,
            if value["stop_reason"] == "end_turn" {
                "stop"
            } else {
                "incomplete"
            },
        )
    };
    if text.chars().count() > 20000 {
        return Err("采样回答过长，未参与分析".into());
    }
    Ok(Answer {
        text,
        finish: finish.into(),
        reported_model: value["model"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect(),
        elapsed_ms: 0,
    })
}
fn completed_response(text: &str) -> Result<Value, String> {
    // Only terminal snapshots are used; deltas are never appended a second time.
    let normalized = text.replace("\r\n", "\n");
    let mut completed = None;
    for event in normalized.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|l| l.strip_prefix("data:").map(str::trim_start))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: Value = serde_json::from_str(&data).map_err(|_| "采样流格式异常")?;
        match value["type"].as_str() {
            Some("error" | "response.failed") => {
                return Err("模型采样失败，未使用错误响应评分".into())
            }
            Some("response.completed" | "response.incomplete") => {
                completed = Some(value["response"].clone())
            }
            _ => {}
        }
    }
    completed.ok_or("采样流未正常结束，未参与分析".into())
}
