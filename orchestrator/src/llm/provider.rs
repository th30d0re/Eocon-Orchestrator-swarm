use std::collections::HashSet;

use serde_json::json;

use super::client::{FinishReason, Request, Response, Role, StreamEvent, ToolCall, Usage};
use super::config::LlmConfig;
use super::LlmError;

/// A backend that can execute LLM requests.
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn complete(&self, request: Request) -> Result<Response, LlmError>;
    fn stream(
        &self,
        request: Request,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, LlmError>>>, LlmError>;
}

// ── Anthropic ────────────────────────────────────────────────────────────────

pub struct AnthropicProvider {
    config: LlmConfig,
}

impl AnthropicProvider {
    pub fn new(config: LlmConfig) -> Self {
        Self { config }
    }

    fn api_key(&self) -> Result<String, LlmError> {
        self.config
            .api_key("anthropic")?
            .ok_or_else(|| LlmError::MissingApiKey("anthropic".to_string(), "ANTHROPIC_API_KEY".to_string()))
    }

    fn base_url(&self) -> String {
        self.config.base_url("anthropic", "https://api.anthropic.com/v1")
    }

    fn version(&self) -> String {
        self.config
            .extra("anthropic", "version")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "2023-06-01".to_string())
    }

    fn build_body(&self, request: &Request) -> serde_json::Value {
        let mut messages = Vec::new();
        for msg in &request.messages {
            messages.push(json!({
                "role": role_to_anthropic(msg.role),
                "content": msg.content,
            }));
        }

        let mut body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "messages": messages,
            "temperature": request.temperature.unwrap_or(0.2),
        });

        if let Some(system) = &request.system_prompt {
            body["system"] = json!(system);
        }

        body
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn complete(&self, request: Request) -> Result<Response, LlmError> {
        let url = format!("{}/messages", self.base_url());
        let body = self.build_body(&request);
        let api_key = self.api_key()?;

        let resp = ureq::post(&url)
            .set("x-api-key", &api_key)
            .set("anthropic-version", &self.version())
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(LlmError::Http)?;

        let json: serde_json::Value = resp.into_json().map_err(|e| {
            LlmError::UnexpectedResponse(format!("failed to read Anthropic response body: {}", e))
        })?;

        let content = json["content"]
            .as_array()
            .and_then(|arr| arr.iter().find(|c| c["type"] == "text"))
            .and_then(|c| c["text"].as_str())
            .unwrap_or("")
            .to_string();

        let raw_tool_calls: Vec<ToolCall> = json["content"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|c| c["type"] == "tool_use")
                    .filter_map(|c| {
                        Some(ToolCall {
                            id: c["id"].as_str()?.to_string(),
                            name: c["name"].as_str()?.to_string(),
                            arguments: c["input"].clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let parsed_tool_calls = parse_tool_calls(&content, &self.config.tool_names());
        let tool_calls = merge_tool_calls(raw_tool_calls, parsed_tool_calls);
        let clean_content = strip_tool_call_spans(&content, &self.config.tool_names());

        Ok(Response {
            id: json["id"].as_str().unwrap_or("anthropic-unknown").to_string(),
            model: json["model"].as_str().unwrap_or(&request.model).to_string(),
            provider: "anthropic".to_string(),
            content: clean_content,
            tool_calls,
            usage: parse_usage(&json, "usage"),
            finish_reason: parse_finish_reason(&json["stop_reason"]),
        })
    }

    fn stream(
        &self,
        request: Request,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, LlmError>>>, LlmError> {
        // Phase 1: stream the complete response as a single content chunk.
        // A future Phase 2 implementation will parse Server-Sent Events.
        let response = self.complete(request)?;
        let events = vec![
            Ok(StreamEvent::Content(response.content.clone())),
            Ok(StreamEvent::Finish(response.finish_reason.clone())),
        ];
        Ok(Box::new(events.into_iter()))
    }
}

fn role_to_anthropic(role: Role) -> &'static str {
    match role {
        Role::System => "user", // Anthropic uses a top-level system field; map stray system messages to user.
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "user",
    }
}

// ── OpenAI-compatible ────────────────────────────────────────────────────────

pub struct OpenAiCompatibleProvider {
    config: LlmConfig,
    name: String,
    fallback_base_url: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: LlmConfig, name: impl Into<String>, fallback_base_url: impl Into<String>) -> Self {
        Self {
            config,
            name: name.into(),
            fallback_base_url: fallback_base_url.into(),
        }
    }

    fn api_key(&self) -> Result<String, LlmError> {
        let env_name = self
            .config
            .providers
            .get(&self.name)
            .and_then(|c| c.api_key_env.as_deref())
            .unwrap_or_else(|| match self.name.as_str() {
                "openai" => "OPENAI_API_KEY",
                "deepseek" => "DEEPSEEK_API_KEY",
                _ => "API_KEY",
            });

        std::env::var(env_name)
            .map_err(|_| LlmError::MissingApiKey(self.name.clone(), env_name.to_string()))
    }

    fn base_url(&self) -> String {
        self.config
            .base_url(&self.name, &self.fallback_base_url)
            .trim_end_matches("/v1")
            .to_string()
            + "/v1"
    }

    fn build_messages(&self, request: &Request) -> Vec<serde_json::Value> {
        let mut messages = Vec::new();
        if let Some(system) = &request.system_prompt {
            messages.push(json!({"role": "system", "content": system}));
        }
        for msg in &request.messages {
            messages.push(json!({"role": msg.role.to_string(), "content": msg.content}));
        }
        messages
    }

    fn build_body(&self, request: &Request) -> serde_json::Value {
        json!({
            "model": request.model,
            "messages": self.build_messages(request),
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "temperature": request.temperature.unwrap_or(0.2),
        })
    }
}

impl Provider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&self, request: Request) -> Result<Response, LlmError> {
        let url = format!("{}/chat/completions", self.base_url());
        let body = self.build_body(&request);
        let api_key = self.api_key()?;

        let resp = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(body)
            .map_err(LlmError::Http)?;

        let json: serde_json::Value = resp.into_json().map_err(|e| {
            LlmError::UnexpectedResponse(format!(
                "failed to read {} response body: {}",
                self.name, e
            ))
        })?;

        let content = json["choices"]
            .get(0)
            .and_then(|c| c["message"]["content"].as_str())
            .unwrap_or("")
            .to_string();

        let raw_tool_calls: Vec<ToolCall> = json["choices"]
            .get(0)
            .and_then(|c| c["message"]["tool_calls"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|t| t["type"].as_str() == Some("function"))
                    .filter_map(|t| {
                        Some(ToolCall {
                            id: t["id"].as_str()?.to_string(),
                            name: t["function"]["name"].as_str()?.to_string(),
                            arguments: serde_json::from_str(t["function"]["arguments"].as_str()?)
                                .unwrap_or(json!({})),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let parsed_tool_calls = parse_tool_calls(&content, &self.config.tool_names());
        let tool_calls = merge_tool_calls(raw_tool_calls, parsed_tool_calls);
        let clean_content = strip_tool_call_spans(&content, &self.config.tool_names());

        Ok(Response {
            id: json["id"].as_str().unwrap_or("openai-unknown").to_string(),
            model: json["model"].as_str().unwrap_or(&request.model).to_string(),
            provider: self.name.clone(),
            content: clean_content,
            tool_calls,
            usage: parse_usage(&json, "usage"),
            finish_reason: parse_finish_reason(
                &json["choices"]
                    .get(0)
                    .and_then(|c| Some(c["finish_reason"].clone()))
                    .unwrap_or_default(),
            ),
        })
    }

    fn stream(
        &self,
        request: Request,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, LlmError>>>, LlmError> {
        let response = self.complete(request)?;
        let events = vec![
            Ok(StreamEvent::Content(response.content.clone())),
            Ok(StreamEvent::Finish(response.finish_reason.clone())),
        ];
        Ok(Box::new(events.into_iter()))
    }
}

// ── Shared helpers ───────────────────────────────────────────────────────────

fn parse_usage(json: &serde_json::Value, key: &str) -> Usage {
    json.get(key)
        .map(|u| Usage {
            prompt_tokens: u["prompt_tokens"].as_u64().map(|n| n as u32),
            completion_tokens: u["completion_tokens"].as_u64().map(|n| n as u32),
            total_tokens: u["total_tokens"].as_u64().map(|n| n as u32),
        })
        .unwrap_or_default()
}

fn parse_finish_reason(value: &serde_json::Value) -> FinishReason {
    match value.as_str() {
        Some("end_turn") | Some("stop") => FinishReason::Stop,
        Some("max_tokens") => FinishReason::MaxTokens,
        Some("tool_use") | Some("tool_calls") => FinishReason::ToolCall,
        Some(other) => FinishReason::Error(other.to_string()),
        None => FinishReason::Stop,
    }
}

fn merge_tool_calls(native: Vec<ToolCall>, parsed: Vec<ToolCall>) -> Vec<ToolCall> {
    let mut out = native;
    out.extend(parsed);
    out
}

// ── Provider-agnostic text tool-call parsing ─────────────────────────────────
//
// Inspired by ORBOS src/harness/engine.py. We parse plain assistant text and
// extract tool calls so local/offline models that emit tool-shaped prose can
// still drive the agent loop.

/// Parse tool calls from assistant text.
///
/// Recognises three shapes:
///   ToolName: {"arg": "value"}
///   ```tool
///   {"name": "ToolName", "input": {"arg": "value"}}
///   ```
///   <tool_call><name>ToolName</name><arguments>{"arg":"value"}</arguments></tool_call>
pub fn parse_tool_calls(text: &str, tool_names: &[String]) -> Vec<ToolCall> {
    if text.is_empty() || tool_names.is_empty() {
        return Vec::new();
    }

    let mut extracted = Vec::new();
    extracted.extend(extract_line_prefixed(text, tool_names));
    extracted.extend(extract_fenced(text, tool_names));
    extracted.extend(extract_xml(text, tool_names));

    // Deduplicate by (name, canonical-arguments) and sort by source position.
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut deduped: Vec<Extracted> = Vec::new();
    for e in extracted {
        let canonical = serde_json::to_string(
            &serde_json::from_str::<serde_json::Value>(&e.arguments_json).unwrap_or(json!({})),
        )
        .ok()
        .map(|s| {
            serde_json::from_str::<serde_json::Value>(&s)
                .ok()
                .and_then(|v| serde_json::to_string(&v).ok())
                .unwrap_or(s)
        })
        .unwrap_or_default();
        let key = (e.name.clone(), canonical);
        if seen.insert(key) {
            deduped.push(e);
        }
    }
    deduped.sort_by_key(|e| e.span.0);

    deduped
        .into_iter()
        .map(|e| ToolCall {
            id: format!("call_{}_{}", provider_safe(&e.name), uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x")),
            name: e.name,
            arguments: serde_json::from_str(&e.arguments_json).unwrap_or(json!({})),
        })
        .collect()
}

fn provider_safe(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Remove recognised tool-call spans from text.
pub fn strip_tool_call_spans(text: &str, tool_names: &[String]) -> String {
    if text.is_empty() || tool_names.is_empty() {
        return text.to_string();
    }

    let mut spans: Vec<(usize, usize)> = Vec::new();
    spans.extend(extract_line_prefixed(text, tool_names).iter().map(|e| e.span));
    spans.extend(extract_fenced(text, tool_names).iter().map(|e| e.span));
    spans.extend(extract_xml(text, tool_names).iter().map(|e| e.span));

    if spans.is_empty() {
        return text.to_string();
    }

    spans.sort_by_key(|s| s.0);
    spans.reverse();

    let mut out = text.to_string();
    for (start, end) in spans {
        out.replace_range(start..end, "");
    }

    // Collapse excessive blank lines.
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

#[derive(Debug, Clone)]
struct Extracted {
    name: String,
    arguments_json: String,
    span: (usize, usize),
}

fn extract_line_prefixed(text: &str, tool_names: &[String]) -> Vec<Extracted> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        for name in tool_names {
            let prefix = format!("{}:", name);
            if let Some(pos) = line.find(&prefix) {
                let after = line[pos + prefix.len()..].trim_start();
                // Skip optional leading index like "Bash:1{".
                let after = after.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start();
                if after.starts_with('{') {
                    let global_offset = text
                        .lines()
                        .take(idx)
                        .map(|l| l.len() + 1)
                        .sum::<usize>()
                        + pos;
                    if let Some((args_json, end)) = extract_balanced_json(text, global_offset + pos + prefix.len() + (line[pos + prefix.len()..].len() - after.len())) {
                        if serde_json::from_str::<serde_json::Value>(&args_json).is_ok() {
                            out.push(Extracted {
                                name: name.clone(),
                                arguments_json: args_json,
                                span: (global_offset, end),
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

fn extract_fenced(text: &str, tool_names: &[String]) -> Vec<Extracted> {
    let mut out = Vec::new();
    let pattern = "```tool";
    let mut start = 0;
    while let Some(idx) = text[start..].find(pattern) {
        let block_start = start + idx;
        let body_start = block_start + pattern.len();
        let body_end = match text[body_start..].find("\n```") {
            Some(end) => body_start + end,
            None => break,
        };
        let body = text[body_start..body_end].trim();
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(name) = obj
                .get("name")
                .or_else(|| obj.get("tool"))
                .and_then(|v| v.as_str())
            {
                if tool_names.iter().any(|n| n == name) {
                    let args = obj
                        .get("input")
                        .or_else(|| obj.get("arguments"))
                        .or_else(|| obj.get("params"))
                        .cloned()
                        .unwrap_or(json!({}));
                    if let Ok(args_json) = serde_json::to_string(&args) {
                        out.push(Extracted {
                            name: name.to_string(),
                            arguments_json: args_json,
                            span: (block_start, body_end + 4),
                        });
                    }
                }
            }
        }
        start = body_end + 4;
    }
    out
}

fn extract_xml(text: &str, tool_names: &[String]) -> Vec<Extracted> {
    let mut out = Vec::new();
    let open = "<tool_call>";
    let mut start = 0;
    while let Some(idx) = text[start..].find(open) {
        let block_start = start + idx;
        let body_start = block_start + open.len();
        let body_end = match text[body_start..].find("</tool_call>") {
            Some(end) => body_start + end,
            None => break,
        };
        let body = &text[body_start..body_end];

        let name = extract_xml_tag(body, "name");
        let args = extract_xml_tag(body, "arguments");

        if let (Some(name), Some(args)) = (name, args) {
            if tool_names.iter().any(|n| n == &name) && serde_json::from_str::<serde_json::Value>(&args).is_ok() {
                out.push(Extracted {
                    name,
                    arguments_json: args,
                    span: (block_start, body_end + "</tool_call>".len()),
                });
            }
        }
        start = body_end + "</tool_call>".len();
    }
    out
}

fn extract_xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)?;
    Some(text[start..start + end].trim().to_string())
}

/// Find a balanced JSON object starting at `start` (which must point at '{'
/// or somewhere before it on the same logical line). Returns the JSON text
/// and the index just past the closing brace.
fn extract_balanced_json(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut i = start;
    while i < bytes.len() && bytes[i] as char != '{' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }

    let json_start = i;
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else {
            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((text[json_start..=i].to_string(), i + 1));
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_names() -> Vec<String> {
        vec!["Bash".to_string(), "Read".to_string(), "Write".to_string()]
    }

    #[test]
    fn parses_line_prefix_tool_call() {
        let text = "I'll check the directory.\nBash: {\"command\": \"ls -la\"}\nDone.";
        let calls = parse_tool_calls(text, &tool_names());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "Bash");
        assert_eq!(calls[0].arguments["command"], "ls -la");
    }

    #[test]
    fn parses_fenced_tool_call() {
        let text = "```tool\n{\"name\": \"Read\", \"input\": {\"file_path\": \"Cargo.toml\"}}\n```";
        let calls = parse_tool_calls(text, &tool_names());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "Read");
        assert_eq!(calls[0].arguments["file_path"], "Cargo.toml");
    }

    #[test]
    fn parses_xml_tool_call() {
        let text = "<tool_call>\n<name>Write</name>\n<arguments>{\"file_path\": \"x.txt\", \"content\": \"hi\"}</arguments>\n</tool_call>";
        let calls = parse_tool_calls(text, &tool_names());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "Write");
        assert_eq!(calls[0].arguments["file_path"], "x.txt");
    }

    #[test]
    fn strips_tool_call_spans() {
        let text = "I'll run it.\n\nBash: {\"command\": \"echo hi\"}\n\nDone.";
        let clean = strip_tool_call_spans(text, &tool_names());
        assert!(!clean.contains("Bash:"));
        assert!(clean.contains("I'll run it."));
        assert!(clean.contains("Done."));
    }

    #[test]
    fn ignores_unknown_tool_names() {
        let text = "Unknown: {\"x\": 1}";
        let calls = parse_tool_calls(text, &tool_names());
        assert!(calls.is_empty());
    }

    #[test]
    fn deduplicates_identical_calls() {
        let text = "Bash: {\"command\": \"pwd\"}\nBash: {\"command\": \"pwd\"}";
        let calls = parse_tool_calls(text, &tool_names());
        assert_eq!(calls.len(), 1);
    }
}
