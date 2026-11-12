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
    answer(
        text,
        choice["finish_reason"] == "stop" && !has_tool_call(&choice["message"]),
        value["model"].as_str().unwrap_or_default(),
    )
}

pub(super) fn parse_stream(bytes: &[u8]) -> Result<Answer, String> {
    let stream = std::str::from_utf8(bytes).map_err(|_| "采样流格式异常")?;
    let stream = stream
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut text = String::new();
    let mut model = String::new();
    let mut stopped = None;
    let mut called_tool = false;
    for data in sse_data(&stream) {
        if data.trim() == "[DONE]" {
            break;
        }
        let value: Value = serde_json::from_str(&data).map_err(|_| "采样流格式异常")?;
        if !value["error"].is_null() {
            return Err("模型采样失败，未使用错误响应评分".into());
        }
        if let Some(id) = value["model"].as_str().filter(|id| !id.is_empty()) {
            model = id.into();
        }
        let Some(choice) = value["choices"].as_array().and_then(|rows| {
            rows.iter()
                .find(|row| row["index"].as_u64().unwrap_or(0) == 0)
        }) else {
            continue;
        };
        let delta = &choice["delta"];
        if let Some(content) = delta["content"].as_str() {
            text.push_str(content);
        }
        called_tool |= has_tool_call(delta) || !delta["function_call"].is_null();
        if let Some(reason) = choice["finish_reason"].as_str() {
            stopped = Some(reason == "stop");
        }
    }
    let stopped = stopped.ok_or("采样流未正常结束，未参与分析")?;
    if text.is_empty() {
        return Err("采样响应缺少文本回答".into());
    }
    answer(&text, stopped && !called_tool, &model)
}

pub(super) fn sse_data(stream: &str) -> impl Iterator<Item = String> + '_ {
    stream
        .split("\n\n")
        .map(|event| {
            event
                .lines()
                .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|data| !data.is_empty())
}

fn has_tool_call(message: &Value) -> bool {
    let calls = &message["tool_calls"];
    !(calls.is_null() || calls.as_array().is_some_and(Vec::is_empty))
}

fn answer(text: &str, stopped: bool, model: &str) -> Result<Answer, String> {
    if text.chars().count() > 20000 {
        return Err("采样回答过长，未参与分析".into());
    }
    Ok(Answer {
        text: text.into(),
        finish: if stopped { "stop" } else { "incomplete" }.into(),
        reported_model: model.chars().take(200).collect(),
        elapsed_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_stream_without_terminal_choice_and_redacts_error_events() {
        let partial = "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n";
        let error = "data: {\"error\":{\"message\":\"private\"}}\n\n";
        assert_eq!(
            parse_stream(partial.as_bytes()).unwrap_err(),
            "采样流未正常结束，未参与分析"
        );
        assert_eq!(
            parse_stream(format!("{partial}{error}").as_bytes()).unwrap_err(),
            "模型采样失败，未使用错误响应评分"
        );
        let length = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n";
        assert_eq!(
            parse_stream(format!("{partial}{length}").as_bytes())
                .unwrap()
                .finish,
            "incomplete"
        );
    }
}
