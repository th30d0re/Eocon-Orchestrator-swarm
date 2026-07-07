use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::LlmConfig;
use super::provider::{AnthropicProvider, OpenAiCompatibleProvider, Provider};
use super::LlmError;

/// Role of a conversation message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

/// A single message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_results: Vec<ToolResult>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
        }
    }
}

/// Definition of a tool that may be offered to a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A tool invocation produced by the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// The result of executing a tool call, fed back as a user/tool message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}

/// Token usage reported by the provider.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    MaxTokens,
    ToolCall,
    #[serde(untagged)]
    Error(String),
}

/// A complete LLM request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub provider: String,
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub stream: bool,
}

impl Request {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            messages: Vec::new(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: Vec::new(),
            stream: false,
        }
    }

    pub fn with_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    pub fn with_system(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_user(mut self, content: impl Into<String>) -> Self {
        self.messages.push(Message::user(content));
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }
}

/// A complete LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    pub model: String,
    pub provider: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub usage: Usage,
    pub finish_reason: FinishReason,
}

/// A single event from a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum StreamEvent {
    Content(String),
    ToolCall(ToolCall),
    Usage(Usage),
    Finish(FinishReason),
    Error(String),
}

/// The unified LLM client interface.
///
/// Implementations are synchronous. Streaming is exposed as an iterator of
/// events so the caller can bridge it to channels, async streams, or a
/// blocking consumer as needed.
pub trait LlmClient: Send + Sync {
    /// Send a request and return the complete response.
    fn complete(&self, request: Request) -> Result<Response, LlmError>;

    /// Send a request and return a streaming iterator of events.
    fn stream(
        &self,
        request: Request,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, LlmError>>>, LlmError>;
}

/// A concrete client that selects a provider implementation from its name.
pub struct UnifiedLlmClient {
    config: LlmConfig,
    providers: HashMap<String, Box<dyn Provider>>,
}

impl UnifiedLlmClient {
    pub fn new(config: LlmConfig) -> Self {
        let mut providers: HashMap<String, Box<dyn Provider>> = HashMap::new();

        providers.insert(
            "anthropic".to_string(),
            Box::new(AnthropicProvider::new(config.clone())),
        );
        providers.insert(
            "openai".to_string(),
            Box::new(OpenAiCompatibleProvider::new(
                config.clone(),
                "openai",
                "https://api.openai.com/v1",
            )),
        );
        providers.insert(
            "deepseek".to_string(),
            Box::new(OpenAiCompatibleProvider::new(
                config.clone(),
                "deepseek",
                "https://api.deepseek.com/v1",
            )),
        );

        Self { config, providers }
    }

    pub fn with_provider<P: Provider + 'static>(mut self, provider: P) -> Self {
        self.providers.insert(provider.name().to_string(), Box::new(provider));
        self
    }

    fn resolve(&self, request: &Request) -> Result<&dyn Provider, LlmError> {
        let provider_name = if request.provider.is_empty() {
            self.config
                .default_provider
                .as_deref()
                .unwrap_or("anthropic")
        } else {
            &request.provider
        };

        self.providers
            .get(provider_name)
            .map(|p| p.as_ref())
            .ok_or_else(|| LlmError::ProviderNotConfigured(provider_name.to_string()))
    }

    fn fill_defaults(&self, mut request: Request) -> Request {
        if request.model.is_empty() {
            if let Some(model) = self.config.default_model(&request.provider) {
                request.model = model;
            }
        }
        if request.max_tokens.is_none() {
            request.max_tokens = Some(4096);
        }
        if request.temperature.is_none() {
            request.temperature = Some(0.2);
        }
        request
    }
}

impl LlmClient for UnifiedLlmClient {
    fn complete(&self, request: Request) -> Result<Response, LlmError> {
        let request = self.fill_defaults(request);
        let provider = self.resolve(&request)?;
        provider.complete(request)
    }

    fn stream(
        &self,
        request: Request,
    ) -> Result<Box<dyn Iterator<Item = Result<StreamEvent, LlmError>>>, LlmError> {
        let request = self.fill_defaults(request);
        let provider = self.resolve(&request)?;
        provider.stream(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builder() {
        let req = Request::new("anthropic", "claude-sonnet-4-6")
            .with_system("You are a tester.")
            .with_user("Hello");

        assert_eq!(req.provider, "anthropic");
        assert_eq!(req.model, "claude-sonnet-4-6");
        assert_eq!(req.system_prompt.as_deref(), Some("You are a tester."));
        assert_eq!(req.messages.len(), 1);
    }

    #[test]
    fn unified_client_fills_defaults() {
        let mut config = LlmConfig::default();
        config.default_provider = Some("anthropic".to_string());
        config.providers.insert(
            "anthropic".to_string(),
            super::super::config::ProviderConfig {
                api_key_env: None,
                base_url: None,
                default_model: Some("claude-sonnet-4-6".to_string()),
                extras: HashMap::new(),
            },
        );

        let client = UnifiedLlmClient::new(config);
        let req = Request::new("anthropic", "").with_user("hi");

        // Defaults are only applied inside complete/stream; resolve still works.
        let provider = client.resolve(&client.fill_defaults(req.clone())).unwrap();
        assert_eq!(provider.name(), "anthropic");

        let filled = client.fill_defaults(req);
        assert_eq!(filled.model, "claude-sonnet-4-6");
        assert_eq!(filled.max_tokens, Some(4096));
        assert_eq!(filled.temperature, Some(0.2));
    }
}
