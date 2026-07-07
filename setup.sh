#!/usr/bin/env bash
# Automated setup for Eocon Orchestrator Swarm.
# Usage: ./setup.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ORCHESTRATOR_DIR="${REPO_ROOT}/orchestrator"
SIDECAR_DIR="${REPO_ROOT}/localmodel-lm"
DEPLOY_DIR="${ORCHESTRATOR_DIR}/deploy"
VENV_DIR="${REPO_ROOT}/.venv"

warn() {
  echo "⚠️  $1" >&2
}

info() {
  echo "▶ $1"
}

fail() {
  echo "❌ $1" >&2
  exit 1
}

# -----------------------------------------------------------------------------
# 1. Platform check
# -----------------------------------------------------------------------------
OS="$(uname -s)"
ARCH="$(uname -m)"

if [[ "${OS}" != "Darwin" ]]; then
  warn "This project is developed and tested on macOS. Detected OS: ${OS}"
fi

if [[ "${ARCH}" != "arm64" ]]; then
  warn "Apple Silicon (arm64) is strongly recommended for MLX / Core AI. Detected arch: ${ARCH}"
fi

# -----------------------------------------------------------------------------
# 2. Rust toolchain
# -----------------------------------------------------------------------------
install_rust() {
  if ! command -v rustup >/dev/null 2>&1; then
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck source=/dev/null
    source "${HOME}/.cargo/env"
  fi
  rustup update stable
}

if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
  install_rust
fi

info "Rust toolchain: $(rustc --version), $(cargo --version)"

# -----------------------------------------------------------------------------
# 3. Swift toolchain
# -----------------------------------------------------------------------------
if ! command -v swift >/dev/null 2>&1; then
  warn "Swift toolchain not found. Install Xcode or the Swift toolchain to build localmodel-lm."
  warn "  https://www.swift.org/install/"
else
  info "Swift toolchain: $(swift --version 2>&1 | head -n 1)"
fi

# -----------------------------------------------------------------------------
# 4. Apple container CLI
# -----------------------------------------------------------------------------
if ! command -v container >/dev/null 2>&1; then
  warn "Apple 'container' CLI not found. Container sandboxing will be unavailable."
  warn "Install it from https://github.com/apple/container/releases, then run:"
  warn "  container system start"
else
  info "Apple container CLI: $(container --version 2>&1 | head -n 1)"
fi

# -----------------------------------------------------------------------------
# 5. Python dependencies (best-effort)
# -----------------------------------------------------------------------------
install_python_deps() {
  if ! command -v python3 >/dev/null 2>&1; then
    warn "python3 not found. Skipping Python dependency installation."
    return 0
  fi

  if [[ ! -d "${VENV_DIR}" ]]; then
    info "Creating Python virtual environment at ${VENV_DIR}..."
    python3 -m venv "${VENV_DIR}"
  fi

  # shellcheck source=/dev/null
  source "${VENV_DIR}/bin/activate"

  info "Upgrading pip..."
  pip install --upgrade pip setuptools wheel

  if [[ -f "${REPO_ROOT}/scripts/requirements.txt" ]]; then
    info "Installing scripts/requirements.txt..."
    pip install -r "${REPO_ROOT}/scripts/requirements.txt" || warn "Some script requirements failed to install."
  fi

  if [[ -f "${REPO_ROOT}/.deps/coreai-models/python/pyproject.toml" ]]; then
    info "Installing local coreai-models package (best-effort)..."
    pip install -e "${REPO_ROOT}/.deps/coreai-models/python" || warn "coreai-models install failed (private coreai-* packages may be missing)."
  fi
}

install_python_deps

# -----------------------------------------------------------------------------
# 6. Build orchestrator
# -----------------------------------------------------------------------------
info "Building Rust orchestrator..."
cd "${ORCHESTRATOR_DIR}"
cargo build --release

# -----------------------------------------------------------------------------
# 7. Run tests
# -----------------------------------------------------------------------------
info "Running Rust tests..."
cargo test

# -----------------------------------------------------------------------------
# 8. Build agent-runner container image (best-effort)
# -----------------------------------------------------------------------------
if command -v container >/dev/null 2>&1; then
  info "Building agent-runner container image..."
  cd "${ORCHESTRATOR_DIR}"
  "${DEPLOY_DIR}/run-eocon-agent-runner.sh" build
else
  warn "Skipping agent-runner image build because the Apple 'container' CLI is missing."
fi

# -----------------------------------------------------------------------------
# 9. Set executable permissions on helper scripts
# -----------------------------------------------------------------------------
info "Setting executable permissions on helper scripts..."
chmod +x "${DEPLOY_DIR}/run-eocon-agent-runner.sh" || true
find "${SIDECAR_DIR}/scripts" -type f -name '*.sh' -exec chmod +x {} + || true
find "${REPO_ROOT}/scripts" -type f -name '*.sh' -exec chmod +x {} + || true

# Ensure this script itself is executable
chmod +x "${REPO_ROOT}/setup.sh" || true

info "Setup complete."
echo ""
echo "Next steps:"
echo "  cd orchestrator"
echo "  cargo run -- --swarm-demo"
echo ""
echo "For container sandboxing, install the Apple container CLI and run:"
echo "  cd orchestrator && ./deploy/run-eocon-agent-runner.sh build"
