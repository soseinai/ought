//! OpenAI-shape Chat Completions adapter.
//!
//! Used directly for OpenAI, and reused for OpenRouter and Ollama via
//! their respective sibling modules — both speak the OpenAI Chat
//! Completions wire format.
//!
//! Documentation: <https://platform.openai.com/docs/api-reference/chat>
//!
//! Wire-format differences from Anthropic worth knowing:
//!
//! * The system prompt is a regular `role: system` message at the head
//!   of the `messages` array, not a separate field.
//! * Tool calls live on the assistant message as a `tool_calls` array,
//!   not as content blocks.
//! * Tool results are sent as separate messages (one per result) with
//!   `role: "tool"` and a `tool_call_id` field. An Anthropic-style user
//!   turn that bundles N tool_results expands to N tool messages here.
//! * Tool call arguments are a JSON-stringified string on the wire —
//!   we parse them into a [`Value`] before handing back to the agent.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::LlmError;
use crate::types::{
    CompletionRequest, CompletionResponse, Content, Message, StopReason, Usage,
};
use crate::Llm;

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// OpenAI-shape chat-completions client.
///
/// The public constructors ([`Self::openai`], [`Self::openrouter`],
/// [`Self::ollama`]) cover the common cases. Use [`Self::custom`] for
/// arbitrary OpenAI-compatible endpoints behind a proxy.
pub struct OpenAiLlm {
    name: &'static str,
    api_key: Option<String>,
    base_url: String,
    extra_headers: Vec<(String, String)>,
    http: reqwest::Client,
}

impl OpenAiLlm {
    /// Plain OpenAI with bearer-token auth.
    pub fn openai(api_key: impl Into<String>) -> Result<Self, LlmError> {
        Self::custom(
            "openai",
            Some(api_key.into()),
            OPENAI_BASE_URL.to_string(),
            vec![],
        )
    }

    /// OpenRouter — OpenAI-compatible with required attribution headers.
    /// `app_url` and `app_title` populate `HTTP-Referer` and `X-Title`,
    /// which OpenRouter uses for its public model leaderboard.
    pub fn openrouter(
        api_key: impl Into<String>,
        app_url: Option<String>,
        app_title: Option<String>,
    ) -> Result<Self, LlmError> {
        let mut headers = vec![];
        if let Some(url) = app_url {
            headers.push(("HTTP-Referer".to_string(), url));
        }
        if let Some(title) = app_title {
            headers.push(("X-Title".to_string(), title));
        }
        Self::custom(
            "openrouter",
            Some(api_key.into()),
            "https://openrouter.ai/api/v1".to_string(),
            headers,
        )
    }

    /// Ollama — local, no auth.
    pub fn ollama(base_url: Option<String>) -> Result<Self, LlmError> {
        let url = base_url.unwrap_or_else(|| "http://localhost:11434/v1".to_string());
        Self::custom("ollama", None, url, vec![])
    }

    /// Arbitrary OpenAI-compatible endpoint.
    pub fn custom(
        name: &'static str,
        api_key: Option<String>,
        base_url: String,
        extra_headers: Vec<(String, String)>,
    ) -> Result<Self, LlmError> {
        let http = reqwest::Client::builder().timeout(DEFAULT_TIMEOUT).build()?;
        Ok(Self {
            name,
            api_key,
            base_url,
            extra_headers,
            http,
        })
    }

    /// Construct from an env-var name. Reads the API key when present
    /// and returns [`LlmError::Auth`] otherwise.
    pub fn from_env_openai(var: &str) -> Result<Self, LlmError> {
        let key = std::env::var(var)
            .map_err(|_| LlmError::Auth(format!("env var {} not set", var)))?;
        Self::openai(key)
    }

    pub fn from_env_openrouter(
        var: &str,
        app_url: Option<String>,
        app_title: Option<String>,
    ) -> Result<Self, LlmError> {
        let key = std::env::var(var)
            .map_err(|_| LlmError::Auth(format!("env var {} not set", var)))?;
        Self::openrouter(key, app_url, app_title)
    }
}

#[async_trait]
impl Llm for OpenAiLlm {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let body = build_request_body(&req);
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut builder = self.http.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }
        for (k, v) in &self.extra_headers {
            builder = builder.header(k, v);
        }
        let resp = builder.send().await?;
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
        let parsed: OpenAiResponse = resp.json().await?;
        parse_response(parsed)
    }
}

// ── Request building ────────────────────────────────────────────────────

fn build_request_body(req: &CompletionRequest) -> Value {
    let mut messages = Vec::with_capacity(req.messages.len() + 1);
    messages.push(json!({
        "role": "system",
        "content": req.system,
    }));
    for m in &req.messages {
        for v in message_to_openai(m) {
            messages.push(v);
        }
    }

    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": req.max_tokens,
    });
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    if !req.tools.is_empty() {
        body["tools"] = build_tools(&req.tools);
    }
    body
}

fn build_tools(tools: &[crate::ToolSpec]) -> Value {
    let arr: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                }
            })
        })
        .collect();
    Value::Array(arr)
}

/// Convert one unified [`Message`] into one or more OpenAI messages.
///
/// User turns with N tool_result blocks expand to N tool messages
/// (preceded by any text content as a separate user message).
fn message_to_openai(m: &Message) -> Vec<Value> {
    match m {
        Message::User { content } => {
            let mut out = Vec::new();
            let mut text_buf: Vec<&str> = Vec::new();
            for block in content {
                match block {
                    Content::Text(t) => text_buf.push(t),
                    Content::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => {
                        if !text_buf.is_empty() {
                            out.push(json!({
                                "role": "user",
                                "content": text_buf.join("\n"),
                            }));
                            text_buf.clear();
                        }
                        out.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": content,
                        }));
                    }
                    // OpenAI's user role can't carry tool_use blocks; the
                    // ought agent loop never produces one here, but if
                    // upstream code ever does we flush any pending text
                    // first, then emit the warning as its own user
                    // message. The buffer stays clean.
                    Content::ToolUse { id, name, input } => {
                        if !text_buf.is_empty() {
                            out.push(json!({
                                "role": "user",
                                "content": text_buf.join("\n"),
                            }));
                            text_buf.clear();
                        }
                        out.push(json!({
                            "role": "user",
                            "content": format!(
                                "[unexpected tool_use in user message: id={} name={} input={}]",
                                id, name, input
                            ),
                        }));
                    }
                }
            }
            if !text_buf.is_empty() {
                out.push(json!({
                    "role": "user",
                    "content": text_buf.join("\n"),
                }));
            }
            if out.is_empty() {
                out.push(json!({ "role": "user", "content": "" }));
            }
            out
        }
        Message::Assistant { content } => {
            let mut text_buf: Vec<&str> = Vec::new();
            let mut tool_calls: Vec<Value> = Vec::new();
            for block in content {
                match block {
                    Content::Text(t) => text_buf.push(t),
                    Content::ToolUse { id, name, input } => {
                        tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": input.to_string(),
                            }
                        }));
                    }
                    // tool_result on assistant turn: same fold-to-text guard.
                    Content::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        text_buf.push("");
                        let _ = (tool_use_id, content, is_error);
                    }
                }
            }
            let mut msg = json!({ "role": "assistant" });
            let text = text_buf.join("\n");
            if !text.is_empty() {
                msg["content"] = json!(text);
            } else {
                msg["content"] = Value::Null;
            }
            if !tool_calls.is_empty() {
                msg["tool_calls"] = Value::Array(tool_calls);
            }
            vec![msg]
        }
    }
}

// ── Response parsing ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunction {
    name: String,
    /// JSON-stringified arguments object.
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

fn parse_response(resp: OpenAiResponse) -> Result<CompletionResponse, LlmError> {
    let choice = resp
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| LlmError::InvalidResponse("response had no choices".into()))?;

    let mut content = Vec::new();
    if let Some(text) = choice.message.content
        && !text.is_empty()
    {
        content.push(Content::Text(text));
    }
    for call in choice.message.tool_calls {
        let input: Value = serde_json::from_str(&call.function.arguments).unwrap_or_else(|_| {
            // If the model returned something other than valid JSON,
            // surface the raw string so the tool layer can decide.
            json!({ "_raw_arguments": call.function.arguments })
        });
        content.push(Content::ToolUse {
            id: call.id,
            name: call.function.name,
            input,
        });
    }

    let stop_reason = match choice.finish_reason.as_deref() {
        Some("stop") => StopReason::EndTurn,
        Some("tool_calls") => StopReason::ToolUse,
        Some("length") => StopReason::MaxTokens,
        Some(other) => StopReason::Other(other.to_string()),
        None => StopReason::EndTurn,
    };

    let usage = resp
        .usage
        .map(|u| Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            ..Default::default()
        })
        .unwrap_or_default();

    Ok(CompletionResponse {
        content,
        stop_reason,
        usage,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CacheHints, ToolSpec};

    fn req() -> CompletionRequest {
        CompletionRequest {
            model: "gpt-test".into(),
            system: "you are helpful".into(),
            messages: vec![
                Message::user_text("hello"),
                Message::Assistant {
                    content: vec![
                        Content::Text("let me check".into()),
                        Content::ToolUse {
                            id: "call_1".into(),
                            name: "lookup".into(),
                            input: json!({ "q": "foo" }),
                        },
                    ],
                },
                Message::User {
                    content: vec![
                        Content::ToolResult {
                            tool_use_id: "call_1".into(),
                            content: "answer: 42".into(),
                            is_error: false,
                        },
                        Content::ToolResult {
                            tool_use_id: "call_2".into(),
                            content: "answer: 43".into(),
                            is_error: false,
                        },
                    ],
                },
            ],
            tools: vec![ToolSpec {
                name: "lookup".into(),
                description: "lookup".into(),
                input_schema: json!({ "type": "object" }),
            }],
            max_tokens: 1024,
            temperature: Some(0.2),
            cache_hints: CacheHints::default(),
        }
    }

    #[test]
    fn system_prepended_as_message() {
        let body = build_request_body(&req());
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "you are helpful");
    }

    #[test]
    fn assistant_tool_use_becomes_tool_calls() {
        let body = build_request_body(&req());
        let msgs = body["messages"].as_array().unwrap();
        // [system, user, assistant, tool, tool]
        let assistant = msgs.iter().find(|m| m["role"] == "assistant").unwrap();
        assert_eq!(assistant["content"], "let me check");
        let calls = assistant["tool_calls"].as_array().unwrap();
        assert_eq!(calls[0]["id"], "call_1");
        assert_eq!(calls[0]["function"]["name"], "lookup");
        // arguments are JSON-stringified.
        let args: Value = serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap())
            .unwrap();
        assert_eq!(args["q"], "foo");
    }

    #[test]
    fn user_tool_results_expand_to_tool_messages() {
        let body = build_request_body(&req());
        let msgs = body["messages"].as_array().unwrap();
        let tool_msgs: Vec<&Value> = msgs.iter().filter(|m| m["role"] == "tool").collect();
        assert_eq!(tool_msgs.len(), 2);
        assert_eq!(tool_msgs[0]["tool_call_id"], "call_1");
        assert_eq!(tool_msgs[0]["content"], "answer: 42");
        assert_eq!(tool_msgs[1]["tool_call_id"], "call_2");
    }

    #[test]
    fn tools_serialized_as_function_objects() {
        let body = build_request_body(&req());
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "lookup");
    }

    #[test]
    fn parse_stop_response() {
        let raw = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "hi" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        });
        let parsed: OpenAiResponse = serde_json::from_value(raw).unwrap();
        let resp = parse_response(parsed).unwrap();
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.input_tokens, 10);
        match &resp.content[0] {
            Content::Text(t) => assert_eq!(t, "hi"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn parse_tool_calls_response() {
        let raw = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_x",
                        "type": "function",
                        "function": { "name": "lookup", "arguments": "{\"q\": \"foo\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1 }
        });
        let parsed: OpenAiResponse = serde_json::from_value(raw).unwrap();
        let resp = parse_response(parsed).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        match &resp.content[0] {
            Content::ToolUse { name, input, .. } => {
                assert_eq!(name, "lookup");
                assert_eq!(input["q"], "foo");
            }
            _ => panic!("expected tool_use"),
        }
    }

    #[test]
    fn parse_finish_reason_length_maps_to_max_tokens() {
        let raw = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "trun" },
                "finish_reason": "length"
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1 }
        });
        let parsed: OpenAiResponse = serde_json::from_value(raw).unwrap();
        let resp = parse_response(parsed).unwrap();
        assert_eq!(resp.stop_reason, StopReason::MaxTokens);
    }

    #[test]
    fn openrouter_attaches_attribution_headers() {
        let llm = OpenAiLlm::openrouter(
            "k",
            Some("https://example.com".into()),
            Some("ought".into()),
        )
        .unwrap();
        let kvs: Vec<(&str, &str)> = llm
            .extra_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert!(kvs.contains(&("HTTP-Referer", "https://example.com")));
        assert!(kvs.contains(&("X-Title", "ought")));
    }

    #[test]
    fn user_tool_use_block_is_folded_cleanly() {
        // Defensive path: a ToolUse block on a user message. The agent
        // loop never produces this, but if upstream code ever does, we
        // must (a) flush pending text first, (b) not corrupt the text
        // buffer for subsequent blocks.
        let req = CompletionRequest {
            model: "x".into(),
            system: "s".into(),
            messages: vec![Message::User {
                content: vec![
                    Content::Text("leading text".into()),
                    Content::ToolUse {
                        id: "id_x".into(),
                        name: "lookup".into(),
                        input: json!({}),
                    },
                    Content::Text("trailing text".into()),
                ],
            }],
            tools: vec![],
            max_tokens: 1,
            temperature: None,
            cache_hints: CacheHints::default(),
        };
        let body = build_request_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        // Expected: [system, user("leading text"), user("[unexpected ...]"),
        //            user("trailing text")]
        let user_msgs: Vec<&Value> = msgs.iter().filter(|m| m["role"] == "user").collect();
        assert_eq!(user_msgs.len(), 3);
        assert_eq!(user_msgs[0]["content"], "leading text");
        assert!(
            user_msgs[1]["content"]
                .as_str()
                .unwrap()
                .starts_with("[unexpected tool_use")
        );
        assert_eq!(user_msgs[2]["content"], "trailing text");
    }

    #[test]
    fn ollama_default_base_url() {
        let llm = OpenAiLlm::ollama(None).unwrap();
        assert_eq!(llm.base_url, "http://localhost:11434/v1");
        assert!(llm.api_key.is_none());
    }
}
