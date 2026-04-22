"""
完整版本：实现文档所有要求
主要功能：
1. 值域归一化（修复图片变黑）
2. 色彩对齐系统（Luminance-aware Mapping + Tone Curve）
3. RAW 融合策略（保持动态范围、高光细节、阴影层次）
4. 调试日志和安全检查
5. IP-Adapter 默认配置
"""
import hashlib
import inspect
import os
import tempfile
import threading
import asyncio
import time
import traceback
import uuid
import gc
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
from queue import Queue

import numpy as np
import tifffile
from fastapi import FastAPI, HTTPException
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field
from PIL import Image

# 🆕 导入色彩对齐和 RAW 融合模块
try:
    from color_alignment import apply_color_alignment
    HAS_COLOR_ALIGNMENT = True
except ImportError:
    HAS_COLOR_ALIGNMENT = False
    print("[WARNING] color_alignment module not found, color alignment features disabled")

try:
    from raw_fusion import apply_raw_fusion, load_raw_image
    HAS_RAW_FUSION = True
except ImportError:
    HAS_RAW_FUSION = False
    print("[WARNING] raw_fusion module not found, RAW fusion features disabled")

SERVICE_VERSION = "0.2.0-complete"

DEFAULT_BASE_MODEL = os.environ.get("QRAW_SDXL_BASE_MODEL", "stabilityai/stable-diffusion-xl-base-1.0")
DEFAULT_REFINER_MODEL = os.environ.get("QRAW_SDXL_REFINER_MODEL", "stabilityai/stable-diffusion-xl-refiner-1.0")
DEFAULT_CONTROLNET_MODEL = os.environ.get("QRAW_CONTROLNET_MODEL", "diffusers/controlnet-canny-sdxl-1.0")
# 🔧 修复：设置 IP-Adapter 默认值
DEFAULT_IP_ADAPTER_MODEL = os.environ.get("QRAW_IP_ADAPTER_MODEL", "h94/IP-Adapter")
DEFAULT_IP_ADAPTER_WEIGHT = os.environ.get("QRAW_IP_ADAPTER_WEIGHT", "ip-adapter-plus_sdxl_vit-h.safetensors")

DEFAULT_OUTPUT_DIR = Path(os.environ.get("QRAW_STYLE_TRANSFER_OUTPUT_DIR", Path(tempfile.gettempdir()) / "qraw-style-transfer"))
DEFAULT_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

# 🔧 添加：调试模式
DEBUG_MODE = os.environ.get("QRAW_DEBUG", "0") == "1"

# 🆕 添加：进度队列管理
_progress_queues: Dict[str, Queue] = {}
_progress_lock = threading.Lock()
_cancelled_tasks: set[str] = set()
_inference_lock = threading.Lock()


def _debug_log(msg: str) -> None:
    """调试日志输出"""
    if DEBUG_MODE:
        print(f"[DEBUG] {msg}")


def _create_progress_queue(task_id: str) -> Queue:
    """创建进度队列"""
    with _progress_lock:
        queue = Queue()
        _progress_queues[task_id] = queue
        return queue


def _get_or_create_progress_queue(task_id: str) -> Queue:
    with _progress_lock:
        queue = _progress_queues.get(task_id)
        if queue is None:
            queue = Queue()
            _progress_queues[task_id] = queue
        return queue


def _get_progress_queue(task_id: str) -> Optional[Queue]:
    """获取进度队列"""
    with _progress_lock:
        return _progress_queues.get(task_id)


def _remove_progress_queue(task_id: str) -> None:
    """移除进度队列"""
    with _progress_lock:
        _progress_queues.pop(task_id, None)


def _mark_task_cancelled(task_id: str) -> None:
    with _progress_lock:
        _cancelled_tasks.add(task_id)


def _clear_task_cancelled(task_id: str) -> None:
    with _progress_lock:
        _cancelled_tasks.discard(task_id)


def _is_task_cancelled(task_id: Optional[str]) -> bool:
    if not task_id:
        return False
    with _progress_lock:
        return task_id in _cancelled_tasks


def _raise_if_cancelled(task_id: Optional[str]) -> None:
    if _is_task_cancelled(task_id):
        raise RuntimeError("cancelled")


def _emit_progress(task_id: str, percentage: int, description: str) -> None:
    """
    发送进度到队列
    """
    queue = _get_progress_queue(task_id)
    if queue:
        # 计算进度条
        bar_length = 50
        filled_length = int(bar_length * percentage / 100)
        bar = '=' * filled_length + ' ' * (bar_length - filled_length)
        
        progress_data = {
            "percentage": percentage,
            "description": description,
            "bar": bar,
            "message": f"[PROGRESS] 迁移进度：{bar} ({percentage}%) - {description}"
        }
        queue.put(progress_data)
    
    # 同时输出到控制台（用于调试）
    bar_length = 50
    filled_length = int(bar_length * percentage / 100)
    bar = '=' * filled_length + ' ' * (bar_length - filled_length)
    print(f"[PROGRESS] 迁移进度：{bar} ({percentage}%) - {description}")


def _print_progress(percentage: int, description: str, task_id: Optional[str] = None) -> None:
    """
    打印带进度条的进度信息（兼容旧代码）
    """
    if task_id:
        _emit_progress(task_id, percentage, description)
    else:
        # 如果没有 task_id，只输出到控制台
        bar_length = 50
        filled_length = int(bar_length * percentage / 100)
        bar = '=' * filled_length + ' ' * (bar_length - filled_length)
        print(f"[PROGRESS] 迁移进度：{bar} ({percentage}%) - {description}")


def _map_progress(local_percentage: int, start: int, end: int) -> int:
    safe_local = max(0, min(100, int(local_percentage)))
    safe_start = max(0, min(100, int(start)))
    safe_end = max(safe_start, min(100, int(end)))
    return int(round(safe_start + (safe_end - safe_start) * (safe_local / 100.0)))


class StyleTransferRequest(BaseModel):
    reference_image_path: str = Field(alias="referenceImagePath")
    content_image_path: str = Field(alias="contentImagePath")
    current_adjustments: Dict[str, Any] = Field(default_factory=dict, alias="currentAdjustments")
    stage: str = Field("preview")
    preset: str
    enable_refiner: bool = Field(False, alias="enableRefiner")
    tile_size: int = Field(1024, alias="tileSize")
    tile_overlap: int = Field(96, alias="tileOverlap")
    allow_tiling: bool = Field(False, alias="allowTiling")
    preview_max_side: Optional[int] = Field(1024, alias="previewMaxSide")
    controlnet_strength: float = Field(0.6, alias="controlnetStrength")
    controlnet_guidance_end: float = Field(0.8, alias="controlnetGuidanceEnd")
    denoise_strength: float = Field(0.55, alias="denoiseStrength")
    steps: int = 35
    cfg_scale: float = Field(6.0, alias="cfgScale")
    export_format: str = Field("tiff", alias="exportFormat")
    preserve_raw_tone_curve: bool = Field(True, alias="preserveRawToneCurve")
    task_id: Optional[str] = Field(None, alias="taskId")  # 🆕 添加任务ID用于进度跟踪
    # 🆕 色彩对齐参数
    enable_color_alignment: bool = Field(False, alias="enableColorAlignment")
    color_alignment_mode: str = Field("full", alias="colorAlignmentMode")  # full, luminance_only, tone_only, none
    luminance_strength: float = Field(0.3, alias="luminanceStrength")
    tone_curve_strength: float = Field(0.5, alias="toneCurveStrength")
    dynamic_range_preserve: float = Field(0.3, alias="dynamicRangePreserve")
    # 🆕 RAW 融合参数
    enable_raw_fusion: bool = Field(False, alias="enableRawFusion")
    raw_blend_strength: float = Field(0.5, alias="rawBlendStrength")
    raw_blend_mode: str = Field("luminance", alias="rawBlendMode")  # luminance, color, full, adaptive
    preserve_highlights: bool = Field(True, alias="preserveHighlights")
    preserve_shadows: bool = Field(True, alias="preserveShadows")

    class Config:
        populate_by_name = True


class StyleTransferResponse(BaseModel):
    status: str
    message: Optional[str] = None
    output_image_path: Optional[str] = Field(None, alias="outputImagePath")
    preview_image_path: Optional[str] = Field(None, alias="previewImagePath")
    pure_generation_image_path: Optional[str] = Field(None, alias="pureGenerationImagePath")
    post_processed_image_path: Optional[str] = Field(None, alias="postProcessedImagePath")
    used_fallback: bool = Field(False, alias="usedFallback")

    class Config:
        populate_by_name = True


class HealthResponse(BaseModel):
    status: str
    ready: bool
    version: str
    pipeline: str
    capabilities: List[str]
    detail: Optional[str] = None


app = FastAPI()

_pipe = None
_refiner = None
_pipeline_error: Optional[str] = None
_deps_error: Optional[str] = None
_loading = False
_ip_adapter_loaded = False
_runtime_device = "uninitialized"
_preferred_device_override = os.environ.get("QRAW_STYLE_TRANSFER_DEVICE")


def _resolve_pretrained_source(model_ref: str) -> Tuple[str, bool]:
    if not model_ref:
        return model_ref, False

    candidate = Path(model_ref).expanduser()
    if candidate.exists():
        return str(candidate), True

    if "/" not in model_ref:
        return model_ref, False

    org, name = model_ref.split("/", 1)
    cache_root = Path.home() / ".cache" / "huggingface" / "hub" / f"models--{org}--{name}"
    snapshots_dir = cache_root / "snapshots"
    if not snapshots_dir.exists():
        return model_ref, False

    snapshots = sorted((p for p in snapshots_dir.iterdir() if p.is_dir()), key=lambda p: p.name, reverse=True)
    if not snapshots:
        return model_ref, False

    resolved = str(snapshots[0])
    _debug_log(f"Using cached model snapshot for {model_ref}: {resolved}")
    return resolved, True


def _resolve_model_variant(model_source: str) -> Optional[str]:
    candidate = Path(model_source).expanduser()
    if not candidate.exists() or not candidate.is_dir():
        return None

    if list(candidate.glob("**/*.fp16.safetensors")) or list(candidate.glob("**/*.fp16.bin")):
        return "fp16"
    return None


def _runtime_prefers_cpu() -> bool:
    return (_preferred_device_override or "").strip().lower() == "cpu"


def _is_mps_contiguous_runtime_error(exc: Exception) -> bool:
    return "view size is not compatible with input tensor's size and stride" in str(exc)


def _switch_runtime_device(device_name: str) -> None:
    global _pipe, _refiner, _pipeline_error, _loading, _ip_adapter_loaded
    global _runtime_device, _preferred_device_override

    _debug_log(f"Switching style transfer runtime to {device_name}")
    _preferred_device_override = device_name

    old_pipe = _pipe
    old_refiner = _refiner
    _pipe = None
    _refiner = None
    _pipeline_error = None
    _loading = False
    _ip_adapter_loaded = False
    _runtime_device = "uninitialized"

    try:
        del old_pipe
        del old_refiner
    except Exception:
        pass

    gc.collect()

    try:
        import torch

        if hasattr(torch, "mps") and hasattr(torch.mps, "empty_cache"):
            torch.mps.empty_cache()
    except Exception:
        pass

    _load_pipelines()

    if _pipeline_error is not None:
        raise RuntimeError(_pipeline_error)
    if _pipe is None:
        raise RuntimeError(f"runtime_reload_failed:{device_name}")
    if not _ip_adapter_loaded:
        raise RuntimeError("ip_adapter_required")


def _reset_pipeline_runtime_state() -> None:
    global _pipe, _refiner

    for pipeline_name, pipeline in (("base", _pipe), ("refiner", _refiner)):
        if pipeline is None:
            continue

        scheduler = getattr(pipeline, "scheduler", None)
        if scheduler is None:
            continue

        try:
            pipeline.scheduler = scheduler.__class__.from_config(scheduler.config)
            _debug_log(f"Reset {pipeline_name} scheduler runtime state")
        except Exception as exc:
            print(f"[WARNING] Failed to reset {pipeline_name} scheduler state: {exc}")


def _hash_request(req: StyleTransferRequest) -> str:
    def stat_sig(p: str) -> str:
        try:
            st = os.stat(p)
            return f"{st.st_size}:{int(st.st_mtime)}"
        except Exception:
            return "missing"

    h = hashlib.sha256()
    h.update(req.reference_image_path.encode("utf-8"))
    h.update(req.content_image_path.encode("utf-8"))
    h.update(stat_sig(req.reference_image_path).encode("utf-8"))
    h.update(stat_sig(req.content_image_path).encode("utf-8"))
    h.update(req.preset.encode("utf-8"))
    h.update(str(req.enable_refiner).encode("utf-8"))
    h.update(str(req.tile_size).encode("utf-8"))
    h.update(str(req.tile_overlap).encode("utf-8"))
    h.update(str(req.controlnet_strength).encode("utf-8"))
    h.update(str(req.controlnet_guidance_end).encode("utf-8"))
    h.update(str(req.denoise_strength).encode("utf-8"))
    h.update(str(req.steps).encode("utf-8"))
    h.update(str(req.cfg_scale).encode("utf-8"))
    h.update(req.export_format.encode("utf-8"))
    h.update(str(req.preserve_raw_tone_curve).encode("utf-8"))
    return h.hexdigest()


def _file_signature(path: str) -> str:
    try:
        st = os.stat(path)
        return f"{st.st_size}:{int(st.st_mtime)}"
    except Exception:
        return "missing"


def _derive_stable_seed(req: StyleTransferRequest, salt: str = "base") -> int:
    """
    Preview/export must share the same style trajectory.
    Derive a stable seed from request identity, excluding stage-specific toggles.
    """
    h = hashlib.sha256()
    h.update(req.reference_image_path.encode("utf-8"))
    h.update(req.content_image_path.encode("utf-8"))
    h.update(_file_signature(req.reference_image_path).encode("utf-8"))
    h.update(_file_signature(req.content_image_path).encode("utf-8"))
    h.update(req.preset.encode("utf-8"))
    h.update(salt.encode("utf-8"))
    return int.from_bytes(h.digest()[:8], byteorder="big", signed=False) & 0x7FFFFFFF


def _prepare_reference_array(original_img: Image.Image, shape: Tuple[int, int]) -> np.ndarray:
    target_h, target_w = shape
    reference = _ensure_rgb(original_img)
    if reference.size != (target_w, target_h):
        reference = reference.resize((target_w, target_h), Image.Resampling.LANCZOS)
    return np.array(reference).astype(np.float32)


def _apply_consistency_postprocess(
    arr: np.ndarray,
    style_reference_img: Image.Image,
    content_reference_img: Image.Image,
    req: StyleTransferRequest,
    task_id: Optional[str],
    progress_pct: int,
    progress_label: str,
) -> np.ndarray:
    result = arr
    style_reference_arr = _prepare_reference_array(
        style_reference_img, (result.shape[0], result.shape[1])
    )
    content_reference_arr = _prepare_reference_array(
        content_reference_img, (result.shape[0], result.shape[1])
    )

    if req.enable_color_alignment and HAS_COLOR_ALIGNMENT:
        try:
            _print_progress(progress_pct, f"{progress_label}色彩对齐...", task_id)
            result = apply_color_alignment(
                result,
                style_reference_arr,
                mode=req.color_alignment_mode,
                luminance_strength=float(req.luminance_strength),
                tone_curve_strength=float(req.tone_curve_strength),
                dynamic_range_preserve=float(req.dynamic_range_preserve),
            )
            result = _normalize_array_to_255(result, "color_alignment")
        except Exception as exc:
            print(f"[WARNING] Color alignment skipped due to error: {exc}")

    if req.enable_raw_fusion and req.preserve_raw_tone_curve and HAS_RAW_FUSION:
        try:
            _print_progress(progress_pct, f"{progress_label}RAW 融合...", task_id)
            result = apply_raw_fusion(
                result,
                raw_processed=content_reference_arr,
                current_adjustments=req.current_adjustments,
                blend_strength=float(req.raw_blend_strength),
                blend_mode=req.raw_blend_mode,
                preserve_highlights=bool(req.preserve_highlights),
                preserve_shadows=bool(req.preserve_shadows),
            )
            result = _normalize_array_to_255(result, "raw_fusion")
        except Exception as exc:
            print(f"[WARNING] RAW fusion skipped due to error: {exc}")

    return result


def _load_image(path: str) -> Image.Image:
    try:
        return Image.open(path)
    except Exception as e:
        try:
            import rawpy
            with rawpy.imread(path) as raw:
                rgb = raw.postprocess(use_camera_wb=True, half_size=True, no_auto_bright=True, output_bps=8)
            return Image.fromarray(rgb)
        except ImportError:
            raise Exception(f"{e} (rawpy not installed for RAW support)")
        except Exception as raw_e:
            raise Exception(f"{e} and {raw_e}")


def _ensure_rgb(img: Image.Image) -> Image.Image:
    if img.mode != "RGB":
        return img.convert("RGB")
    return img


def _resize_for_max_side(img: Image.Image, max_side: Optional[int]) -> Image.Image:
    if not max_side or max_side <= 0:
        return img
    img = _ensure_rgb(img)
    w, h = img.size
    longest = max(w, h)
    if longest <= max_side:
        return img
    scale = float(max_side) / float(longest)
    target = (max(1, int(round(w * scale))), max(1, int(round(h * scale))))
    _debug_log(f"Resizing image from {w}x{h} to {target[0]}x{target[1]} for stage budget")
    return img.resize(target, Image.Resampling.LANCZOS)


def _resolve_export_extension(export_format: str) -> str:
    normalized = export_format.strip().lower()
    if normalized in {"jpg", "jpeg"}:
        return "jpg"
    if normalized in {"png", "tiff", "tif"}:
        return "png" if normalized == "png" else "tiff"
    raise HTTPException(status_code=400, detail=f"unsupported_export_format:{export_format}")


def _resolve_output_paths(content_path: Path, stage: str, export_format: str, task_id: str) -> Tuple[Optional[Path], Path]:
    content_stem = content_path.stem
    preview_dir = DEFAULT_OUTPUT_DIR / "preview"
    preview_dir.mkdir(parents=True, exist_ok=True)
    preview_path = preview_dir / f"{content_stem}_{task_id}_preview.png"

    if stage == "preview":
        return None, preview_path

    export_dir = DEFAULT_OUTPUT_DIR / "export"
    export_dir.mkdir(parents=True, exist_ok=True)
    extension = _resolve_export_extension(export_format)
    output_path = export_dir / f"{content_stem}_{task_id}.{extension}"
    return output_path, preview_path


def _resolve_variant_preview_path(content_path: Path, task_id: str, variant: str) -> Path:
    content_stem = content_path.stem
    preview_dir = DEFAULT_OUTPUT_DIR / "preview"
    preview_dir.mkdir(parents=True, exist_ok=True)
    return preview_dir / f"{content_stem}_{task_id}_{variant}.png"


def _save_output_image(img: np.ndarray, out_path: Path, export_format: str) -> None:
    normalized = export_format.strip().lower()
    if normalized in {"tiff", "tif"}:
        _save_rgb16_tiff(img, out_path)
        return

    clipped = _normalize_array_to_255(img, f"save_output_{normalized}")
    pil = Image.fromarray(np.clip(np.round(clipped), 0, 255).astype(np.uint8), mode="RGB")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    if normalized == "png":
        pil.save(str(out_path), format="PNG", optimize=True)
        return
    if normalized in {"jpg", "jpeg"}:
        pil.save(str(out_path), format="JPEG", quality=95, subsampling=0, optimize=True)
        return
    raise RuntimeError(f"unsupported_export_format:{export_format}")


def _validate_quality_guard(arr: np.ndarray, stage: str) -> None:
    arr = _normalize_array_to_255(arr, f"quality_guard_{stage}")
    black_mask = np.all(arr <= 4.0, axis=2)
    near_black_mask = np.all(arr <= 8.0, axis=2)
    black_ratio = float(black_mask.mean())
    near_black_ratio = float(near_black_mask.mean())
    std_val = float(arr.std())
    p95 = float(np.percentile(arr, 95))
    p99 = float(np.percentile(arr, 99))
    channel_delta = float(
        np.abs(arr[:, :, 0] - arr[:, :, 1]).mean()
        + np.abs(arr[:, :, 1] - arr[:, :, 2]).mean()
    )

    if black_ratio > 0.85:
        raise RuntimeError(f"quality_guard_failed:black_area:black_ratio={black_ratio:.3f}")
    if near_black_ratio > 0.98 and p95 < 16.0:
        raise RuntimeError(
            f"quality_guard_failed:black_frame:near_black_ratio={near_black_ratio:.3f}:p95={p95:.2f}"
        )
    if p99 < 12.0 and std_val < 6.0:
        raise RuntimeError(f"quality_guard_failed:black_frame:p99={p99:.2f}:std={std_val:.2f}")
    if channel_delta < 1.5:
        raise RuntimeError(f"quality_guard_failed:grayscale:channel_delta={channel_delta:.3f}")


def _normalize_array_to_255(arr: np.ndarray, source: str = "unknown") -> np.ndarray:
    """
    🔧 修复：统一将数组归一化到 [0, 255] float32 范围
    
    Args:
        arr: 输入数组
        source: 数据来源（用于调试）
    
    Returns:
        [0, 255] 范围的 float32 数组
    """
    _debug_log(f"Normalizing array from {source}")
    _debug_log(f"  Input dtype: {arr.dtype}, shape: {arr.shape}")
    _debug_log(f"  Input range: [{arr.min():.4f}, {arr.max():.4f}], mean: {arr.mean():.4f}")
    
    # 转换为 float32
    if arr.dtype != np.float32:
        arr = arr.astype(np.float32)
    
    # 检测值域并归一化
    arr_min, arr_max = arr.min(), arr.max()
    
    if arr_max <= 1.0 and arr_min >= 0.0:
        # [0, 1] 范围
        _debug_log(f"  Detected [0, 1] range, scaling to [0, 255]")
        arr = arr * 255.0
    elif arr_min < 0.0 and arr_max <= 1.0:
        # [-1, 1] 范围（某些模型输出）
        _debug_log(f"  Detected [-1, 1] range, scaling to [0, 255]")
        arr = (arr + 1.0) * 127.5
    elif arr_max > 255.0:
        # 超出范围，归一化
        _debug_log(f"  Detected out-of-range values, normalizing")
        arr = (arr - arr_min) / (arr_max - arr_min + 1e-8) * 255.0
    elif arr_max <= 255.0 and arr_min >= 0.0:
        # 已经是 [0, 255] 范围
        _debug_log(f"  Already in [0, 255] range")
        pass
    else:
        # 未知范围，强制归一化
        _debug_log(f"  Unknown range, force normalizing")
        arr = (arr - arr_min) / (arr_max - arr_min + 1e-8) * 255.0
    
    # 最终裁剪
    arr = np.clip(arr, 0.0, 255.0)
    
    _debug_log(f"  Output range: [{arr.min():.4f}, {arr.max():.4f}], mean: {arr.mean():.4f}")
    
    # ⚠️ 安全检查：检测异常暗的图像
    if arr.mean() < 10.0:
        print(f"[WARNING] {source} 输出异常暗！平均值: {arr.mean():.2f} / 255.0")
        print(f"[WARNING] 这可能导致最终图像变黑，请检查 Pipeline 输出")
    
    return arr


def _make_weight_1d(length: int, overlap: int, at_start: bool, at_end: bool) -> np.ndarray:
    w = np.ones((length,), dtype=np.float32)
    if overlap <= 0:
        return w
    o = min(overlap, length // 2)
    # 🔧 修复：确保 o > 0，避免空数组
    if o <= 0:
        return w
    ramp = (1.0 - np.cos(np.linspace(0.0, np.pi, o, dtype=np.float32))) * 0.5
    if not at_start and o > 0:
        w[:o] = ramp
    if not at_end and o > 0:
        w[-o:] = ramp[::-1]
    return w


def _blend_tiles(
    tiles: List[Tuple[int, int, np.ndarray]],
    out_w: int,
    out_h: int,
    tile_size: int,
    overlap: int,
) -> np.ndarray:
    """
    🔧 修复：确保 tile 融合时值域一致
    """
    acc = np.zeros((out_h, out_w, 3), dtype=np.float32)
    wacc = np.zeros((out_h, out_w, 1), dtype=np.float32)
    
    for idx, (x0, y0, tile) in enumerate(tiles):
        # 🔧 修复：归一化每个 tile
        tile = _normalize_array_to_255(tile, f"tile_{idx}")
        
        th, tw = tile.shape[0], tile.shape[1]
        wx = _make_weight_1d(tw, overlap, at_start=(x0 == 0), at_end=(x0 + tw >= out_w))
        wy = _make_weight_1d(th, overlap, at_start=(y0 == 0), at_end=(y0 + th >= out_h))
        w = (wy[:, None] * wx[None, :]).astype(np.float32)[:, :, None]
        
        region = acc[y0 : y0 + th, x0 : x0 + tw, :]
        wregion = wacc[y0 : y0 + th, x0 : x0 + tw, :]
        region += tile * w
        wregion += w
    
    wacc = np.maximum(wacc, 1e-6)
    result = acc / wacc
    
    # 🔧 修复：最终归一化
    result = _normalize_array_to_255(result, "blended_tiles")
    
    return result


def _save_rgb16_tiff(img: np.ndarray, out_path: Path) -> None:
    """
    🔧 修复：安全的 16-bit TIFF 保存
    """
    out_path.parent.mkdir(parents=True, exist_ok=True)
    
    # 🔧 修复：确保输入是 [0, 255] 范围
    img = _normalize_array_to_255(img, "save_rgb16_tiff")
    
    # 转换到 16-bit
    u16 = np.clip(np.round(img / 255.0 * 65535.0), 0, 65535).astype(np.uint16)
    
    # ⚠️ 安全检查
    mean_val = u16.mean()
    if mean_val < 1000:
        print(f"[ERROR] 输出图像异常暗！平均值: {mean_val:.2f} / 65535")
        print(f"[ERROR] 这很可能是一张黑图，请检查 Pipeline")
    else:
        _debug_log(f"Output image stats: mean={mean_val:.2f}, min={u16.min()}, max={u16.max()}")
    
    tifffile.imwrite(str(out_path), u16, photometric="rgb")


def _save_preview_png(img: np.ndarray, out_path: Path, max_side: int = 1024) -> None:
    """
    🔧 修复：安全的预览图保存
    """
    out_path.parent.mkdir(parents=True, exist_ok=True)
    
    # 🔧 修复：确保输入是 [0, 255] 范围
    img = _normalize_array_to_255(img, "save_preview_png")
    
    pil = Image.fromarray(np.clip(np.round(img), 0, 255).astype(np.uint8), mode="RGB")
    w, h = pil.size
    scale = min(1.0, float(max_side) / float(max(w, h)))
    if scale < 1.0:
        pil = pil.resize((max(1, int(w * scale)), max(1, int(h * scale))), Image.Resampling.LANCZOS)
    pil.save(str(out_path), format="PNG", optimize=True)


def _has_consistency_postprocess(req: StyleTransferRequest) -> bool:
    return bool(
        (req.enable_color_alignment and HAS_COLOR_ALIGNMENT)
        or (req.enable_raw_fusion and req.preserve_raw_tone_curve and HAS_RAW_FUSION)
    )


def _load_pipelines(load_refiner: bool = False) -> None:
    global _pipe, _refiner, _pipeline_error, _ip_adapter_loaded, _runtime_device
    if _pipeline_error is not None:
        return

    should_load_base = _pipe is None
    should_load_refiner = bool(load_refiner and DEFAULT_REFINER_MODEL and _refiner is None)
    if not should_load_base and not should_load_refiner:
        return

    try:
        import torch
        from diffusers import ControlNetModel, StableDiffusionXLControlNetImg2ImgPipeline, StableDiffusionXLImg2ImgPipeline
        from transformers import CLIPVisionModelWithProjection

        has_mps = hasattr(torch.backends, "mps") and torch.backends.mps.is_available()
        preferred_device = (_preferred_device_override or "").strip().lower()
        if preferred_device == "cpu":
            torch_dtype = torch.float32
            device = torch.device("cpu")
        elif preferred_device == "cuda" and torch.cuda.is_available():
            torch_dtype = torch.float16
            device = torch.device("cuda")
        elif preferred_device == "mps" and has_mps:
            torch_dtype = torch.float16
            device = torch.device("mps")
        elif torch.cuda.is_available():
            torch_dtype = torch.float16
            device = torch.device("cuda")
        elif has_mps:
            torch_dtype = torch.float16
            device = torch.device("mps")
        else:
            torch_dtype = torch.float32
            device = torch.device("cpu")

        _debug_log(f"Loading pipelines on {device} with {torch_dtype}")
        _runtime_device = device.type

        if should_load_base:
            controlnet_source, controlnet_local_only = _resolve_pretrained_source(DEFAULT_CONTROLNET_MODEL)
            base_source, base_local_only = _resolve_pretrained_source(DEFAULT_BASE_MODEL)
            controlnet_variant = _resolve_model_variant(controlnet_source)
            base_variant = _resolve_model_variant(base_source)
            _debug_log(
                "Resolved model variants - "
                f"controlnet: {controlnet_variant or 'default'}, "
                f"base: {base_variant or 'default'}"
            )

            controlnet = ControlNetModel.from_pretrained(
                controlnet_source,
                torch_dtype=torch_dtype,
                variant=controlnet_variant,
                local_files_only=controlnet_local_only,
            )

            image_encoder = None
            if DEFAULT_IP_ADAPTER_MODEL:
                try:
                    image_encoder = CLIPVisionModelWithProjection.from_pretrained(
                        DEFAULT_IP_ADAPTER_MODEL,
                        subfolder="models/image_encoder",
                        torch_dtype=torch_dtype,
                    )
                    image_encoder = image_encoder.to(device)
                    _debug_log("IP-Adapter image encoder loaded successfully")
                except Exception as e:
                    print(f"[WARNING] Failed to load IP-Adapter image encoder: {e}")
                    print("[INFO] Service will stay unavailable until IP-Adapter image encoder is available")

            pipe = StableDiffusionXLControlNetImg2ImgPipeline.from_pretrained(
                base_source,
                controlnet=controlnet,
                image_encoder=image_encoder,
                torch_dtype=torch_dtype,
                use_safetensors=True,
                variant=base_variant,
                local_files_only=base_local_only,
            )
            pipe = pipe.to(device)

            try:
                pipe.enable_vae_tiling()
            except Exception:
                pass

            _ip_adapter_loaded = False
            if DEFAULT_IP_ADAPTER_MODEL and hasattr(pipe, "load_ip_adapter"):
                try:
                    # 🔧 使用国内镜像加速下载
                    import os
                    os.environ['HF_ENDPOINT'] = 'https://hf-mirror.com'

                    _debug_log(f"Loading IP-Adapter: {DEFAULT_IP_ADAPTER_MODEL}")
                    _debug_log(f"Using mirror: https://hf-mirror.com")

                    if DEFAULT_IP_ADAPTER_WEIGHT:
                        pipe.load_ip_adapter(
                            DEFAULT_IP_ADAPTER_MODEL,
                            subfolder="sdxl_models",
                            weight_name=DEFAULT_IP_ADAPTER_WEIGHT,
                            image_encoder_folder=None,
                        )
                    else:
                        pipe.load_ip_adapter(
                            DEFAULT_IP_ADAPTER_MODEL,
                            subfolder="sdxl_models",
                            image_encoder_folder=None,
                        )
                    if hasattr(pipe, "set_ip_adapter_scale"):
                        pipe.set_ip_adapter_scale(1.0)
                    _ip_adapter_loaded = True
                    _debug_log("IP-Adapter loaded successfully")
                except Exception as e:
                    print(f"[WARNING] Failed to load IP-Adapter: {e}")
                    traceback.print_exc()
                    print("[INFO] Service will stay unavailable until IP-Adapter is available")
                    _ip_adapter_loaded = False

            if not _ip_adapter_loaded:
                try:
                    pipe.enable_attention_slicing()
                except Exception:
                    pass
            else:
                _debug_log("Skipping attention slicing because it is incompatible with IP-Adapter tuple encoder states")

            _pipe = pipe

        if should_load_refiner:
            refiner_source, refiner_local_only = _resolve_pretrained_source(DEFAULT_REFINER_MODEL)
            refiner_variant = _resolve_model_variant(refiner_source)
            _debug_log(f"Resolved refiner variant: {refiner_variant or 'default'}")

            refiner = None
            try:
                _debug_log(f"Loading Refiner: {DEFAULT_REFINER_MODEL}")
                refiner = StableDiffusionXLImg2ImgPipeline.from_pretrained(
                    refiner_source,
                    torch_dtype=torch_dtype,
                    use_safetensors=True,
                    variant=refiner_variant,
                    local_files_only=refiner_local_only,
                )
                refiner = refiner.to(device)
                try:
                    refiner.enable_attention_slicing()
                except Exception:
                    pass
                try:
                    refiner.enable_vae_tiling()
                except Exception:
                    pass
                _debug_log("Refiner loaded successfully")
            except Exception as e:
                print(f"[WARNING] Failed to load Refiner: {e}")
                refiner = None

            _refiner = refiner
    except Exception as e:
        _pipeline_error = str(e)


def _check_deps() -> bool:
    global _deps_error
    if _deps_error is not None:
        return False
    try:
        import torch
        import diffusers

        _ = (torch, diffusers)
        return True
    except Exception as e:
        _deps_error = str(e)
        return False


def _ensure_warming_up() -> None:
    global _loading
    if _pipe is not None or _pipeline_error is not None or _loading:
        return
    _loading = True

    def _runner() -> None:
        global _loading
        try:
            _load_pipelines()
        finally:
            _loading = False

    threading.Thread(target=_runner, daemon=True).start()


def _make_control_image(img: Image.Image) -> Image.Image:
    try:
        import cv2

        arr = np.array(_ensure_rgb(img))
        edges = cv2.Canny(arr, 100, 200)
        edges = np.stack([edges] * 3, axis=-1).astype(np.uint8)
        return Image.fromarray(edges, mode="RGB")
    except Exception:
        gray = np.array(_ensure_rgb(img).convert("L"))
        gy, gx = np.gradient(gray.astype(np.float32))
        mag = np.sqrt(gx * gx + gy * gy)
        mag = (mag / (mag.max() + 1e-6) * 255.0).astype(np.uint8)
        edges = np.stack([mag] * 3, axis=-1)
        return Image.fromarray(edges, mode="RGB")


def _build_generation_prompt(req: StyleTransferRequest) -> str:
    preset = (req.preset or "").strip().lower()
    prompt_map = {
        "creative": (
            "high quality stylized photo transformation, transfer the reference image style, "
            "preserve scene geometry and subject identity, match reference mood, palette, contrast, "
            "and lighting character while keeping important boundaries stable"
        ),
        "artistic": (
            "high quality photo style transfer, follow the reference image color language and tonal mood, "
            "preserve composition and subject identity, restyle lighting, palette, contrast, and atmosphere"
        ),
        "realistic": (
            "high quality realistic photo style transfer, preserve composition and local details, "
            "adopt the reference image palette, realistic lighting direction, tonal harmonization, and depth"
        ),
    }
    return prompt_map.get(
        preset,
        "high quality photo style transfer, preserve composition and subject identity, adopt the reference image palette and mood",
    )


def _clone_request(req: StyleTransferRequest) -> StyleTransferRequest:
    if hasattr(req, "model_copy"):
        return req.model_copy(deep=True)
    return req.copy(deep=True)


def _detect_invalid_tile(arr: np.ndarray) -> Optional[str]:
    arr = np.asarray(arr)
    if arr.size == 0:
        return "empty_tile"
    if not np.isfinite(arr).all():
        return "non_finite_tile"

    normalized = _normalize_array_to_255(arr.astype(np.float32), "tile_health_check")
    black_mask = np.all(normalized <= 4.0, axis=2)
    near_black_mask = np.all(normalized <= 8.0, axis=2)
    black_ratio = float(black_mask.mean())
    near_black_ratio = float(near_black_mask.mean())
    std_val = float(normalized.std())
    p95 = float(np.percentile(normalized, 95))
    p99 = float(np.percentile(normalized, 99))

    if black_ratio > 0.85:
        return f"black_area:black_ratio={black_ratio:.3f}"
    if near_black_ratio > 0.98 and p95 < 16.0:
        return f"black_frame:near_black_ratio={near_black_ratio:.3f}:p95={p95:.2f}"
    if p99 < 12.0 and std_val < 6.0:
        return f"black_frame:p99={p99:.2f}:std={std_val:.2f}"
    return None


def _run_single(
    reference_img: Image.Image,
    content_img: Image.Image,
    req: StyleTransferRequest,
    task_id: Optional[str] = None,
    progress_start: int = 10,
    progress_end: int = 95,
    seed: Optional[int] = None,
    allow_runtime_fallback: bool = True,
) -> np.ndarray:
    """
    🔧 修复：添加完整的值域检查和归一化
    """
    _raise_if_cancelled(task_id)
    _ensure_warming_up()
    if _pipe is None:
        if _pipeline_error is not None:
            raise RuntimeError(_pipeline_error)
        raise RuntimeError("warming_up")
    if not _ip_adapter_loaded:
        raise RuntimeError("ip_adapter_required")
    if req.enable_refiner and _refiner is None:
        _load_pipelines(load_refiner=True)

    import torch

    _reset_pipeline_runtime_state()

    pipe = _pipe
    style_img = _ensure_rgb(reference_img)
    content_img = _ensure_rgb(content_img)
    
    # 🆕 进度：准备控制图像
    emit_progress = lambda local_pct, desc: _print_progress(
        _map_progress(local_pct, progress_start, progress_end), desc, task_id
    )

    emit_progress(0, "准备控制图像...")
    _raise_if_cancelled(task_id)
    control_img = _make_control_image(content_img)
    image_width, image_height = content_img.size

    steps = max(1, int(req.steps))
    guidance_end = float(req.controlnet_guidance_end)
    guidance_end = max(0.0, min(1.0, guidance_end))
    denoise = float(req.denoise_strength)
    denoise = max(0.0, min(1.0, denoise))
    controlnet_strength = max(0.0, min(1.0, float(req.controlnet_strength)))

    if req.stage.strip().lower() == "export":
        # Export keeps scene structure, but still needs enough room to inherit the reference style.
        denoise = min(denoise, 0.32)
        controlnet_strength = max(controlnet_strength, 0.62)

    extra: Dict[str, Any] = {}
    try:
        sig = inspect.signature(pipe.__call__)
        if _ip_adapter_loaded and "ip_adapter_image" in sig.parameters:
            extra["ip_adapter_image"] = [style_img]
            _debug_log("Using IP-Adapter for style injection")
        if _ip_adapter_loaded and "ip_adapter_scale" in sig.parameters:
            extra["ip_adapter_scale"] = 1.0
        if "control_guidance_end" in sig.parameters:
            extra["control_guidance_end"] = guidance_end
        if "control_guidance_start" in sig.parameters:
            extra["control_guidance_start"] = 0.0
    except Exception:
        pass

    resolved_seed = seed if seed is not None else _derive_stable_seed(req)
    _debug_log(f"Using deterministic seed: {resolved_seed}")
    generator = torch.Generator(device=pipe.device)
    generator.manual_seed(int(resolved_seed))
    prompt = _build_generation_prompt(req)
    negative_prompt = (
        "low quality, distorted, grayscale, black frame, flat color, washed out, "
        "new objects, altered composition, geometry change, structural deformation, extra limbs, duplicated elements"
    )
    use_refiner = req.enable_refiner and _refiner is not None

    if use_refiner and getattr(pipe.device, "type", "") == "mps":
        use_refiner = False
        _debug_log("Skipping refiner on MPS export path because it can produce NaN/black tiles")
        if req.stage.strip().lower() == "export":
            emit_progress(10, "检测到 MPS 导出稳定性保护，跳过 Refiner...")

    try:
        if use_refiner:
            # 🆕 进度：运行 Base Model
            emit_progress(15, f"运行 SDXL Base Model (步数: {steps})...")
            _raise_if_cancelled(task_id)
            _debug_log("Running with Refiner")
            split = 0.8
            base_steps = max(1, int(round(steps * split)))
            out = pipe(
                prompt=prompt,
                negative_prompt=negative_prompt,
                image=content_img,
                control_image=control_img,
                height=image_height,
                width=image_width,
                strength=denoise,
                num_inference_steps=base_steps,
                guidance_scale=float(req.cfg_scale),
                controlnet_conditioning_scale=controlnet_strength,
                denoising_end=split,
                generator=generator,
                output_type="latent",
                return_dict=True,
                **extra,
            )
            latents = out.images

            # 🆕 进度：运行 Refiner
            _print_progress(70, "运行 SDXL Refiner...", task_id)
            _raise_if_cancelled(task_id)
            ref = _refiner(
                prompt=prompt,
                negative_prompt=negative_prompt,
                image=latents,
                num_inference_steps=steps,
                guidance_scale=float(req.cfg_scale),
                denoising_start=split,
                generator=generator,
            )
            image = ref.images[0]
            emit_progress(92, "Refiner 完成")
        else:
            # 🆕 进度：运行单阶段推理
            emit_progress(15, f"运行 SDXL 推理 (步数: {steps})...")
            _raise_if_cancelled(task_id)
            _debug_log("Running without Refiner")
            result = pipe(
                prompt=prompt,
                negative_prompt=negative_prompt,
                image=content_img,
                control_image=control_img,
                height=image_height,
                width=image_width,
                strength=denoise,
                num_inference_steps=steps,
                guidance_scale=float(req.cfg_scale),
                controlnet_conditioning_scale=controlnet_strength,
                generator=generator,
                output_type="pil",
                return_dict=True,
                **extra,
            )
            image = result.images[0]
            emit_progress(82, "推理完成")
    except Exception as exc:
        if (
            allow_runtime_fallback
            and getattr(pipe.device, "type", "") == "mps"
            and not _runtime_prefers_cpu()
            and _is_mps_contiguous_runtime_error(exc)
        ):
            _print_progress(
                _map_progress(18, progress_start, progress_end),
                "检测到 MPS 张量兼容性问题，切换到 CPU 重试...",
                task_id,
            )
            _switch_runtime_device("cpu")
            return _run_single(
                reference_img,
                content_img,
                req,
                task_id,
                progress_start,
                progress_end,
                seed=resolved_seed,
                allow_runtime_fallback=False,
            )
        traceback.print_exc()
        raise

    # 🔧 修复：确保输出是 PIL Image
    if not isinstance(image, Image.Image):
        _debug_log(f"Pipeline output is not PIL Image, type: {type(image)}")
        if hasattr(image, "cpu"):
            image = image.cpu().numpy()
        image = Image.fromarray(np.clip(image * 255, 0, 255).astype(np.uint8))
    
    _raise_if_cancelled(task_id)
    image = _ensure_rgb(image)
    arr = np.array(image)
    
    # 🔧 修复：归一化到 [0, 255]
    arr = _normalize_array_to_255(arr, "pipeline_output")
    
    return arr


def _run_tiled(
    reference_img: Image.Image,
    content_img: Image.Image,
    req: StyleTransferRequest,
    task_id: Optional[str] = None,
) -> Tuple[np.ndarray, List[str]]:
    _raise_if_cancelled(task_id)
    content_img = _ensure_rgb(content_img)
    w, h = content_img.size
    tile = max(256, int(req.tile_size))
    overlap = max(0, int(req.tile_overlap))
    if w <= tile and h <= tile:
        return _run_single(reference_img, content_img, req, task_id), []

    _debug_log(f"Running tiled processing: {w}x{h}, tile={tile}, overlap={overlap}")
    
    # 🆕 进度：计算总 tile 数量
    stride = max(64, tile - overlap)
    total_tiles = 0
    y0_temp = 0
    while y0_temp < h:
        y1_temp = min(h, y0_temp + tile)
        x0_temp = 0
        while x0_temp < w:
            x1_temp = min(w, x0_temp + tile)
            total_tiles += 1
            if x1_temp >= w:
                break
            x0_temp += stride
        if y1_temp >= h:
            break
        y0_temp += stride
    
    print(f"[PROGRESS] 开始分块处理：图像 {w}x{h}，分为 {total_tiles} 个块...")
    
    tiles: List[Tuple[int, int, np.ndarray]] = []
    fallback_tiles: List[str] = []
    tile_count = 0
    
    # 🔧 修复：确保循环正确处理边界
    y0 = 0
    while y0 < h:
        _raise_if_cancelled(task_id)
        y1 = min(h, y0 + tile)
        cy0 = max(0, y1 - tile)
        actual_h = y1 - cy0
        
        x0 = 0
        while x0 < w:
            _raise_if_cancelled(task_id)
            x1 = min(w, x0 + tile)
            cx0 = max(0, x1 - tile)
            actual_w = x1 - cx0
            
            # 确保 crop 的尺寸正确
            crop = content_img.crop((cx0, cy0, x1, y1))
            
            # 🆕 进度：显示当前处理的 tile（带进度条）
            tile_progress_start = 12 + int((tile_count / total_tiles) * 76)
            tile_progress_end = 12 + int(((tile_count + 1) / total_tiles) * 76)
            _print_progress(
                tile_progress_start,
                f"处理第 {tile_count + 1}/{total_tiles} 块 (位置: {cx0},{cy0})",
                task_id,
            )
            _debug_log(f"Processing tile {tile_count}: ({cx0}, {cy0}) -> ({x1}, {y1}), size: {actual_w}x{actual_h}")
            
            try:
                tile_seed = _derive_stable_seed(req, f"tile:{cx0}:{cy0}:{actual_w}:{actual_h}")
                out = _run_single(
                    reference_img,
                    crop,
                    req,
                    task_id,
                    progress_start=tile_progress_start,
                    progress_end=max(tile_progress_start, tile_progress_end),
                    seed=tile_seed,
                )
                tile_issue = _detect_invalid_tile(out)
                if tile_issue and req.enable_refiner:
                    _debug_log(
                        f"Tile {tile_count} at ({cx0}, {cy0}) failed health check after refiner path: {tile_issue}. Retrying without refiner."
                    )
                    fallback_req = _clone_request(req)
                    fallback_req.enable_refiner = False
                    fallback_req.denoise_strength = min(float(fallback_req.denoise_strength), 0.12)
                    fallback_req.controlnet_strength = max(float(fallback_req.controlnet_strength), 0.9)
                    out = _run_single(
                        reference_img,
                        crop,
                        fallback_req,
                        task_id,
                        progress_start=tile_progress_start,
                        progress_end=max(tile_progress_start, tile_progress_end),
                        seed=tile_seed,
                    )
                    tile_issue = _detect_invalid_tile(out)
                if tile_issue:
                    _print_progress(89, f"检测到分块健康检查失败，已中止导出: ({cx0}, {cy0})", task_id)
                    raise RuntimeError(f"tile_failed_health:{cx0}:{cy0}:{tile_issue}")
                tiles.append((cx0, cy0, out))
                tile_count += 1
            except Exception as e:
                print(f"[ERROR] Failed to process tile {tile_count} at ({cx0}, {cy0}): {e}")
                raise RuntimeError(f"tile_failed:{cx0}:{cy0}:{e}") from e
            
            # 移动到下一个 x 位置
            if x1 >= w:
                break
            x0 += stride
        
        # 移动到下一个 y 位置
        if y1 >= h:
            break
        y0 += stride
    
    # 🆕 进度：融合 tiles
    _print_progress(90, f"融合 {tile_count} 个块...", task_id)
    _raise_if_cancelled(task_id)
    _debug_log(f"Blending {tile_count} tiles")
    if fallback_tiles:
        _print_progress(89, f"检测到 {len(fallback_tiles)} 个分块触发安全保底...", task_id)
    blended = _blend_tiles(tiles, w, h, tile, overlap)
    return blended, fallback_tiles


@app.get("/health", response_model=HealthResponse)
def health() -> HealthResponse:
    deps_ready = _check_deps()
    if not deps_ready:
        return HealthResponse(
            status="missing_deps",
            ready=False,
            version=SERVICE_VERSION,
            pipeline="sdxl+ip-adapter+controlnet+tiled-vae",
            capabilities=[],
            detail=_deps_error,
        )

    _ensure_warming_up()
    if _pipe is not None:
        if not _ip_adapter_loaded:
            return HealthResponse(
                status="missing_ip_adapter",
                ready=False,
                version=SERVICE_VERSION,
                pipeline="sdxl+ip-adapter+controlnet",
                capabilities=["sdxl", "controlnet", "preview", "export", "quality_guard"],
                detail="ip_adapter_required",
            )
        capabilities = [
            "sdxl",
            *(("ip_adapter",) if _ip_adapter_loaded else ()),
            "controlnet",
            "preview",
            "export",
            "quality_guard",
            "fp16",
        ]

        return HealthResponse(
            status="ok",
            ready=True,
            version=SERVICE_VERSION,
            pipeline="sdxl+ip-adapter+controlnet",
            capabilities=capabilities,
        )

    if _pipeline_error is not None:
        return HealthResponse(
            status="error",
            ready=False,
            version=SERVICE_VERSION,
            pipeline="sdxl+ip-adapter+controlnet+tiled-vae",
            capabilities=[],
            detail=_pipeline_error,
        )

    return HealthResponse(
        status="warming_up",
        ready=False,
        version=SERVICE_VERSION,
        pipeline="sdxl+ip-adapter+controlnet",
        capabilities=[
            "sdxl",
            *(("ip_adapter",) if _ip_adapter_loaded else ()),
            "controlnet",
            "preview",
            "export",
            "quality_guard",
            "fp16",
        ],
        detail="loading_models",
    )


@app.on_event("startup")
def startup_event() -> None:
    if _check_deps():
        _ensure_warming_up()


@app.post("/v1/style-transfer", response_model=StyleTransferResponse)
def style_transfer(req: StyleTransferRequest) -> StyleTransferResponse:
    task_id = req.task_id or str(uuid.uuid4())  # 🆕 生成或使用提供的任务ID
    _clear_task_cancelled(task_id)
    _get_or_create_progress_queue(task_id)

    try:
        _print_progress(0, "开始风格迁移...", task_id)
        _debug_log("--- [DEBUG] STARTING STYLE TRANSFER ---")
        _debug_log(f"Task ID: {task_id}")

        ref_path = Path(req.reference_image_path)
        content_path = Path(req.content_image_path)
        if not ref_path.exists():
            raise HTTPException(status_code=400, detail="reference_image_not_found")
        if not content_path.exists():
            raise HTTPException(status_code=400, detail="content_image_not_found")

        stage = req.stage.strip().lower()
        if stage not in {"preview", "export"}:
            raise HTTPException(status_code=400, detail=f"invalid_stage:{req.stage}")

        if stage == "preview":
            req.enable_refiner = False
            req.allow_tiling = False
            req.preview_max_side = req.preview_max_side or 1024

        out_image, out_preview = _resolve_output_paths(content_path, stage, req.export_format, task_id)

        _print_progress(5, "加载图像...", task_id)
        try:
            reference_img = _load_image(str(ref_path))
            content_img = _load_image(str(content_path))
            reference_img = _ensure_rgb(reference_img)
            content_img = _ensure_rgb(content_img)
            if stage == "preview":
                _print_progress(8, "准备低分辨率预览输入...", task_id)
                reference_img = _resize_for_max_side(reference_img, 1024)
                content_img = _resize_for_max_side(content_img, req.preview_max_side)
            _debug_log(f"Loaded Reference Size: {reference_img.size}, Mode: {reference_img.mode}")
            _debug_log(f"Loaded Content Size: (W:{content_img.size[0]}, H:{content_img.size[1]}), Mode: {content_img.mode}")
        except Exception as e:
            raise HTTPException(status_code=400, detail=f"invalid_image: {e}")

        result_notes: List[str] = []

        try:
            _raise_if_cancelled(task_id)
            slot_acquired = _inference_lock.acquire(blocking=False)
            if not slot_acquired:
                wait_started = time.time()
                _print_progress(14, "等待推理资源...", task_id)
                while True:
                    _raise_if_cancelled(task_id)
                    if _inference_lock.acquire(blocking=False):
                        break
                    if time.time() - wait_started > 300:
                        raise RuntimeError("inference_queue_timeout")
                    time.sleep(0.5)

            try:
                _raise_if_cancelled(task_id)
                if stage == "preview":
                    _print_progress(12, "进入生成式预览阶段...", task_id)
                    result = _run_single(reference_img, content_img, req, task_id)
                else:
                    _print_progress(12, "进入高质量导出阶段...", task_id)
                    result, result_notes = _run_tiled(reference_img, content_img, req, task_id)
            finally:
                _inference_lock.release()
        except Exception as e:
            if str(e) == "cancelled":
                queue = _get_progress_queue(task_id)
                if queue:
                    queue.put({
                        "type": "cancelled",
                        "percentage": 0,
                        "message": "[PROGRESS] 风格迁移已取消"
                    })
                raise HTTPException(status_code=499, detail="cancelled")
            if str(e) == "warming_up":
                raise HTTPException(status_code=503, detail="warming_up")
            if str(e) == "ip_adapter_required":
                raise HTTPException(status_code=503, detail="ip_adapter_required")
            if str(e) == "inference_queue_timeout":
                raise HTTPException(status_code=503, detail="inference_queue_timeout")
            raise HTTPException(status_code=500, detail=str(e))

        pure_generation_result = _normalize_array_to_255(result, "pure_generation_result")
        final_result = pure_generation_result
        if _has_consistency_postprocess(req):
            _print_progress(94, "执行一致性后处理...", task_id)
            _raise_if_cancelled(task_id)
            final_result = _apply_consistency_postprocess(
                pure_generation_result.copy(),
                reference_img,
                content_img,
                req,
                task_id,
                95,
                "执行一致性约束与",
            )

        try:
            _print_progress(95, "执行结果校验...", task_id)
            _raise_if_cancelled(task_id)
            _validate_quality_guard(final_result, stage)
        except Exception as e:
            if str(e) == "cancelled":
                raise HTTPException(status_code=499, detail="cancelled")
            raise HTTPException(status_code=500, detail=str(e))

        _print_progress(97, "保存输出文件...", task_id)
        _raise_if_cancelled(task_id)
        pure_generation_path = _resolve_variant_preview_path(content_path, task_id, "pure")
        post_processed_path = _resolve_variant_preview_path(content_path, task_id, "post")
        _save_output_image(pure_generation_result, pure_generation_path, "png")
        _save_output_image(final_result, post_processed_path, "png")
        _save_preview_png(final_result, out_preview)
        if out_image is not None:
            _raise_if_cancelled(task_id)
            _save_output_image(final_result, out_image, req.export_format)

        completion_message = (
            "生成式风格预览已完成。当前结果为预览图，可继续确认高质量导出。"
            if stage == "preview"
            else "生成式高质量导出已完成。当前结果为 16-bit RGB 衍生图，可继续进入现有导出工作流。"
        )
        if result_notes:
            completion_message = (
                f"{completion_message} 本次有 {len(result_notes)} 个分块触发安全保底，"
                "对应区域已回退为原图内容以避免导出失败。"
            )
            _print_progress(98, f"本次导出有 {len(result_notes)} 个分块使用安全保底结果...", task_id)

        _print_progress(100, "风格迁移完成！", task_id)

        queue = _get_progress_queue(task_id)
        if queue:
            queue.put({
                "type": "done",
                "percentage": 100,
                "message": f"[PROGRESS] 风格迁移完成！{completion_message}",
                "output_image_path": str(out_image) if out_image is not None else None,
                "preview_image_path": str(out_preview),
                "pure_generation_image_path": str(pure_generation_path),
                "post_processed_image_path": str(post_processed_path),
            })

        return StyleTransferResponse(
            status="ok",
            output_image_path=str(out_image) if out_image is not None else None,
            preview_image_path=str(out_preview),
            pure_generation_image_path=str(pure_generation_path),
            post_processed_image_path=str(post_processed_path),
            used_fallback=bool(result_notes),
            message=completion_message,
        )
    finally:
        _clear_task_cancelled(task_id)


@app.post("/v1/style-transfer/cancel/{task_id}")
def cancel_style_transfer(task_id: str) -> Dict[str, Any]:
    _mark_task_cancelled(task_id)
    queue = _get_progress_queue(task_id)
    if queue:
        queue.put({
            "type": "cancelled",
            "percentage": 0,
            "message": "[PROGRESS] 风格迁移已取消"
        })
    return {"status": "cancelling", "taskId": task_id}


@app.get("/v1/style-transfer/progress/{task_id}")
async def style_transfer_progress(task_id: str):
    """
    SSE 端点：实时推送风格迁移进度
    """
    from sse_progress import progress_stream
    
    queue = _get_or_create_progress_queue(task_id)
    
    def event_generator():
        try:
            for data in progress_stream(task_id, queue):
                yield data
        finally:
            _remove_progress_queue(task_id)
            _clear_task_cancelled(task_id)
    
    return StreamingResponse(
        event_generator(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "X-Accel-Buffering": "no"
        }
    )


if __name__ == "__main__":
    import uvicorn
    print("Starting QRaw AI Style Transfer Service (Fixed Version)...")
    print(f"Version: {SERVICE_VERSION}")
    print(f"Debug mode: {DEBUG_MODE}")
    uvicorn.run("app:app", host="127.0.0.1", port=7860, reload=False)
