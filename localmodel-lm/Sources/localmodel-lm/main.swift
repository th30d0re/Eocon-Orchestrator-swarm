import Foundation

/// On-device inference sidecar.
/// Reads JSON-lines requests from stdin and writes JSON-lines responses to stdout.
/// Status/progress/errors go to stderr so they never interleave with the data channel.
/// Backends: `coreai` (stub until Core AI is wired), `mlx` (real when built with MLX +
/// the Metal Toolchain; otherwise fails loudly), default → stub.
struct Request: Codable {
    var session_id: String
    var prompt: String
    var role: String?
}

struct Response: Codable {
    var session_id: String
    var text: String
    var tokens: Int
    var latency_ms: Int
}

struct ErrorResponse: Codable {
    var session_id: String
    var error: String
}

struct Status: Codable {
    var status: String
    var backend: String?
    var phase: String?
    var model: String?
    var progress: Double?
    var models: [String]?
}

let args = CommandLine.arguments
// Default to the dev-friendly stub; `coreai` and `mlx` are the real backends.
let backendName = args.dropFirst().first ?? "stub"
let env = ProcessInfo.processInfo.environment
let mlxModelId = env["EOCON_MLX_MODEL"] ?? "mlx-community/Qwen3-4B-4bit"
let mlxLocalDir = env["EOCON_MLX_LOCAL_DIR"]
let coreaiModelPath = env["EOCON_COREAI_MODEL"] ?? ""
let coreaiTemperature = Float(env["EOCON_COREAI_TEMPERATURE"] ?? "") ?? 0.7

/// The model name surfaced in status JSON. For MLX, prefer the local-dir path when set
/// so status messages match the actual weights being loaded.
func mlxDisplayModel() -> String {
    mlxLocalDir ?? mlxModelId
}

/// Error thrown when a backend is requested on a platform/runtime that cannot support it.
enum UnsupportedBackendError: Error, CustomStringConvertible {
    case runtimeUnavailable(String)

    var description: String {
        switch self {
        case .runtimeUnavailable(let reason):
            return "Runtime unavailable: \(reason)"
        }
    }
}

/// Backend that fails immediately with a clear message when a backend is requested on a
/// platform/runtime that cannot support it (e.g. Core AI on macOS < 27).
struct UnsupportedBackend: InferenceBackend {
    let name: String
    let reason: String

    func prepare() async throws {
        throw UnsupportedBackendError.runtimeUnavailable(reason)
    }

    func generate(session_id: String, prompt: String) async throws -> (text: String, tokens: Int, latency_ms: Int) {
        throw UnsupportedBackendError.runtimeUnavailable(reason)
    }

    func modelInfo() -> [String: String] {
        ["backend": name, "error": reason]
    }
}

func makeBackend() -> InferenceBackend {
    switch backendName {
    case "mlx":
        return MLXBackend(modelId: mlxModelId)
    case "coreai":
        if #available(macOS 27.0, *) {
            return CoreAIBackend(modelURL: URL(fileURLWithPath: coreaiModelPath), temperature: coreaiTemperature)
        } else {
            return UnsupportedBackend(name: backendName, reason: "Core AI requires macOS 27.0 or newer")
        }
    default:
        return StubBackend(name: backendName)
    }
}

// `info <backend>` prints model metadata and exits; it does not start the stdin loop.
if backendName == "info" {
    let infoBackendName = args.dropFirst(2).first ?? "stub"
    let infoBackend: InferenceBackend
    switch infoBackendName {
    case "mlx":
        infoBackend = MLXBackend(modelId: mlxModelId)
    case "coreai":
        if #available(macOS 27.0, *) {
            infoBackend = CoreAIBackend(modelURL: URL(fileURLWithPath: coreaiModelPath), temperature: coreaiTemperature)
        } else {
            infoBackend = UnsupportedBackend(name: infoBackendName, reason: "Core AI requires macOS 27.0 or newer")
        }
    default:
        infoBackend = StubBackend(name: infoBackendName)
    }
    let info = infoBackend.modelInfo()
    if let data = try? JSONSerialization.data(withJSONObject: info),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(0)
}

let backend = makeBackend()

emitStatus(Status(status: "preparing", backend: backendName, phase: "init",
                  model: backendName == "mlx" ? mlxDisplayModel() : nil, progress: nil, models: nil))

do {
    try await backend.prepare()
} catch {
    emitStatus(Status(status: "error", backend: backendName, phase: "prepare",
                      model: nil, progress: nil, models: nil))
    FileHandle.standardError.write(Data("{\"status\":\"error\",\"backend\":\"\(backendName)\",\"detail\":\"\(error)\"}\n".utf8))
    exit(1)
}

emitStatus(Status(status: "ready", backend: backendName, phase: nil,
                  model: backendName == "mlx" ? mlxDisplayModel() : nil, progress: nil, models: nil))

while let line = readLine() {
    guard let data = line.data(using: .utf8),
          let req = try? JSONDecoder().decode(Request.self, from: data) else {
        continue
    }
    do {
        let result = try await backend.generate(session_id: req.session_id, prompt: req.prompt)
        emit(Response(session_id: req.session_id, text: result.text,
                      tokens: result.tokens, latency_ms: result.latency_ms))
    } catch {
        emit(ErrorResponse(session_id: req.session_id, error: "\(error)"))
    }
}

/// Response/result JSON goes to stdout (the data channel the Rust provider reads).
func emit<T: Encodable>(_ value: T) {
    if let data = try? JSONEncoder().encode(value),
       let str = String(data: data, encoding: .utf8) {
        print(str)
        fflush(stdout)
    }
}

/// Status/progress JSON goes to stderr so it never interleaves with the
/// response stream on stdout. The Rust provider gates spawn on a stderr `ready`.
func emitStatus(_ value: Status) {
    if let data = try? JSONEncoder().encode(value),
       var str = String(data: data, encoding: .utf8) {
        str += "\n"
        FileHandle.standardError.write(Data(str.utf8))
    }
}
