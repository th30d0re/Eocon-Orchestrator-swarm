import Foundation

/// Per-`session_id` private memory for the MLX backend (the swarm's "private thinking
/// memory" boundary). In the mlx-swift-lm **2.x** API there is no `ChatSession`, so we
/// retain the running message list and re-prepare it each turn — exactly the pattern the
/// shipping ChAi app uses (`ChAi_3/Managers/MLXModelManager.swift`).
///
/// In **3.x** this is replaced by a real `ChatSession` + isolated `KVCache` per session,
/// giving true cross-turn cache reuse instead of re-prepare. See mlx-swift-lm `upgrade.md`.
/// Until then this struct is the seam: swap its internals for `ChatSession` without
/// changing `MLXBackend`'s call sites.
struct MLXSessionState {
    /// OpenAI-style role/content history, the shape MLXLMCommon's `UserInput` accepts.
    private(set) var messages: [[String: String]]

    init(systemPrompt: String) {
        if systemPrompt.isEmpty {
            messages = []
        } else {
            messages = [["role": "system", "content": systemPrompt]]
        }
    }

    mutating func appendUser(_ content: String) {
        messages.append(["role": "user", "content": content])
    }

    mutating func appendAssistant(_ content: String) {
        messages.append(["role": "assistant", "content": content])
    }
}
