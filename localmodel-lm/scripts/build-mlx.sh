#!/usr/bin/env bash
# Build the MLX-enabled sidecar.
#
# WHY xcodebuild (not `swift build`): mlx-swift ships 42 `.metal` shaders that must be
# compiled into `default.metallib`. SwiftPM's `swift build` compiles the C++/Swift but
# NOT the Metal shaders, so the resulting binary dies at runtime with
#   "MLX error: Failed to load the default metallib".
# Xcode's build system has the Metal compile rule and emits the metallib into
#   <Products>/mlx-swift_Cmlx.bundle/Contents/Resources/default.metallib
# next to the executable, where MLX finds it.
#
# Verified working: downloads from HF, loads, and generates on device.
set -euo pipefail

cd "$(dirname "$0")/.."

DERIVED="${DERIVED:-.build/xcode}"

echo "Building localmodel-lm (MLX) via xcodebuild …"
xcodebuild \
  -scheme localmodel-lm \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$DERIVED" \
  build

BIN="$DERIVED/Build/Products/Debug/localmodel-lm"
BUNDLE="$DERIVED/Build/Products/Debug/mlx-swift_Cmlx.bundle"

echo
echo "Executable: $BIN"
echo "Metal bundle: $BUNDLE"
echo
echo "IMPORTANT: the binary must run with mlx-swift_Cmlx.bundle in the same directory."
echo "Smoke test (downloads a small model from Hugging Face on first run):"
echo "  printf '{\"session_id\":\"s1\",\"prompt\":\"hello\"}\\n' \\"
echo "    | EOCON_MLX_MODEL=mlx-community/Qwen2.5-0.5B-Instruct-4bit \"$BIN\" mlx"
