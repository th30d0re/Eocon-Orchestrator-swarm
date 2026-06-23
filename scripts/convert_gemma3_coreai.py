#!/usr/bin/env python3
# Copyright 2026 Apple Inc. and Emmanuel Ogunjirin
#
# Adapted from apple/coreai-models under BSD-3-Clause license.
# Converts a local HuggingFace-style Gemma 3 text checkpoint to Apple Core AI
# .aimodel format using the coreai-models export pipeline.

from __future__ import annotations

import argparse
import gc
import json
import logging
import os
import shutil
from datetime import datetime
from pathlib import Path
from typing import Any

import torch
from transformers import AutoConfig, AutoTokenizer

# Import coreai_models helpers (these are module-private but stable in the
# upstream source tree for memory-efficient loading).
from coreai_models.export.metadata import build_aimodel_metadata
from coreai_models.export.pipeline import ExportConfig
from coreai_models.export.macos import export_macos_model
from coreai_models.models.base import (
    _build_safetensors_key_index,
    _load_tensors_for_keys,
    _resolve_safetensors_files,
    _save_and_mmap_safetensors,
)
from coreai_models.models.macos.gemma3_text import Gemma3ForCausalLM
from coreai_models.models.registry import get_model_entry
from coreai_models.primitives.macos.cache import KVCache

KV_K_BUFFER = "_full_cached_k"
KV_V_BUFFER = "_full_cached_v"

logger = logging.getLogger("convert_gemma3_coreai")


def load_config(
    local_dir: str,
    max_context_length: int | None,
    num_layers: int | None = None,
) -> Any:
    """Load the Gemma 3 text config from a local checkpoint directory."""
    raw_config = AutoConfig.from_pretrained(local_dir)
    entry = get_model_entry(getattr(raw_config, "model_type", "gemma3"))
    if entry.hf_config_attr is None:
        raise ValueError("Expected hf_config_attr for Gemma 3 multimodal config")
    hf_config = getattr(raw_config, entry.hf_config_attr)

    if max_context_length is not None:
        hf_config.max_position_embeddings = max_context_length
        logger.info(f"Overrode max_position_embeddings to {max_context_length}")

    if num_layers is not None:
        hf_config.num_hidden_layers = num_layers
        logger.info(f"Truncated to {num_layers} layers")

    # Gemma-3 ties input/output embeddings. The MLX-fine-tuned checkpoints omit
    # lm_head.weight; without this flag the Apple model leaves lm_head uninitialized.
    if not hasattr(hf_config, "tie_word_embeddings") or hf_config.tie_word_embeddings is None:
        hf_config.tie_word_embeddings = True
        logger.info("Set tie_word_embeddings=True (Gemma-3 default)")

    return hf_config


def load_model_from_local(
    local_dir: str,
    config: Any,
    target_dtype: torch.dtype,
    mmap_dir: str | None = None,
    num_layers: int | None = None,
) -> Gemma3ForCausalLM:
    """Load a Gemma3ForCausalLM from local safetensors with layer-by-layer offloading.

    Mirrors coreai_models.models.base.BaseForCausalLM.from_hf_memory_efficient but
    reads from a local directory instead of downloading from HuggingFace Hub.
    """
    entry = get_model_entry("gemma3_text")
    prefix = entry.hf_state_dict_prefix  # e.g. "language_model."

    model = Gemma3ForCausalLM(config, model_device="meta")
    model.to(dtype=target_dtype)

    safetensors_files = _resolve_safetensors_files(local_dir)
    per_layer_index, shared_index = _build_safetensors_key_index(
        safetensors_files,
        num_layers=num_layers,
        hf_state_dict_prefix=prefix,
    )

    # Shared params first.
    shared_dict = _load_tensors_for_keys(shared_index, target_dtype)
    shared_dict = {k.removeprefix(prefix): v for k, v in shared_dict.items()}
    del shared_index

    if mmap_dir is not None:
        os.makedirs(mmap_dir, exist_ok=True)
        shared_path = os.path.join(mmap_dir, "shared.safetensors")
        _save_and_mmap_safetensors(model, shared_dict, shared_path)
    else:
        model.load_state_dict(shared_dict, assign=True, strict=False)
    del shared_dict
    gc.collect()

    # One transformer layer at a time.
    exclude_buffers = {KVCache.HF_K_BUFFER_NAME, KVCache.HF_V_BUFFER_NAME}
    for layer_idx in sorted(per_layer_index.keys()):
        layer_key_to_file = per_layer_index.pop(layer_idx)
        layer_sd = _load_tensors_for_keys(layer_key_to_file, target_dtype)
        layer_sd = {k.removeprefix(prefix): v for k, v in layer_sd.items()}
        del layer_key_to_file

        model._mutate_state_dict(layer_sd)

        if mmap_dir is not None:
            layer_prefix = f"model.layers.{layer_idx}."
            relative_sd = {
                k.removeprefix(layer_prefix): v
                for k, v in layer_sd.items()
                if k.removeprefix(layer_prefix) not in exclude_buffers
            }
            layer_module = model.model.layers[layer_idx]
            layer_path = os.path.join(mmap_dir, f"layer_{layer_idx}.safetensors")
            _save_and_mmap_safetensors(layer_module, relative_sd, layer_path)
            del relative_sd
        else:
            model.load_state_dict(layer_sd, assign=True, strict=False)

        del layer_sd
        gc.collect()

    meta_params = [n for n, p in model.named_parameters() if p.is_meta]
    if meta_params:
        raise RuntimeError(f"Parameters not loaded: {meta_params}")

    return model


def export_local_gemma3(
    local_dir: str,
    output_dir: str,
    output_name: str | None = None,
    max_context_length: int | None = None,
    compute_precision: str = "bfloat16",
    compression: str = "none",
    overwrite: bool = False,
    num_layers: int | None = None,
) -> str:
    """Export a local Gemma 3 checkpoint to a Core AI LLM bundle."""
    precision_map = {"float16": torch.float16, "bfloat16": torch.bfloat16, "float32": torch.float32}
    target_dtype = precision_map.get(compute_precision)
    if target_dtype is None:
        raise ValueError(f"Unsupported compute_precision: {compute_precision}")

    config = load_config(local_dir, max_context_length, num_layers=num_layers)
    logger.info(f"Loaded config: layers={config.num_hidden_layers}, vocab={config.vocab_size}, "
                f"hidden={config.hidden_size}, max_pos={config.max_position_embeddings}")

    import tempfile
    with tempfile.TemporaryDirectory(prefix="coreai_export_") as temp_dir:
        layer_mmap_dir = os.path.join(temp_dir, "layers")
        os.makedirs(layer_mmap_dir, exist_ok=True)

        logger.info(f"Loading model weights from {local_dir} (dtype={compute_precision})...")
        model = load_model_from_local(
            local_dir,
            config,
            target_dtype=target_dtype,
            mmap_dir=layer_mmap_dir,
            num_layers=num_layers,
        )
        model = model.eval()
        logger.info("Model loaded successfully.")

        export_cfg = ExportConfig(
            hf_model_id=Path(local_dir).name,
            variant="macOS",
            max_context_length=max_context_length,
            compute_precision=compute_precision,
            compression=compression,
            output_dir=output_dir,
            output_name=output_name,
            overwrite=overwrite,
            compression_config_object=None,
        )

        logger.info("Exporting to Core AI (this may take a while)...")
        coreai_program = export_macos_model(model, config, export_cfg)
        del model
        gc.collect()

        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)
        bundle_name = output_name or f"{Path(local_dir).name}_coreai"
        bundle_path = output_path / bundle_name

        if bundle_path.exists():
            if overwrite:
                shutil.rmtree(bundle_path)
            else:
                raise FileExistsError(f"{bundle_path} already exists. Use --overwrite to replace.")
        bundle_path.mkdir(parents=True, exist_ok=True)

        aimodel_path = bundle_path / f"{bundle_name}.aimodel"
        logger.info(f"Saving .aimodel to {aimodel_path}...")
        metadata = build_aimodel_metadata(export_cfg.hf_model_id)
        coreai_program.save_asset(aimodel_path, metadata)

        # Write tokenizer from the local checkpoint directory.
        tokenizer_dest = bundle_path / "tokenizer"
        logger.info(f"Saving tokenizer from {local_dir}...")
        tokenizer = AutoTokenizer.from_pretrained(local_dir, trust_remote_code=True)
        tokenizer.save_pretrained(str(tokenizer_dest))

        # Write metadata.json (0.2 schema).
        metadata_json: dict[str, Any] = {
            "metadata_version": "0.2",
            "kind": "llm",
            "name": bundle_name,
            "assets": {"main": f"{bundle_name}.aimodel"},
            "language": {
                "tokenizer": export_cfg.hf_model_id,
                "vocab_size": getattr(config, "vocab_size", None),
                "max_context_length": getattr(config, "max_position_embeddings", None),
                "embedded_tokenizer": True,
                "function_map": {"main": ["main"]},
            },
            "source": {
                "model_definition": "torch",
                "hf_model_id": export_cfg.hf_model_id,
            },
            "compression": compression if compression != "none" else None,
            "compilation": {
                "date": datetime.now().astimezone().isoformat(),
                "targets": [],
            },
        }
        metadata_path = bundle_path / "metadata.json"
        with open(metadata_path, "w") as f:
            json.dump(metadata_json, f, indent=2)
        logger.info(f"Wrote metadata to {metadata_path}")

    logger.info(f"Export complete: {bundle_path}")
    return str(bundle_path)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Convert a local HugmingFace Gemma 3 checkpoint to Apple Core AI .aimodel",
    )
    parser.add_argument("local_dir", help="Path to local checkpoint directory (HF format)")
    parser.add_argument("--output-dir", default=".", help="Output directory")
    parser.add_argument("--output-name", default=None, help="Output bundle name")
    parser.add_argument("--max-context-length", type=int, default=8192,
                        help="Maximum context length (default: 8192)")
    parser.add_argument("--num-layers", type=int, default=None,
                        help="Truncate to N layers (useful for debugging)")
    parser.add_argument("--compute-precision", default="bfloat16",
                        choices=["float16", "bfloat16", "float32"],
                        help="Compute precision (default: bfloat16)")
    parser.add_argument("--compression", default="none",
                        help="Compression preset or 'none' (default: none)")
    parser.add_argument("--overwrite", action="store_true", help="Overwrite existing output")
    parser.add_argument("--verbose", "-v", action="store_true", help="Enable debug logging")
    args = parser.parse_args()

    logging.basicConfig(
        level=logging.DEBUG if args.verbose else logging.INFO,
        format="%(levelname)s: %(message)s",
    )

    export_local_gemma3(
        local_dir=args.local_dir,
        output_dir=args.output_dir,
        output_name=args.output_name,
        max_context_length=args.max_context_length,
        compute_precision=args.compute_precision,
        compression=args.compression,
        overwrite=args.overwrite,
        num_layers=args.num_layers,
    )


if __name__ == "__main__":
    main()
