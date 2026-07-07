//! Unified multi-provider LLM client.
//!
//! This module provides a provider-agnostic interface for sending requests to
//! language-model backends. It supports both native provider APIs and a text
//! tool-call harness for local/offline models that do not expose structured
//! tool calling.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use eocon_swarm::llm::{LlmClient, Request, UnifiedLlmClient, LlmConfig};
//!
//! let config = LlmConfig::load_default();
//! let client = UnifiedLlmClient::new(config);
//! let response = client.complete(
//!     Request::new("anthropic", "claude-sonnet-4-6").with_user("Hello")
//! ).unwrap();
//! println!("{}", response.content);
//! ```

pub mod client;
pub mod config;
pub mod provider;

pub use client::{
    FinishReason, LlmClient, Message, Request, Response, Role, StreamEvent, ToolCall,
    ToolDefinition, ToolResult, UnifiedLlmClient, Usage,
};
pub use config::{LlmConfig, ProviderConfig, ToolHarnessConfig};
pub use provider::{parse_tool_calls, strip_tool_call_spans, Provider};

use thiserror::Error;

/// Errors returned by the unified LLM client.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("provider {0} is not configured")]
    ProviderNotConfigured(String),

    #[error("API key for provider {0} is missing (env: {1})")]
    MissingApiKey(String, String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] ureq::Error),

    #[error("JSON parse/serialize failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("provider returned unexpected response: {0}")]
    UnexpectedResponse(String),

    #[error("tool call parse failed: {0}")]
    ToolParse(String),

    #[error("configuration error: {0}")]
    Config(String),
}
