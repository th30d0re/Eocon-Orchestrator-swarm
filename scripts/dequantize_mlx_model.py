#!/usr/bin/env python3
"""Dequantize an MLX-native fused model into FP16 safetensors.

Usage:
    source .venv-coreai/bin/activate
    python3 scripts/dequantize_mlx_model.py \
        --model "$HOME/Documents/Theory/Redefining_racism/training/fused_models/RootLedger-Gemma3-12B-NPBP-Abliterated-v3-Fused" \
        --save-path "$HOME/Documents/Theory/Redefining_racism/training/fused_models/RootLedger-Gemma3-12B-NPBP-Abliterated-v3-Fused-Dequantized"

Override the model/output paths with environment variables:
    EOCON_MLX_MODEL_DIR
    EOCON_MLX_OUTPUT_DIR
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from mlx_lm import utils


def main() -> int:
    parser = argparse.ArgumentParser(description="Dequantize an MLX fused model to FP16")
    parser.add_argument("--model", type=str, required=True, help="Path to MLX model dir")
    parser.add_argument("--save-path", type=str, required=True, help="Output directory")
    args = parser.parse_args()

    model_path = Path(args.model)
    save_path = Path(args.save_path)
    save_path.mkdir(parents=True, exist_ok=True)

    print(f"Loading MLX model from {model_path} ...")
    model, tokenizer, config = utils.load(str(model_path), return_config=True)

    print("Dequantizing ...")
    model = utils.dequantize_model(model)

    print(f"Saving FP16 model to {save_path} ...")
    utils.save(save_path, model_path, model, tokenizer, config)

    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
