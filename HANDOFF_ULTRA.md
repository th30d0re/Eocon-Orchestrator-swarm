# Core AI 12B conversion + sidecar test — Ultra handoff

## For the human (read this part — ~60 seconds)

We need to do **one heavy step on a big-memory Mac** (the 512 GB M3 Ultra), then a quick test.
The 12B model conversion peaks well above 64 GB of RAM, which is why it must run here and not
on the laptop.

**What you need on this machine:**
1. **The fused model weights** — ~6.2 GB. They are *not* online; bring them on the drive.
   Folder name: `RootLedger-Gemma3-12B-NPBP-Abliterated-v3-Fused/` (contains `*.safetensors`).
2. **Xcode 27** (or Xcode-beta) installed — needed to build the Core AI test program.
3. **Python 3.11** + internet (to install the conversion tools).

**What happens:** convert the model to a Core AI `.aimodel`, build a tiny program that loads
it, and check it produces coherent chat text. Then tell us the result.

**Hand the rest of this file to your coding agent** and point it at the section below — it has
the exact commands. If anything is ambiguous, the agent should prefer the
`apple/coreai-models` README for the Python install and `HANDOFF_ULTRA.md` for the run order.

---

## For the agent (dense)

### Goal
Produce `RootLedger-Gemma3-12B-CoreAI.aimodel` from the fused MLX checkpoint, build the
`localmodel-lm` Core AI sidecar, and smoke-test it. Report whether `AIModel(contentsOf:)`
specializes and whether generation is coherent (the chat template is applied in the sidecar).

### Hard constraints / why
- The **convert** step (FP16 weights + export graph) peaks well above 64 GB; this Ultra (512 GB)
  is the target. `scripts/convert-coreai.sh` has a RAM/disk preflight guard — it will pass here.
- The Core AI sidecar **must** build with **Xcode 27 / the 27.0 SDK** (`CoreAI.framework` only
  ships there). `scripts/build-coreai.sh` sets `DEVELOPER_DIR=/Applications/Xcode-beta.app/...`.
  Adjust that path if Xcode 27 is the default `Xcode.app`.
- Do **not** commit weights or `.aimodel` (already in `.gitignore`).

### 0. Clone + place inputs
```bash
git clone <THIS_REPO_URL> Eocon-Orchestrator-swarm
cd Eocon-Orchestrator-swarm
# Put the fused model (from the drive) somewhere, e.g.:
#   ~/models/RootLedger-Gemma3-12B-NPBP-Abliterated-v3-Fused/
```

### 1. Python envs (two of them)
**venv A — dequantize (MLX):**
```bash
python3.11 -m venv .venv-coreai
.venv-coreai/bin/pip install "mlx-lm==0.31.3"
```
**venv B — convert (Apple Core AI):**
```bash
python3.11 -m venv .venv-coreai-models
git clone https://github.com/apple/coreai-models .deps/coreai-models
( cd .deps/coreai-models && git checkout 5a0f161eaa093212f1693a56d0c2c45ebd64bf94 )
.venv-coreai-models/bin/pip install -e .deps/coreai-models/python
# Apple Core AI Python packages (ship with the Core AI tooling — see the
# apple/coreai-models README if pip can't find them directly):
.venv-coreai-models/bin/pip install \
  "coreai-core==1.0.0b1" "coreai-torch==0.4.0" "coreai-opt==0.2.0" \
  "torch==2.9.0" "transformers==4.57.6" "safetensors==0.7.0" \
  "tokenizers==0.22.2" "huggingface_hub==0.36.2" "numpy==2.3.5"
```

### 2. Dequantize (4-bit MLX → FP16)  — fast, ~22 GB output
```bash
FUSED=~/models/RootLedger-Gemma3-12B-NPBP-Abliterated-v3-Fused
.venv-coreai/bin/python scripts/dequantize_mlx_model.py \
  --model "$FUSED" --save-path "${FUSED}-Dequantized"
```

### 3. Convert (FP16 → .aimodel)  — the heavy step (fine on 512 GB)
```bash
# On the Ultra you can use the model's real context (8192). MAX_CTX shrinks the export peak;
# only lower it on a memory-constrained box.
SRC="${FUSED}-Dequantized" OUT_DIR="$PWD" OUT_NAME="RootLedger-Gemma3-12B-CoreAI" \
  MAX_CTX=8192 ./scripts/convert-coreai.sh
# Produces ./RootLedger-Gemma3-12B-CoreAI.aimodel (+ a tokenizer/ dir is needed next to it).
```
The sidecar's tokenizer loader expects tokenizer files either next to the `.aimodel` or in a
sibling `tokenizer/` dir. Copy them from the fused model:
```bash
mkdir -p RootLedger-Gemma3-12B-CoreAI/tokenizer
cp "$FUSED"/tokenizer*.json "$FUSED"/special_tokens_map.json "$FUSED"/chat_template.jinja \
   RootLedger-Gemma3-12B-CoreAI/tokenizer/ 2>/dev/null || true
```

### 4. Build the Core AI sidecar (Xcode 27)
```bash
cd localmodel-lm
./scripts/build-coreai.sh        # uses DEVELOPER_DIR=Xcode-beta; needs the Metal Toolchain
cd ..
```

### 5. Smoke test
```bash
BIN=localmodel-lm/.build/xcode-coreai/Build/Products/Debug/localmodel-lm
printf '{"session_id":"s1","prompt":"Hello, who are you?"}\n' \
  | EOCON_COREAI_MODEL="$PWD/RootLedger-Gemma3-12B-CoreAI/RootLedger-Gemma3-12B-CoreAI.aimodel" \
    "$BIN" coreai
```
Expected: stderr shows `preparing/specialize` → `ready`; stdout is one JSON line
`{"session_id":"s1","text":"…","tokens":N,"latency_ms":…}` with **coherent** text (the chat
template is applied automatically because `tokenizer_config.json` carries one).

### What to report back
1. Did `AIModel(contentsOf:)` **specialize** (vs `failedToSpecialize` / "Missing hash file")?
   A missing-hash-file error means the export didn't finalize — re-run step 3.
2. Is the generated text coherent? (Raw-prompt garbage would mean the chat template didn't apply.)
3. Any runtime error in `run(inputs:states:outputViews:)` — especially anything about KV state
   shape. Apple's KVCache layout is `(n_layers, batch, n_kv_heads, seq_len, head_dim)`; the
   sidecar allocates states from the function descriptor, so report the exact error if it fails.
4. Paste the full stderr lifecycle + the stdout line.

### Background (only if needed)
- Full project context: `NEXT_STEPS.mdx` (esp. §7 Core AI, §12 conversion validation).
- Fast local checks (no GPU/CoreAI): `./scripts/verify.sh`.
- The Python validation already confirmed the **weights + Apple's `Gemma3ForCausalLM` are
  correct** (top-20 tokens match HF). So any failure here is in the Core AI runtime /
  specialization, not the model.
