use super::{response, Answer};
use reqwest::blocking::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    #[default]
    Chat,
    Responses,
    Messages,
}

impl Protocol {
    pub fn path(self) -> &'static str {
        match self {
            Self::Chat => "/chat/completions",
            Self::Responses => "/responses",
            Self::Messages => "/messages",
        }
    }

    pub fn payload(self, model: &str, prompt: &str) -> Value {
        let messages = json!([{"role":"user","content":prompt}]);
        match self {
            Self::Chat => {
                json!({"model":model, "messages":messages, "stream":false, "max_tokens":6000})
            }
            Self::Responses => {
                json!({"model":model, "input":prompt, "stream":false, "store":false, "max_output_tokens":6000})
            }
            Self::Messages => {
                json!({"model":model, "messages":messages, "stream":false, "max_tokens":8192})
            }
        }
    }

    pub fn authorize(self, request: RequestBuilder, key: &str) -> RequestBuilder {
        match self {
            Self::Messages => {
                let request = request.header("anthropic-version", "2023-06-01");
                if key.is_empty() {
                    request
                } else {
                    request.header("x-api-key", key)
                }
            }
            _ if key.is_empty() => request,
            _ => request.bearer_auth(key),
        }
    }

    pub fn parse(self, value: &Value) -> Result<Answer, String> {
        if !value["error"].is_null() || value["type"] == "error" {
            return Err("模型采样失败，未使用错误响应评分".into());
        }
        match self {
            Self::Chat => response::parse(value),
            Self::Responses => parse_responses(value),
            Self::Messages => {
                let blocks = blocks(value, "content")?;
                let stopped = matches!(
                    value["stop_reason"].as_str(),
                    Some("end_turn" | "stop_sequence")
                ) && !blocks
                    .iter()
                    .any(|b| matches!(b["type"].as_str(), Some("tool_use" | "server_tool_use")));
                text_answer(blocks.iter(), "text", stopped, value)
            }
        }
    }

    pub fn parse_stream(self, bytes: &[u8]) -> Result<Answer, String> {
        if self == Self::Chat {
            return response::parse_stream(bytes);
        }
        let stream = response::normalize_stream(bytes)?;
        let mut message = Value::Null;
        let mut content = Vec::<Value>::new();
        let mut active_block = None;
        for data in response::sse_data(&stream).take_while(|data| data.trim() != "[DONE]") {
            let event: Value = serde_json::from_str(&data).map_err(|_| "采样流格式异常")?;
            if !event["error"].is_null()
                || event["type"] == "error"
                || event["type"] == "response.failed"
            {
                return Err("模型采样失败，未使用错误响应评分".into());
            }
            match (self, event["type"].as_str().unwrap_or_default()) {
                (Self::Responses, kind @ ("response.completed" | "response.incomplete")) => {
                    if event["response"]["status"].as_str() != kind.strip_prefix("response.") {
                        return Err("采样流结束状态不一致".into());
                    }
                    return self.parse(&event["response"]);
                }
                (Self::Messages, "message_start") if message.is_null() => {
                    message = event["message"].clone();
                }
                (Self::Messages, "content_block_start") => {
                    let index = content.len() as u64;
                    if !message.is_object()
                        || active_block.is_some()
                        || event["index"].as_u64() != Some(index)
                    {
                        return Err("采样流内容块顺序异常".into());
                    }
                    active_block = Some(index);
                    content.push(event["content_block"].clone());
                }
                (Self::Messages, "content_block_delta")
                    if event["delta"]["type"] == "text_delta" =>
                {
                    let block = event["index"]
                        .as_u64()
                        .filter(|index| Some(*index) == active_block)
                        .and_then(|i| content.get_mut(i as usize))
                        .filter(|block| block["type"] == "text")
                        .ok_or("采样流内容块无效")?;
                    let mut text = block["text"].as_str().unwrap_or_default().to_owned();
                    text.push_str(event["delta"]["text"].as_str().ok_or("采样流缺少文本")?);
                    if text.chars().count() > 20_000 {
                        return Err("采样回答过长，未参与分析".into());
                    }
                    block["text"] = Value::String(text);
                }
                (Self::Messages, "content_block_stop") => {
                    if active_block.is_none() || active_block != event["index"].as_u64() {
                        return Err("采样流内容块未正常结束".into());
                    }
                    active_block = None;
                }
                (Self::Messages, "message_delta")
                    if message.is_object()
                        && active_block.is_none()
                        && event["delta"]["stop_reason"].is_string() =>
                {
                    message["stop_reason"] = event["delta"]["stop_reason"].clone()
                }
                (Self::Messages, "message_stop")
                    if message.is_object() && active_block.is_none() =>
                {
                    message["content"] = Value::Array(content);
                    return self.parse(&message);
                }
                _ => {}
            }
        }
        Err("采样流未正常结束，未参与分析".into())
    }
}

fn blocks<'a>(value: &'a Value, field: &str) -> Result<&'a [Value], String> {
    value[field]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "采样响应缺少回答".into())
}

fn text_answer<'a>(
    parts: impl Iterator<Item = &'a Value>,
    kind: &str,
    stopped: bool,
    value: &Value,
) -> Result<Answer, String> {
    let text = parts
        .filter(|part| part["type"] == kind)
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        return Err("采样响应缺少文本回答".into());
    }
    response::answer(&text, stopped, value["model"].as_str().unwrap_or_default())
}

fn parse_responses(value: &Value) -> Result<Answer, String> {
    let output = blocks(value, "output")?;
    let parts: Vec<_> = output
        .iter()
        .filter(|item| item["type"] == "message" && item["role"] == "assistant")
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .collect();
    let stopped = value["status"] == "completed"
        && output
            .iter()
            .all(|item| matches!(item["type"].as_str(), Some("message" | "reasoning")))
        && !parts.iter().any(|part| part["type"] == "refusal");
    text_answer(parts.into_iter(), "output_text", stopped, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(events: &[Value]) -> String {
        events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect()
    }

    #[test]
    fn responses_stream_uses_terminal_text_and_handles_incomplete_or_failed() {
        let mut result = json!({"status":"completed","model":"m","output":[
            {"type":"message","role":"assistant","content":[{"type":"output_text","text":"1, 2"}]}
        ]});
        let delta = json!({"type":"response.output_text.delta","delta":"ignored"});
        let done = json!({"type":"response.completed","response":result});
        let reply = Protocol::Responses
            .parse_stream(stream(&[delta.clone(), done]).as_bytes())
            .unwrap();
        assert_eq!(
            (reply.text.as_str(), reply.finish.as_str()),
            ("1, 2", "stop")
        );
        assert!(Protocol::Responses
            .parse_stream(stream(&[delta]).as_bytes())
            .is_err());
        result["status"] = json!("incomplete");
        let partial = json!({"type":"response.incomplete","response":result});
        assert_eq!(
            Protocol::Responses
                .parse_stream(stream(&[partial]).as_bytes())
                .unwrap()
                .finish,
            "incomplete"
        );
        let failed = json!({"type":"response.failed","response":{"error":{"message":"private"}}});
        assert!(!Protocol::Responses
            .parse_stream(stream(&[failed]).as_bytes())
            .unwrap_err()
            .contains("private"));
    }

    #[test]
    fn messages_stream_joins_text_and_waits_for_terminal_message() {
        let mut events = vec![
            json!({"type":"message_start","message":{"model":"m","stop_reason":null,"content":[]}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"999"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"1, "}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"2"}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
        ];
        assert!(Protocol::Messages
            .parse_stream(stream(&events).as_bytes())
            .is_err());
        events.push(json!({"type":"message_stop"}));
        let reply = Protocol::Messages
            .parse_stream(stream(&events).as_bytes())
            .unwrap();
        assert_eq!(
            (reply.text.as_str(), reply.finish.as_str()),
            ("1, 2", "stop")
        );
        let mut truncated = events.clone();
        truncated.remove(6);
        assert!(Protocol::Messages
            .parse_stream(stream(&truncated).as_bytes())
            .is_err());
        let stop_reason = events.len() - 2;
        events[stop_reason]["delta"]["stop_reason"] = json!("max_tokens");
        assert_eq!(
            Protocol::Messages
                .parse_stream(stream(&events).as_bytes())
                .unwrap()
                .finish,
            "incomplete"
        );
    }
}
