import Foundation

#if canImport(CoreAI)

import CoreAI

/// Real Core AI backend — the plan's `RawInferenceFunctionBackend`.
///
/// API verified against Apple's "Integrating on-device AI models in your app with Core AI":
///   `let model = try await AIModel(contentsOf: url)`            // async; specializes for device
///   `guard let fn = try model.loadFunction(named: "main")`      // InferenceFunction
///   `var out = try await fn.run(inputs: ["input": ndArray])`    // NDArray in / out
///   write inputs via `NDArray.mutableView(as:)`, read via `NDArray.view()`.
///
/// Build requirement: Core AI is iOS/macOS **27.0+ (Beta)** and needs the Metal Toolchain;
/// build with xcodebuild against a 27.0+ SDK. On older SDKs the `#else` path below compiles
/// instead and fails loudly (this is why `swift build` here uses the fallback — the SDK is
/// 26.2, which does not ship `CoreAI`).
actor CoreAIBackend: InferenceBackend {
    private let modelURL: URL
    private let functionName: String
    private var model: AIModel?
    private var function: InferenceFunction?
    private var sessions: [String: CoreAISessionState] = [:]

    init(modelURL: URL, functionName: String = "main") {
        self.modelURL = modelURL
        self.functionName = functionName
    }

    func prepare() async throws {
        // Specialization is the expensive, one-time step the watchdog must treat as
        // `Preparing`, never `Stalled`. Reported on stderr so the data channel stays clean.
        Self.emitStderr(["status": "preparing", "backend": "coreai", "phase": "specialize",
                         "model": modelURL.lastPathComponent])

        // `AIModel(contentsOf:)` is async precisely because it specializes before returning.
        let model = try await AIModel(contentsOf: modelURL)
        guard let function = try model.loadFunction(named: functionName) else {
            throw CoreAIBackendError.functionNotFound(functionName)
        }
        self.model = model
        self.function = function
    }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        guard let function = self.function else { throw CoreAIBackendError.notPrepared }
        let start = Date()

        // Private memory for this agent (KV state lives here once the loop is wired).
        var state = sessions[session_id] ?? CoreAISessionState()

        // ── Model-specific glue (TODO before this returns real text) ────────────────
        // The raw InferenceFunction is tensor-in / tensor-out. A text LLM needs:
        //   1. tokenizer.encode(prompt) -> input `NDArray` (names from the model's
        //      Functions descriptor; introspect via `function.descriptor`).
        //   2. an autoregressive decode loop calling `function.run(inputs:states:)`,
        //      carrying KV `states` across steps (the states variant; see
        //      "Managing model specialization and caching"), appending sampled tokens.
        //   3. sampling from the logits `NDArray` (temperature/top-p).
        //   4. tokenizer.decode(tokenIds) -> text.
        // The tokenizer and exact input/output names are model-specific, so this is the
        // honest stopping point: fail loudly rather than emit fabricated text.
        _ = function
        _ = prompt
        state.steps += 1
        sessions[session_id] = state
        _ = start
        throw CoreAIBackendError.decodeLoopNotImplemented
        // ────────────────────────────────────────────────────────────────────────────
    }

    private static func emitStderr(_ obj: [String: String]) {
        if let data = try? JSONSerialization.data(withJSONObject: obj),
           var s = String(data: data, encoding: .utf8) {
            s += "\n"
            FileHandle.standardError.write(Data(s.utf8))
        }
    }
}

enum CoreAIBackendError: Error, CustomStringConvertible {
    case notPrepared
    case functionNotFound(String)
    case decodeLoopNotImplemented
    var description: String {
        switch self {
        case .notPrepared:
            return "Core AI backend used before prepare() completed"
        case .functionNotFound(let name):
            return "Core AI model has no function named '\(name)'"
        case .decodeLoopNotImplemented:
            return "Core AI raw-inference loaded, but the tokenizer + states decode loop is not wired yet (model-specific)"
        }
    }
}

#else

/// Fallback when `CoreAI` is not available (e.g. SDK < 27.0, as on this machine's 26.2 SDK,
/// or a plain `swift build`). Fails loudly instead of silently stubbing.
///
/// To get the real backend: build with xcodebuild against a 27.0+ SDK that ships `CoreAI`,
/// with the Metal Toolchain installed (see `CoreAIBackend.swift` header / plan step 34).
struct CoreAIBackend: InferenceBackend {
    let modelURL: URL
    init(modelURL: URL, functionName: String = "main") {
        self.modelURL = modelURL
    }

    func prepare() async throws { throw CoreAIBackendError.notCompiledIn }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        throw CoreAIBackendError.notCompiledIn
    }
}

enum CoreAIBackendError: Error, CustomStringConvertible {
    case notCompiledIn
    var description: String {
        "Core AI is not available in this build. CoreAI requires the 27.0+ SDK (this SDK lacks it). "
        + "Build with xcodebuild against a 27.0+ SDK + Metal Toolchain."
    }
}

#endif
