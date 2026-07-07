#!/usr/bin/env bash
# Convert the (already dequantized) RootLedger-Gemma3-12B FP16 checkpoint to a Core AI
# .aimodel. This is the MEMORY-HEAVY step that twice crashed a 64 GB / no-swap machine.
#
# SAFETY: this script refuses to run unless the machine has enough RAM and free disk, so it
# cannot panic an under-provisioned box. Intended home: the 512 GB M3 Ultra. To override the
# guard on the dev machine (at your own risk of a forced restart), set FORCE=1.
#
# The dequantize step is NOT re-run — the 22 GB FP16 checkpoint already exists on disk.
set -uo pipefail

# --- Config (override via env) ---------------------------------------------------------
DEFAULT_SRC="${HOME}/Documents/Theory/Redefining_racism/training/fused_models/RootLedger-Gemma3-12B-NPBP-Abliterated-v3-Fused-Dequantized"
DEFAULT_OUT="${EOCON_SWARM_ROOT:-${HOME}/Dev/Tools/Eocon-Orchestrator-swarm}"
SRC="${SRC:-$DEFAULT_SRC}"
OUT_DIR="${OUT_DIR:-$DEFAULT_OUT}"
OUT_NAME="${OUT_NAME:-RootLedger-Gemma3-12B-CoreAI}"
MAX_CTX="${MAX_CTX:-2048}"                 # reduced from the 8192 default to shrink the export peak
PRECISION="${PRECISION:-bfloat16}"
VENV="${VENV:-.venv-coreai-models}"        # the venv that has coreai_models + torch
MIN_RAM_GB="${MIN_RAM_GB:-96}"             # below this, the export risks OOM-panic; 64 GB crashed twice
MIN_DISK_GB="${MIN_DISK_GB:-40}"           # ~11 GB .aimodel + temp + headroom

cd "$(dirname "$0")/.."

ram_gb=$(( $(sysctl -n hw.memsize) / 1073741824 ))
disk_gb=$(df -g . | awk 'NR==2 {print $4}')

echo "RAM: ${ram_gb} GB | free disk: ${disk_gb} GB | source: $SRC"
echo "Output: ${OUT_DIR}/${OUT_NAME}.aimodel | max-context: ${MAX_CTX} | precision: ${PRECISION}"

# --- Preflight guards ------------------------------------------------------------------
fail=0
[ -d "$SRC" ] || { echo "ERROR: source checkpoint not found: $SRC"; fail=1; }
[ -x "$VENV/bin/python" ] || { echo "ERROR: venv python not found: $VENV/bin/python"; fail=1; }
if [ "$ram_gb" -lt "$MIN_RAM_GB" ]; then
    echo "GUARD: ${ram_gb} GB RAM < ${MIN_RAM_GB} GB. The 12B export peaks above this and will"
    echo "       likely make macOS unresponsive and force a restart (it did twice on 64 GB)."
    echo "       Run this on the 512 GB M3 Ultra, or set FORCE=1 to override at your own risk."
    [ "${FORCE:-0}" = "1" ] || fail=1
fi
if [ "$disk_gb" -lt "$MIN_DISK_GB" ]; then
    echo "GUARD: only ${disk_gb} GB free disk < ${MIN_DISK_GB} GB needed for the .aimodel + temp/swap."
    [ "${FORCE:-0}" = "1" ] || fail=1
fi
if [ "$fail" -ne 0 ]; then
    echo "Aborting (preflight failed). No heavy work started."
    exit 1
fi
[ "${FORCE:-0}" = "1" ] && [ "$ram_gb" -lt "$MIN_RAM_GB" ] && \
    echo ">>> FORCE=1: proceeding on under-provisioned RAM. A forced restart is possible. <<<"

# --- Convert ---------------------------------------------------------------------------
echo "Starting Core AI export (this is the heavy step)…"
exec "$VENV/bin/python" scripts/convert_gemma3_coreai.py "$SRC" \
    --output-dir "$OUT_DIR" \
    --output-name "$OUT_NAME" \
    --max-context-length "$MAX_CTX" \
    --compute-precision "$PRECISION" \
    --overwrite
