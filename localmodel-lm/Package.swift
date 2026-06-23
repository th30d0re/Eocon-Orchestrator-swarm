// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "localmodel-lm",
    platforms: [
        .macOS(.v15),
    ],
    products: [
        .executable(name: "localmodel-lm", targets: ["localmodel-lm"]),
        .executable(name: "coreai-lm", targets: ["localmodel-lm"]),
    ],
    dependencies: [
        // MLX backend — 3.x line (ChatSession/KVCache + wired memory). Build with
        // `scripts/build-mlx.sh` (xcodebuild) so MLX's Metal `default.metallib` is
        // compiled into mlx-swift_Cmlx.bundle next to the executable; plain `swift build`
        // omits it. (The proven 2.x baseline lives on the `main` branch.)
        .package(url: "https://github.com/ml-explore/mlx-swift.git", from: "0.31.0"),
        .package(url: "https://github.com/ml-explore/mlx-swift-lm", from: "3.31.0"),
        // 3.x split HF download + tokenizers into adapter packages (per mlx-swift-lm upgrade.md).
        .package(url: "https://github.com/DePasqualeOrg/swift-tokenizers-mlx", from: "0.1.0"),
        .package(url: "https://github.com/DePasqualeOrg/swift-hf-api-mlx", from: "0.1.0"),
    ],
    targets: [
        .executableTarget(
            name: "localmodel-lm",
            dependencies: [
                .product(name: "MLX", package: "mlx-swift"),
                .product(name: "MLXLLM", package: "mlx-swift-lm"),
                .product(name: "MLXLMCommon", package: "mlx-swift-lm"),
                .product(name: "MLXLMTokenizers", package: "swift-tokenizers-mlx"),
                .product(name: "MLXLMHFAPI", package: "swift-hf-api-mlx"),
            ]
        ),
    ]
)
