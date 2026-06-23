import Foundation

#if canImport(CoreAI)

import CoreAI
import Tokenizers

/// Real Core AI backend — the plan's `RawInferenceFunctionBackend`.
///
/// API verified against Apple's "Integrating on-device AI models in your app with Core AI":
///   `let model = try await AIModel(contentsOf: url, options: .default)` // async; specializes
///   `guard let fn = try model.loadFunction(named: "main")`             // InferenceFunction
///   `let outputs = try await fn.run(inputs: [String: NDArray], states: ..., outputViews: ...)`
///   write inputs via `NDArray(scalars:shape:)` / `mutableView(as:)`, read via `NDArray.view(as:)`.
///
/// Build requirement: Core AI is iOS/macOS **27.0+ (Beta)** and needs the Metal Toolchain;
/// build with xcodebuild against a 27.0+ SDK. On older SDKs the `#else` path below compiles
/// instead and fails loudly (this is why `swift build` here uses the fallback — the SDK is
/// 26.2, which does not ship `CoreAI`).
final class CoreAIBackend: InferenceBackend, @unchecked Sendable {
    private let modelURL: URL
    private let functionName: String
    private let maxTokens: Int
    private let maxContextLength: Int
    private let temperature: Float
    private var model: AIModel?
    private var function: InferenceFunction?
    private var tokenizer: (any Tokenizer)?
    private var sessions: [String: CoreAISessionState] = [:]

    init(modelURL: URL, functionName: String = "main", maxTokens: Int = 512, temperature: Float = 0.7) {
        self.modelURL = modelURL
        self.functionName = functionName
        self.maxTokens = maxTokens
        // Leave headroom beyond maxTokens for the prompt and a small cache margin.
        self.maxContextLength = max(maxTokens * 2, 2048)
        self.temperature = temperature
    }

    func prepare() async throws {
        Self.emitStderr(["status": "preparing", "backend": "coreai", "phase": "specialize",
                         "model": modelURL.lastPathComponent])

        let model = try await AIModel(contentsOf: modelURL, options: .default)
        guard let function = try model.loadFunction(named: functionName) else {
            throw CoreAIBackendError.functionNotFound(functionName)
        }
        self.model = model
        self.function = function

        Self.emitStderr(["status": "preparing", "backend": "coreai", "phase": "tokenizer",
                         "model": modelURL.lastPathComponent])
        self.tokenizer = try await loadTokenizer()

        let descriptor = function.descriptor
        Self.emitStderr(["status": "preparing", "backend": "coreai", "phase": "descriptor",
                         "inputs": descriptor.inputNames.joined(separator: ","),
                         "outputs": descriptor.outputNames.joined(separator: ","),
                         "states": descriptor.stateNames.joined(separator: ",")])
    }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        guard let function = self.function else { throw CoreAIBackendError.notPrepared }
        guard let tokenizer = self.tokenizer else { throw CoreAIBackendError.noTokenizer }
        let start = Date()

        let descriptor = function.descriptor
        let inputNames = descriptor.inputNames
        let outputNames = descriptor.outputNames
        let stateNames = descriptor.stateNames

        guard !inputNames.isEmpty else { throw CoreAIBackendError.missingInput("function has no inputs") }
        guard !outputNames.isEmpty else { throw CoreAIBackendError.missingOutput("function has no outputs") }
        guard inputNames.count <= 2 else {
            throw CoreAIBackendError.unsupportedInputCount(inputNames.count)
        }

        let inputIdsName = inputNames[0]
        let positionIdsName = inputNames.count > 1 ? inputNames[1] : nil
        let logitsName = outputNames[0]

        // Chat-tuned models (e.g. Gemma-3) require their chat template — encoding the raw
        // prompt makes them stall/repeat (validated: this was the cause of "garbage" output,
        // not the conversion). Apply the bundled tokenizer's template with a generation
        // prompt; fall back to plain encoding for base (non-chat) models without a template.
        let promptTokenIDs: [Int]
        if tokenizer.hasChatTemplate {
            let messages: [Message] = [["role": "user", "content": prompt]]
            promptTokenIDs = try tokenizer.applyChatTemplate(messages: messages, addGenerationPrompt: true)
        } else {
            promptTokenIDs = tokenizer.encode(text: prompt, addSpecialTokens: true)
        }
        guard !promptTokenIDs.isEmpty else { throw CoreAIBackendError.emptyPrompt }

        var state = sessions[session_id] ?? CoreAISessionState()
        let vocabSize = tokenizer.getVocabSize(withAddedTokens: true)
        let eosID = tokenizer.eosTokenId ?? 2

        // Allocate KV-cache state arrays once per session.
        if state.kvStates.isEmpty && !stateNames.isEmpty {
            state.kvStates = try allocateStates(stateNames: stateNames, descriptor: descriptor)
        }

        var generatedTokenIDs: [Int] = []
        var stopReason = "maxTokens"

        // ── Prefill ────────────────────────────────────────────────────────────────
        let promptLength = promptTokenIDs.count
        if promptLength > maxContextLength {
            throw CoreAIBackendError.contextTooLong(promptLength, maxContextLength)
        }

        let inputDescriptor = descriptor.inputDescriptor(of: inputIdsName)
        let useSingleTokenPrefill = inputDescriptor.map { desc in
            guard case .ndArray(let ndarrayDesc) = desc else { return false }
            return expectsSingleTokenInput(descriptor: ndarrayDesc)
        } ?? false

        let prefillLogits: NDArray
        if useSingleTokenPrefill {
            var lastLogits: NDArray?
            for (offset, token) in promptTokenIDs.enumerated() {
                lastLogits = try await runInference(
                    function: function,
                    tokenIDs: [token],
                    startPosition: offset,
                    descriptor: descriptor,
                    inputIdsName: inputIdsName,
                    positionIdsName: positionIdsName,
                    logitsName: logitsName,
                    vocabSize: vocabSize,
                    kvStates: &state.kvStates
                )
                state.steps += 1
            }
            guard let logits = lastLogits else {
                throw CoreAIBackendError.emptyPrompt
            }
            prefillLogits = logits
        } else {
            prefillLogits = try await runInference(
                function: function,
                tokenIDs: promptTokenIDs,
                startPosition: 0,
                descriptor: descriptor,
                inputIdsName: inputIdsName,
                positionIdsName: positionIdsName,
                logitsName: logitsName,
                vocabSize: vocabSize,
                kvStates: &state.kvStates
            )
            state.steps += 1
        }

        state.tokenHistory = promptTokenIDs

        if let firstToken = try sampleNextToken(
            logits: prefillLogits,
            sequenceLength: useSingleTokenPrefill ? 1 : promptLength,
            vocabSize: vocabSize,
            temperature: temperature
        ) {
            generatedTokenIDs.append(firstToken)
            state.tokenHistory.append(firstToken)
            if firstToken == eosID { stopReason = "eos" }
        }

        // ── Autoregressive decode ──────────────────────────────────────────────────
        while stopReason != "eos"
                && state.tokenHistory.count < maxContextLength
                && generatedTokenIDs.count < maxTokens {
            let currentPosition = state.tokenHistory.count - 1
            guard let lastToken = state.tokenHistory.last else { break }

            let decodeLogits = try await runInference(
                function: function,
                tokenIDs: [lastToken],
                startPosition: currentPosition,
                descriptor: descriptor,
                inputIdsName: inputIdsName,
                positionIdsName: positionIdsName,
                logitsName: logitsName,
                vocabSize: vocabSize,
                kvStates: &state.kvStates
            )

            state.steps += 1

            if let nextToken = try sampleNextToken(
                logits: decodeLogits,
                sequenceLength: 1,
                vocabSize: vocabSize,
                temperature: temperature
            ) {
                generatedTokenIDs.append(nextToken)
                state.tokenHistory.append(nextToken)
                if nextToken == eosID { stopReason = "eos" }
            } else {
                break
            }
        }

        sessions[session_id] = state

        let text = tokenizer.decode(tokenIds: generatedTokenIDs, skipSpecialTokens: true)
        let latency = Int(Date().timeIntervalSince(start) * 1000)
        return (text, generatedTokenIDs.count, latency)
    }

    // MARK: - Tokenizer loading

    private func loadTokenizer() async throws -> any Tokenizer {
        let bundleDirectory = modelURL.deletingLastPathComponent()
        let tokenizerDirectory = bundleDirectory.appendingPathComponent("tokenizer")
        let candidates = ["tokenizer.json", "tokenizer.model", "tokenizer_config.json"]

        let hasAnyInBundle = candidates.contains { filename in
            (bundleDirectory.appendingPathComponent(filename) as NSURL).checkResourceIsReachableAndReturnError(nil)
        }
        let hasAnyInSubdirectory = candidates.contains { filename in
            (tokenizerDirectory.appendingPathComponent(filename) as NSURL).checkResourceIsReachableAndReturnError(nil)
        }

        let directory: URL
        if hasAnyInSubdirectory {
            directory = tokenizerDirectory
        } else if hasAnyInBundle {
            directory = bundleDirectory
        } else {
            throw CoreAIBackendError.noTokenizerFiles(bundleDirectory.path)
        }
        return try await AutoTokenizer.from(directory: directory)
    }

    // MARK: - State allocation

    private func allocateStates(stateNames: [String], descriptor: InferenceFunctionDescriptor) throws -> [String: NDArray] {
        var states: [String: NDArray] = [:]
        for name in stateNames {
            guard let valueDesc = descriptor.stateDescriptor(of: name) else {
                throw CoreAIBackendError.missingStateDescriptor(name)
            }
            guard case .ndArray(let ndarrayDesc) = valueDesc else {
                throw CoreAIBackendError.nonNDArrayState(name)
            }
            let shape = concreteStateShape(from: ndarrayDesc)
            states[name] = NDArray(shape: shape, scalarType: ndarrayDesc.scalarType)
        }
        return states
    }

    private func concreteStateShape(from descriptor: NDArrayDescriptor) -> [Int] {
        let shape = descriptor.shape
        guard descriptor.hasDynamicShape else { return shape }
        return shape.map { $0 <= 0 ? maxContextLength : $0 }
    }

    // MARK: - Inference step

    /// Run one forward pass and return the pre-allocated logits NDArray.
    private func runInference(
        function: InferenceFunction,
        tokenIDs: [Int],
        startPosition: Int,
        descriptor: InferenceFunctionDescriptor,
        inputIdsName: String,
        positionIdsName: String?,
        logitsName: String,
        vocabSize: Int,
        kvStates: inout [String: NDArray]
    ) async throws -> NDArray {
        let seqLen = tokenIDs.count
        guard let inputValueDesc = descriptor.inputDescriptor(of: inputIdsName) else {
            throw CoreAIBackendError.missingInputDescriptor(inputIdsName)
        }
        guard case .ndArray(let inputNDDesc) = inputValueDesc else {
            throw CoreAIBackendError.nonNDArrayInput(inputIdsName)
        }

        // Build input NDArrays.
        var inputs: [String: NDArray] = [:]
        inputs[inputIdsName] = makeInputNDArray(tokenIDs: tokenIDs, descriptor: inputNDDesc, fallbackShape: [1, seqLen])

        if let positionName = positionIdsName {
            let positions = (startPosition..<(startPosition + seqLen)).map { $0 }
            if let posValueDesc = descriptor.inputDescriptor(of: positionName),
               case .ndArray(let posNDDesc) = posValueDesc {
                inputs[positionName] = makeInputNDArray(tokenIDs: positions, descriptor: posNDDesc, fallbackShape: [1, seqLen])
            } else {
                inputs[positionName] = makeInputNDArray(tokenIDs: positions, scalarType: inputNDDesc.scalarType, shape: [1, seqLen])
            }
        }

        // Pre-allocate logits output view.
        guard let outputValueDesc = descriptor.outputDescriptor(of: logitsName) else {
            throw CoreAIBackendError.missingOutputDescriptor(logitsName)
        }
        guard case .ndArray(let outputNDDesc) = outputValueDesc else {
            throw CoreAIBackendError.nonNDArrayOutput(logitsName)
        }
        let logitsShape = try logitsShape(descriptor: outputNDDesc, sequenceLength: seqLen, vocabSize: vocabSize)
        var outputNDArray = NDArray(shape: logitsShape, scalarType: outputNDDesc.scalarType)

        // Assemble state and output MutableViews.  Core AI's MutableViews is lifetime-
        // dependent on the backing NDArrays, so keep the state arrays in function-scoped
        // variables (not dictionary/array subscripts) for the duration of the async run.
        // This model exports exactly two states: keyCache and valueCache.
        guard descriptor.stateNames.count == 2 else {
            throw CoreAIBackendError.unsupportedStateCount(descriptor.stateNames.count)
        }
        let keyCacheName = descriptor.stateNames[0]
        let valueCacheName = descriptor.stateNames[1]
        guard var keyCache = kvStates[keyCacheName] else {
            throw CoreAIBackendError.missingState(keyCacheName)
        }
        guard var valueCache = kvStates[valueCacheName] else {
            throw CoreAIBackendError.missingState(valueCacheName)
        }

        var statesView = InferenceFunction.MutableViews()
        statesView.insert(&keyCache, for: keyCacheName)
        statesView.insert(&valueCache, for: valueCacheName)

        var outputViews = InferenceFunction.MutableViews()
        outputViews.insert(&outputNDArray, for: logitsName)

        _ = try await function.run(
            inputs: inputs,
            states: consume statesView,
            outputViews: consume outputViews
        )

        // Write updated state arrays back into the session's KV state dictionary.
        kvStates[keyCacheName] = keyCache
        kvStates[valueCacheName] = valueCache

        // OutputViews consumed the mutable raw view, but the backing NDArray still owns the data.
        return outputNDArray
    }

    // MARK: - NDArray helpers

    private func makeInputNDArray(tokenIDs: [Int], descriptor: NDArrayDescriptor, fallbackShape: [Int]) -> NDArray {
        return makeInputNDArray(tokenIDs: tokenIDs, scalarType: descriptor.scalarType, shape: fallbackShape)
    }

    private func makeInputNDArray(tokenIDs: [Int], scalarType: NDArray.ScalarType, shape: [Int]) -> NDArray {
        switch scalarType {
        case .int32:
            return NDArray(scalars: tokenIDs.map(Int32.init), shape: shape)
        case .int64:
            return NDArray(scalars: tokenIDs.map(Int64.init), shape: shape)
        case .uint32:
            return NDArray(scalars: tokenIDs.map(UInt32.init), shape: shape)
        case .uint64:
            return NDArray(scalars: tokenIDs.map(UInt64.init), shape: shape)
        default:
            // Most text models expect signed 32-bit token IDs; default to that.
            return NDArray(scalars: tokenIDs.map(Int32.init), shape: shape)
        }
    }

    private func logitsShape(descriptor: NDArrayDescriptor, sequenceLength: Int, vocabSize: Int) throws -> [Int] {
        let shape = descriptor.shape
        switch shape.count {
        case 2:
            return [
                replaceIfDynamic(shape[0], sequenceLength),
                replaceIfDynamic(shape[1], vocabSize)
            ]
        case 3:
            return [
                replaceIfDynamic(shape[0], 1),
                replaceIfDynamic(shape[1], sequenceLength),
                replaceIfDynamic(shape[2], vocabSize)
            ]
        default:
            throw CoreAIBackendError.unsupportedLogitsShape(shape)
        }
    }

    private func replaceIfDynamic(_ dim: Int, _ value: Int) -> Int {
        return dim <= 0 ? value : dim
    }

    private func expectsSingleTokenInput(descriptor: NDArrayDescriptor) -> Bool {
        guard !descriptor.hasDynamicShape else { return false }
        let shape = descriptor.shape
        switch shape.count {
        case 1:
            return shape[0] == 1
        case 2:
            return shape[0] == 1 && shape[1] == 1
        default:
            return false
        }
    }

    // MARK: - Sampling

    private func sampleNextToken(
        logits: NDArray,
        sequenceLength: Int,
        vocabSize: Int,
        temperature: Float
    ) throws -> Int? {
        let lastLogits = try readLastLogits(from: logits, sequenceLength: sequenceLength, vocabSize: vocabSize)
        guard !lastLogits.isEmpty else { return nil }
        return sample(logits: lastLogits, temperature: temperature)
    }

    private func readLastLogits(from ndarray: NDArray, sequenceLength: Int, vocabSize: Int) throws -> [Float] {
        let totalElements = ndarray.shape.reduce(1, *)
        guard totalElements >= vocabSize else {
            throw CoreAIBackendError.unsupportedLogitsShape(ndarray.shape)
        }
        let lastOffset = totalElements - vocabSize

        switch ndarray.scalarType {
        case .float32:
            return ndarray.view(as: Float.self).withUnsafePointer { ptr, _, _ in
                let buffer = UnsafeBufferPointer(start: ptr.advanced(by: lastOffset), count: vocabSize)
                return Array(buffer)
            }
        case .float16:
            return ndarray.view(as: Float16.self).withUnsafePointer { ptr, _, _ in
                let buffer = UnsafeBufferPointer(start: ptr.advanced(by: lastOffset), count: vocabSize)
                return buffer.map { Float($0) }
            }
        default:
            throw CoreAIBackendError.unsupportedLogitsType(ndarray.scalarType)
        }
    }

    private func sample(logits: [Float], temperature: Float) -> Int {
        if temperature <= 0 {
            return argmax(logits)
        }
        let scaled = logits.map { $0 / temperature }
        let maxLogit = scaled.max() ?? 0
        var expScaled = scaled.map { exp($0 - maxLogit) }
        let sumExp = expScaled.reduce(0, +)
        if sumExp > 0 {
            expScaled = expScaled.map { $0 / sumExp }
        }
        let r = Float.random(in: 0..<1)
        var cumsum: Float = 0
        for (i, p) in expScaled.enumerated() {
            cumsum += p
            if r < cumsum { return i }
        }
        return expScaled.count - 1
    }

    private func argmax(_ values: [Float]) -> Int {
        var bestIndex = 0
        var bestValue = values[0]
        for (i, v) in values.enumerated().dropFirst() {
            if v > bestValue {
                bestValue = v
                bestIndex = i
            }
        }
        return bestIndex
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
    case noTokenizer
    case noTokenizerFiles(String)
    case emptyPrompt
    case missingInput(String)
    case missingOutput(String)
    case unsupportedInputCount(Int)
    case missingInputDescriptor(String)
    case nonNDArrayInput(String)
    case missingOutputDescriptor(String)
    case nonNDArrayOutput(String)
    case unsupportedLogitsShape([Int])
    case unsupportedLogitsType(NDArray.ScalarType)
    case missingStateDescriptor(String)
    case nonNDArrayState(String)
    case missingState(String)
    case contextTooLong(Int, Int)
    case unsupportedStateCount(Int)

    var description: String {
        switch self {
        case .notPrepared:
            return "Core AI backend used before prepare() completed"
        case .functionNotFound(let name):
            return "Core AI model has no function named '\(name)'"
        case .noTokenizer:
            return "Core AI backend has no tokenizer loaded"
        case .noTokenizerFiles(let path):
            return "No tokenizer files (tokenizer.json / tokenizer.model / tokenizer_config.json) found next to model at '\(path)'"
        case .emptyPrompt:
            return "Prompt encoded to empty token list"
        case .missingInput(let detail):
            return "Core AI function missing inputs: \(detail)"
        case .missingOutput(let detail):
            return "Core AI function missing outputs: \(detail)"
        case .unsupportedInputCount(let count):
            return "Core AI function has \(count) inputs; only 1–2 are supported"
        case .missingInputDescriptor(let name):
            return "Core AI function missing descriptor for input '\(name)'"
        case .nonNDArrayInput(let name):
            return "Core AI input '\(name)' is not an NDArray"
        case .missingOutputDescriptor(let name):
            return "Core AI function missing descriptor for output '\(name)'"
        case .nonNDArrayOutput(let name):
            return "Core AI output '\(name)' is not an NDArray"
        case .unsupportedLogitsShape(let shape):
            return "Core AI logits output has unsupported shape \(shape)"
        case .unsupportedLogitsType(let scalarType):
            return "Core AI logits output has unsupported scalar type \(scalarType)"
        case .missingStateDescriptor(let name):
            return "Core AI function missing descriptor for state '\(name)'"
        case .nonNDArrayState(let name):
            return "Core AI state '\(name)' is not an NDArray"
        case .missingState(let name):
            return "Core AI state '\(name)' not allocated"
        case .contextTooLong(let got, let max):
            return "Context length \(got) exceeds pre-allocated maximum \(max)"
        case .unsupportedStateCount(let count):
            return "Core AI model has \(count) states; only 2 (keyCache, valueCache) are supported"
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
    // Signature must mirror the real (#if canImport(CoreAI)) initializer so the call site
    // in main.swift compiles under plain `swift build` too (where this fallback is used).
    init(modelURL: URL, functionName: String = "main", maxTokens: Int = 512, temperature: Float = 0.7) {
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
