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
        // MLX backend (2.x API, the proven ChAi combo). VERIFIED building + running real
        // on-device inference. Build with `scripts/build-mlx.sh` (xcodebuild) — NOT plain
        // `swift build`, which compiles the code but does not produce MLX's Metal
        // `default.metallib`, causing a runtime "Failed to load the default metallib".
        // xcodebuild emits it into mlx-swift_Cmlx.bundle next to the executable.
        // Migrate to 3.x (ChatSession/KVCache) via mlx-swift-lm upgrade.md later.
        .package(url: "https://github.com/ml-explore/mlx-swift.git", exact: "0.29.1"),
        .package(url: "https://github.com/ml-explore/mlx-swift-lm", exact: "2.29.3"),
        .package(url: "https://github.com/huggingface/swift-transformers", exact: "1.1.9"),
    ],
    targets: [
        .executableTarget(
            name: "localmodel-lm",
            dependencies: [
                .product(name: "MLX", package: "mlx-swift"),
                .product(name: "MLXLLM", package: "mlx-swift-lm"),
                .product(name: "MLXLMCommon", package: "mlx-swift-lm"),
                .product(name: "Hub", package: "swift-transformers"),
            ]
        ),
    ]
)
