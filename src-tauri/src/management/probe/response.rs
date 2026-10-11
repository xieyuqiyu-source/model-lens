use super::Answer;
use serde_json::Value;

pub(super) fn parse(value: &Value) -> Result<Answer, String> {
    if value.get("error").is_some_and(|e| !e.is_null()) {
        return Err("模型采样失败，未使用错误响应评分".into());
    }
    let choice = value["choices"]
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or("采样响应缺少回答")?;
    let text = choice["message"]["content"]
        .as_str()
        .ok_or("采样响应缺少文本回答")?;
    if text.chars().count() > 20000 {
        return Err("采样回答过长，未参与分析".into());
    }
    Ok(Answer {
        text: text.into(),
        finish: if choice["finish_reason"] == "stop"
            && choice["message"]
                .get("tool_calls")
                .is_none_or(|v| v.is_null() || v.as_array().is_some_and(Vec::is_empty))
        {
            "stop"
        } else {
            "incomplete"
        }
        .into(),
        reported_model: value["model"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect(),
        elapsed_ms: 0,
    })
}
