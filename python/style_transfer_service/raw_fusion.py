"""
RAW 融合策略
实现文档第 7.3 节：AI结果 → 与 RAW tone curve 融合
保持动态范围、高光细节、阴影层次
"""
import numpy as np
from typing import Dict, Any, Optional, Tuple
from pathlib import Path
from PIL import Image

try:
    import rawpy
    HAS_RAWPY = True
except ImportError:
    HAS_RAWPY = False

from color_alignment import rgb_to_luma, replace_luma


def load_raw_image(
    raw_path: str,
    half_size: bool = False,
    use_camera_wb: bool = True,
    no_auto_bright: bool = True,
) -> np.ndarray:
    """
    加载 RAW 图像
    
    Args:
        raw_path: RAW 文件路径
        half_size: 是否使用半尺寸（加速）
        use_camera_wb: 使用相机白平衡
        no_auto_bright: 禁用自动亮度
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    if not HAS_RAWPY:
        raise ImportError("rawpy is required for RAW processing. Install: pip install rawpy")
    
    with rawpy.imread(raw_path) as raw:
        rgb = raw.postprocess(
            use_camera_wb=use_camera_wb,
            half_size=half_size,
            no_auto_bright=no_auto_bright,
            output_bps=16,  # 16-bit 输出
        )
    
    # 转换到 [0, 255] float32
    rgb = rgb.astype(np.float32) / 65535.0 * 255.0
    
    return rgb


def extract_tone_curve_from_adjustments(
    adjustments: Dict[str, Any]
) -> Optional[np.ndarray]:
    """
    从调整参数中提取 tone curve
    
    Args:
        adjustments: 调整参数字典，包含 exposure, contrast, highlights, shadows 等
    
    Returns:
        [256] 数组，或 None（如果没有曲线信息）
    """
    # 创建基础曲线（线性）
    curve = np.arange(256, dtype=np.float32)
    
    # 应用曝光
    if "exposure" in adjustments:
        exposure = float(adjustments["exposure"])
        # exposure 通常是 EV 值，范围 -5 到 +5
        # 转换为乘数：2^exposure
        multiplier = 2.0 ** (exposure / 100.0)  # 假设输入是 -500 到 +500
        curve = curve * multiplier
    
    # 应用对比度
    if "contrast" in adjustments:
        contrast = float(adjustments["contrast"])
        # contrast 范围 -100 到 +100
        # 转换为斜率调整
        factor = 1.0 + (contrast / 100.0)
        curve = (curve - 128.0) * factor + 128.0
    
    # 应用高光
    if "highlights" in adjustments:
        highlights = float(adjustments["highlights"])
        # highlights 范围 -100 到 +100
        # 影响高光区域（> 192）
        mask = np.maximum(0, (curve - 192.0) / 63.0)
        adjustment = highlights / 100.0 * 63.0
        curve = curve + mask * adjustment
    
    # 应用阴影
    if "shadows" in adjustments:
        shadows = float(adjustments["shadows"])
        # shadows 范围 -100 到 +100
        # 影响阴影区域（< 64）
        mask = np.maximum(0, (64.0 - curve) / 64.0)
        adjustment = shadows / 100.0 * 64.0
        curve = curve + mask * adjustment
    
    # 应用白色
    if "whites" in adjustments:
        whites = float(adjustments["whites"])
        # 影响高光区域（> 128）
        mask = np.maximum(0, (curve - 128.0) / 127.0)
        adjustment = whites / 100.0 * 50.0
        curve = curve + mask * adjustment
    
    # 应用黑色
    if "blacks" in adjustments:
        blacks = float(adjustments["blacks"])
        # 影响阴影区域（< 128）
        mask = np.maximum(0, (128.0 - curve) / 128.0)
        adjustment = blacks / 100.0 * 50.0
        curve = curve + mask * adjustment
    
    # 裁剪到有效范围
    curve = np.clip(curve, 0.0, 255.0)
    
    return curve


def apply_tone_curve_to_image(
    img: np.ndarray,
    curve: np.ndarray,
) -> np.ndarray:
    """
    应用 tone curve 到图像
    
    Args:
        img: [H, W, 3] float32 数组，范围 [0, 255]
        curve: [256] 数组，tone curve LUT
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    # 创建 LUT
    lut = np.clip(curve, 0, 255).astype(np.uint8)
    
    # 应用到每个通道
    result = lut[img.astype(np.uint8)]
    
    return result.astype(np.float32)


def blend_with_raw_tone_curve(
    ai_result: np.ndarray,
    raw_processed: np.ndarray,
    blend_strength: float = 0.5,
    mode: str = "luminance",
) -> np.ndarray:
    """
    文档第 7.3 节：RAW 融合策略
    
    将 AI 结果与 RAW 处理结果融合，保持动态范围和细节
    
    Args:
        ai_result: AI 生成结果，[H, W, 3] float32，范围 [0, 255]
        raw_processed: RAW 处理结果（应用了 tone curve），[H, W, 3] float32，范围 [0, 255]
        blend_strength: RAW 的混合强度，0.0-1.0
        mode: 融合模式
            - 'luminance': 仅融合亮度，保持 AI 的色彩
            - 'color': 仅融合色彩，保持 AI 的亮度
            - 'full': 全通道融合
            - 'adaptive': 自适应融合（高光和阴影区域增强融合）
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    if mode == "luminance":
        # 提取亮度
        ai_luma = rgb_to_luma(ai_result)
        raw_luma = rgb_to_luma(raw_processed)
        
        # 混合亮度
        blended_luma = ai_luma * (1.0 - blend_strength) + raw_luma * blend_strength
        
        # 替换 AI 的亮度，保持色彩
        result = replace_luma(ai_result, blended_luma)
    
    elif mode == "color":
        # 提取亮度
        ai_luma = rgb_to_luma(ai_result)
        
        # 混合色彩
        result = ai_result * (1.0 - blend_strength) + raw_processed * blend_strength
        
        # 恢复 AI 的亮度
        result = replace_luma(result, ai_luma)
    
    elif mode == "full":
        # 全通道混合
        result = ai_result * (1.0 - blend_strength) + raw_processed * blend_strength
    
    elif mode == "adaptive":
        # 自适应融合
        raw_luma = rgb_to_luma(raw_processed)
        
        # 高光区域（> 200）增强融合
        highlight_mask = np.clip((raw_luma - 200.0) / 55.0, 0.0, 1.0)
        
        # 阴影区域（< 50）增强融合
        shadow_mask = np.clip((50.0 - raw_luma) / 50.0, 0.0, 1.0)
        
        # 组合掩码
        adaptive_strength = blend_strength * (1.0 + highlight_mask * 0.5 + shadow_mask * 0.5)
        adaptive_strength = np.clip(adaptive_strength, 0.0, 1.0)
        
        # 应用自适应混合
        result = (
            ai_result * (1.0 - adaptive_strength[:, :, np.newaxis]) +
            raw_processed * adaptive_strength[:, :, np.newaxis]
        )
    
    else:
        raise ValueError(f"Unknown blend mode: {mode}")
    
    return np.clip(result, 0.0, 255.0)


def preserve_highlight_details(
    ai_result: np.ndarray,
    original_img: np.ndarray,
    threshold: float = 200.0,
    strength: float = 0.6,
) -> np.ndarray:
    """
    文档第 7.3 节：保持高光细节
    
    在高光区域混合原图，避免过曝
    
    Args:
        ai_result: AI 生成结果，[H, W, 3] float32，范围 [0, 255]
        original_img: 原始图像，[H, W, 3] float32，范围 [0, 255]
        threshold: 高光阈值，> threshold 的区域被视为高光
        strength: 保护强度，0.0-1.0
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    # 计算亮度
    orig_luma = rgb_to_luma(original_img)
    
    # 创建高光掩码（软边缘）
    highlight_mask = np.clip((orig_luma - threshold) / (255.0 - threshold), 0.0, 1.0)
    
    # 应用强度
    highlight_mask = highlight_mask * strength
    
    # 混合
    result = (
        ai_result * (1.0 - highlight_mask[:, :, np.newaxis]) +
        original_img * highlight_mask[:, :, np.newaxis]
    )
    
    return result


def preserve_shadow_details(
    ai_result: np.ndarray,
    original_img: np.ndarray,
    threshold: float = 50.0,
    strength: float = 0.5,
) -> np.ndarray:
    """
    文档第 7.3 节：保持阴影层次
    
    在阴影区域混合原图，避免死黑
    
    Args:
        ai_result: AI 生成结果，[H, W, 3] float32，范围 [0, 255]
        original_img: 原始图像，[H, W, 3] float32，范围 [0, 255]
        threshold: 阴影阈值，< threshold 的区域被视为阴影
        strength: 保护强度，0.0-1.0
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    # 计算亮度
    orig_luma = rgb_to_luma(original_img)
    
    # 创建阴影掩码（软边缘）
    shadow_mask = np.clip((threshold - orig_luma) / threshold, 0.0, 1.0)
    
    # 应用强度
    shadow_mask = shadow_mask * strength
    
    # 混合
    result = (
        ai_result * (1.0 - shadow_mask[:, :, np.newaxis]) +
        original_img * shadow_mask[:, :, np.newaxis]
    )
    
    return result


def apply_raw_fusion(
    ai_result: np.ndarray,
    raw_path: Optional[str] = None,
    raw_processed: Optional[np.ndarray] = None,
    current_adjustments: Optional[Dict[str, Any]] = None,
    blend_strength: float = 0.5,
    blend_mode: str = "luminance",
    preserve_highlights: bool = True,
    preserve_shadows: bool = True,
    highlight_threshold: float = 200.0,
    shadow_threshold: float = 50.0,
) -> np.ndarray:
    """
    完整的 RAW 融合流程（文档第 7.3 节）
    
    Args:
        ai_result: AI 生成结果，[H, W, 3] float32，范围 [0, 255]
        raw_path: RAW 文件路径（可选）
        raw_processed: 已处理的 RAW 图像（可选，如果提供则不加载 raw_path）
        current_adjustments: 当前调整参数（用于提取 tone curve）
        blend_strength: RAW 融合强度
        blend_mode: 融合模式（'luminance', 'color', 'full', 'adaptive'）
        preserve_highlights: 是否保护高光细节
        preserve_shadows: 是否保护阴影层次
        highlight_threshold: 高光阈值
        shadow_threshold: 阴影阈值
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    # 获取 RAW 处理结果
    if raw_processed is None:
        if raw_path is None:
            raise ValueError("Either raw_path or raw_processed must be provided")
        
        # 加载 RAW
        raw_processed = load_raw_image(raw_path)
        
        # 应用调整参数
        if current_adjustments is not None:
            tone_curve = extract_tone_curve_from_adjustments(current_adjustments)
            if tone_curve is not None:
                raw_processed = apply_tone_curve_to_image(raw_processed, tone_curve)
    
    # 确保尺寸匹配
    if ai_result.shape[:2] != raw_processed.shape[:2]:
        from PIL import Image
        h, w = ai_result.shape[:2]
        raw_pil = Image.fromarray(raw_processed.astype(np.uint8))
        raw_pil = raw_pil.resize((w, h), Image.Resampling.LANCZOS)
        raw_processed = np.array(raw_pil).astype(np.float32)
    
    result = ai_result.copy()
    
    # 步骤 1: RAW tone curve 融合
    if blend_strength > 0:
        result = blend_with_raw_tone_curve(
            result,
            raw_processed,
            blend_strength=blend_strength,
            mode=blend_mode,
        )
    
    # 步骤 2: 高光细节保护
    if preserve_highlights:
        result = preserve_highlight_details(
            result,
            raw_processed,
            threshold=highlight_threshold,
            strength=0.6,
        )
    
    # 步骤 3: 阴影层次保护
    if preserve_shadows:
        result = preserve_shadow_details(
            result,
            raw_processed,
            threshold=shadow_threshold,
            strength=0.5,
        )
    
    return result


def create_raw_fusion_preview(
    ai_result: np.ndarray,
    raw_processed: np.ndarray,
    blend_strengths: list = [0.0, 0.3, 0.5, 0.7, 1.0],
) -> list:
    """
    创建不同融合强度的预览
    
    Args:
        ai_result: AI 生成结果
        raw_processed: RAW 处理结果
        blend_strengths: 融合强度列表
    
    Returns:
        预览图像列表
    """
    previews = []
    
    for strength in blend_strengths:
        preview = blend_with_raw_tone_curve(
            ai_result,
            raw_processed,
            blend_strength=strength,
            mode="luminance",
        )
        previews.append({
            "strength": strength,
            "image": preview,
            "label": f"RAW Fusion: {int(strength * 100)}%"
        })
    
    return previews


# 测试函数
if __name__ == "__main__":
    print("RAW 融合系统测试")
    print("=" * 60)
    
    # 创建测试图像
    test_ai = np.random.rand(512, 512, 3).astype(np.float32) * 255
    test_raw = np.random.rand(512, 512, 3).astype(np.float32) * 255
    
    # 测试 tone curve 提取
    print("✓ extract_tone_curve_from_adjustments")
    adjustments = {
        "exposure": 50,
        "contrast": 20,
        "highlights": -30,
        "shadows": 40,
    }
    curve = extract_tone_curve_from_adjustments(adjustments)
    print(f"  Curve shape: {curve.shape}, range: [{curve.min():.2f}, {curve.max():.2f}]")
    
    # 测试融合
    print("✓ blend_with_raw_tone_curve (luminance)")
    result1 = blend_with_raw_tone_curve(test_ai, test_raw, blend_strength=0.5, mode="luminance")
    print(f"  Result shape: {result1.shape}, range: [{result1.min():.2f}, {result1.max():.2f}]")
    
    print("✓ blend_with_raw_tone_curve (adaptive)")
    result2 = blend_with_raw_tone_curve(test_ai, test_raw, blend_strength=0.5, mode="adaptive")
    print(f"  Result shape: {result2.shape}, range: [{result2.min():.2f}, {result2.max():.2f}]")
    
    # 测试高光保护
    print("✓ preserve_highlight_details")
    result3 = preserve_highlight_details(test_ai, test_raw, threshold=200.0, strength=0.6)
    print(f"  Result shape: {result3.shape}, range: [{result3.min():.2f}, {result3.max():.2f}]")
    
    # 测试阴影保护
    print("✓ preserve_shadow_details")
    result4 = preserve_shadow_details(test_ai, test_raw, threshold=50.0, strength=0.5)
    print(f"  Result shape: {result4.shape}, range: [{result4.min():.2f}, {result4.max():.2f}]")
    
    # 测试完整流程
    print("✓ apply_raw_fusion (full)")
    result5 = apply_raw_fusion(
        test_ai,
        raw_processed=test_raw,
        current_adjustments=adjustments,
        blend_strength=0.5,
        blend_mode="luminance",
    )
    print(f"  Result shape: {result5.shape}, range: [{result5.min():.2f}, {result5.max():.2f}]")
    
    print("\n所有测试通过！")
