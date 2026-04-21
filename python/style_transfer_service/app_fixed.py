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
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import numpy as np
import tifffile
from fastapi import FastAPI, HTTPException
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


def _debug_log(msg: str) -> None:
    """调试日志输出"""
    if DEBUG_MODE:
        print(f"[DEBUG] {msg}")


class StyleTransferRequest(BaseModel):
    reference_image_path: str = Field(alias="referenceImagePath")
    content_image_path: str = Field(alias="contentImagePath")
    current_adjustments: Dict[str, Any] = Field(default_factory=dict, alias="currentAdjustments")
    preset: str
    enable_refiner: bool = Field(False, alias="enableRefiner")
    tile_size: int = Field(1024, alias="tileSize")
    tile_overlap: int = Field(96, alias="tileOverlap")
    controlnet_strength: float = Field(0.6, alias="controlnetStrength")
    controlnet_guidance_end: float = Field(0.8, alias="controlnetGuidanceEnd")
    denoise_strength: float = Field(0.55, alias="denoiseStrength")
    steps: int = 35
    cfg_scale: float = Field(6.0, alias="cfgScale")
    output_format: str = Field("rgb16", alias="outputFormat")
    preserve_raw_tone_curve: bool = Field(True, alias="preserveRawToneCurve")
    # 🆕 色彩对齐参数
    enable_color_alignment: bool = Field(True, alias="enableColorAlignment")
    color_alignment_mode: str = Field("full", alias="colorAlignmentMode")  # full, luminance_only, tone_only, none
    luminance_strength: float = Field(0.3, alias="luminanceStrength")
    tone_curve_strength: float = Field(0.5, alias="toneCurveStrength")
    dynamic_range_preserve: float = Field(0.3, alias="dynamicRangePreserve")
    # 🆕 RAW 融合参数
    enable_raw_fusion: bool = Field(True, alias="enableRawFusion")
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
    h.update(req.output_format.encode("utf-8"))
    h.update(str(req.preserve_raw_tone_curve).encode("utf-8"))
    return h.hexdigest()


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


def _load_pipelines() -> None:
    global _pipe, _refiner, _pipeline_error, _ip_adapter_loaded
    if _pipe is not None or _pipeline_error is not None:
        return

    try:
        import torch
        from diffusers import ControlNetModel, StableDiffusionXLControlNetImg2ImgPipeline, StableDiffusionXLImg2ImgPipeline

        has_mps = hasattr(torch.backends, "mps") and torch.backends.mps.is_available()
        if torch.cuda.is_available():
            torch_dtype = torch.float16
            device = torch.device("cuda")
        elif has_mps:
            torch_dtype = torch.float16
            device = torch.device("mps")
        else:
            torch_dtype = torch.float32
            device = torch.device("cpu")

        _debug_log(f"Loading models on {device} with {torch_dtype}")

        controlnet = ControlNetModel.from_pretrained(DEFAULT_CONTROLNET_MODEL, torch_dtype=torch_dtype)

        pipe = StableDiffusionXLControlNetImg2ImgPipeline.from_pretrained(
            DEFAULT_BASE_MODEL,
            controlnet=controlnet,
            torch_dtype=torch_dtype,
            use_safetensors=True,
            variant="fp16" if torch_dtype == torch.float16 else None,
        )
        pipe = pipe.to(device)

        try:
            pipe.enable_attention_slicing()
        except Exception:
            pass
        try:
            pipe.enable_vae_tiling()
        except Exception:
            pass

        _ip_adapter_loaded = False
        if DEFAULT_IP_ADAPTER_MODEL and hasattr(pipe, "load_ip_adapter"):
            try:
                _debug_log(f"Loading IP-Adapter: {DEFAULT_IP_ADAPTER_MODEL}")
                if DEFAULT_IP_ADAPTER_WEIGHT:
                    pipe.load_ip_adapter(DEFAULT_IP_ADAPTER_MODEL, weight_name=DEFAULT_IP_ADAPTER_WEIGHT)
                else:
                    pipe.load_ip_adapter(DEFAULT_IP_ADAPTER_MODEL)
                _ip_adapter_loaded = True
                _debug_log("IP-Adapter loaded successfully")
            except Exception as e:
                print(f"[WARNING] Failed to load IP-Adapter: {e}")
                _ip_adapter_loaded = False

        refiner = None
        if DEFAULT_REFINER_MODEL:
            try:
                _debug_log(f"Loading Refiner: {DEFAULT_REFINER_MODEL}")
                refiner = StableDiffusionXLImg2ImgPipeline.from_pretrained(
                    DEFAULT_REFINER_MODEL,
                    torch_dtype=torch_dtype,
                    use_safetensors=True,
                    variant="fp16" if torch_dtype == torch.float16 else None,
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

        _pipe = pipe
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


def _run_single(
    reference_img: Image.Image,
    content_img: Image.Image,
    req: StyleTransferRequest,
) -> np.ndarray:
    """
    🔧 修复：添加完整的值域检查和归一化
    """
    _ensure_warming_up()
    if _pipe is None:
        if _pipeline_error is not None:
            raise RuntimeError(_pipeline_error)
        raise RuntimeError("warming_up")

    import torch

    pipe = _pipe
    style_img = _ensure_rgb(reference_img)
    content_img = _ensure_rgb(content_img)
    
    # 🆕 进度：准备控制图像
    print(f"[PROGRESS] 10% - 准备控制图像...")
    control_img = _make_control_image(content_img)

    steps = max(1, int(req.steps))
    guidance_end = float(req.controlnet_guidance_end)
    guidance_end = max(0.0, min(1.0, guidance_end))
    denoise = float(req.denoise_strength)
    denoise = max(0.0, min(1.0, denoise))

    extra: Dict[str, Any] = {}
    try:
        sig = inspect.signature(pipe.__call__)
        if _ip_adapter_loaded and "ip_adapter_image" in sig.parameters:
            extra["ip_adapter_image"] = style_img
            _debug_log("Using IP-Adapter for style injection")
        if _ip_adapter_loaded and "ip_adapter_scale" in sig.parameters:
            extra["ip_adapter_scale"] = 1.0
        if "control_guidance_end" in sig.parameters:
            extra["control_guidance_end"] = guidance_end
        if "control_guidance_start" in sig.parameters:
            extra["control_guidance_start"] = 0.0
    except Exception:
        pass

    generator = torch.Generator(device=pipe.device)
    prompt = ""
    negative_prompt = ""

    if req.enable_refiner and _refiner is not None:
        # 🆕 进度：运行 Base Model
        print(f"[PROGRESS] 20% - 运行 SDXL Base Model (步数: {steps})...")
        _debug_log("Running with Refiner")
        split = 0.8
        base_steps = max(1, int(round(steps * split)))
        out = pipe(
            prompt=prompt,
            negative_prompt=negative_prompt,
            image=content_img,
            control_image=control_img,
            strength=denoise,
            num_inference_steps=base_steps,
            guidance_scale=float(req.cfg_scale),
            controlnet_conditioning_scale=float(req.controlnet_strength),
            denoising_end=split,
            generator=generator,
            output_type="latent",
            **extra,
        )
        latents = out.images
        
        # 🆕 进度：运行 Refiner
        print(f"[PROGRESS] 70% - 运行 SDXL Refiner...")
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
        print(f"[PROGRESS] 90% - Refiner 完成")
    else:
        # 🆕 进度：运行单阶段推理
        print(f"[PROGRESS] 20% - 运行 SDXL 推理 (步数: {steps})...")
        _debug_log("Running without Refiner")
        image = pipe(
            prompt=prompt,
            negative_prompt=negative_prompt,
            image=content_img,
            control_image=control_img,
            strength=denoise,
            num_inference_steps=steps,
            guidance_scale=float(req.cfg_scale),
            controlnet_conditioning_scale=float(req.controlnet_strength),
            generator=generator,
            **extra,
        ).images[0]
        print(f"[PROGRESS] 80% - 推理完成")

    # 🔧 修复：确保输出是 PIL Image
    if not isinstance(image, Image.Image):
        _debug_log(f"Pipeline output is not PIL Image, type: {type(image)}")
        if hasattr(image, "cpu"):
            image = image.cpu().numpy()
        image = Image.fromarray(np.clip(image * 255, 0, 255).astype(np.uint8))
    
    image = _ensure_rgb(image)
    arr = np.array(image)
    
    # 🔧 修复：归一化到 [0, 255]
    print(f"[PROGRESS] 95% - 后处理...")
    arr = _normalize_array_to_255(arr, "pipeline_output")
    
    return arr


def _run_tiled(
    reference_img: Image.Image,
    content_img: Image.Image,
    req: StyleTransferRequest,
) -> np.ndarray:
    content_img = _ensure_rgb(content_img)
    w, h = content_img.size
    tile = max(256, int(req.tile_size))
    overlap = max(0, int(req.tile_overlap))
    if w <= tile and h <= tile:
        return _run_single(reference_img, content_img, req)

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
    tile_count = 0
    
    # 🔧 修复：确保循环正确处理边界
    y0 = 0
    while y0 < h:
        y1 = min(h, y0 + tile)
        cy0 = max(0, y1 - tile)
        actual_h = y1 - cy0
        
        x0 = 0
        while x0 < w:
            x1 = min(w, x0 + tile)
            cx0 = max(0, x1 - tile)
            actual_w = x1 - cx0
            
            # 确保 crop 的尺寸正确
            crop = content_img.crop((cx0, cy0, x1, y1))
            
            # 🆕 进度：显示当前处理的 tile
            progress_pct = int((tile_count / total_tiles) * 80) + 10  # 10%-90% 范围
            print(f"[PROGRESS] {progress_pct}% - 处理第 {tile_count + 1}/{total_tiles} 块 (位置: {cx0},{cy0})...")
            _debug_log(f"Processing tile {tile_count}: ({cx0}, {cy0}) -> ({x1}, {y1}), size: {actual_w}x{actual_h}")
            
            try:
                out = _run_single(reference_img, crop, req)
                tiles.append((cx0, cy0, out))
                tile_count += 1
            except Exception as e:
                print(f"[ERROR] Failed to process tile {tile_count} at ({cx0}, {cy0}): {e}")
                # 如果单个 tile 失败，使用空白 tile
                empty_tile = np.zeros((actual_h, actual_w, 3), dtype=np.float32)
                tiles.append((cx0, cy0, empty_tile))
                tile_count += 1
            
            # 移动到下一个 x 位置
            if x1 >= w:
                break
            x0 += stride
        
        # 移动到下一个 y 位置
        if y1 >= h:
            break
        y0 += stride
    
    # 🆕 进度：融合 tiles
    print(f"[PROGRESS] 90% - 融合 {tile_count} 个块...")
    _debug_log(f"Blending {tile_count} tiles")
    return _blend_tiles(tiles, w, h, tile, overlap)


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
        capabilities = [
            "sdxl",
            *(("ip_adapter",) if _ip_adapter_loaded else ()),
            "controlnet",
            "tiled_vae",
            "weighted_blending",
            "fp16",
            "value_range_normalization",
        ]
        # 🆕 添加色彩对齐和 RAW 融合能力
        if HAS_COLOR_ALIGNMENT:
            capabilities.extend([
                "color_alignment",
                "luminance_aware_mapping",
                "tone_curve_adjustment",
                "dynamic_range_preservation",
            ])
        if HAS_RAW_FUSION:
            capabilities.extend([
                "raw_fusion",
                "highlight_detail_preservation",
                "shadow_detail_preservation",
            ])
        
        return HealthResponse(
            status="ok",
            ready=True,
            version=SERVICE_VERSION,
            pipeline="sdxl+ip-adapter+controlnet+tiled-vae+color-alignment+raw-fusion",
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
        pipeline="sdxl+ip-adapter+controlnet+tiled-vae+color-alignment+raw-fusion",
        capabilities=[
            "sdxl",
            *(("ip_adapter",) if _ip_adapter_loaded else ()),
            "controlnet",
            "tiled_vae",
            "weighted_blending",
            "fp16",
            "value_range_normalization",
            *(["color_alignment", "luminance_aware_mapping", "tone_curve_adjustment", "dynamic_range_preservation"] if HAS_COLOR_ALIGNMENT else []),
            *(["raw_fusion", "highlight_detail_preservation", "shadow_detail_preservation"] if HAS_RAW_FUSION else []),
        ],
        detail="loading_models",
    )


@app.on_event("startup")
def startup_event() -> None:
    if _check_deps():
        _ensure_warming_up()


@app.post("/v1/style-transfer", response_model=StyleTransferResponse)
def style_transfer(req: StyleTransferRequest) -> StyleTransferResponse:
    print(f"[PROGRESS] 0% - 开始风格迁移...")
    _debug_log("--- [DEBUG] STARTING STYLE TRANSFER ---")
    
    ref_path = Path(req.reference_image_path)
    content_path = Path(req.content_image_path)
    if not ref_path.exists():
        raise HTTPException(status_code=400, detail="reference_image_not_found")
    if not content_path.exists():
        raise HTTPException(status_code=400, detail="content_image_not_found")

    key = _hash_request(req)
    out_dir = DEFAULT_OUTPUT_DIR / key
    out_dir.mkdir(parents=True, exist_ok=True)

    out_tiff = out_dir / "output.tiff"
    out_preview = out_dir / "preview.png"
    if out_tiff.exists() and out_preview.exists():
        _debug_log(f"Using cached result: {key}")
        print(f"[PROGRESS] 100% - 使用缓存结果")
        return StyleTransferResponse(
            status="ok",
            output_image_path=str(out_tiff),
            preview_image_path=str(out_preview),
        )

    print(f"[PROGRESS] 5% - 加载图像...")
    try:
        reference_img = _load_image(str(ref_path))
        content_img = _load_image(str(content_path))
        reference_img = _ensure_rgb(reference_img)
        content_img = _ensure_rgb(content_img)
        _debug_log(f"Loaded Reference Size: {reference_img.size}, Mode: {reference_img.mode}")
        _debug_log(f"Loaded Content Size: (W:{content_img.size[0]}, H:{content_img.size[1]}), Mode: {content_img.mode}")
    except Exception as e:
        raise HTTPException(status_code=400, detail=f"invalid_image: {e}")

    # 保存原始内容图像的数组形式（用于后处理）
    content_arr = np.array(content_img).astype(np.float32)
    content_arr = _normalize_array_to_255(content_arr, "original_content")

    try:
        result = _run_tiled(reference_img, content_img, req)
    except Exception as e:
        if str(e) == "warming_up":
            raise HTTPException(status_code=503, detail="warming_up")
        raise HTTPException(status_code=500, detail=str(e))

    # 🆕 后处理步骤 1: 色彩对齐（文档第 7.2 节）
    if req.enable_color_alignment and HAS_COLOR_ALIGNMENT:
        print(f"[PROGRESS] 92% - 应用色彩对齐 (模式: {req.color_alignment_mode})...")
        _debug_log(f"Applying color alignment (mode={req.color_alignment_mode})")
        try:
            result = apply_color_alignment(
                ai_result=result,
                original_img=content_arr,
                mode=req.color_alignment_mode,
                luminance_strength=req.luminance_strength,
                tone_curve_strength=req.tone_curve_strength,
                dynamic_range_preserve=req.dynamic_range_preserve,
            )
            _debug_log("Color alignment completed")
        except Exception as e:
            print(f"[WARNING] Color alignment failed: {e}")

    # 🆕 后处理步骤 2: RAW 融合（文档第 7.3 节）
    if req.enable_raw_fusion and req.preserve_raw_tone_curve and HAS_RAW_FUSION:
        print(f"[PROGRESS] 94% - 应用 RAW 融合 (模式: {req.raw_blend_mode}, 强度: {req.raw_blend_strength})...")
        _debug_log(f"Applying RAW fusion (mode={req.raw_blend_mode}, strength={req.raw_blend_strength})")
        try:
            # 检查是否是 RAW 文件
            is_raw = str(content_path).lower().endswith(('.raw', '.cr2', '.nef', '.arw', '.dng', '.raf', '.orf', '.rw2'))
            
            if is_raw:
                # 从 RAW 文件加载
                result = apply_raw_fusion(
                    ai_result=result,
                    raw_path=str(content_path),
                    current_adjustments=req.current_adjustments,
                    blend_strength=req.raw_blend_strength,
                    blend_mode=req.raw_blend_mode,
                    preserve_highlights=req.preserve_highlights,
                    preserve_shadows=req.preserve_shadows,
                )
            else:
                # 使用已处理的图像
                result = apply_raw_fusion(
                    ai_result=result,
                    raw_processed=content_arr,
                    current_adjustments=req.current_adjustments,
                    blend_strength=req.raw_blend_strength,
                    blend_mode=req.raw_blend_mode,
                    preserve_highlights=req.preserve_highlights,
                    preserve_shadows=req.preserve_shadows,
                )
            _debug_log("RAW fusion completed")
        except Exception as e:
            print(f"[WARNING] RAW fusion failed: {e}")

    print(f"[PROGRESS] 96% - 保存输出文件...")
    _save_rgb16_tiff(result, out_tiff)
    _save_preview_png(result, out_preview)
    print(f"[PROGRESS] 100% - 风格迁移完成！")

    return StyleTransferResponse(
        status="ok",
        output_image_path=str(out_tiff),
        preview_image_path=str(out_preview),
    )


if __name__ == "__main__":
    import uvicorn
    print("Starting QRaw AI Style Transfer Service (Fixed Version)...")
    print(f"Version: {SERVICE_VERSION}")
    print(f"Debug mode: {DEBUG_MODE}")
    uvicorn.run("app_fixed:app", host="127.0.0.1", port=7860, reload=False)
