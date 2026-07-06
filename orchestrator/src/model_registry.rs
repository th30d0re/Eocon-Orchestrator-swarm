use std::collections::HashMap;

use crate::swarm::agent::AgentRole;

/// A concrete model variant behind a user-facing `@handle`.
#[derive(Debug, Clone, Copy)]
pub struct ModelVariant {
    pub name: &'static str,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub model: &'static str,
    pub role: AgentRole,
}

/// A user-facing `@handle` that groups related model variants.
#[derive(Debug, Clone, Copy)]
pub struct ModelHandle {
    pub handle: &'static str,
    pub display_name: &'static str,
    pub default_variant: usize,
    pub variants: &'static [ModelVariant],
}

/// Built-in model handles available for `@mention` addressing in the TUI.
pub const MODEL_HANDLES: &[ModelHandle] = &[
    ModelHandle {
        handle: "kimi",
        display_name: "Moonshot Kimi",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "k2.7-code",
            display_name: "Kimi K2.7 Code",
            provider: "moonshot",
            model: "kimi-k2.7-code",
            role: AgentRole::Generalist,
        }],
    },
    ModelHandle {
        handle: "claude",
        display_name: "Claude",
        default_variant: 0,
        variants: &[
            ModelVariant {
                name: "sonnet-4.6",
                display_name: "Claude Sonnet 4.6",
                provider: "anthropic",
                model: "claude-sonnet-4-6",
                role: AgentRole::Reviewer,
            },
            ModelVariant {
                name: "opus-4.8",
                display_name: "Claude Opus 4.8",
                provider: "anthropic",
                model: "claude-opus-4-8",
                role: AgentRole::Reviewer,
            },
        ],
    },
    ModelHandle {
        handle: "gpt",
        display_name: "OpenAI GPT",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "gpt-4o",
            display_name: "GPT-4o",
            provider: "openai",
            model: "gpt-4o",
            role: AgentRole::Generalist,
        }],
    },
    ModelHandle {
        handle: "codex",
        display_name: "OpenAI Codex",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "codex-5.5",
            display_name: "Codex 5.5",
            provider: "codex",
            model: "codex-5.5",
            role: AgentRole::Coder,
        }],
    },
    ModelHandle {
        handle: "gemini",
        display_name: "Google Gemini",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "gemini-3.1-pro",
            display_name: "Gemini 3.1 Pro",
            provider: "google",
            model: "gemini-3.1-pro",
            role: AgentRole::Researcher,
        }],
    },
    ModelHandle {
        handle: "ollama",
        display_name: "Ollama (local)",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "llama3.1",
            display_name: "Llama 3.1",
            provider: "ollama",
            model: "llama3.1",
            role: AgentRole::Generalist,
        }],
    },
    ModelHandle {
        handle: "mlx",
        display_name: "MLX (local)",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "qwen3-4b",
            display_name: "Qwen3 4B (MLX)",
            provider: "mlx",
            model: "mlx-community/Qwen3-4B-4bit",
            role: AgentRole::Generalist,
        }],
    },
    ModelHandle {
        handle: "coreai",
        display_name: "Core AI (local)",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "rootledger-gemma3-12b",
            display_name: "RootLedger Gemma3 12B",
            provider: "coreai",
            model: "/Users/emmanuel/Dev/Tools/Eocon-Orchestrator-swarm/RootLedger-Gemma3-12B-CoreAI/RootLedger-Gemma3-12B-CoreAI.aimodel",
            role: AgentRole::Generalist,
        }],
    },
    ModelHandle {
        handle: "traces",
        display_name: "Trace Replay (offline)",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "fable5-sessions",
            display_name: "Fable 5 Claude Code session replays",
            provider: "traces",
            model: "/Users/emmanuel/.eocon/traces/sessions.jsonl",
            role: AgentRole::Generalist,
        }],
    },
    ModelHandle {
        handle: "lmstudio",
        display_name: "LM Studio (local)",
        default_variant: 0,
        variants: &[ModelVariant {
            name: "local",
            display_name: "LM Studio Local",
            provider: "lmstudio",
            model: "local",
            role: AgentRole::Generalist,
        }],
    },
];

pub fn find_handle(handle: &str) -> Option<&'static ModelHandle> {
    MODEL_HANDLES
        .iter()
        .find(|h| h.handle.eq_ignore_ascii_case(handle))
}

pub fn find_variant(handle: &str, variant_name: &str) -> Option<&'static ModelVariant> {
    find_handle(handle)?.variants.iter().find(|v| v.name.eq_ignore_ascii_case(variant_name))
}

/// Resolve the active variant for a handle, falling back to the handle's default.
pub fn active_variant(
    handle: &str,
    active_variants: &HashMap<String, usize>,
) -> Option<&'static ModelVariant> {
    let h = find_handle(handle)?;
    let idx = active_variants
        .get(&h.handle.to_lowercase())
        .copied()
        .unwrap_or(h.default_variant);
    h.variants.get(idx).or_else(|| h.variants.first())
}

/// Parse a CLI/TUI model spec like `claude:opus-4.8` into `(handle, variant)`.
pub fn parse_model_spec(spec: &str) -> Option<(&str, &str)> {
    let (handle, variant) = spec.split_once(':')?;
    if handle.is_empty() || variant.is_empty() {
        return None;
    }
    Some((handle, variant))
}

pub fn filter_by_prefix(prefix: &str) -> Vec<usize> {
    let prefix_lower = prefix.to_lowercase();
    MODEL_HANDLES
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            h.handle.to_lowercase().starts_with(&prefix_lower)
                || h.display_name.to_lowercase().contains(&prefix_lower)
                || h.variants.iter().any(|v| {
                    v.name.to_lowercase().starts_with(&prefix_lower)
                        || v.display_name.to_lowercase().contains(&prefix_lower)
                })
        })
        .map(|(i, _)| i)
        .collect()
}

#[derive(Debug, Clone)]
pub struct Mention {
    pub handle: String,
    pub text: String,
}

/// Parse an input string into a sequence of `@handle message` mentions.
///
/// Example:
///   `@kimi look at X? @claude check it`
/// returns two mentions.
pub fn parse_mentions(input: &str) -> Vec<Mention> {
    let mut mentions = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((idx, c)) = chars.next() {
        if c == '@' {
            // Read the handle: consecutive alphanumeric/underscore/hyphen characters.
            let start = idx;
            let mut end = start + 1;
            while let Some((i, ch)) = chars.peek() {
                if ch.is_alphanumeric() || *ch == '_' || *ch == '-' {
                    end = *i + ch.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            let handle = &input[start + 1..end];

            // Read the message text until the next '@'.
            let text_start = end;
            let mut text_end = text_start;
            while let Some((i, ch)) = chars.peek() {
                if *ch == '@' {
                    break;
                }
                text_end = *i + ch.len_utf8();
                chars.next();
            }
            let text = input[text_start..text_end].trim().to_string();

            if !handle.is_empty() {
                mentions.push(Mention {
                    handle: handle.to_string(),
                    text,
                });
            }
        }
    }

    mentions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_mention() {
        let ms = parse_mentions("@kimi can you look at X?");
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].handle, "kimi");
        assert_eq!(ms[0].text, "can you look at X?");
    }

    #[test]
    fn parses_multiple_mentions() {
        let ms = parse_mentions(
            "@kimi can you look at X? @claude can you check Kimi's work when they're done? @gemini implement y",
        );
        assert_eq!(ms.len(), 3);
        assert_eq!(ms[0].handle, "kimi");
        assert_eq!(ms[0].text, "can you look at X?");
        assert_eq!(ms[1].handle, "claude");
        assert_eq!(ms[1].text, "can you check Kimi's work when they're done?");
        assert_eq!(ms[2].handle, "gemini");
        assert_eq!(ms[2].text, "implement y");
    }

    #[test]
    fn empty_text_is_ignored() {
        let ms = parse_mentions("@kimi");
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].text, "");
    }

    #[test]
    fn no_mentions_returns_empty() {
        let ms = parse_mentions("just a normal message");
        assert!(ms.is_empty());
    }

    #[test]
    fn active_variant_uses_default() {
        let active = HashMap::new();
        let v = active_variant("claude", &active).unwrap();
        assert_eq!(v.name, "sonnet-4.6");
    }

    #[test]
    fn active_variant_uses_override() {
        let mut active = HashMap::new();
        active.insert("claude".to_string(), 1);
        let v = active_variant("claude", &active).unwrap();
        assert_eq!(v.name, "opus-4.8");
    }

    #[test]
    fn parse_model_spec_splits() {
        assert_eq!(parse_model_spec("claude:opus-4.8"), Some(("claude", "opus-4.8")));
        assert_eq!(parse_model_spec("gpt-4o"), None);
    }
}
