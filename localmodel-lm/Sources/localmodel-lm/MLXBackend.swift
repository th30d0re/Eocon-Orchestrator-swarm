import Foundation

#if canImport(MLXLMCommon)

import MLX
import MLXLMCommon
import MLXLLM
// 3.x HF adapters (separate packages per mlx-swift-lm upgrade.md) — PENDING dependency
// approval; these are third-party (DePasqualeOrg) personal-account packages, not ml-explore.
import MLXLMHFAPI      // Downloader adapter (HubClient)
import MLXLMTokenizers // Tokenizer adapter (TokenizersLoader)

/// Real on-device MLX backend — **3.x API** (ChatSession + KVCache + wired-memory admission).
///
/// Each `session_id` gets its own `ChatSession`, which owns an isolated `KVCache` and reuses
/// it across turns — true multi-turn caching, no per-turn re-prepare (the 2.x baseline on
/// `main` re-prepared the full message list each turn). Generation runs inside a wired-memory
/// ticket scope so concurrent agents share a GPU budget and can't OOM.
///
/// Load uses the 3.x `#huggingFaceLoadModelContainer` macro (default hub client + HF
/// tokenizer loader). Build with `scripts/build-mlx.sh` (xcodebuild) so MLX's Metal
/// `default.metallib` is compiled into `mlx-swift_Cmlx.bundle` next to the executable.
actor MLXBackend: InferenceBackend {
    private let modelId: String
    private let systemPrompt: String
    private let maxTokens: Int
    private var container: ModelContainer?
    private var sessions: [String: ChatSession] = [:]
    private let admission = WiredMemoryAdmission()
    private var activeBytes: Int = 0

    init(modelId: String, systemPrompt: String = "You are a focused swarm agent. Answer concisely.", maxTokens: Int = 512) {
        self.modelId = modelId
        self.systemPrompt = systemPrompt
        self.maxTokens = maxTokens
    }

    func prepare() async throws {
        #if !targetEnvironment(simulator)
        MLX.GPU.set(cacheLimit: 20 * 1024 * 1024)
        #endif

        Self.emitStderr(["status": "preparing", "backend": "mlx", "phase": "download", "model": modelId])
        let configuration = ModelConfiguration(id: modelId)
        // 3.x explicit loader (upgrade.md / SKILL.md): downloader + tokenizer come from the
        // adapter packages. TODO(verify-symbols): confirm `HubClient.default` /
        // `TokenizersLoader()` once swift-hf-api-mlx / swift-tokenizers-mlx are approved and
        // resolved — the exact adapter symbol names differ between adapter package versions.
        let container = try await LLMModelFactory.shared.loadContainer(
            from: HubClient.default,
            using: TokenizersLoader(),
            configuration: configuration
        ) { progress in
            Self.emitStderr(["status": "preparing", "backend": "mlx", "phase": "download",
                             "progress": String(format: "%.3f", progress.fractionCompleted)])
        }
        self.container = container

        // Size wired-memory tickets to the model's resident footprint (runtime-derived).
        #if !targetEnvironment(simulator)
        self.activeBytes = MLX.GPU.snapshot().activeMemory
        #endif
    }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        guard let container else { throw MLXBackendError.notPrepared }

        // One ChatSession per agent session — owns its KVCache, reused across turns.
        let session: ChatSession
        if let existing = sessions[session_id] {
            session = existing
        } else {
            session = ChatSession(
                container,
                instructions: systemPrompt,
                generateParameters: GenerateParameters(maxTokens: maxTokens, temperature: 0.7, topP: 1.0))
            sessions[session_id] = session
        }

        let start = Date()
        // Admission gating: raise the wired-memory limit for this inference, then lower it
        // when done. We use the ticket's manual start()/end() rather than the closure form
        // of withWiredLimit, because the closure would capture this actor's `session`
        // (non-Sendable) and Swift 6 rejects sending self-isolated state into the
        // nonisolated withWiredLimit method.
        let ticket = admission.ticket(activeBytes: activeBytes)
        _ = await ticket.start()
        var text = ""
        var genTokens = 0
        do {
            for try await gen in session.streamDetails(to: prompt, images: [], videos: []) {
                if case .chunk(let t) = gen {
                    text += t
                } else if case .info(let info) = gen {
                    genTokens = info.generationTokenCount
                }
            }
        } catch {
            _ = await ticket.end()
            throw error
        }
        _ = await ticket.end()
        let latency = Int(Date().timeIntervalSince(start) * 1000)
        return (text, genTokens, latency)
    }

    private static func emitStderr(_ obj: [String: String]) {
        if let data = try? JSONSerialization.data(withJSONObject: obj),
           var s = String(data: data, encoding: .utf8) {
            s += "\n"
            FileHandle.standardError.write(Data(s.utf8))
        }
    }
}

enum MLXBackendError: Error, CustomStringConvertible {
    case notPrepared
    var description: String {
        switch self {
        case .notPrepared: return "MLX backend used before prepare() completed"
        }
    }
}

#else

/// Fallback used by the default `swift build`, where the MLX libraries are not linked.
/// Fails loudly instead of silently stubbing. Build the real backend with
/// `scripts/build-mlx.sh` (xcodebuild + Metal Toolchain).
struct MLXBackend: InferenceBackend {
    let modelId: String
    init(modelId: String, systemPrompt: String = "", maxTokens: Int = 512) {
        self.modelId = modelId
    }

    func prepare() async throws { throw MLXBackendError.notCompiledIn }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        throw MLXBackendError.notCompiledIn
    }
}

enum MLXBackendError: Error, CustomStringConvertible {
    case notCompiledIn
    var description: String {
        "MLX backend is not compiled in. Build with scripts/build-mlx.sh (xcodebuild + Metal Toolchain)."
    }
}

#endif
