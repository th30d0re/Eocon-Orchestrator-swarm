#!/usr/bin/env bash
# Build the Core AI-enabled sidecar against the macOS 27.0 SDK.
#
# WHY xcodebuild (not `swift build`): Core AI is only available in the 27.0+ SDK,
# which ships with Xcode-beta. The sidecar must be linked against CoreAI.framework
# and the Metal toolchain must be present so the model specialization pipeline can
# compile kernels for the target device.
#
# Set DEVELOPER_DIR to Xcode-beta so xcodebuild uses the 27.0 SDK that ships CoreAI.
# If you hit "MetalToolchain not installed", run:
#   sudo xcodebuild -downloadComponent MetalToolchain
#
# Verified target: macOS 27.0 (Beta) on Apple Silicon.
set -euo pipefail

export DEVELOPER_DIR=/Applications/Xcode-beta.app/Contents/Developer

cd "$(dirname "$0")/.."

DERIVED="${DERIVED:-.build/xcode-coreai}"

echo "Ensuring MetalToolchain is available …"
# This may require `sudo xcodebuild -downloadComponent MetalToolchain` on a fresh machine.
# Do not fail the script here so the user can see the subsequent build error details.
xcodebuild -downloadComponent MetalToolchain || echo "WARNING: MetalToolchain download failed or requires sudo; build will fail if the toolchain is not already installed."

echo "Building localmodel-lm (Core AI) via xcodebuild against 27.0 SDK …"
xcodebuild \
  -scheme localmodel-lm \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$DERIVED" \
  MACOSX_DEPLOYMENT_TARGET=27.0 \
  build

BIN="$DERIVED/Build/Products/Debug/localmodel-lm"

echo
echo "Executable: $BIN"
echo
echo "Run with a local .aimodel bundle and tokenizer files next to it:"
echo "  printf '{\"session_id\":\"s1\",\"prompt\":\"hello\"}\\n' \\"
echo "    | EOCON_COREAI_MODEL=/path/to/model.aimodel \"$BIN\" coreai"
