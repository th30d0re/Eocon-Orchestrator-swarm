#!/usr/bin/env bash
# Build/run/test helper for the Eocon agent-runner Apple Containerization image.
# Usage: ./deploy/run-eocon-agent-runner.sh [build|run|test|config|machine-create|machine-run|machine-test|machine-shell]

set -euo pipefail

IMAGE_NAME="${IMAGE_NAME:-eocon-agent-runner:latest}"
MACHINE_NAME="${MACHINE_NAME:-eocon-pool}"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Offline mode: set EOCON_OFFLINE=1 to discourage network fetches inside the
# container (e.g. cargo/pip resolution). CARGO_HOME and RUSTUP_HOME point to
# predictable paths so a persistent machine can cache toolchains.
COMMON_ENV=(
  "-e" "EOCON_OFFLINE=${EOCON_OFFLINE:-0}"
  "-e" "CARGO_HOME=/root/.cargo"
  "-e" "RUSTUP_HOME=/root/.rustup"
)

# 8 GiB is enough for concurrent rust/python builds and leaves headroom for
# model weights or media processing inside the sandbox.
COMMON_RES=(
  "-m" "8G"
)

# Optional build arguments. Set INSTALL_SWIFT=true to include the Swift
# toolchain in the image.
INSTALL_SWIFT="${INSTALL_SWIFT:-false}"

require_container() {
  if ! command -v container >/dev/null 2>&1; then
    echo "Error: the Apple 'container' CLI was not found." >&2
    echo "Install it from https://github.com/apple/container/releases and run:" >&2
    echo "  container system start" >&2
    exit 1
  fi
}

cmd_build() {
  require_container
  echo "Building ${IMAGE_NAME} ..."
  container build \
    -f "${PROJECT_DIR}/deploy/Containerfile.agent-runner" \
    -t "${IMAGE_NAME}" \
    --build-arg "INSTALL_SWIFT=${INSTALL_SWIFT}" \
    "${PROJECT_DIR}"
  echo "Build complete: ${IMAGE_NAME}"
}

cmd_run() {
  require_container
  echo "Running interactive shell in ${IMAGE_NAME} ..."
  # Override the entrypoint to bash and pass a command to replace the image CMD,
  # otherwise the image's CMD ["/bin/bash"] would be appended as an argument.
  exec container run -i --rm \
    "${COMMON_RES[@]}" \
    "${COMMON_ENV[@]}" \
    --entrypoint /bin/bash \
    -w /app \
    "${IMAGE_NAME}" \
    -c "exec /bin/bash"
}

cmd_test() {
  require_container
  echo "Smoke-testing ${IMAGE_NAME} ..."

  container run --rm \
    "${COMMON_RES[@]}" \
    "${COMMON_ENV[@]}" \
    --entrypoint /bin/bash \
    -w /app \
    "${IMAGE_NAME}" \
    -c "set -euo pipefail; \
        echo '=> uname'; uname -a; \
        echo '=> python3'; python3 --version; \
        echo '=> cargo'; cargo --version; \
        echo '=> rustc'; rustc --version"

  echo "Smoke tests passed."
}

cmd_config() {
  cat <<EOF
{
  "runner": "container",
  "image": "${IMAGE_NAME}",
  "machine": "${MACHINE_NAME}",
  "memory": "8G",
  "env": {
    "EOCON_OFFLINE": "${EOCON_OFFLINE:-0}",
    "CARGO_HOME": "/root/.cargo",
    "RUSTUP_HOME": "/root/.rustup"
  }
}
EOF
}

cmd_machine_create() {
  require_container
  echo "Creating persistent Container machine: ${MACHINE_NAME} ..."
  container machine create \
    --name "${MACHINE_NAME}" \
    --set-default \
    "${IMAGE_NAME}"
  echo "Machine created. Use './deploy/run-eocon-agent-runner.sh machine-run' to start a shell inside it."
}

cmd_machine_run() {
  require_container
  echo "Running command inside Container machine: ${MACHINE_NAME} ..."
  # Pass the command as a single quoted string because 'container machine run'
  # executes it through /bin/bash -c and strips outer quoting if split.
  exec container machine run \
    --name "${MACHINE_NAME}" \
    -i \
    -w "${PROJECT_DIR}" \
    "${COMMON_ENV[@]}" \
    "/bin/bash"
}

cmd_machine_test() {
  require_container
  echo "Smoke-testing inside Container machine: ${MACHINE_NAME} ..."
  container machine run \
    --name "${MACHINE_NAME}" \
    -w "${PROJECT_DIR}" \
    "${COMMON_ENV[@]}" \
    "/bin/bash -c 'set -euo pipefail; echo \"=> uname\"; uname -a; echo \"=> python3\"; python3 --version; echo \"=> cargo\"; cargo --version; echo \"=> rustc\"; rustc --version'"
  echo "Machine smoke tests passed."
}

cmd_machine_shell() {
  require_container
  echo "Opening interactive shell inside Container machine: ${MACHINE_NAME} ..."
  exec container machine run \
    --name "${MACHINE_NAME}" \
    -i -t \
    -w "${PROJECT_DIR}" \
    "${COMMON_ENV[@]}"
}

cmd_help() {
  cat <<EOF
Usage: $(basename "$0") [build|run|test|config|machine-create|machine-run|machine-test|machine-shell]

  build          Build the Apple Containerization agent-runner image.
  run            Run an interactive shell in an ephemeral container.
  test           Run smoke tests in an ephemeral container.
  config         Print a sample runner config JSON.
  machine-create Create a persistent Container machine from the image.
  machine-run    Run a shell inside the persistent machine (reads stdin).
  machine-test   Run smoke tests inside the persistent machine.
  machine-shell  Open an interactive TTY shell inside the persistent machine.

Environment variables:
  IMAGE_NAME      Image tag (default: ${IMAGE_NAME})
  MACHINE_NAME    Container machine name (default: ${MACHINE_NAME})
  EOCON_OFFLINE   Set to 1 to prefer cached/local resources (default: 0)
  INSTALL_SWIFT   Set to true to include Swift toolchain at build time (default: false)
EOF
}

main() {
  local subcommand="${1:-help}"
  case "${subcommand}" in
    build)          cmd_build ;;
    run)            cmd_run ;;
    test)           cmd_test ;;
    config)         cmd_config ;;
    machine-create) cmd_machine_create ;;
    machine-run)    cmd_machine_run ;;
    machine-test)   cmd_machine_test ;;
    machine-shell)  cmd_machine_shell ;;
    help|--help|-h) cmd_help ;;
    *)
      echo "Unknown command: ${subcommand}" >&2
      cmd_help >&2
      exit 1
      ;;
  esac
}

main "$@"
