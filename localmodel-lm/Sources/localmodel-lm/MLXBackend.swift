import Foundation

#if canImport(MLXLMCommon)

@preconcurrency import MLX
@preconcurrency import MLXLLM
@preconcurrency import MLXLMCommon
import Hub  // HubApi, from swift-transformers

/// Real on-device MLX backend, ported from the proven ChAi pattern
/// (`ChAi_3/Managers/MLXModelManager.swift`, mlx-swift-lm **2.x** API).
///
/// Flow: download HF weights → `LLMModelFactory.shared.loadContainer` → per-request
/// `ModelContainer.perform { prepare(UserInput) ; MLXLMCommon.generate }`, draining the
/// `Generation` stream into a single response. Private memory is per-`session_id`
/// (`MLXSessionState`).
///
/// Build requirement: MLX compiles Metal shaders, so this target must be built with
/// **xcodebuild + the Metal Toolchain** (`xcodebuild -downloadComponent MetalToolchain`),
/// not plain `swift build`. Activate by uncommenting the MLX deps in `Package.swift`.
actor MLXBackend: InferenceBackend {
    private let modelId: String
    private let systemPrompt: String
    private let maxTokens: Int
    private var container: ModelContainer?
    private var sessions: [String: MLXSessionState] = [:]

    init(modelId: String, systemPrompt: String = "You are a focused swarm agent. Answer concisely.", maxTokens: Int = 512) {
        self.modelId = modelId
        self.systemPrompt = systemPrompt
        self.maxTokens = maxTokens
    }

    func prepare() async throws {
        // Bound the GPU cache (ChAi pattern). No GPU on the Simulator.
        #if !targetEnvironment(simulator)
        MLX.GPU.set(cacheLimit: 20 * 1024 * 1024)
        #endif

        let modelDir = try Self.modelDirectory(for: modelId)
        let hub = HubApi(downloadBase: modelDir)
        let configuration = ModelConfiguration(id: modelId)

        // Download weights if needed; report progress on stderr (data channel stays clean).
        Self.emitStderr(["status": "preparing", "backend": "mlx", "phase": "download", "model": modelId])
        _ = try await downloadModel(hub: hub, configuration: configuration) { progress in
            Self.emitStderr(["status": "preparing", "backend": "mlx", "phase": "download",
                             "progress": String(format: "%.3f", progress.fractionCompleted)])
        }

        // Load the model container (cached for the process lifetime).
        self.container = try await LLMModelFactory.shared.loadContainer(
            hub: hub,
            configuration: configuration
        ) { _ in }
    }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        guard let container = self.container else {
            throw MLXBackendError.notPrepared
        }

        // Private memory: append the new user turn to this session's history.
        var state = sessions[session_id] ?? MLXSessionState(systemPrompt: systemPrompt)
        state.appendUser(prompt)

        struct GenCtx: Sendable { let messages: [[String: String]]; let maxTokens: Int }
        let genCtx = GenCtx(messages: state.messages, maxTokens: maxTokens)
        let start = Date()

        let (text, genTokens): (String, Int) = try await container.perform(values: genCtx) { @Sendable context, ctx in
            let input = try await context.processor.prepare(input: UserInput(messages: ctx.messages))
            let params = GenerateParameters(maxTokens: ctx.maxTokens, temperature: 0.7, topP: 1.0)
            let stream = try MLXLMCommon.generate(input: input, parameters: params, context: context)
            var out = ""
            var count = 0
            for await gen in stream {
                if case .chunk(let t) = gen {
                    out += t
                } else if case .info(let info) = gen {
                    count = info.generationTokenCount
                }
            }
            return (out, count)
        }

        // Persist the assistant turn back into private memory.
        state.appendAssistant(text)
        sessions[session_id] = state

        let latency = Int(Date().timeIntervalSince(start) * 1000)
        return (text, genTokens, latency)
    }

    /// Weights live under Application Support, namespaced per model id.
    private static func modelDirectory(for id: String) throws -> URL {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
            .appendingPathComponent("eocon-swarm/MLXModels", isDirectory: true)
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        return base.appendingPathComponent(id.replacingOccurrences(of: "/", with: "_"), isDirectory: true)
    }

    private static func emitStderr(_ obj: [String: String]) {
        if let data = try? JSONSerialization.data(withJSONObject: obj),
           var str = String(data: data, encoding: .utf8) {
            str += "\n"
            FileHandle.standardError.write(Data(str.utf8))
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
/// Keeps `--backend mlx` resolvable and fails *loudly* instead of silently stubbing,
/// so nobody mistakes a non-MLX build for a working on-device backend.
///
/// To get the real backend: uncomment the MLX deps in `Package.swift` and build with
/// `xcodebuild` + the Metal Toolchain (see `MLXBackend.swift` header and plan step 34).
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
        "MLX backend is not compiled in. Uncomment the mlx-swift / mlx-swift-lm deps in "
        + "Package.swift and build with xcodebuild + the Metal Toolchain."
    }
}

#endif
