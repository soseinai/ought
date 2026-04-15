//! Anthropic Messages API adapter.
//!
//! Documentation: <https://docs.anthropic.com/en/api/messages>
//!
//! Translates [`CompletionRequest`] to Anthropic's `/v1/messages` request
//! body, including `cache_control` markers when [`CacheHints`] flags are
//! set, and parses the response back into a [`CompletionResponse`].

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::LlmError;
use crate::types::{
    CacheHints, CompletionRequest, CompletionResponse, Content, Message, StopReason, Usage,
};
use crate::Llm;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Messages API client.
pub struct AnthropicLlm {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl AnthropicLlm {
    /// Construct from an explicit API key. Uses the default base URL.
    pub fn new(api_key: impl Into<String>) -> Result<Self, LlmError> {
        Self::with_base_url(api_key, DEFAULT_BASE_URL.to_string())
    }

    /// Construct with a custom base URL (for proxies / Bedrock / Vertex
    /// gateways that speak the Anthropic protocol).
    pub fn with_base_url(api_key: impl Into<String>, base_url: String) -> Result<Self, LlmError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;
        Ok(Self {
            api_key: api_key.into(),
            base_url,
            http,
        })
    }

    /// Construct by reading the API key from the named env var.
    pub fn from_env(var: &str) -> Result<Self, LlmError> {
        let key = std::env::var(var)
            .map_err(|_| LlmError::Auth(format!("env var {} not set", var)))?;
        Self::new(key)
    }
}

#[async_trait]
impl Llm for AnthropicLlm {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let body = build_request_body(&req);
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_else(|_| String::new());
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(LlmError::Auth(message));
            }
            return Err(LlmError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let parsed: AnthropicResponse = resp.json().await?;
        parse_response(parsed)
    }
}

// ── Request building ────────────────────────────────────────────────────

fn build_request_body(req: &CompletionRequest) -> Value {
    let mut body = json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "system": build_system(&req.system, req.cache_hints.cache_system),
        "messages": build_messages(&req.messages, &req.cache_hints),
    });

    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    if !req.tools.is_empty() {
        body["tools"] = build_tools(&req.tools, req.cache_hints.cache_tools);
    }

    body
}

fn build_system(system: &str, cache: bool) -> Value {
    let mut block = json!({ "type": "text", "text": system });
    if cache {
        block["cache_control"] = json!({ "type": "ephemeral" });
    }
    Value::Array(vec![block])
}

fn build_tools(tools: &[crate::ToolSpec], cache_last: bool) -> Value {
    let last_idx = tools.len().saturating_sub(1);
    let arr: Vec<Value> = tools
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut tool = json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            });
            if cache_last && i == last_idx {
                tool["cache_control"] = json!({ "type": "ephemeral" });
            }
            tool
        })
        .collect();
    Value::Array(arr)
}

fn build_messages(messages: &[Message], hints: &CacheHints) -> Value {
    let cache_idx = hints.cache_messages_up_to;
    let arr: Vec<Value> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let (role, blocks) = match m {
                Message::User { content } => ("user", content),
                Message::Assistant { content } => ("assistant", content),
            };
            let last_block = blocks.len().saturating_sub(1);
            let cache_this_message = cache_idx.is_some_and(|idx| i == idx);
            let content_arr: Vec<Value> = blocks
                .iter()
                .enumerate()
                .map(|(bi, b)| {
                    let mut v = content_to_json(b);
                    if cache_this_message
                        && bi == last_block
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("cache_control".into(), json!({ "type": "ephemeral" }));
                    }
                    v
                })
                .collect();
            json!({ "role": role, "content": content_arr })
        })
        .collect();
    Value::Array(arr)
}

fn content_to_json(c: &Content) -> Value {
    match c {
        Content::Text(text) => json!({ "type": "text", "text": text }),
        Content::ToolUse { id, name, input } => json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
        Content::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let mut v = json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": content,
            });
            if *is_error {
                v["is_error"] = json!(true);
            }
            v
        }
    }
}

// ── Response parsing ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// Anything else the API may emit (e.g. `thinking`). We pass through
    /// such blocks as `Text` with an empty body so the trace stays
    /// well-formed but we don't crash on unknown variants.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: u32,
}

fn parse_response(resp: AnthropicResponse) -> Result<CompletionResponse, LlmError> {
    let content = resp
        .content
        .into_iter()
        .filter_map(|b| match b {
            AnthropicBlock::Text { text } => Some(Content::Text(text)),
            AnthropicBlock::ToolUse { id, name, input } => {
                Some(Content::ToolUse { id, name, input })
            }
            AnthropicBlock::Unknown => None,
        })
        .collect();

    let stop_reason = match resp.stop_reason.as_deref() {
        Some("end_turn") => StopReason::EndTurn,
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("stop_sequence") => StopReason::StopSequence,
        Some(other) => StopReason::Other(other.to_string()),
        None => StopReason::EndTurn,
    };

    Ok(CompletionResponse {
        content,
        stop_reason,
        usage: Usage {
            input_tokens: resp.usage.input_tokens,
            output_tokens: resp.usage.output_tokens,
            cache_read_tokens: resp.usage.cache_read_input_tokens,
            cache_creation_tokens: resp.usage.cache_creation_input_tokens,
        },
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolSpec;

    fn sample_request() -> CompletionRequest {
        CompletionRequest {
            model: "claude-sonnet-4-6".into(),
            system: "you are helpful".into(),
            messages: vec![
                Message::user_text("hello"),
                Message::Assistant {
                    content: vec![Content::ToolUse {
                        id: "toolu_1".into(),
                        name: "lookup".into(),
                        input: json!({ "q": "foo" }),
                    }],
                },
                Message::User {
                    content: vec![Content::ToolResult {
                        tool_use_id: "toolu_1".into(),
                        content: "answer: 42".into(),
                        is_error: false,
                    }],
                },
            ],
            tools: vec![ToolSpec {
                name: "lookup".into(),
                description: "look something up".into(),
                input_schema: json!({ "type": "object", "properties": { "q": { "type": "string" } } }),
            }],
            max_tokens: 1024,
            temperature: Some(0.5),
            cache_hints: CacheHints::default(),
        }
    }

    #[test]
    fn request_body_basic_shape() {
        let body = build_request_body(&sample_request());
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["temperature"], 0.5);
        let system = &body["system"];
        assert!(system.is_array());
        assert_eq!(system[0]["type"], "text");
        assert_eq!(system[0]["text"], "you are helpful");
        assert!(system[0].get("cache_control").is_none());
        let tools = &body["tools"];
        assert_eq!(tools[0]["name"], "lookup");
    }

    #[test]
    fn request_body_messages_shape() {
        let body = build_request_body(&sample_request());
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"][0]["type"], "text");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"][0]["type"], "tool_use");
        assert_eq!(msgs[1]["content"][0]["id"], "toolu_1");
        assert_eq!(msgs[2]["role"], "user");
        assert_eq!(msgs[2]["content"][0]["type"], "tool_result");
        assert_eq!(msgs[2]["content"][0]["tool_use_id"], "toolu_1");
    }

    #[test]
    fn cache_hints_attach_cache_control() {
        let mut req = sample_request();
        req.cache_hints = CacheHints {
            cache_system: true,
            cache_tools: true,
            cache_messages_up_to: Some(0),
        };
        let body = build_request_body(&req);
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["tools"][0]["cache_control"]["type"], "ephemeral");
        let m0_blocks = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(
            m0_blocks.last().unwrap()["cache_control"]["type"],
            "ephemeral"
        );
    }

    #[test]
    fn empty_tools_omitted() {
        let mut req = sample_request();
        req.tools.clear();
        let body = build_request_body(&req);
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn tool_result_error_flag_preserved() {
        let req = CompletionRequest {
            model: "x".into(),
            system: "s".into(),
            messages: vec![Message::User {
                content: vec![Content::ToolResult {
                    tool_use_id: "t".into(),
                    content: "boom".into(),
                    is_error: true,
                }],
            }],
            tools: vec![],
            max_tokens: 1,
            temperature: None,
            cache_hints: CacheHints::default(),
        };
        let body = build_request_body(&req);
        assert_eq!(body["messages"][0]["content"][0]["is_error"], true);
    }

    #[test]
    fn parse_end_turn_response() {
        let raw = json!({
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 10, "output_tokens": 5 }
        });
        let parsed: AnthropicResponse = serde_json::from_value(raw).unwrap();
        let resp = parse_response(parsed).unwrap();
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.input_tokens, 10);
        assert_eq!(resp.usage.output_tokens, 5);
        match &resp.content[0] {
            Content::Text(t) => assert_eq!(t, "ok"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn parse_tool_use_response() {
        let raw = json!({
            "content": [
                { "type": "text", "text": "let me check" },
                { "type": "tool_use", "id": "toolu_x", "name": "lookup", "input": { "q": "foo" } }
            ],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 30,
                "cache_creation_input_tokens": 50,
                "cache_read_input_tokens": 20
            }
        });
        let parsed: AnthropicResponse = serde_json::from_value(raw).unwrap();
        let resp = parse_response(parsed).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.usage.cache_read_tokens, 20);
        assert_eq!(resp.usage.cache_creation_tokens, 50);
        assert_eq!(resp.content.len(), 2);
        match &resp.content[1] {
            Content::ToolUse { name, input, .. } => {
                assert_eq!(name, "lookup");
                assert_eq!(input["q"], "foo");
            }
            _ => panic!("expected tool_use"),
        }
    }

    #[test]
    fn parse_unknown_block_skipped() {
        let raw = json!({
            "content": [
                { "type": "text", "text": "hi" },
                { "type": "thinking", "thinking": "private reasoning" }
            ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        });
        let parsed: AnthropicResponse = serde_json::from_value(raw).unwrap();
        let resp = parse_response(parsed).unwrap();
        // Unknown block is silently dropped.
        assert_eq!(resp.content.len(), 1);
    }

    #[test]
    fn from_env_missing_key_errors() {
        let res = AnthropicLlm::from_env("__OUGHT_TEST_NEVER_SET_VAR__");
        assert!(matches!(res, Err(LlmError::Auth(_))));
    }
}
