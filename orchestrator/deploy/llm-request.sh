#!/usr/bin/env bash
#
# Foundation-compatible shell wrapper around the unified Rust LLM client.
#
# Usage:
#   llm_request.sh <provider> <model> <max_tokens> < prompt.txt
#   echo "prompt" | llm_request.sh anthropic claude-sonnet-4-6 3500
#
# Environment:
#   EOCON_LLM_BINARY - path to the `eos` binary (optional; searches PATH).
#
# This script mirrors the interface used by Eocon-Foundation-V1's
# `ai_provider_request()` so existing phases can migrate to the Rust core
# without changing their call sites.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LLM_BINARY="${EOCON_LLM_BINARY:-$(command -v eos || true)}"

if [ -z "$LLM_BINARY" ] || [ ! -x "$LLM_BINARY" ]; then
    # Try cargo debug build fallback.
    CANDIDATE="${SCRIPT_DIR}/../target/debug/eos"
    if [ -x "$CANDIDATE" ]; then
        LLM_BINARY="$CANDIDATE"
    else
        echo "Error: eos binary not found. Set EOCON_LLM_BINARY or build with cargo build." >&2
        exit 1
    fi
fi

provider="${1:-anthropic}"
model="${2:-}"
max_tokens="${3:-3500}"

prompt=$(cat)
if [ -z "$prompt" ]; then
    echo "Error: prompt is empty" >&2
    exit 1
fi

request=$(jq -n \
    --arg provider "$provider" \
    --arg model "$model" \
    --argjson max_tokens "$max_tokens" \
    --arg content "$prompt" \
    '{
        provider: $provider,
        model: $model,
        messages: [{role: "user", content: $content}],
        max_tokens: $max_tokens,
        temperature: 0.2
    }')

response=$(echo "$request" | "$LLM_BINARY" llm complete)

# Emit only the assistant text to stdout, matching ai_provider_request().
echo "$response" | jq -r '.content // empty'
