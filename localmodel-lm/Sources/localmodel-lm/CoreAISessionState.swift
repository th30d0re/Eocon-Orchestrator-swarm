import Foundation

/// Per-`session_id` Core AI inference state — the swarm's private-memory primitive for the
/// Core AI backend.
///
/// In Core AI, a chat's KV-cache lives in the model's **states**, carried across decode
/// steps via `InferenceFunction.run(inputs:states:)`. Distinct sessions must never share
/// states (the LCGuard boundary). This struct holds the per-agent decode bookkeeping; the
/// opaque Core AI `states` handle is attached once the decode loop is wired (it can't be
/// referenced until the `CoreAI` module is linked — see `CoreAIBackend.swift`).
struct CoreAISessionState {
    /// Token ids produced/consumed so far — context-length tracking and re-feed.
    var tokenHistory: [Int] = []
    /// Number of decode steps run for this session (telemetry).
    var steps: Int = 0
}
