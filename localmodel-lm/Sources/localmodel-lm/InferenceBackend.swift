import Foundation

/// Protocol for pluggable on-device inference backends.
/// v1 has stub implementations; real Core AI / MLX backends are v2.
protocol InferenceBackend {
    func prepare() async throws
    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int)
}

/// Stub backend used until Core AI or MLX is wired.
struct StubBackend: InferenceBackend {
    let name: String

    func prepare() async throws {
        try await Task.sleep(nanoseconds: 200_000_000)
    }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        let text = "[\(name)] processed: \(prompt.prefix(40))..."
        return (text, 8, 1)
    }
}
